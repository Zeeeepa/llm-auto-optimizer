//! Parameter Search Strategies
//!
//! This module provides various strategies for exploring the parameter space
//! including grid search, random search, and Bayesian optimization.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::adaptive_params::{ParameterConfig, ParameterRange};

/// Search strategy type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchStrategy {
    /// Systematic grid search
    Grid,
    /// Random sampling
    Random,
    /// Latin hypercube sampling
    LatinHypercube,
    /// Sobol sequence (quasi-random)
    Sobol,
}

/// Grid search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridSearchConfig {
    /// Number of temperature steps
    pub temp_steps: usize,
    /// Number of top-p steps
    pub top_p_steps: usize,
    /// Number of max token steps
    pub max_tokens_steps: usize,
}

impl Default for GridSearchConfig {
    fn default() -> Self {
        Self {
            temp_steps: 5,
            top_p_steps: 4,
            max_tokens_steps: 4,
        }
    }
}

/// Grid search iterator for parameter space exploration
pub struct GridSearch {
    /// Parameter range to search
    range: ParameterRange,
    /// Grid configuration
    config: GridSearchConfig,
    /// Generated configurations
    configurations: VecDeque<ParameterConfig>,
}

impl GridSearch {
    /// Create new grid search
    pub fn new(range: ParameterRange, config: GridSearchConfig) -> Self {
        let mut search = Self {
            range,
            config,
            configurations: VecDeque::new(),
        };
        search.generate_grid();
        search
    }

    /// Create with default configuration
    pub fn with_defaults(range: ParameterRange) -> Self {
        Self::new(range, GridSearchConfig::default())
    }

    /// Generate grid of configurations
    fn generate_grid(&mut self) {
        let temp_values = linspace(
            self.range.temp_min,
            self.range.temp_max,
            self.config.temp_steps,
        );

        let top_p_values = linspace(
            self.range.top_p_min,
            self.range.top_p_max,
            self.config.top_p_steps,
        );

        let max_tokens_values = linspace_usize(
            self.range.max_tokens_min,
            self.range.max_tokens_max,
            self.config.max_tokens_steps,
        );

        for &temp in &temp_values {
            for &top_p in &top_p_values {
                for &max_tokens in &max_tokens_values {
                    if let Ok(config) = ParameterConfig::new(temp, top_p, max_tokens) {
                        self.configurations.push_back(config);
                    }
                }
            }
        }
    }

    /// Get next configuration
    pub fn next(&mut self) -> Option<ParameterConfig> {
        self.configurations.pop_front()
    }

    /// Get all configurations
    pub fn all_configs(&self) -> Vec<ParameterConfig> {
        self.configurations.iter().cloned().collect()
    }

    /// Get total number of configurations
    pub fn total_configs(&self) -> usize {
        self.config.temp_steps * self.config.top_p_steps * self.config.max_tokens_steps
    }

    /// Check if search is complete
    pub fn is_complete(&self) -> bool {
        self.configurations.is_empty()
    }
}

/// Random search for parameter exploration
pub struct RandomSearch {
    /// Parameter range to search
    range: ParameterRange,
    /// Number of samples to generate
    num_samples: usize,
    /// Generated samples
    samples_generated: usize,
}

impl RandomSearch {
    /// Create new random search
    pub fn new(range: ParameterRange, num_samples: usize) -> Self {
        Self {
            range,
            num_samples,
            samples_generated: 0,
        }
    }

    /// Generate next random configuration
    pub fn next(&mut self) -> Option<ParameterConfig> {
        if self.samples_generated >= self.num_samples {
            return None;
        }

        let mut rng = rand::thread_rng();

        let temp = rng.gen_range(self.range.temp_min..=self.range.temp_max);
        let top_p = rng.gen_range(self.range.top_p_min..=self.range.top_p_max);
        let max_tokens = rng.gen_range(self.range.max_tokens_min..=self.range.max_tokens_max);

        self.samples_generated += 1;

        ParameterConfig::new(temp, top_p, max_tokens).ok()
    }

    /// Generate all random configurations at once
    pub fn generate_all(&mut self) -> Vec<ParameterConfig> {
        let mut configs = Vec::new();
        while let Some(config) = self.next() {
            configs.push(config);
        }
        configs
    }

