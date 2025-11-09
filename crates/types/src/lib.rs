//! Core types and data models for LLM Auto-Optimizer
//!
//! This crate provides all the fundamental data structures used throughout
//! the LLM Auto-Optimizer system.

pub mod events;
pub mod decisions;
pub mod models;
pub mod experiments;
pub mod metrics;
pub mod errors;
pub mod strategies;

pub use errors::{OptimizerError, Result};
