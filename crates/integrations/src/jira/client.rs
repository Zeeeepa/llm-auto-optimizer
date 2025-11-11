//! Jira REST API client
//!
//! Production-ready client for Jira Cloud and Server APIs with comprehensive
//! error handling, rate limiting, and retry logic.

use super::auth::AuthManager;
use super::types::*;
use anyhow::{anyhow, Context, Result};
use governor::{Quota, RateLimiter};
use reqwest::StatusCode;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Jira API client
#[derive(Clone)]
pub struct JiraClient {
    /// HTTP client
    client: reqwest::Client,
    /// Authentication manager
    auth: AuthManager,
    /// Rate limiter
    rate_limiter: Arc<RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
}

impl JiraClient {
    /// Create a new Jira client
    ///
    /// # Arguments
    ///
    /// * `config` - Jira configuration
    ///
    /// # Returns
    ///
    /// Returns a new JiraClient instance
    pub async fn new(config: JiraConfig) -> Result<Self> {
        let auth = AuthManager::new(config.clone());
        let timeout = Duration::from_secs(config.timeout_secs);

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("llm-auto-optimizer/1.0")
            .build()
            .context("Failed to create HTTP client")?;

        // Setup rate limiter based on config
        let rate_limit = NonZeroU32::new(config.rate_limit_per_minute)
            .ok_or_else(|| anyhow!("Rate limit must be greater than 0"))?;
        let quota = Quota::per_minute(rate_limit);
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        info!(
            "Initialized Jira client for: {}",
            config.base_url
        );

        Ok(Self {
            client,
            auth,
            rate_limiter,
        })
    }

    /// Create a new issue
    ///
    /// # Arguments
    ///
    /// * `request` - Issue creation request
    ///
    /// # Returns
    ///
    /// Returns the created issue
    pub async fn create_issue(&self, request: CreateIssueRequest) -> Result<Issue> {
        let url = format!("{}/rest/api/3/issue", self.auth.get_base_url().await);

        debug!("Creating issue: {}", request.fields.summary);

        let response: serde_json::Value = self
            .execute_request(
                self.client
                    .post(&url)
                    .json(&request),
            )
            .await?;

        let issue_key = response["key"]
            .as_str()
            .ok_or_else(|| anyhow!("No issue key in response"))?;

        info!("Created issue: {}", issue_key);

        // Fetch the full issue details
        self.get_issue(issue_key).await
    }

    /// Get an issue by key
    ///
    /// # Arguments
    ///
    /// * `issue_key` - Issue key (e.g., "PROJ-123")
    ///
    /// # Returns
    ///
    /// Returns the issue details
    pub async fn get_issue(&self, issue_key: &str) -> Result<Issue> {
        let url = format!(
            "{}/rest/api/3/issue/{}",
            self.auth.get_base_url().await,
            issue_key
        );

        debug!("Fetching issue: {}", issue_key);

        let issue: Issue = self
            .execute_request(self.client.get(&url))
            .await?;

        Ok(issue)
    }

    /// Update an issue
    ///
    /// # Arguments
    ///
    /// * `issue_key` - Issue key to update
    /// * `request` - Update request
    ///
    /// # Returns
    ///
    /// Returns Ok(()) on success
    pub async fn update_issue(
        &self,
        issue_key: &str,
        request: UpdateIssueRequest,
    ) -> Result<()> {
        let url = format!(
            "{}/rest/api/3/issue/{}",
            self.auth.get_base_url().await,
            issue_key
        );

        debug!("Updating issue: {}", issue_key);

        self.execute_request_no_response(
            self.client
                .put(&url)
                .json(&request),
        )
        .await?;

        info!("Updated issue: {}", issue_key);
        Ok(())
    }

    /// Delete an issue
    ///
    /// # Arguments
    ///
    /// * `issue_key` - Issue key to delete
    ///
    /// # Returns
    ///
    /// Returns Ok(()) on success
    pub async fn delete_issue(&self, issue_key: &str) -> Result<()> {
        let url = format!(
            "{}/rest/api/3/issue/{}",
            self.auth.get_base_url().await,
            issue_key
        );

        debug!("Deleting issue: {}", issue_key);

        self.execute_request_no_response(self.client.delete(&url))
            .await?;

        info!("Deleted issue: {}", issue_key);
        Ok(())
    }

    /// Search issues using JQL
    ///
    /// # Arguments
    ///
    /// * `request` - JQL search request
    ///
    /// # Returns
    ///
    /// Returns search results
    pub async fn search_issues(&self, request: JqlSearchRequest) -> Result<JqlSearchResponse> {
        let url = format!("{}/rest/api/3/search", self.auth.get_base_url().await);

        debug!("Searching issues with JQL: {}", request.jql);

        let response: JqlSearchResponse = self
            .execute_request(
                self.client
                    .post(&url)
                    .json(&request),
            )
            .await?;

        info!(
            "Found {} issues (showing {}-{})",
            response.total,
            response.start_at,
            response.start_at + response.issues.len() as u32
        );

        Ok(response)
    }

    /// Get all projects
    ///
    /// # Returns
    ///
    /// Returns a list of projects
    pub async fn get_projects(&self) -> Result<Vec<Project>> {
        let url = format!("{}/rest/api/3/project", self.auth.get_base_url().await);

        debug!("Fetching all projects");

        let projects: Vec<Project> = self
            .execute_request(self.client.get(&url))
            .await?;

        info!("Found {} projects", projects.len());
        Ok(projects)
    }