    /// Check if search is complete
    pub fn is_complete(&self) -> bool {
        self.samples_generated >= self.num_samples
    }

    /// Reset the search
    pub fn reset(&mut self) {
        self.samples_generated = 0;
    }
}

/// Latin Hypercube Sampling for better coverage
pub struct LatinHypercubeSampling {
    /// Parameter range
    range: ParameterRange,
    /// Number of samples
    num_samples: usize,
    /// Generated configurations
    configurations: VecDeque<ParameterConfig>,
}

impl LatinHypercubeSampling {
    /// Create new LHS sampler
    pub fn new(range: ParameterRange, num_samples: usize) -> Self {
        let mut lhs = Self {
            range,
            num_samples,
            configurations: VecDeque::new(),
        };
        lhs.generate_samples();
        lhs
    }

    /// Generate Latin Hypercube samples
    fn generate_samples(&mut self) {
        let mut rng = rand::thread_rng();

        // Create permutations for each dimension
        let mut temp_indices: Vec<usize> = (0..self.num_samples).collect();
        let mut top_p_indices: Vec<usize> = (0..self.num_samples).collect();
        let mut tokens_indices: Vec<usize> = (0..self.num_samples).collect();

        // Shuffle each dimension independently
        shuffle(&mut temp_indices);
        shuffle(&mut top_p_indices);
        shuffle(&mut tokens_indices);

        // Generate samples
        for i in 0..self.num_samples {
            // Add random jitter within each cell
            let temp_cell = temp_indices[i] as f64 + rng.gen::<f64>();
            let top_p_cell = top_p_indices[i] as f64 + rng.gen::<f64>();
            let tokens_cell = tokens_indices[i] as f64 + rng.gen::<f64>();

            // Scale to parameter range
            let temp = self.range.temp_min
                + (temp_cell / self.num_samples as f64)
                    * (self.range.temp_max - self.range.temp_min);

            let top_p = self.range.top_p_min
                + (top_p_cell / self.num_samples as f64)
                    * (self.range.top_p_max - self.range.top_p_min);

            let max_tokens = self.range.max_tokens_min
                + ((tokens_cell / self.num_samples as f64)
                    * (self.range.max_tokens_max - self.range.max_tokens_min) as f64)
                    as usize;

            if let Ok(config) = ParameterConfig::new(temp, top_p, max_tokens) {
                self.configurations.push_back(config);
            }
        }
    }

    /// Get next configuration
    pub fn next(&mut self) -> Option<ParameterConfig> {
        self.configurations.pop_front()
    }

    /// Get all configurations
    pub fn all_configs(&self) -> Vec<ParameterConfig> {
        self.configurations.iter().cloned().collect()
    }

    /// Check if complete
    pub fn is_complete(&self) -> bool {
        self.configurations.is_empty()
    }
}

/// Parameter search manager
pub struct ParameterSearchManager {
    /// Current search strategy
    strategy: SearchStrategy,
    /// Parameter range
    range: ParameterRange,
    /// Grid search instance
    pub grid_search: Option<GridSearch>,
    /// Random search instance
    pub random_search: Option<RandomSearch>,
    /// LHS instance
    pub lhs_search: Option<LatinHypercubeSampling>,
}

impl ParameterSearchManager {
    /// Create new search manager with grid search
    pub fn with_grid_search(range: ParameterRange, config: GridSearchConfig) -> Self {
        Self {
            strategy: SearchStrategy::Grid,
            range: range.clone(),
            grid_search: Some(GridSearch::new(range, config)),
            random_search: None,
            lhs_search: None,
        }
    }

    /// Create new search manager with random search
    pub fn with_random_search(range: ParameterRange, num_samples: usize) -> Self {
        Self {
            strategy: SearchStrategy::Random,
            range: range.clone(),
            grid_search: None,
            random_search: Some(RandomSearch::new(range, num_samples)),
            lhs_search: None,
        }
    }

    /// Create new search manager with Latin Hypercube Sampling
    pub fn with_lhs(range: ParameterRange, num_samples: usize) -> Self {
        Self {
            strategy: SearchStrategy::LatinHypercube,
            range: range.clone(),
            grid_search: None,
            random_search: None,
            lhs_search: Some(LatinHypercubeSampling::new(range, num_samples)),
        }
    }

