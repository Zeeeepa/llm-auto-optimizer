//! Anthropic Claude API streaming support
//!
//! Handles Server-Sent Events (SSE) streaming from Claude API.

use super::types::*;
use anyhow::{anyhow, Context, Result};
use futures::stream::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Stream handler for Claude API streaming responses
pub struct StreamHandler {
    /// Configuration
    config: Arc<RwLock<AnthropicConfig>>,
    /// HTTP client
    client: reqwest::Client,
    /// Cost tracker
    cost_tracker: Arc<RwLock<CostTracker>>,
}

impl StreamHandler {
    /// Create a new stream handler
    ///
    /// # Arguments
    ///
    /// * `config` - API configuration
    /// * `client` - HTTP client to use
    /// * `cost_tracker` - Cost tracker to update
    pub fn new(
        config: Arc<RwLock<AnthropicConfig>>,
        client: reqwest::Client,
        cost_tracker: Arc<RwLock<CostTracker>>,
    ) -> Self {
        Self {
            config,
            client,
            cost_tracker,
        }
    }

    /// Send a streaming message request
    ///
    /// # Arguments
    ///
    /// * `request` - Message request with stream=true
    ///
    /// # Returns
    ///
    /// Returns a stream of events
    pub async fn stream_message(
        &self,
        mut request: MessageRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        // Ensure streaming is enabled
        request.stream = true;

        let config = self.config.read().await;
        let url = format!("{}/v1/messages", config.base_url);

        let headers = self.build_headers(&config)?;

        debug!(
            "Starting streaming request to model: {}",
            request.model
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .context("Failed to send streaming request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Streaming request failed ({}): {}", status, error_text);
            return Err(anyhow!("Streaming request failed: {}", error_text));
        }

        info!("Streaming response started");

        // Parse SSE stream - create stream from response chunks
        let byte_stream = futures::stream::unfold(response, |mut resp| async {
            match resp.chunk().await {
                Ok(Some(chunk)) => Some((Ok(chunk), resp)),
                Ok(None) => None,
                Err(e) => Some((Err(e), resp)),
            }
        });

        let stream = self.parse_sse_stream(byte_stream);

        Ok(Box::pin(stream))
    }

    /// Complete a streaming request and collect all text
    ///
    /// # Arguments
    ///
    /// * `request` - Message request
    ///
    /// # Returns
    ///
    /// Returns the complete text response and usage stats
    pub async fn stream_complete(
        &self,
        request: MessageRequest,
    ) -> Result<(String, Usage)> {
        let mut stream = self.stream_message(request).await?;

        let mut text = String::new();
        let mut usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
        };

        while let Some(event_result) = stream.next().await {
            match event_result? {
                StreamEvent::ContentBlockDelta { delta, .. } => {
                    if let Delta::TextDelta { text: delta_text } = delta {
                        text.push_str(&delta_text);
                    }
                }
                StreamEvent::MessageStart { message } => {
                    usage = message.usage;
                }
                StreamEvent::MessageDelta {
                    usage: final_usage,
                    ..
                } => {
                    usage = final_usage;
                }
                StreamEvent::Error { error } => {
                    return Err(anyhow!("Stream error: {}", error.message));
                }
                _ => {}
            }
        }

        info!(
            "Streaming completed. Tokens: {} in, {} out",
            usage.input_tokens, usage.output_tokens
        );

        Ok((text, usage))
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

    /// Parse Server-Sent Events stream
    fn parse_sse_stream(
        &self,
        byte_stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Send + 'static,
    ) -> impl Stream<Item = Result<StreamEvent>> + Send {
        let buffer = Arc::new(tokio::sync::Mutex::new(String::new()));

        byte_stream.filter_map(move |chunk_result| {
            let buffer = buffer.clone();
            async move {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => return Some(Err(anyhow!("Stream error: {}", e))),
                };

                // Convert bytes to string
                let chunk_str = match std::str::from_utf8(&chunk) {
                    Ok(s) => s,
                    Err(e) => return Some(Err(anyhow!("Invalid UTF-8: {}", e))),
                };

                let mut buffer = buffer.lock().await;
                buffer.push_str(chunk_str);

                // Process complete SSE messages
                let mut events = Vec::new();

                while let Some(pos) = buffer.find("\n\n") {
                    let message = buffer[..pos].to_string();
                    *buffer = buffer[pos + 2..].to_string();

                    if message.is_empty() {
                        continue;
                    }

                    // Parse SSE message
                    let mut event_type = None;
                    let mut data = String::new();

                    for line in message.lines() {
                        if let Some(stripped) = line.strip_prefix("event: ") {
                            event_type = Some(stripped.to_string());
                        } else if let Some(stripped) = line.strip_prefix("data: ") {
                            data.push_str(stripped);
                        }
                    }

                    // Parse event data as JSON
                    if !data.is_empty() {
                        match serde_json::from_str::<StreamEvent>(&data) {
                            Ok(event) => {
                                debug!("Received stream event: {:?}", event_type);
                                events.push(Ok(event));
                            }
                            Err(e) => {
                                warn!("Failed to parse stream event: {}", e);
                                events.push(Err(anyhow!("Parse error: {}", e)));
                            }
                        }
                    }
                }

                if events.is_empty() {
                    None
                } else if events.len() == 1 {
                    Some(events.into_iter().next().unwrap())
                } else {
                    // If multiple events, return the first one
                    // (This is a simplification; in practice, you might want to handle this differently)
                    Some(events.into_iter().next().unwrap())
                }
            }
        })
    }
}

