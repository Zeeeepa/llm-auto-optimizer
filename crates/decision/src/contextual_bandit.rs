//! Contextual bandit algorithms for adaptive model selection
//!
//! This module implements LinUCB (Linear Upper Confidence Bound) and
//! Contextual Thompson Sampling for context-aware variant selection.

use crate::context::RequestContext;
use crate::errors::{DecisionError, Result};
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Linear Upper Confidence Bound (LinUCB) arm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinUCBArm {
    /// Arm identifier (variant ID)
    pub arm_id: Uuid,
    /// Design matrix A = D^T D + I
    /// Where D is the matrix of observed contexts
    pub a_matrix: Vec<Vec<f64>>,
    /// b vector = D^T c where c is the vector of rewards
    pub b_vector: Vec<f64>,
    /// Dimension of context features
    pub d: usize,
    /// Number of times this arm was selected
    pub num_selections: u64,
    /// Total reward received
    pub total_reward: f64,
}

impl LinUCBArm {
    /// Create a new LinUCB arm with given feature dimension
    pub fn new(arm_id: Uuid, dimension: usize) -> Self {
        // Initialize A as identity matrix
        let mut a_matrix = vec![vec![0.0; dimension]; dimension];
        for i in 0..dimension {
            a_matrix[i][i] = 1.0;
        }

        Self {
            arm_id,
            a_matrix,
            b_vector: vec![0.0; dimension],
            d: dimension,
            num_selections: 0,
            total_reward: 0.0,
        }
    }

    /// Calculate UCB score for given context
    pub fn calculate_ucb(&self, context: &[f64], alpha: f64) -> Result<f64> {
        if context.len() != self.d {
            return Err(DecisionError::InvalidConfig(format!(
                "Context dimension {} doesn't match arm dimension {}",
                context.len(),
                self.d
            )));
        }

        // Calculate A^{-1}
        let a_inv = self.invert_matrix(&self.a_matrix)?;

        // theta = A^{-1} * b
        let theta = matrix_vector_multiply(&a_inv, &self.b_vector);

        // Expected reward: theta^T * x
        let expected_reward = dot_product(&theta, context);

        // Uncertainty: sqrt(x^T * A^{-1} * x)
        let a_inv_x = matrix_vector_multiply(&a_inv, context);
        let uncertainty = dot_product(context, &a_inv_x).sqrt();

        // UCB = expected_reward + alpha * uncertainty
        Ok(expected_reward + alpha * uncertainty)
    }

    /// Update arm with observed context and reward
    pub fn update(&mut self, context: &[f64], reward: f64) -> Result<()> {
        if context.len() != self.d {
            return Err(DecisionError::InvalidConfig(format!(
                "Context dimension {} doesn't match arm dimension {}",
                context.len(),
                self.d
            )));
        }

        // A = A + x * x^T
        for i in 0..self.d {
            for j in 0..self.d {
                self.a_matrix[i][j] += context[i] * context[j];
            }
        }

        // b = b + reward * x
        for i in 0..self.d {
            self.b_vector[i] += reward * context[i];
        }

        self.num_selections += 1;
        self.total_reward += reward;

        Ok(())
    }

    /// Get average reward
    pub fn average_reward(&self) -> f64 {
        if self.num_selections == 0 {
            0.0
        } else {
            self.total_reward / self.num_selections as f64
        }
    }

    /// Invert matrix using Gauss-Jordan elimination
    fn invert_matrix(&self, matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
        let n = matrix.len();
        let mut aug = vec![vec![0.0; 2 * n]; n];

        // Create augmented matrix [A | I]
        for i in 0..n {
            for j in 0..n {
                aug[i][j] = matrix[i][j];
            }
            aug[i][i + n] = 1.0;
        }

        // Forward elimination
        for i in 0..n {
            // Find pivot
            let mut max_row = i;
            for k in i + 1..n {
                if aug[k][i].abs() > aug[max_row][i].abs() {
                    max_row = k;
                }
            }

            // Swap rows
            aug.swap(i, max_row);

            // Check for singular matrix
            if aug[i][i].abs() < 1e-10 {
                return Err(DecisionError::StatisticalError(
                    "Matrix is singular or nearly singular".to_string(),
                ));
            }

            // Scale pivot row
            let pivot = aug[i][i];
            for j in 0..2 * n {
                aug[i][j] /= pivot;
            }

            // Eliminate column
            for k in 0..n {
                if k != i {
                    let factor = aug[k][i];
                    for j in 0..2 * n {
                        aug[k][j] -= factor * aug[i][j];
                    }
                }
            }
        }

        // Extract inverse from augmented matrix
        let mut inv = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                inv[i][j] = aug[i][j + n];
            }
        }

        Ok(inv)
    }
}