    /// Get a project by key
    ///
    /// # Arguments
    ///
    /// * `project_key` - Project key
    ///
    /// # Returns
    ///
    /// Returns project details
    pub async fn get_project(&self, project_key: &str) -> Result<Project> {
        let url = format!(
            "{}/rest/api/3/project/{}",
            self.auth.get_base_url().await,
            project_key
        );

        debug!("Fetching project: {}", project_key);

        let project: Project = self
            .execute_request(self.client.get(&url))
            .await?;

        Ok(project)
    }

    /// Get all boards (Agile API)
    ///
    /// # Returns
    ///
    /// Returns a list of boards
    pub async fn get_boards(&self) -> Result<Vec<Board>> {
        let url = format!(
            "{}/rest/agile/1.0/board",
            self.auth.get_base_url().await
        );

        debug!("Fetching all boards");

        #[derive(serde::Deserialize)]
        struct BoardsResponse {
            values: Vec<Board>,
        }

        let response: BoardsResponse = self
            .execute_request(self.client.get(&url))
            .await?;

        info!("Found {} boards", response.values.len());
        Ok(response.values)
    }

    /// Get sprints for a board
    ///
    /// # Arguments
    ///
    /// * `board_id` - Board ID
    ///
    /// # Returns
    ///
    /// Returns a list of sprints
    pub async fn get_board_sprints(&self, board_id: u64) -> Result<Vec<Sprint>> {
        let url = format!(
            "{}/rest/agile/1.0/board/{}/sprint",
            self.auth.get_base_url().await,
            board_id
        );

        debug!("Fetching sprints for board: {}", board_id);

        #[derive(serde::Deserialize)]
        struct SprintsResponse {
            values: Vec<Sprint>,
        }

        let response: SprintsResponse = self
            .execute_request(self.client.get(&url))
            .await?;

        info!("Found {} sprints for board {}", response.values.len(), board_id);
        Ok(response.values)
    }

    /// Execute an HTTP request with retry logic and error handling
    async fn execute_request<T: serde::de::DeserializeOwned>(
        &self,
        request_builder: reqwest::RequestBuilder,
    ) -> Result<T> {
        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        let max_retries = self.auth.get_max_retries().await;
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = Duration::from_secs(2u64.pow(attempt));
                debug!("Retrying after {:?} (attempt {})", delay, attempt);
                sleep(delay).await;
            }

            // Clone the request builder by rebuilding it
            let headers = self.auth.get_auth_headers().await?;
            let request = request_builder
                .try_clone()
                .ok_or_else(|| anyhow!("Failed to clone request"))?
                .headers(headers);

            match request.send().await {
                Ok(response) => {
                    let status = response.status();

                    // Handle rate limiting
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        warn!("Rate limited by Jira API");
                        last_error = Some(anyhow!("Rate limited"));
                        continue;
                    }

                    // Handle auth errors - try token refresh
                    if status == StatusCode::UNAUTHORIZED {
                        debug!("Unauthorized - attempting token refresh");
                        if self.auth.refresh_token_if_needed(&self.client).await? {
                            last_error = Some(anyhow!("Token expired, refreshed"));
                            continue;
                        }
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
                    let error_msg = if let Ok(err) = serde_json::from_str::<ErrorResponse>(&error_text) {
                        format!(
                            "Jira API error: {}",
                            err.error_messages.join(", ")
                        )
                    } else {
                        format!("Jira API error ({}): {}", status, error_text)
                    };

                    error!("{}", error_msg);
                    last_error = Some(anyhow!(error_msg));

                    // Don't retry on client errors (except auth/rate limit)
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

    /// Execute a request that doesn't return a body
    async fn execute_request_no_response(
        &self,
        request_builder: reqwest::RequestBuilder,
    ) -> Result<()> {
        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        let max_retries = self.auth.get_max_retries().await;
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = Duration::from_secs(2u64.pow(attempt));
                debug!("Retrying after {:?} (attempt {})", delay, attempt);
                sleep(delay).await;
            }

            let headers = self.auth.get_auth_headers().await?;
            let request = request_builder
                .try_clone()
                .ok_or_else(|| anyhow!("Failed to clone request"))?
                .headers(headers);

            match request.send().await {
                Ok(response) => {
                    let status = response.status();

                    if status == StatusCode::TOO_MANY_REQUESTS {
                        warn!("Rate limited by Jira API");
                        last_error = Some(anyhow!("Rate limited"));
                        continue;
                    }

                    if status == StatusCode::UNAUTHORIZED {
                        debug!("Unauthorized - attempting token refresh");
                        if self.auth.refresh_token_if_needed(&self.client).await? {
                            last_error = Some(anyhow!("Token expired, refreshed"));
                            continue;
                        }
                    }

                    if response.status().is_success() {
                        return Ok(());
                    }

                    let error_text = response.text().await.unwrap_or_default();
                    error!("Jira API error ({}): {}", status, error_text);
                    last_error = Some(anyhow!("Jira API error: {}", error_text));

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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> JiraConfig {
        JiraConfig {
            base_url: "https://test.atlassian.net".to_string(),
            auth: JiraAuth::Basic {
                email: "test@example.com".to_string(),
                api_token: "test-token".to_string(),
            },
            timeout_secs: 30,
            max_retries: 3,
            rate_limit_per_minute: 100,
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = test_config();
        let client = JiraClient::new(config).await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_rate_limit() {
        let mut config = test_config();
        config.rate_limit_per_minute = 0;
        let client = JiraClient::new(config).await;
        assert!(client.is_err());
    }
}
