//! Anthropic Claude API client
//!
//! Production-ready client for Claude API with comprehensive error handling,
//! rate limiting, and cost tracking.

use super::types::*;
use anyhow::{anyhow, Context, Result};
use governor::{Quota, RateLimiter};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Anthropic API client
#[derive(Clone)]
pub struct AnthropicClient {
    /// HTTP client
    client: reqwest::Client,
    /// Configuration
    config: Arc<RwLock<AnthropicConfig>>,
    /// Rate limiter
    rate_limiter: Arc<RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
    /// Cost tracker
    cost_tracker: Arc<RwLock<CostTracker>>,
}

impl AnthropicClient {
    /// Create a new Anthropic client
    ///
    /// # Arguments
    ///
    /// * `config` - API configuration
    ///
    /// # Returns
    ///
    /// Returns a new AnthropicClient instance
    pub async fn new(config: AnthropicConfig) -> Result<Self> {
        let timeout = Duration::from_secs(config.timeout_secs);

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("llm-auto-optimizer/1.0")
            .build()
            .context("Failed to create HTTP client")?;

        // Setup rate limiter
        let rate_limit = NonZeroU32::new(config.rate_limit_per_minute)
            .ok_or_else(|| anyhow!("Rate limit must be greater than 0"))?;
        let quota = Quota::per_minute(rate_limit);
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        info!("Initialized Anthropic client");

        Ok(Self {
            client,
            config: Arc::new(RwLock::new(config)),
            rate_limiter,
            cost_tracker: Arc::new(RwLock::new(CostTracker::new())),
        })
    }

    /// Send a message to Claude
    ///
    /// # Arguments
    ///
    /// * `request` - Message request
    ///
    /// # Returns
    ///
    /// Returns the message response
    pub async fn send_message(&self, mut request: MessageRequest) -> Result<MessageResponse> {
        // Ensure streaming is disabled for non-streaming requests
        request.stream = false;

        debug!(
            "Sending message to model: {} (max_tokens: {})",
            request.model, request.max_tokens
        );

        let response: MessageResponse = self.execute_request(&request).await?;

        // Track costs
        let model = self.parse_model(&response.model)?;
        let mut tracker = self.cost_tracker.write().await;
        tracker.record_usage(&response.usage, model);

        info!(
            "Message completed. Tokens: {} in, {} out. Stop reason: {:?}",
            response.usage.input_tokens,
            response.usage.output_tokens,
            response.stop_reason
        );

        Ok(response)
    }

    /// Create a simple text message
    ///
    /// # Arguments
    ///
    /// * `model` - Claude model to use
    /// * `prompt` - User prompt
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    ///
    /// Returns the assistant's response text
    pub async fn complete(
        &self,
        model: ClaudeModel,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<String> {
        let request = MessageRequest {
            model: model.as_str().to_string(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt.to_string()),
            }],
            max_tokens,
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: false,
            metadata: None,
        };

        let response = self.send_message(request).await?;

