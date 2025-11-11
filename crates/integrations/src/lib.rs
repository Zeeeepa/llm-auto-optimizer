//! # LLM Auto Optimizer - Integrations
//!
//! Production-ready integrations with external services for the LLM Auto Optimizer.
//!
//! ## Features
//!
//! ### Jira Integration
//!
//! - Full CRUD operations for issues
//! - Project and board management
//! - JQL query support
//! - Webhook event handling
//! - OAuth 2.0 and Basic authentication
//! - Rate limiting and retry logic
//!
//! ### Anthropic Claude Integration
//!
//! - Message/completion endpoints
//! - Streaming support via Server-Sent Events
//! - Token counting and validation
//! - Cost tracking and estimation
//! - Rate limiting
//! - Multiple Claude model support
//!
//! ## Examples
//!
//! ### Jira Client
//!
//! ```no_run
//! use integrations::jira::{JiraClient, JiraConfig, JiraAuth};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = JiraConfig {
//!     base_url: "https://your-domain.atlassian.net".to_string(),
//!     auth: JiraAuth::Basic {
//!         email: "your-email@example.com".to_string(),
//!         api_token: "your-api-token".to_string(),
//!     },
//!     timeout_secs: 30,
//!     max_retries: 3,
//!     rate_limit_per_minute: 100,
//! };
//!
//! let client = JiraClient::new(config).await?;
//! let projects = client.get_projects().await?;
//!
//! for project in projects {
//!     println!("{}: {}", project.key, project.name);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Anthropic Client
//!
//! ```no_run
//! use integrations::anthropic::{AnthropicClient, AnthropicConfig, ClaudeModel};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = AnthropicConfig {
//!     api_key: "your-api-key".to_string(),
//!     base_url: "https://api.anthropic.com".to_string(),
//!     timeout_secs: 60,
//!     max_retries: 3,
//!     rate_limit_per_minute: 50,
//!     api_version: "2023-06-01".to_string(),
//! };
//!
//! let client = AnthropicClient::new(config).await?;
//!
//! let response = client.complete(
//!     ClaudeModel::Claude3Haiku,
//!     "What is the capital of France?",
//!     100,
//! ).await?;
//!
//! println!("Response: {}", response);
//!
//! // Get cost statistics
//! let stats = client.get_cost_stats().await;
//! println!("Total cost: ${:.4}", stats.total_cost);
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! The integrations are designed with:
//!
//! - **Modularity**: Each integration is independent and self-contained
//! - **Type Safety**: Comprehensive type definitions with Serde support
//! - **Error Handling**: Detailed error types and context
//! - **Observability**: Built-in logging and tracing
//! - **Resilience**: Automatic retries, rate limiting, and circuit breakers
//! - **Testing**: Comprehensive unit and integration tests
//!
//! ## Production Readiness
//!
//! All integrations include:
//!
//! - Authentication and authorization
//! - Rate limiting and backoff strategies
//! - Comprehensive error handling
//! - Request/response logging
//! - Input validation
//! - Cost tracking (where applicable)
//! - Full test coverage

#![warn(missing_docs)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

/// Jira REST API integration
#[cfg(feature = "jira")]
pub mod jira;

/// Anthropic Claude API integration
#[cfg(feature = "anthropic")]
pub mod anthropic;

// Re-export commonly used types
#[cfg(feature = "jira")]
pub use jira::{JiraAuth, JiraClient, JiraConfig};

#[cfg(feature = "anthropic")]
pub use anthropic::{AnthropicClient, AnthropicConfig, ClaudeModel};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get the library version
pub fn version() -> &'static str {
    VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = version();
        assert!(!version.is_empty());
    }

    #[cfg(feature = "jira")]
    #[test]
    fn test_jira_module_exists() {
        // Just verify the module compiles and is accessible
        let _ = std::any::TypeId::of::<JiraConfig>();
    }

    #[cfg(feature = "anthropic")]
    #[test]
    fn test_anthropic_module_exists() {
        // Just verify the module compiles and is accessible
        let _ = std::any::TypeId::of::<AnthropicConfig>();
    }
}