/// LinUCB (Linear Upper Confidence Bound) contextual bandit
#[derive(Debug, Clone)]
pub struct LinUCB {
    /// Arms in the bandit
    arms: HashMap<Uuid, LinUCBArm>,
    /// Exploration parameter (typically 0.1 to 2.0)
    alpha: f64,
    /// Feature dimension
    dimension: usize,
}

impl LinUCB {
    /// Create a new LinUCB bandit
    pub fn new(alpha: f64, dimension: usize) -> Self {
        Self {
            arms: HashMap::new(),
            alpha,
            dimension,
        }
    }

    /// Add a new arm
    pub fn add_arm(&mut self, arm_id: Uuid) {
        self.arms.insert(arm_id, LinUCBArm::new(arm_id, self.dimension));
    }

    /// Remove an arm
    pub fn remove_arm(&mut self, arm_id: &Uuid) {
        self.arms.remove(arm_id);
    }

    /// Select best arm for given context
    pub fn select_arm(&self, context: &RequestContext) -> Result<Uuid> {
        if self.arms.is_empty() {
            return Err(DecisionError::InvalidState(
                "No arms available for selection".to_string(),
            ));
        }

        let features = context.to_feature_vector();

        let mut best_arm = None;
        let mut best_ucb = f64::NEG_INFINITY;

        for (arm_id, arm) in &self.arms {
            let ucb = arm.calculate_ucb(&features, self.alpha)?;
            if ucb > best_ucb {
                best_ucb = ucb;
                best_arm = Some(*arm_id);
            }
        }

        best_arm.ok_or_else(|| DecisionError::AllocationError("Failed to select arm".to_string()))
    }

    /// Update arm with observed reward
    pub fn update(&mut self, arm_id: &Uuid, context: &RequestContext, reward: f64) -> Result<()> {
        let arm = self
            .arms
            .get_mut(arm_id)
            .ok_or_else(|| DecisionError::VariantNotFound(arm_id.to_string()))?;

        let features = context.to_feature_vector();
        arm.update(&features, reward)
    }

    /// Get arm statistics
    pub fn get_arm(&self, arm_id: &Uuid) -> Option<&LinUCBArm> {
        self.arms.get(arm_id)
    }

    /// Get all arms
    pub fn get_arms(&self) -> &HashMap<Uuid, LinUCBArm> {
        &self.arms
    }

    /// Get average rewards for all arms
    pub fn get_average_rewards(&self) -> HashMap<Uuid, f64> {
        self.arms
            .iter()
            .map(|(id, arm)| (*id, arm.average_reward()))
            .collect()
    }

    /// Set exploration parameter
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha;
    }

    /// Get exploration parameter
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
}

/// Contextual Thompson Sampling arm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualThompsonArm {
    /// Arm identifier
    pub arm_id: Uuid,
    /// Weight mean (theta)
    pub theta_mean: Vec<f64>,
    /// Weight covariance (simplified: diagonal only for efficiency)
    pub theta_variance: Vec<f64>,
    /// Feature dimension
    pub d: usize,
    /// Number of selections
    pub num_selections: u64,
    /// Total reward
    pub total_reward: f64,
    /// Learning rate
    pub learning_rate: f64,
}

impl ContextualThompsonArm {
    /// Create a new contextual Thompson sampling arm
    pub fn new(arm_id: Uuid, dimension: usize) -> Self {
        Self {
            arm_id,
            theta_mean: vec![0.0; dimension],
            theta_variance: vec![1.0; dimension],
            d: dimension,
            num_selections: 0,
            total_reward: 0.0,
            learning_rate: 0.1,
        }
    }

    /// Sample theta from posterior
    pub fn sample_theta(&self) -> Result<Vec<f64>> {
        let mut rng = rand::thread_rng();
        let mut theta = Vec::with_capacity(self.d);

        for i in 0..self.d {
            let normal = Normal::new(self.theta_mean[i], self.theta_variance[i].sqrt())
                .map_err(|e| DecisionError::StatisticalError(e.to_string()))?;
            theta.push(normal.sample(&mut rng));
        }

        Ok(theta)
    }

    /// Calculate expected reward for context
    pub fn expected_reward(&self, context: &[f64]) -> f64 {
        dot_product(&self.theta_mean, context)
    }