        // Extract text from content blocks
        let text = response
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(text)
    }

    /// Create a message with system prompt
    ///
    /// # Arguments
    ///
    /// * `model` - Claude model to use
    /// * `system` - System prompt
    /// * `prompt` - User prompt
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    ///
    /// Returns the assistant's response text
    pub async fn complete_with_system(
        &self,
        model: ClaudeModel,
        system: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<String> {
        let request = MessageRequest {
            model: model.as_str().to_string(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text(prompt.to_string()),
            }],
            max_tokens,
            system: Some(system.to_string()),
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: false,
            metadata: None,
        };

        let response = self.send_message(request).await?;

        let text = response
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(text)
    }

    /// Count tokens in text (estimation)
    ///
    /// # Arguments
    ///
    /// * `text` - Text to count tokens for
    ///
    /// # Returns
    ///
    /// Returns estimated token count
    pub fn count_tokens(&self, text: &str) -> u32 {
        // Simple estimation: ~4 characters per token
        // For production, use the official token counting API
        (text.len() as f32 / 4.0).ceil() as u32
    }

    /// Validate a request before sending
    ///
    /// # Arguments
    ///
    /// * `request` - Request to validate
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if valid, Err otherwise
    pub fn validate_request(&self, request: &MessageRequest) -> Result<()> {
        // Check max_tokens
        if request.max_tokens == 0 {
            return Err(anyhow!("max_tokens must be greater than 0"));
        }

        let model = self.parse_model(&request.model)?;
        if request.max_tokens > model.max_tokens() {
            return Err(anyhow!(
                "max_tokens {} exceeds model limit {}",
                request.max_tokens,
                model.max_tokens()
            ));
        }

        // Check messages
        if request.messages.is_empty() {
            return Err(anyhow!("messages cannot be empty"));
        }

        // Check temperature
        if let Some(temp) = request.temperature {
            if !(0.0..=1.0).contains(&temp) {
                return Err(anyhow!("temperature must be between 0.0 and 1.0"));
            }
        }

        // Check top_p
        if let Some(top_p) = request.top_p {
            if !(0.0..=1.0).contains(&top_p) {
                return Err(anyhow!("top_p must be between 0.0 and 1.0"));
            }
        }

        Ok(())
    }

    /// Get cost tracking statistics
    pub async fn get_cost_stats(&self) -> CostTracker {
        self.cost_tracker.read().await.clone()
    }

    /// Reset cost tracking
    pub async fn reset_cost_stats(&self) {
        self.cost_tracker.write().await.reset();
    }

    /// Execute an API request with retry logic
    async fn execute_request<T: serde::de::DeserializeOwned>(
        &self,
        request: &MessageRequest,
    ) -> Result<T> {
        // Validate request
        self.validate_request(request)?;

        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        let config = self.config.read().await;
        let url = format!("{}/v1/messages", config.base_url);
        let max_retries = config.max_retries;

        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = Duration::from_secs(2u64.pow(attempt));
                debug!("Retrying after {:?} (attempt {})", delay, attempt);
                sleep(delay).await;
            }

            let headers = self.build_headers(&config)?;
            let http_request = self
                .client
                .post(&url)
                .headers(headers)
                .json(request);

            match http_request.send().await {
                Ok(response) => {
                    let status = response.status();

                    // Handle rate limiting
                    if status.as_u16() == 429 {
                        warn!("Rate limited by Anthropic API");
                        last_error = Some(anyhow!("Rate limited"));
                        continue;
                    }

                    // Handle overloaded
                    if status.as_u16() == 529 {
                        warn!("Anthropic API overloaded");
                        last_error = Some(anyhow!("API overloaded"));
                        continue;
                    }

                    // Handle success
                    if response.status().is_success() {
                        return response
                            .json::<T>()
                            .await
                            .context("Failed to parse response JSON");
                    }

                    // Handle error responses
                    let error_text = response.text().await.unwrap_or_default();
                    let error_msg = if let Ok(err) = serde_json::from_str::<ApiError>(&error_text) {
                        format!("Anthropic API error ({}): {}", err.error_type, err.message)
                    } else {
                        format!("Anthropic API error ({}): {}", status, error_text)
                    };

                    error!("{}", error_msg);
                    last_error = Some(anyhow!(error_msg));

                    // Don't retry on client errors (except rate limit)
                    if status.is_client_error() {
                        break;
                    }
                }
                Err(e) => {
                    warn!("Request failed: {}", e);
                    last_error = Some(anyhow!(e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Request failed after retries")))
    }

    /// Build request headers
    fn build_headers(&self, config: &AnthropicConfig) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&config.api_key)
                .context("Invalid API key")?,
        );

        headers.insert(
            "anthropic-version",
            HeaderValue::from_str(&config.api_version)
                .context("Invalid API version")?,
        );

        Ok(headers)
    }

    /// Parse model string to enum
    fn parse_model(&self, model_str: &str) -> Result<ClaudeModel> {
        match model_str {
            "claude-3-5-sonnet-20241022" => Ok(ClaudeModel::Claude35Sonnet),
            "claude-3-opus-20240229" => Ok(ClaudeModel::Claude3Opus),
            "claude-3-sonnet-20240229" => Ok(ClaudeModel::Claude3Sonnet),
            "claude-3-haiku-20240307" => Ok(ClaudeModel::Claude3Haiku),
            _ => {
                // Try to match partial model names
                if model_str.contains("sonnet") && model_str.contains("3-5") {
                    Ok(ClaudeModel::Claude35Sonnet)
                } else if model_str.contains("opus") {
                    Ok(ClaudeModel::Claude3Opus)
                } else if model_str.contains("sonnet") {
                    Ok(ClaudeModel::Claude3Sonnet)
                } else if model_str.contains("haiku") {
                    Ok(ClaudeModel::Claude3Haiku)
                } else {
                    Err(anyhow!("Unknown model: {}", model_str))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AnthropicConfig {
        AnthropicConfig {
            api_key: "test-key".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            timeout_secs: 60,
            max_retries: 3,
            rate_limit_per_minute: 50,
            api_version: "2023-06-01".to_string(),
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = test_config();
        let client = AnthropicClient::new(config).await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_rate_limit() {
        let mut config = test_config();
        config.rate_limit_per_minute = 0;
        let client = AnthropicClient::new(config).await;
        assert!(client.is_err());
    }

    #[tokio::test]
    async fn test_token_counting() {
        let config = test_config();
        let client = AnthropicClient::new(config).await.unwrap();

        let text = "Hello, world!";
        let tokens = client.count_tokens(text);
        assert!(tokens > 0);
    }

    #[tokio::test]
    async fn test_validate_request() {
        let config = test_config();
        let client = AnthropicClient::new(config).await.unwrap();

        let valid_request = MessageRequest {
            model: ClaudeModel::Claude3Haiku.as_str().to_string(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("Test".to_string()),
            }],
            max_tokens: 100,
            system: None,
            temperature: Some(0.7),
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: false,
            metadata: None,
        };

        assert!(client.validate_request(&valid_request).is_ok());

        let invalid_request = MessageRequest {
            model: ClaudeModel::Claude3Haiku.as_str().to_string(),
            messages: vec![],
            max_tokens: 0,
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: false,
            metadata: None,
        };

        assert!(client.validate_request(&invalid_request).is_err());
    }
}
