//! Validation helper functions for configuration types.

use crate::core::errors::{Result, ValknutError};

/// Validate that a usize value is greater than zero.
pub fn validate_positive_usize(value: usize, field: &str) -> Result<()> {
    if value == 0 {
        return Err(ValknutError::validation(format!(
            "{} must be greater than 0",
            field
        )));
    }
    Ok(())
}

/// Validate that an i64 value is greater than zero.
pub fn validate_positive_i64(value: i64, field: &str) -> Result<()> {
    if value <= 0 {
        return Err(ValknutError::validation(format!(
            "{} must be greater than 0",
            field
        )));
    }
    Ok(())
}

/// Validate that an f64 value is greater than zero.
pub fn validate_positive_f64(value: f64, field: &str) -> Result<()> {
    if value <= 0.0 {
        return Err(ValknutError::validation(format!(
            "{} must be greater than 0.0",
            field
        )));
    }
    Ok(())
}

/// Validate that an f64 value is non-negative.
pub fn validate_non_negative(value: f64, field: &str) -> Result<()> {
    if value < 0.0 {
        return Err(ValknutError::validation(format!(
            "{} must be non-negative",
            field
        )));
    }
    Ok(())
}

/// Validate that an f64 value is in the unit range [0.0, 1.0].
pub fn validate_unit_range(value: f64, field: &str) -> Result<()> {
    if !(0.0..=1.0).contains(&value) {
        return Err(ValknutError::validation(format!(
            "{} must be between 0.0 and 1.0",
            field
        )));
    }
    Ok(())
}

/// Validate that a u32 value is greater than zero.
pub fn validate_positive_u32(value: u32, field: &str) -> Result<()> {
    if value == 0 {
        return Err(ValknutError::validation(format!(
            "{} must be greater than 0",
            field
        )));
    }
    Ok(())
}

/// Validate that a usize value is within a bounded range (inclusive).
pub fn validate_bounded_usize(value: usize, min: usize, max: usize, field: &str) -> Result<()> {
    if value < min || value > max {
        return Err(ValknutError::validation(format!(
            "{} must be between {} and {}",
            field, min, max
        )));
    }
    Ok(())
}

/// Validate that weights sum to approximately 1.0 (within tolerance).
pub fn validate_weights_sum(weights: &[f64], tolerance: f64, field: &str) -> Result<()> {
    let sum: f64 = weights.iter().sum();
    if (sum - 1.0).abs() > tolerance {
        return Err(ValknutError::validation(format!(
            "{} should sum to approximately 1.0",
            field
        )));
    }
    Ok(())
}
