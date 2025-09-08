//! Error types for the valknut-rs library.
//!
//! This module provides comprehensive error handling for all valknut operations,
//! with structured error types that preserve context and enable proper error
//! propagation throughout the analysis pipeline.

use std::fmt;
use std::io;
use std::num::{ParseFloatError, ParseIntError};
use std::str::Utf8Error;

use thiserror::Error;

/// Main result type for valknut operations.
pub type Result<T> = std::result::Result<T, ValknutError>;

/// Comprehensive error type for all valknut operations.
#[derive(Error, Debug)]
pub enum ValknutError {
    /// I/O related errors (file operations, network, etc.)
    #[error("I/O error: {message}")]
    Io {
        /// Human-readable error message
        message: String,
        /// Underlying I/O error
        #[source]
        source: io::Error,
    },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config {
        /// Error description
        message: String,
        /// Configuration field that caused the error
        field: Option<String>,
    },

    /// Parsing and language processing errors
    #[error("Parse error in {language}: {message}")]
    Parse {
        /// Programming language being parsed
        language: String,
        /// Error description
        message: String,
        /// File path where error occurred
        file_path: Option<String>,
        /// Line number (if available)
        line: Option<usize>,
        /// Column number (if available)
        column: Option<usize>,
    },

    /// Mathematical computation errors
    #[error("Mathematical error: {message}")]
    Math {
        /// Error description
        message: String,
        /// Context of the mathematical operation
        context: Option<String>,
    },

    /// Graph algorithm errors
    #[error("Graph analysis error: {message}")]
    Graph {
        /// Error description
        message: String,
        /// Graph node or edge that caused the error
        element: Option<String>,
    },

    /// LSH and similarity detection errors
    #[error("LSH error: {message}")]
    Lsh {
        /// Error description
        message: String,
        /// LSH parameters that may have caused the issue
        parameters: Option<String>,
    },

    /// Database and persistence errors
    #[cfg(feature = "database")]
    #[error("Database error: {message}")]
    Database {
        /// Error description
        message: String,
        /// Database operation that failed
        operation: Option<String>,
        /// Underlying database error
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Analysis pipeline errors
    #[error("Pipeline error at stage '{stage}': {message}")]
    Pipeline {
        /// Pipeline stage where error occurred
        stage: String,
        /// Error description
        message: String,
        /// Number of files processed before error
        processed_count: Option<usize>,
    },

    /// Cache and storage errors
    #[error("Cache error: {message}")]
    Cache {
        /// Error description
        message: String,
        /// Cache key that caused the issue
        key: Option<String>,
    },

    /// Serialization/deserialization errors
    #[error("Serialization error: {message}")]
    Serialization {
        /// Error description
        message: String,
        /// Data type being serialized
        data_type: Option<String>,
        /// Underlying serialization error
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Validation errors for input data
    #[error("Validation error: {message}")]
    Validation {
        /// Error description
        message: String,
        /// Field or input that failed validation
        field: Option<String>,
        /// Expected value or format
        expected: Option<String>,
        /// Actual value received
        actual: Option<String>,
    },

    /// Resource exhaustion errors
    #[error("Resource exhaustion: {message}")]
    ResourceExhaustion {
        /// Error description
        message: String,
        /// Type of resource exhausted
        resource_type: String,
        /// Current usage level
        current_usage: Option<String>,
        /// Maximum allowed usage
        limit: Option<String>,
    },

    /// Concurrency and threading errors
    #[error("Concurrency error: {message}")]
    Concurrency {
        /// Error description
        message: String,
        /// Thread or task identifier
        thread_id: Option<String>,
    },

    /// Feature not implemented or not available
    #[error("Feature not available: {feature}")]
    FeatureUnavailable {
        /// Feature name
        feature: String,
        /// Reason why it's unavailable
        reason: Option<String>,
    },

    /// Generic internal errors
    #[error("Internal error: {message}")]
    Internal {
        /// Error description
        message: String,
        /// Additional context
        context: Option<String>,
    },

    /// Unsupported operation or feature
    #[error("Unsupported: {message}")]
    Unsupported {
        /// Error description
        message: String,
    },
}

impl ValknutError {
    /// Create a new I/O error with context
    pub fn io(message: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            message: message.into(),
            source,
        }
    }

    /// Create a new configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            field: None,
        }
    }

    /// Create a new configuration error with field context
    pub fn config_field(message: impl Into<String>, field: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            field: Some(field.into()),
        }
    }

    /// Create a new parse error
    pub fn parse(language: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Parse {
            language: language.into(),
            message: message.into(),
            file_path: None,
            line: None,
            column: None,
        }
    }

    /// Create a new parse error with file context
    pub fn parse_with_location(
        language: impl Into<String>,
        message: impl Into<String>,
        file_path: impl Into<String>,
        line: Option<usize>,
        column: Option<usize>,
    ) -> Self {
        Self::Parse {
            language: language.into(),
            message: message.into(),
            file_path: Some(file_path.into()),
            line,
            column,
        }
    }

    /// Create a new mathematical error
    pub fn math(message: impl Into<String>) -> Self {
        Self::Math {
            message: message.into(),
            context: None,
        }
    }

    /// Create a new mathematical error with context
    pub fn math_with_context(message: impl Into<String>, context: impl Into<String>) -> Self {
        Self::Math {
            message: message.into(),
            context: Some(context.into()),
        }
    }

    /// Create a new graph analysis error
    pub fn graph(message: impl Into<String>) -> Self {
        Self::Graph {
            message: message.into(),
            element: None,
        }
    }

    /// Create a new LSH error
    pub fn lsh(message: impl Into<String>) -> Self {
        Self::Lsh {
            message: message.into(),
            parameters: None,
        }
    }

    /// Create a new pipeline error
    pub fn pipeline(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Pipeline {
            stage: stage.into(),
            message: message.into(),
            processed_count: None,
        }
    }

    /// Create a new validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            field: None,
            expected: None,
            actual: None,
        }
    }

    /// Create a new feature unavailable error
    pub fn feature_unavailable(feature: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::FeatureUnavailable {
            feature: feature.into(),
            reason: Some(reason.into()),
        }
    }

    /// Create a new internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            context: None,
        }
    }

    /// Create a new unsupported error
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported {
            message: message.into(),
        }
    }

    /// Add context to an existing error
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        match &mut self {
            Self::Math { context: ctx, .. }
            | Self::Internal { context: ctx, .. } => {
                *ctx = Some(context.into());
            }
            _ => {} // Other variants handle context differently
        }
        self
    }
}