    /// Update arm with context and reward (Bayesian update)
    pub fn update(&mut self, context: &[f64], reward: f64) -> Result<()> {
        if context.len() != self.d {
            return Err(DecisionError::InvalidConfig(format!(
                "Context dimension {} doesn't match arm dimension {}",
                context.len(),
                self.d
            )));
        }

        let predicted = self.expected_reward(context);
        let error = reward - predicted;

        // Update theta mean (gradient descent-style update)
        for i in 0..self.d {
            self.theta_mean[i] += self.learning_rate * error * context[i];
        }

        // Update variance (decrease with more observations)
        for i in 0..self.d {
            self.theta_variance[i] *= 0.99; // Gradual decay
            self.theta_variance[i] = self.theta_variance[i].max(0.01); // Lower bound
        }

        self.num_selections += 1;
        self.total_reward += reward;

        Ok(())
    }

    /// Get average reward
    pub fn average_reward(&self) -> f64 {
        if self.num_selections == 0 {
            0.0
        } else {
            self.total_reward / self.num_selections as f64
        }
    }
}

/// Contextual Thompson Sampling bandit
#[derive(Debug, Clone)]
pub struct ContextualThompson {
    /// Arms in the bandit
    arms: HashMap<Uuid, ContextualThompsonArm>,
    /// Feature dimension
    dimension: usize,
}

impl ContextualThompson {
    /// Create a new contextual Thompson sampling bandit
    pub fn new(dimension: usize) -> Self {
        Self {
            arms: HashMap::new(),
            dimension,
        }
    }

    /// Add a new arm
    pub fn add_arm(&mut self, arm_id: Uuid) {
        self.arms
            .insert(arm_id, ContextualThompsonArm::new(arm_id, self.dimension));
    }

    /// Remove an arm
    pub fn remove_arm(&mut self, arm_id: &Uuid) {
        self.arms.remove(arm_id);
    }

    /// Select arm using Thompson sampling
    pub fn select_arm(&self, context: &RequestContext) -> Result<Uuid> {
        if self.arms.is_empty() {
            return Err(DecisionError::InvalidState(
                "No arms available for selection".to_string(),
            ));
        }

        let features = context.to_feature_vector();

        let mut best_arm = None;
        let mut best_reward = f64::NEG_INFINITY;

        for (arm_id, arm) in &self.arms {
            let theta = arm.sample_theta()?;
            let reward = dot_product(&theta, &features);

            if reward > best_reward {
                best_reward = reward;
                best_arm = Some(*arm_id);
            }
        }

        best_arm.ok_or_else(|| DecisionError::AllocationError("Failed to select arm".to_string()))
    }

    /// Update arm with observed reward
    pub fn update(&mut self, arm_id: &Uuid, context: &RequestContext, reward: f64) -> Result<()> {
        let arm = self
            .arms
            .get_mut(arm_id)
            .ok_or_else(|| DecisionError::VariantNotFound(arm_id.to_string()))?;

        let features = context.to_feature_vector();
        arm.update(&features, reward)
    }

    /// Get arm statistics
    pub fn get_arm(&self, arm_id: &Uuid) -> Option<&ContextualThompsonArm> {
        self.arms.get(arm_id)
    }

    /// Get all arms
    pub fn get_arms(&self) -> &HashMap<Uuid, ContextualThompsonArm> {
        &self.arms
    }

    /// Get average rewards for all arms
    pub fn get_average_rewards(&self) -> HashMap<Uuid, f64> {
        self.arms
            .iter()
            .map(|(id, arm)| (*id, arm.average_reward()))
            .collect()
    }
}

// Helper functions

fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn matrix_vector_multiply(matrix: &[Vec<f64>], vector: &[f64]) -> Vec<f64> {
    matrix.iter().map(|row| dot_product(row, vector)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linucb_arm_creation() {
        let arm = LinUCBArm::new(Uuid::new_v4(), 5);
        assert_eq!(arm.d, 5);
        assert_eq!(arm.num_selections, 0);
        assert_eq!(arm.total_reward, 0.0);
    }

    #[test]
    fn test_linucb_arm_update() {
        let mut arm = LinUCBArm::new(Uuid::new_v4(), 3);
        let context = vec![1.0, 0.5, 0.2];
        let reward = 0.8;

        arm.update(&context, reward).unwrap();
        assert_eq!(arm.num_selections, 1);
        assert_eq!(arm.total_reward, reward);
    }

    #[test]
    fn test_linucb_creation() {
        let bandit = LinUCB::new(1.0, 10);
        assert_eq!(bandit.alpha, 1.0);
        assert_eq!(bandit.dimension, 10);
        assert_eq!(bandit.arms.len(), 0);
    }

    #[test]
    fn test_linucb_add_remove_arm() {
        let mut bandit = LinUCB::new(1.0, 5);
        let arm_id = Uuid::new_v4();

        bandit.add_arm(arm_id);
        assert_eq!(bandit.arms.len(), 1);

        bandit.remove_arm(&arm_id);
        assert_eq!(bandit.arms.len(), 0);
    }

    #[test]
    fn test_linucb_select_arm() {
        let mut bandit = LinUCB::new(1.0, RequestContext::feature_dimension());

        let arm1 = Uuid::new_v4();
        let arm2 = Uuid::new_v4();

        bandit.add_arm(arm1);
        bandit.add_arm(arm2);

        let context = RequestContext::new(100);
        let selected = bandit.select_arm(&context).unwrap();

        assert!(selected == arm1 || selected == arm2);
    }

    #[test]
    fn test_linucb_update_and_select() {
        let mut bandit = LinUCB::new(0.5, RequestContext::feature_dimension());

        let arm1 = Uuid::new_v4();
        let arm2 = Uuid::new_v4();

        bandit.add_arm(arm1);
        bandit.add_arm(arm2);

        // Simulate arm1 being better
        for _ in 0..10 {
            let context = RequestContext::new(100);
            bandit.update(&arm1, &context, 0.9).unwrap();
            bandit.update(&arm2, &context, 0.3).unwrap();
        }

        // After updates, should prefer arm1
        let rewards = bandit.get_average_rewards();
        assert!(rewards[&arm1] > rewards[&arm2]);
    }

    #[test]
    fn test_contextual_thompson_arm_creation() {
        let arm = ContextualThompsonArm::new(Uuid::new_v4(), 5);
        assert_eq!(arm.d, 5);
        assert_eq!(arm.num_selections, 0);
    }

    #[test]
    fn test_contextual_thompson_sample_theta() {
        let arm = ContextualThompsonArm::new(Uuid::new_v4(), 5);
        let theta = arm.sample_theta().unwrap();
        assert_eq!(theta.len(), 5);
    }

    #[test]
    fn test_contextual_thompson_update() {
        let mut arm = ContextualThompsonArm::new(Uuid::new_v4(), 3);
        let context = vec![1.0, 0.5, 0.2];

        arm.update(&context, 0.8).unwrap();
        assert_eq!(arm.num_selections, 1);
    }

    #[test]
    fn test_contextual_thompson_creation() {
        let bandit = ContextualThompson::new(10);
        assert_eq!(bandit.dimension, 10);
    }

    #[test]
    fn test_contextual_thompson_select() {
        let mut bandit = ContextualThompson::new(RequestContext::feature_dimension());

        let arm1 = Uuid::new_v4();
        let arm2 = Uuid::new_v4();

        bandit.add_arm(arm1);
        bandit.add_arm(arm2);

        let context = RequestContext::new(100);
        let selected = bandit.select_arm(&context).unwrap();

        assert!(selected == arm1 || selected == arm2);
    }

    #[test]
    fn test_contextual_thompson_convergence() {
        let mut bandit = ContextualThompson::new(RequestContext::feature_dimension());

        let good_arm = Uuid::new_v4();
        let bad_arm = Uuid::new_v4();

        bandit.add_arm(good_arm);
        bandit.add_arm(bad_arm);

        // Train: good_arm gets high rewards, bad_arm gets low rewards
        for _ in 0..50 {
            let context = RequestContext::new(100);
            bandit.update(&good_arm, &context, 0.9).unwrap();
            bandit.update(&bad_arm, &context, 0.2).unwrap();
        }

        let rewards = bandit.get_average_rewards();
        assert!(rewards[&good_arm] > rewards[&bad_arm]);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let result = dot_product(&a, &b);
        assert_eq!(result, 32.0);
    }

    #[test]
    fn test_matrix_vector_multiply() {
        let matrix = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let vector = vec![5.0, 6.0];
        let result = matrix_vector_multiply(&matrix, &vector);
        assert_eq!(result, vec![17.0, 39.0]);
    }

    #[test]
    fn test_matrix_inversion() {
        let arm = LinUCBArm::new(Uuid::new_v4(), 2);
        let matrix = vec![vec![4.0, 3.0], vec![3.0, 2.0]];

        let inv = arm.invert_matrix(&matrix).unwrap();

        // A * A^{-1} should be approximately I
        for i in 0..2 {
            for j in 0..2 {
                let mut sum = 0.0;
                for k in 0..2 {
                    sum += matrix[i][k] * inv[k][j];
                }
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((sum - expected).abs() < 1e-10);
            }
        }
    }
}
