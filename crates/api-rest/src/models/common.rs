//! Common request/response models

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct Pagination {
    /// Page number (starting from 1)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Page size
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
        }
    }
}

impl Pagination {
    /// Calculate offset for database queries
    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.page_size
    }

    /// Get limit
    pub fn limit(&self) -> u32 {
        self.page_size
    }
}

/// Paginated response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    /// Items in current page
    pub items: Vec<T>,
    /// Total number of items
    pub total: u64,
    /// Current page number
    pub page: u32,
    /// Page size
    pub page_size: u32,
    /// Total number of pages
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response
    pub fn new(items: Vec<T>, total: u64, pagination: &Pagination) -> Self {
        let total_pages = ((total as f64) / (pagination.page_size as f64)).ceil() as u32;

        Self {
            items,
            total,
            page: pagination.page,
            page_size: pagination.page_size,
            total_pages,
        }
    }
}

/// Sort direction
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        Self::Desc
    }
}

/// Date range filter
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DateRange {
    /// Start date (ISO 8601)
    pub start: chrono::DateTime<chrono::Utc>,
    /// End date (ISO 8601)
    pub end: chrono::DateTime<chrono::Utc>,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Response data
    pub data: T,
    /// Request ID for tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Response metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl<T> ApiResponse<T> {
    /// Create a new API response
    pub fn new(data: T) -> Self {
        Self {
            data,
            request_id: None,
            metadata: None,
        }
    }

    /// Add request ID
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_offset() {
        let p = Pagination {
            page: 1,
            page_size: 20,
        };
        assert_eq!(p.offset(), 0);

        let p = Pagination {
            page: 3,
            page_size: 20,
        };
        assert_eq!(p.offset(), 40);
    }

    #[test]
    fn test_paginated_response() {
        let items = vec![1, 2, 3];
        let pagination = Pagination {
            page: 1,
            page_size: 3,
        };

        let response = PaginatedResponse::new(items, 10, &pagination);

        assert_eq!(response.total, 10);
        assert_eq!(response.page, 1);
        assert_eq!(response.total_pages, 4);
    }

    #[test]
    fn test_api_response() {
        let response = ApiResponse::new("test data")
            .with_request_id("req-123".to_string())
            .with_metadata(serde_json::json!({"key": "value"}));

        assert_eq!(response.data, "test data");
        assert_eq!(response.request_id, Some("req-123".to_string()));
        assert!(response.metadata.is_some());
    }
}