    /// Get next configuration from current strategy
    pub fn next(&mut self) -> Option<ParameterConfig> {
        match self.strategy {
            SearchStrategy::Grid => self.grid_search.as_mut().and_then(|s| s.next()),
            SearchStrategy::Random => self.random_search.as_mut().and_then(|s| s.next()),
            SearchStrategy::LatinHypercube => self.lhs_search.as_mut().and_then(|s| s.next()),
            SearchStrategy::Sobol => None, // TODO: Implement Sobol sequence
        }
    }

    /// Check if search is complete
    pub fn is_complete(&self) -> bool {
        match self.strategy {
            SearchStrategy::Grid => self
                .grid_search
                .as_ref()
                .map(|s| s.is_complete())
                .unwrap_or(true),
            SearchStrategy::Random => self
                .random_search
                .as_ref()
                .map(|s| s.is_complete())
                .unwrap_or(true),
            SearchStrategy::LatinHypercube => self
                .lhs_search
                .as_ref()
                .map(|s| s.is_complete())
                .unwrap_or(true),
            SearchStrategy::Sobol => true,
        }
    }

    /// Get current strategy
    pub fn strategy(&self) -> SearchStrategy {
        self.strategy
    }
}

/// Helper: Generate linearly spaced values
fn linspace(start: f64, end: f64, num: usize) -> Vec<f64> {
    if num == 0 {
        return vec![];
    }
    if num == 1 {
        return vec![start];
    }

    let step = (end - start) / (num - 1) as f64;
    (0..num).map(|i| start + i as f64 * step).collect()
}

/// Helper: Generate linearly spaced usize values
fn linspace_usize(start: usize, end: usize, num: usize) -> Vec<usize> {
    if num == 0 {
        return vec![];
    }
    if num == 1 {
        return vec![start];
    }

    let step = (end - start) as f64 / (num - 1) as f64;
    (0..num).map(|i| start + (i as f64 * step) as usize).collect()
}

