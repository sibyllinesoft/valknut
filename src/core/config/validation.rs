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