// Implement From traits for common error types
impl From<io::Error> for ValknutError {
    fn from(err: io::Error) -> Self {
        Self::io("I/O operation failed", err)
    }
}

impl From<serde_json::Error> for ValknutError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            message: format!("JSON serialization failed: {err}"),
            data_type: Some("JSON".to_string()),
            source: Some(Box::new(err)),
        }
    }
}

impl From<serde_yaml::Error> for ValknutError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Serialization {
            message: format!("YAML serialization failed: {err}"),
            data_type: Some("YAML".to_string()),
            source: Some(Box::new(err)),
        }
    }
}

impl From<ParseIntError> for ValknutError {
    fn from(err: ParseIntError) -> Self {
        Self::validation(format!("Invalid integer: {err}"))
    }
}

impl From<ParseFloatError> for ValknutError {
    fn from(err: ParseFloatError) -> Self {
        Self::validation(format!("Invalid float: {err}"))
    }
}

impl From<Utf8Error> for ValknutError {
    fn from(err: Utf8Error) -> Self {
        Self::parse("unknown", format!("UTF-8 encoding error: {err}"))
    }
}

#[cfg(feature = "database")]
impl From<sqlx::Error> for ValknutError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database {
            message: format!("Database operation failed: {err}"),
            operation: None,
            source: Some(Box::new(err)),
        }
    }
}

/// Helper macro for creating context-aware errors
#[macro_export]
macro_rules! valknut_error {
    ($kind:ident, $msg:expr) => {
        $crate::core::errors::ValknutError::$kind($msg.to_string())
    };
    ($kind:ident, $msg:expr, $($arg:tt)*) => {
        $crate::core::errors::ValknutError::$kind(format!($msg, $($arg)*))
    };
}

/// Result extension trait for adding context to errors
pub trait ResultExt<T> {
    /// Add context to an error result
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// Add static context to an error result
    fn context(self, msg: &'static str) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<ValknutError>,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.into().with_context(f()))
    }

    fn context(self, msg: &'static str) -> Result<T> {
        self.map_err(|e| e.into().with_context(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = ValknutError::config("Invalid configuration");
        assert!(matches!(err, ValknutError::Config { .. }));
        
        let err = ValknutError::parse("python", "Syntax error");
        assert!(matches!(err, ValknutError::Parse { .. }));
    }

    #[test]
    fn test_error_with_context() {
        let err = ValknutError::internal("Something went wrong")
            .with_context("During file processing");
        
        if let ValknutError::Internal { context, .. } = err {
            assert_eq!(context, Some("During file processing".to_string()));
        } else {
            panic!("Expected Internal error");
        }
    }

    #[test]
    fn test_result_extension() {
        let result: std::result::Result<i32, std::io::Error> = 
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"));
        
        let valknut_result = result.context("Failed to read configuration file");
        assert!(valknut_result.is_err());
    }
}