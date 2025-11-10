//! Stream operators for transforming and processing events
//!
//! This module provides the core operators for stream processing pipelines:
//! - Map: Transform events one-to-one
//! - Filter: Select events based on predicates
//! - FlatMap: Transform events one-to-many
//! - KeyBy: Partition events by key
//! - Window: Assign events to windows
//! - Aggregate: Compute aggregations over windows

use crate::aggregation::Aggregator;
use crate::core::{EventKey, EventTimeExtractor, KeyExtractor};
use crate::error::Result;
use crate::watermark::Watermark;
use crate::window::{Window, WindowAssigner};
use async_trait::async_trait;
use serde::Serialize;
use std::fmt::Debug;
use std::sync::Arc;

/// Context provided to operators during execution
#[derive(Debug, Clone)]
pub struct OperatorContext {
    /// Current watermark
    pub watermark: Watermark,

    /// Partition ID
    pub partition: u32,

    /// Processing timestamp
    pub processing_time: i64,
}

impl OperatorContext {
    /// Create a new operator context
    pub fn new(watermark: Watermark, partition: u32) -> Self {
        Self {
            watermark,
            partition,
            processing_time: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Trait for stream operators
///
/// All stream processing operators implement this trait to provide a uniform
/// interface for processing events in the pipeline.
#[async_trait]
pub trait StreamOperator: Send + Sync + Debug {
    /// Process an event and return transformed events
    ///
    /// # Arguments
    /// * `input` - The input event data
    /// * `ctx` - Operator context with watermark and partition info
    ///
    /// # Returns
    /// A vector of output events (may be empty, one, or many)
    async fn process(
        &self,
        input: Vec<u8>,
        ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>>;

    /// Get the operator name
    fn name(&self) -> &str;

    /// Called when the operator is initialized
    async fn open(&mut self) -> Result<()> {
        Ok(())
    }

    /// Called when the operator is shut down
    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Map operator - transforms events one-to-one
///
/// # Example
///
/// ```rust
/// use processor::pipeline::operator::MapOperator;
///
/// // Create a map operator that doubles values
/// let operator = MapOperator::new("double", |x: i32| x * 2);
/// ```
#[derive(Clone)]
pub struct MapOperator<F, T, U>
where
    F: Fn(T) -> U + Send + Sync + Clone,
    T: Send + Sync + Clone,
    U: Send + Sync + Clone,
{
    name: String,
    func: F,
    _phantom: std::marker::PhantomData<(T, U)>,
}

impl<F, T, U> MapOperator<F, T, U>
where
    F: Fn(T) -> U + Send + Sync + Clone,
    T: Send + Sync + Clone + serde::de::DeserializeOwned,
    U: Send + Sync + Clone + Serialize,
{
    /// Create a new map operator
    pub fn new<S: Into<String>>(name: S, func: F) -> Self {
        Self {
            name: name.into(),
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<F, T, U> Debug for MapOperator<F, T, U>
where
    F: Fn(T) -> U + Send + Sync + Clone,
    T: Send + Sync + Clone,
    U: Send + Sync + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapOperator")
            .field("name", &self.name)
            .finish()
    }
}

#[async_trait]
impl<F, T, U> StreamOperator for MapOperator<F, T, U>
where
    F: Fn(T) -> U + Send + Sync + Clone,
    T: Send + Sync + Clone + serde::de::DeserializeOwned,
    U: Send + Sync + Clone + Serialize,
{
    async fn process(
        &self,
        input: Vec<u8>,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        let value: T = bincode::deserialize(&input)?;
        let result = (self.func)(value);
        let serialized = bincode::serialize(&result)?;
        Ok(vec![serialized])
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Filter operator - selects events based on a predicate
///
/// # Example
///
/// ```rust
/// use processor::pipeline::operator::FilterOperator;
///
/// // Create a filter that only passes even numbers
/// let operator = FilterOperator::new("even", |x: &i32| x % 2 == 0);
/// ```
#[derive(Clone)]
pub struct FilterOperator<F, T>
where
    F: Fn(&T) -> bool + Send + Sync + Clone,
    T: Send + Sync + Clone,
{
    name: String,
    predicate: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<F, T> FilterOperator<F, T>
where
    F: Fn(&T) -> bool + Send + Sync + Clone,
    T: Send + Sync + Clone + serde::de::DeserializeOwned,
{
    /// Create a new filter operator
    pub fn new<S: Into<String>>(name: S, predicate: F) -> Self {
        Self {
            name: name.into(),
            predicate,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<F, T> Debug for FilterOperator<F, T>
where
    F: Fn(&T) -> bool + Send + Sync + Clone,
    T: Send + Sync + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterOperator")
            .field("name", &self.name)
            .finish()
    }
}

#[async_trait]
impl<F, T> StreamOperator for FilterOperator<F, T>
where
    F: Fn(&T) -> bool + Send + Sync + Clone,
    T: Send + Sync + Clone + serde::de::DeserializeOwned + Serialize,
{
    async fn process(
        &self,
        input: Vec<u8>,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        let value: T = bincode::deserialize(&input)?;
        if (self.predicate)(&value) {
            Ok(vec![input])
        } else {
            Ok(vec![])
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// FlatMap operator - transforms events one-to-many
///
/// # Example
///
/// ```rust
/// use processor::pipeline::operator::FlatMapOperator;
///
/// // Create a flatmap that splits strings into words
/// let operator = FlatMapOperator::new("split_words", |s: String| {
///     s.split_whitespace().map(|w| w.to_string()).collect::<Vec<_>>()
/// });
/// ```
#[derive(Clone)]
pub struct FlatMapOperator<F, T, U>
where
    F: Fn(T) -> Vec<U> + Send + Sync + Clone,
    T: Send + Sync + Clone,
    U: Send + Sync + Clone,
{
    name: String,
    func: F,
    _phantom: std::marker::PhantomData<(T, U)>,
}

impl<F, T, U> FlatMapOperator<F, T, U>
where
    F: Fn(T) -> Vec<U> + Send + Sync + Clone,
    T: Send + Sync + Clone + serde::de::DeserializeOwned,
    U: Send + Sync + Clone + Serialize,
{
    /// Create a new flatmap operator
    pub fn new<S: Into<String>>(name: S, func: F) -> Self {
        Self {
            name: name.into(),
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<F, T, U> Debug for FlatMapOperator<F, T, U>
where
    F: Fn(T) -> Vec<U> + Send + Sync + Clone,
    T: Send + Sync + Clone,
    U: Send + Sync + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlatMapOperator")
            .field("name", &self.name)
            .finish()
    }
}

#[async_trait]
impl<F, T, U> StreamOperator for FlatMapOperator<F, T, U>
where
    F: Fn(T) -> Vec<U> + Send + Sync + Clone,
    T: Send + Sync + Clone + serde::de::DeserializeOwned,
    U: Send + Sync + Clone + Serialize,
{
    async fn process(
        &self,
        input: Vec<u8>,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        let value: T = bincode::deserialize(&input)?;
        let results = (self.func)(value);
        let mut serialized = Vec::new();
        for result in results {
            serialized.push(bincode::serialize(&result)?);
        }
        Ok(serialized)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// KeyBy operator - partitions events by key
///
/// # Example
///
/// ```rust
/// use processor::pipeline::operator::KeyByOperator;
/// use processor::core::{EventKey, KeyExtractor};
///
/// // Define a key extractor
/// struct MyKeyExtractor;
/// impl KeyExtractor<i32> for MyKeyExtractor {
///     fn extract_key(&self, event: &i32) -> EventKey {
///         EventKey::Session(event.to_string())
///     }
/// }
///
/// let operator = KeyByOperator::new("partition", MyKeyExtractor);
/// ```
pub struct KeyByOperator<T, E>
where
    T: Send + Sync + Clone,
    E: KeyExtractor<T> + Send + Sync,
{
    name: String,
    extractor: E,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, E> KeyByOperator<T, E>
where
    T: Send + Sync + Clone + serde::de::DeserializeOwned,
    E: KeyExtractor<T> + Send + Sync,
{
    /// Create a new key-by operator
    pub fn new<S: Into<String>>(name: S, extractor: E) -> Self {
        Self {
            name: name.into(),
            extractor,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Extract the key from an event
    pub fn extract_key(&self, event: &T) -> EventKey {
        self.extractor.extract_key(event)
    }
}

impl<T, E> Debug for KeyByOperator<T, E>
where
    T: Send + Sync + Clone,
    E: KeyExtractor<T> + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyByOperator")
            .field("name", &self.name)
            .finish()
    }
}

#[async_trait]
impl<T, E> StreamOperator for KeyByOperator<T, E>
where
    T: Send + Sync + Clone + serde::de::DeserializeOwned + Serialize,
    E: KeyExtractor<T> + Send + Sync,
{
    async fn process(
        &self,
        input: Vec<u8>,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        // KeyBy doesn't transform data, just extracts keys for partitioning
        // The actual partitioning is handled by the executor
        Ok(vec![input])
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Window operator - assigns events to windows
///
/// # Example
///
/// ```rust
/// use processor::pipeline::operator::WindowOperator;
/// use processor::window::{TumblingWindowAssigner, WindowAssigner};
/// use processor::core::EventTimeExtractor;
/// use std::time::Duration;
///
/// // Define a time extractor
/// struct MyTimeExtractor;
/// impl EventTimeExtractor<i64> for MyTimeExtractor {
///     fn extract_event_time(&self, event: &i64) -> chrono::DateTime<chrono::Utc> {
///         chrono::DateTime::from_timestamp(*event, 0).unwrap()
///     }
/// }
///
/// let assigner = TumblingWindowAssigner::new(Duration::from_secs(60));
/// let operator = WindowOperator::new("windows", assigner, MyTimeExtractor);
/// ```
pub struct WindowOperator<T, A, E>
where
    T: Send + Sync + Clone,
    A: WindowAssigner + Send + Sync,
    E: EventTimeExtractor<T> + Send + Sync,
{
    name: String,
    assigner: A,
    time_extractor: E,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, A, E> WindowOperator<T, A, E>
where
    T: Send + Sync + Clone + serde::de::DeserializeOwned,
    A: WindowAssigner + Send + Sync,
    E: EventTimeExtractor<T> + Send + Sync,
{
    /// Create a new window operator
    pub fn new<S: Into<String>>(name: S, assigner: A, time_extractor: E) -> Self {
        Self {
            name: name.into(),
            assigner,
            time_extractor,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Assign an event to windows
    pub fn assign_windows(&self, event: &T, _key: &str) -> Vec<Window> {
        let timestamp = self.time_extractor.extract_event_time(event);
        self.assigner.assign_windows(timestamp)
    }
}

impl<T, A, E> Debug for WindowOperator<T, A, E>
where
    T: Send + Sync + Clone,
    A: WindowAssigner + Send + Sync,
    E: EventTimeExtractor<T> + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowOperator")
            .field("name", &self.name)
            .finish()
    }
}

#[async_trait]
impl<T, A, E> StreamOperator for WindowOperator<T, A, E>
where
    T: Send + Sync + Clone + serde::de::DeserializeOwned + Serialize,
    A: WindowAssigner + Send + Sync,
    E: EventTimeExtractor<T> + Send + Sync,
{
    async fn process(
        &self,
        input: Vec<u8>,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        // Window operator doesn't transform data, just assigns windows
        // The actual windowing logic is handled by the executor
        Ok(vec![input])
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Aggregate operator - computes aggregations over windows
///
/// # Example
///
/// ```rust
/// use processor::pipeline::operator::AggregateOperator;
/// use processor::aggregation::{Aggregator, SumAggregator};
///
/// let aggregator = SumAggregator::new();
/// let operator = AggregateOperator::new("sum", aggregator);
/// ```
pub struct AggregateOperator<T, A>
where
    T: Send + Sync + Clone,
    A: Aggregator<Input = T> + Send + Sync,
{
    name: String,
    aggregator: Arc<std::sync::Mutex<A>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, A> AggregateOperator<T, A>
where
    T: Send + Sync + Clone,
    A: Aggregator<Input = T> + Send + Sync,
{
    /// Create a new aggregate operator
    pub fn new<S: Into<String>>(name: S, aggregator: A) -> Self {
        Self {
            name: name.into(),
            aggregator: Arc::new(std::sync::Mutex::new(aggregator)),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get a reference to the aggregator
    pub fn aggregator(&self) -> &Arc<std::sync::Mutex<A>> {
        &self.aggregator
    }
}

impl<T, A> Debug for AggregateOperator<T, A>
where
    T: Send + Sync + Clone,
    A: Aggregator<Input = T> + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AggregateOperator")
            .field("name", &self.name)
            .finish()
    }
}

impl<T, A> Clone for AggregateOperator<T, A>
where
    T: Send + Sync + Clone,
    A: Aggregator<Input = T> + Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            aggregator: Arc::clone(&self.aggregator),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<T, A> StreamOperator for AggregateOperator<T, A>
where
    T: Send + Sync + Clone + serde::de::DeserializeOwned + Serialize,
    A: Aggregator<Input = T, Output = T> + Send + Sync,
{
    async fn process(
        &self,
        input: Vec<u8>,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        let value: T = bincode::deserialize(&input)?;
        let mut agg = self.aggregator.lock().unwrap();
        agg.update(value)?;
        let result = agg.finalize()?;
        let serialized = bincode::serialize(&result)?;
        Ok(vec![serialized])
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Deduplication operator - filters duplicate events
///
/// This operator filters out duplicate events based on a deduplication strategy.
/// It maintains a set of seen event signatures and drops events that match
/// previously processed signatures within the configured TTL window.
///
/// # Example
///
/// ```rust
/// use processor::pipeline::operator::DeduplicationOperator;
/// use processor::config::{DeduplicationConfig, DeduplicationStrategy};
/// use std::time::Duration;
///
/// # async fn example() -> anyhow::Result<()> {
/// let config = DeduplicationConfig::new()
///     .enabled()
///     .with_ttl(Duration::from_secs(3600))
///     .with_strategy(DeduplicationStrategy::ContentHash);
///
/// let operator = DeduplicationOperator::new("dedup", config);
/// # Ok(())
/// # }
/// ```
pub struct DeduplicationOperator {
    name: String,
    config: crate::config::DeduplicationConfig,
    // In-memory cache for recently seen event hashes
    // In production, this would be backed by Redis
    seen_hashes: Arc<std::sync::RwLock<std::collections::HashMap<String, std::time::Instant>>>,
}

impl DeduplicationOperator {
    /// Create a new deduplication operator
    pub fn new<S: Into<String>>(name: S, config: crate::config::DeduplicationConfig) -> Self {
        Self {
            name: name.into(),
            config,
            seen_hashes: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Compute the signature for an event based on the configured strategy
    fn compute_signature(&self, data: &[u8], _strategy: crate::config::DeduplicationStrategy) -> String {
        // For now, use simple content hashing
        // In production, this would handle different strategies
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Check if an event is a duplicate
    fn is_duplicate(&self, signature: &str) -> bool {
        let seen = self.seen_hashes.read().unwrap();
        if let Some(timestamp) = seen.get(signature) {
            // Check if the signature is still within TTL
            timestamp.elapsed() < self.config.ttl
        } else {
            false
        }
    }

    /// Mark an event signature as seen
    fn mark_seen(&self, signature: String) {
        let mut seen = self.seen_hashes.write().unwrap();
        seen.insert(signature, std::time::Instant::now());

        // Periodic cleanup of expired entries (simple implementation)
        // In production, this would be done in a background task
        if seen.len() > 10000 {
            let ttl = self.config.ttl;
            seen.retain(|_, timestamp| timestamp.elapsed() < ttl);
        }
    }

    /// Get deduplication statistics
    pub fn stats(&self) -> DeduplicationStats {
        let seen = self.seen_hashes.read().unwrap();
        DeduplicationStats {
            cached_signatures: seen.len(),
            enabled: self.config.enabled,
        }
    }
}

impl Debug for DeduplicationOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeduplicationOperator")
            .field("name", &self.name)
            .field("enabled", &self.config.enabled)
            .field("strategy", &self.config.strategy)
            .finish()
    }
}

impl Clone for DeduplicationOperator {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            config: self.config.clone(),
            seen_hashes: Arc::clone(&self.seen_hashes),
        }
    }
}

#[async_trait]
impl StreamOperator for DeduplicationOperator {
    async fn process(
        &self,
        input: Vec<u8>,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        // If deduplication is disabled, pass through
        if !self.config.enabled {
            return Ok(vec![input]);
        }

        // Compute event signature
        let signature = self.compute_signature(&input, self.config.strategy);

        // Check for duplicates
        if self.is_duplicate(&signature) {
            // Drop duplicate event
            Ok(vec![])
        } else {
            // Mark as seen and pass through
            self.mark_seen(signature);
            Ok(vec![input])
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Statistics for deduplication operator
#[derive(Debug, Clone)]
pub struct DeduplicationStats {
    /// Number of cached event signatures
    pub cached_signatures: usize,
    /// Whether deduplication is enabled
    pub enabled: bool,
}

/// Chain of operators that can be applied sequentially
pub struct OperatorChain {
    operators: Vec<Arc<dyn StreamOperator>>,
}

impl OperatorChain {
    /// Create a new operator chain
    pub fn new() -> Self {
        Self {
            operators: Vec::new(),
        }
    }

    /// Add an operator to the chain
    pub fn add<O: StreamOperator + 'static>(&mut self, operator: O) {
        self.operators.push(Arc::new(operator));
    }

    /// Get all operators in the chain
    pub fn operators(&self) -> &[Arc<dyn StreamOperator>] {
        &self.operators
    }

    /// Process an event through the entire operator chain
    pub async fn process(
        &self,
        mut input: Vec<Vec<u8>>,
        ctx: &OperatorContext,
    ) -> Result<Vec<Vec<u8>>> {
        for operator in &self.operators {
            let mut output = Vec::new();
            for item in input {
                let results = operator.process(item, ctx).await?;
                output.extend(results);
            }
            input = output;
        }
        Ok(input)
    }

    /// Get the number of operators in the chain
    pub fn len(&self) -> usize {
        self.operators.len()
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.operators.is_empty()
    }
}

impl Default for OperatorChain {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for OperatorChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperatorChain")
            .field("operator_count", &self.operators.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregation::SumAggregator;
    use crate::core::{EventKey, FeedbackEventKeyExtractor, FeedbackEventTimeExtractor};
    use crate::window::TumblingWindowAssigner;
    use llm_optimizer_types::events::{EventSource, EventType, FeedbackEvent};
    use std::time::Duration;

    #[tokio::test]
    async fn test_map_operator() {
        let operator = MapOperator::new("double", |x: i32| x * 2);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let input = bincode::serialize(&5).unwrap();
        let output = operator.process(input, &ctx).await.unwrap();

        assert_eq!(output.len(), 1);
        let result: i32 = bincode::deserialize(&output[0]).unwrap();
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_filter_operator() {
        let operator = FilterOperator::new("even", |x: &i32| x % 2 == 0);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        // Test with even number (should pass)
        let input = bincode::serialize(&4).unwrap();
        let output = operator.process(input, &ctx).await.unwrap();
        assert_eq!(output.len(), 1);

        // Test with odd number (should filter out)
        let input = bincode::serialize(&5).unwrap();
        let output = operator.process(input, &ctx).await.unwrap();
        assert_eq!(output.len(), 0);
    }

    #[tokio::test]
    async fn test_flatmap_operator() {
        let operator = FlatMapOperator::new("range", |n: i32| (0..n).collect::<Vec<_>>());
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let input = bincode::serialize(&3).unwrap();
        let output = operator.process(input, &ctx).await.unwrap();

        assert_eq!(output.len(), 3);
        let v0: i32 = bincode::deserialize(&output[0]).unwrap();
        let v1: i32 = bincode::deserialize(&output[1]).unwrap();
        let v2: i32 = bincode::deserialize(&output[2]).unwrap();
        assert_eq!(v0, 0);
        assert_eq!(v1, 1);
        assert_eq!(v2, 2);
    }

    #[tokio::test]
    async fn test_keyby_operator() {
        let operator = KeyByOperator::new("key-by", FeedbackEventKeyExtractor);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );

        let input = bincode::serialize(&event).unwrap();
        let output = operator.process(input.clone(), &ctx).await.unwrap();

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], input); // KeyBy doesn't transform
    }

    #[tokio::test]
    async fn test_window_operator() {
        let assigner = TumblingWindowAssigner::new(chrono::Duration::seconds(60));
        let operator = WindowOperator::new("window", assigner, FeedbackEventTimeExtractor);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );

        let input = bincode::serialize(&event).unwrap();
        let output = operator.process(input.clone(), &ctx).await.unwrap();

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], input); // Window doesn't transform
    }

    #[tokio::test]
    async fn test_aggregate_operator() {
        let aggregator: SumAggregator<f64> = SumAggregator::new();
        let operator = AggregateOperator::new("sum", aggregator);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        // Process first value
        let input1 = bincode::serialize(&5.0).unwrap();
        operator.process(input1, &ctx).await.unwrap();

        // Process second value
        let input2 = bincode::serialize(&3.0).unwrap();
        let output = operator.process(input2, &ctx).await.unwrap();

        // Result should be the sum
        assert_eq!(output.len(), 1);
        let result: f64 = bincode::deserialize(&output[0]).unwrap();
        assert_eq!(result, 8.0);
    }

    #[tokio::test]
    async fn test_operator_chain() {
        let mut chain = OperatorChain::new();

        // Add operators to the chain
        chain.add(MapOperator::new("add-one", |x: i32| x + 1));
        chain.add(FilterOperator::new("even", |x: &i32| x % 2 == 0));
        chain.add(MapOperator::new("double", |x: i32| x * 2));

        assert_eq!(chain.len(), 3);
        assert!(!chain.is_empty());

        let ctx = OperatorContext::new(Watermark::min(), 0);

        // Test with 3: 3 -> 4 (add-one) -> 4 (filter pass) -> 8 (double)
        let input = bincode::serialize(&3).unwrap();
        let output = chain.process(vec![input], &ctx).await.unwrap();
        assert_eq!(output.len(), 1);
        let result: i32 = bincode::deserialize(&output[0]).unwrap();
        assert_eq!(result, 8);

        // Test with 4: 4 -> 5 (add-one) -> filtered out
        let input = bincode::serialize(&4).unwrap();
        let output = chain.process(vec![input], &ctx).await.unwrap();
        assert_eq!(output.len(), 0);
    }

    #[tokio::test]
    async fn test_operator_context() {
        let watermark = Watermark::new(12345);
        let ctx = OperatorContext::new(watermark, 7);

        assert_eq!(ctx.watermark, watermark);
        assert_eq!(ctx.partition, 7);
        assert!(ctx.processing_time > 0);
    }

    #[test]
    fn test_map_operator_name() {
        let operator = MapOperator::new("my-map", |x: i32| x + 1);
        assert_eq!(operator.name(), "my-map");
    }

    #[test]
    fn test_filter_operator_name() {
        let operator = FilterOperator::new("my-filter", |x: &i32| *x > 0);
        assert_eq!(operator.name(), "my-filter");
    }

    #[test]
    fn test_flatmap_operator_name() {
        let operator = FlatMapOperator::new("my-flatmap", |x: i32| vec![x]);
        assert_eq!(operator.name(), "my-flatmap");
    }

    #[test]
    fn test_keyby_operator_extract_key() {
        let operator = KeyByOperator::new("my-keyby", FeedbackEventKeyExtractor);

        let mut event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );
        event.metadata.insert("model_id".to_string(), "test-model".to_string());

        let key = operator.extract_key(&event);
        match key {
            EventKey::Model(id) => assert_eq!(id, "test-model"),
            _ => panic!("Expected Model key"),
        }
    }

    #[test]
    fn test_window_operator_assign_windows() {
        let assigner = TumblingWindowAssigner::new(chrono::Duration::seconds(60));
        let operator = WindowOperator::new("my-window", assigner, FeedbackEventTimeExtractor);

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );

        let windows = operator.assign_windows(&event, "test-key");
        assert!(!windows.is_empty());
    }

    #[test]
    fn test_aggregate_operator_clone() {
        let aggregator: SumAggregator<f64> = SumAggregator::new();
        let operator = AggregateOperator::new("sum", aggregator);
        let _cloned = operator.clone();
    }

    #[test]
    fn test_operator_chain_default() {
        let chain = OperatorChain::default();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
    }

    #[tokio::test]
    async fn test_deduplication_operator_disabled() {
        use crate::config::DeduplicationConfig;

        let config = DeduplicationConfig::default(); // disabled by default
        let operator = DeduplicationOperator::new("dedup", config);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let input = bincode::serialize(&42).unwrap();
        let output1 = operator.process(input.clone(), &ctx).await.unwrap();
        let output2 = operator.process(input.clone(), &ctx).await.unwrap();

        // Both should pass through when disabled
        assert_eq!(output1.len(), 1);
        assert_eq!(output2.len(), 1);
    }

    #[tokio::test]
    async fn test_deduplication_operator_enabled() {
        use crate::config::DeduplicationConfig;

        let config = DeduplicationConfig::new().enabled();
        let operator = DeduplicationOperator::new("dedup", config);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let input = bincode::serialize(&42).unwrap();

        // First event should pass through
        let output1 = operator.process(input.clone(), &ctx).await.unwrap();
        assert_eq!(output1.len(), 1);

        // Duplicate event should be filtered
        let output2 = operator.process(input.clone(), &ctx).await.unwrap();
        assert_eq!(output2.len(), 0);
    }

    #[tokio::test]
    async fn test_deduplication_operator_different_events() {
        use crate::config::DeduplicationConfig;

        let config = DeduplicationConfig::new().enabled();
        let operator = DeduplicationOperator::new("dedup", config);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let input1 = bincode::serialize(&42).unwrap();
        let input2 = bincode::serialize(&43).unwrap();

        // Both should pass through as they're different
        let output1 = operator.process(input1, &ctx).await.unwrap();
        let output2 = operator.process(input2, &ctx).await.unwrap();

        assert_eq!(output1.len(), 1);
        assert_eq!(output2.len(), 1);
    }

    #[tokio::test]
    async fn test_deduplication_operator_ttl() {
        use crate::config::DeduplicationConfig;
        use std::time::Duration;

        let config = DeduplicationConfig::new()
            .enabled()
            .with_ttl(Duration::from_millis(100));

        let operator = DeduplicationOperator::new("dedup", config);
        let ctx = OperatorContext::new(Watermark::min(), 0);

        let input = bincode::serialize(&42).unwrap();

        // First event should pass
        let output1 = operator.process(input.clone(), &ctx).await.unwrap();
        assert_eq!(output1.len(), 1);

        // Wait for TTL to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        // Should pass again after TTL expires
        let output2 = operator.process(input.clone(), &ctx).await.unwrap();
        assert_eq!(output2.len(), 1);
    }

    #[test]
    fn test_deduplication_operator_stats() {
        use crate::config::DeduplicationConfig;

        let config = DeduplicationConfig::new().enabled();
        let operator = DeduplicationOperator::new("dedup", config);

        let stats = operator.stats();
        assert_eq!(stats.cached_signatures, 0);
        assert!(stats.enabled);
    }

    #[test]
    fn test_deduplication_operator_clone() {
        use crate::config::DeduplicationConfig;

        let config = DeduplicationConfig::new().enabled();
        let operator = DeduplicationOperator::new("dedup", config);
        let _cloned = operator.clone();
    }
}