/// Helper: Fisher-Yates shuffle
fn shuffle<T>(vec: &mut [T]) {
    let mut rng = rand::thread_rng();
    let len = vec.len();
    for i in 0..len {
        let j = rng.gen_range(i..len);
        vec.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linspace() {
        let values = linspace(0.0, 1.0, 5);
        assert_eq!(values.len(), 5);
        assert_eq!(values[0], 0.0);
        assert_eq!(values[4], 1.0);
        assert!((values[2] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_linspace_usize() {
        let values = linspace_usize(0, 100, 5);
        assert_eq!(values.len(), 5);
        assert_eq!(values[0], 0);
        assert_eq!(values[4], 100);
    }

    #[test]
    fn test_grid_search_creation() {
        let range = ParameterRange::default();
        let config = GridSearchConfig {
            temp_steps: 3,
            top_p_steps: 3,
            max_tokens_steps: 2,
        };

        let search = GridSearch::new(range, config);
        assert_eq!(search.total_configs(), 3 * 3 * 2);
    }

    #[test]
    fn test_grid_search_iteration() {
        let range = ParameterRange::default();
        let config = GridSearchConfig {
            temp_steps: 2,
            top_p_steps: 2,
            max_tokens_steps: 2,
        };

        let mut search = GridSearch::new(range, config);
        let mut count = 0;

        while search.next().is_some() {
            count += 1;
        }

        assert_eq!(count, 8);
        assert!(search.is_complete());
    }

    #[test]
    fn test_grid_search_coverage() {
        let range = ParameterRange {
            temp_min: 0.0,
            temp_max: 1.0,
            top_p_min: 0.8,
            top_p_max: 1.0,
            max_tokens_min: 512,
            max_tokens_max: 2048,
        };

        let config = GridSearchConfig {
            temp_steps: 3,
            top_p_steps: 3,
            max_tokens_steps: 2,
        };

        let search = GridSearch::new(range.clone(), config);
        let configs = search.all_configs();

        // Check boundary coverage
        assert!(configs.iter().any(|c| c.temperature == range.temp_min));
        assert!(configs.iter().any(|c| c.temperature == range.temp_max));
        assert!(configs.iter().any(|c| c.top_p == range.top_p_min));
        assert!(configs.iter().any(|c| c.top_p == range.top_p_max));
    }

    #[test]
    fn test_random_search_creation() {
        let range = ParameterRange::default();
        let search = RandomSearch::new(range, 10);
        assert!(!search.is_complete());
    }

    #[test]
    fn test_random_search_sampling() {
        let range = ParameterRange::default();
        let mut search = RandomSearch::new(range.clone(), 20);

        let mut count = 0;
        while let Some(config) = search.next() {
            assert!(range.contains(&config));
            count += 1;
        }

        assert_eq!(count, 20);
        assert!(search.is_complete());
    }

    #[test]
    fn test_random_search_reset() {
        let range = ParameterRange::default();
        let mut search = RandomSearch::new(range, 5);

        while search.next().is_some() {}
        assert!(search.is_complete());

        search.reset();
        assert!(!search.is_complete());
    }

    #[test]
    fn test_random_search_generate_all() {
        let range = ParameterRange::default();
        let mut search = RandomSearch::new(range, 15);

        let configs = search.generate_all();
        assert_eq!(configs.len(), 15);
        assert!(search.is_complete());
    }

    #[test]
    fn test_lhs_creation() {
        let range = ParameterRange::default();
        let lhs = LatinHypercubeSampling::new(range, 10);
        assert!(!lhs.is_complete());
    }

    #[test]
    fn test_lhs_sampling() {
        let range = ParameterRange::default();
        let mut lhs = LatinHypercubeSampling::new(range.clone(), 20);

        let mut count = 0;
        while let Some(config) = lhs.next() {
            assert!(range.contains(&config));
            count += 1;
        }

        assert!(count > 0);
        assert!(lhs.is_complete());
    }

    #[test]
    fn test_lhs_coverage() {
        let range = ParameterRange::default();
        let lhs = LatinHypercubeSampling::new(range.clone(), 50);

        let configs = lhs.all_configs();

        // LHS should provide good coverage across the range
        let avg_temp: f64 = configs.iter().map(|c| c.temperature).sum::<f64>() / configs.len() as f64;
        let avg_top_p: f64 = configs.iter().map(|c| c.top_p).sum::<f64>() / configs.len() as f64;

        // Average should be near middle of range
        let temp_mid = (range.temp_min + range.temp_max) / 2.0;
        let top_p_mid = (range.top_p_min + range.top_p_max) / 2.0;

        assert!((avg_temp - temp_mid).abs() < 0.3);
        assert!((avg_top_p - top_p_mid).abs() < 0.1);
    }

    #[test]
    fn test_shuffle() {
        let mut vec: Vec<usize> = (0..10).collect();
        let original = vec.clone();

        shuffle(&mut vec);

        // Should contain same elements
        let mut sorted = vec.clone();
        sorted.sort();
        assert_eq!(sorted, original);

        // Should be different order (very high probability)
        // This could theoretically fail, but with probability 1/10!
        assert_ne!(vec, original);
    }

    #[test]
    fn test_search_manager_grid() {
        let range = ParameterRange::default();
        let config = GridSearchConfig {
            temp_steps: 2,
            top_p_steps: 2,
            max_tokens_steps: 2,
        };

        let mut manager = ParameterSearchManager::with_grid_search(range, config);
        assert_eq!(manager.strategy(), SearchStrategy::Grid);

        let mut count = 0;
        while manager.next().is_some() {
            count += 1;
        }

        assert_eq!(count, 8);
        assert!(manager.is_complete());
    }

    #[test]
    fn test_search_manager_random() {
        let range = ParameterRange::default();
        let mut manager = ParameterSearchManager::with_random_search(range, 10);
        assert_eq!(manager.strategy(), SearchStrategy::Random);

        let mut count = 0;
        while manager.next().is_some() {
            count += 1;
        }

        assert_eq!(count, 10);
        assert!(manager.is_complete());
    }

    #[test]
    fn test_search_manager_lhs() {
        let range = ParameterRange::default();
        let mut manager = ParameterSearchManager::with_lhs(range, 15);
        assert_eq!(manager.strategy(), SearchStrategy::LatinHypercube);

        let mut count = 0;
        while manager.next().is_some() {
            count += 1;
        }

        assert!(count > 0);
        assert!(manager.is_complete());
    }
}