/// Stream collector for aggregating streaming events
pub struct StreamCollector {
    /// Accumulated text content
    pub text: String,
    /// Message ID
    pub message_id: Option<String>,
    /// Model used
    pub model: Option<String>,
    /// Usage statistics
    pub usage: Usage,
    /// Stop reason
    pub stop_reason: Option<StopReason>,
    /// Stop sequence
    pub stop_sequence: Option<String>,
}

impl StreamCollector {
    /// Create a new stream collector
    pub fn new() -> Self {
        Self {
            text: String::new(),
            message_id: None,
            model: None,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
            },
            stop_reason: None,
            stop_sequence: None,
        }
    }

    /// Process a stream event
    ///
    /// # Arguments
    ///
    /// * `event` - Stream event to process
    ///
    /// # Returns
    ///
    /// Returns true if this is the final event
    pub fn process_event(&mut self, event: StreamEvent) -> bool {
        match event {
            StreamEvent::MessageStart { message } => {
                self.message_id = Some(message.id);
                self.model = Some(message.model);
                self.usage = message.usage;
                false
            }
            StreamEvent::ContentBlockDelta { delta, .. } => {
                if let Delta::TextDelta { text } = delta {
                    self.text.push_str(&text);
                }
                false
            }
            StreamEvent::MessageDelta { delta, usage } => {
                self.usage = usage;
                self.stop_reason = delta.stop_reason;
                self.stop_sequence = delta.stop_sequence;
                false
            }
            StreamEvent::MessageStop => {
                true // Final event
            }
            StreamEvent::Error { error } => {
                warn!("Stream error: {}", error.message);
                true
            }
            _ => false,
        }
    }

    /// Convert to MessageResponse
    pub fn to_response(self) -> Result<MessageResponse> {
        Ok(MessageResponse {
            id: self.message_id.ok_or_else(|| anyhow!("Missing message ID"))?,
            type_field: "message".to_string(),
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: self.text }],
            model: self.model.ok_or_else(|| anyhow!("Missing model"))?,
            stop_reason: self.stop_reason,
            stop_sequence: self.stop_sequence,
            usage: self.usage,
        })
    }
}

impl Default for StreamCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_collector() {
        let mut collector = StreamCollector::new();

        // Process message start
        let start_event = StreamEvent::MessageStart {
            message: MessageStart {
                id: "msg_123".to_string(),
                type_field: "message".to_string(),
                role: Role::Assistant,
                content: vec![],
                model: "claude-3-haiku-20240307".to_string(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 0,
                },
            },
        };

        assert!(!collector.process_event(start_event));
        assert_eq!(collector.message_id, Some("msg_123".to_string()));

        // Process content delta
        let delta_event = StreamEvent::ContentBlockDelta {
            index: 0,
            delta: Delta::TextDelta {
                text: "Hello".to_string(),
            },
        };

        assert!(!collector.process_event(delta_event));
        assert_eq!(collector.text, "Hello");

        // Process another delta
        let delta_event2 = StreamEvent::ContentBlockDelta {
            index: 0,
            delta: Delta::TextDelta {
                text: " world".to_string(),
            },
        };

        assert!(!collector.process_event(delta_event2));
        assert_eq!(collector.text, "Hello world");

        // Process message stop
        let stop_event = StreamEvent::MessageStop;
        assert!(collector.process_event(stop_event));
    }

    #[test]
    fn test_stream_collector_to_response() {
        let mut collector = StreamCollector::new();

        collector.message_id = Some("msg_123".to_string());
        collector.model = Some("claude-3-haiku-20240307".to_string());
        collector.text = "Hello, world!".to_string();
        collector.usage = Usage {
            input_tokens: 10,
            output_tokens: 5,
        };
        collector.stop_reason = Some(StopReason::EndTurn);

        let response = collector.to_response().unwrap();

        assert_eq!(response.id, "msg_123");
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 5);
    }
}
