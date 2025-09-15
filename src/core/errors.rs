//! Error types for the valknut-rs library.
//!
//! This module provides comprehensive error handling for all valknut operations,
//! with structured error types that preserve context and enable proper error
//! propagation throughout the analysis pipeline.

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
            Self::Math { context: ctx, .. } | Self::Internal { context: ctx, .. } => {
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
    use std::num::{ParseFloatError, ParseIntError};

    #[test]
    fn test_error_creation() {
        let err = ValknutError::config("Invalid configuration");
        assert!(matches!(err, ValknutError::Config { .. }));

        let err = ValknutError::parse("python", "Syntax error");
        assert!(matches!(err, ValknutError::Parse { .. }));
    }

    #[test]
    fn test_error_with_context() {
        let err =
            ValknutError::internal("Something went wrong").with_context("During file processing");

        if let ValknutError::Internal { context, .. } = err {
            assert_eq!(context, Some("During file processing".to_string()));
        } else {
            panic!("Expected Internal error");
        }
    }

    #[test]
    fn test_result_extension() {
        let result: std::result::Result<i32, std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ));

        let valknut_result = result.context("Failed to read configuration file");
        assert!(valknut_result.is_err());
    }

    #[test]
    fn test_io_error_creation() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access denied");
        let err = ValknutError::io("Failed to write file", io_err);

        if let ValknutError::Io { message, source } = &err {
            assert_eq!(message, "Failed to write file");
            assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
        } else {
            panic!("Expected Io error");
        }
    }

    #[test]
    fn test_config_field_error() {
        let err = ValknutError::config_field("Invalid value", "max_files");

        if let ValknutError::Config { message, field } = err {
            assert_eq!(message, "Invalid value");
            assert_eq!(field, Some("max_files".to_string()));
        } else {
            panic!("Expected Config error");
        }
    }

    #[test]
    fn test_parse_with_location() {
        let err = ValknutError::parse_with_location(
            "rust",
            "Missing semicolon",
            "main.rs",
            Some(42),
            Some(10),
        );

        if let ValknutError::Parse {
            language,
            message,
            file_path,
            line,
            column,
        } = err
        {
            assert_eq!(language, "rust");
            assert_eq!(message, "Missing semicolon");
            assert_eq!(file_path, Some("main.rs".to_string()));
            assert_eq!(line, Some(42));
            assert_eq!(column, Some(10));
        } else {
            panic!("Expected Parse error");
        }
    }

    #[test]
    fn test_math_with_context() {
        let err = ValknutError::math_with_context("Division by zero", "normalize_features");

        if let ValknutError::Math { message, context } = err {
            assert_eq!(message, "Division by zero");
            assert_eq!(context, Some("normalize_features".to_string()));
        } else {
            panic!("Expected Math error");
        }
    }

    #[test]
    fn test_graph_error() {
        let err = ValknutError::graph("Cycle detected");

        if let ValknutError::Graph { message, element } = err {
            assert_eq!(message, "Cycle detected");
            assert_eq!(element, None);
        } else {
            panic!("Expected Graph error");
        }
    }

    #[test]
    fn test_lsh_error() {
        let err = ValknutError::lsh("Invalid hash function");

        if let ValknutError::Lsh {
            message,
            parameters,
        } = err
        {
            assert_eq!(message, "Invalid hash function");
            assert_eq!(parameters, None);
        } else {
            panic!("Expected Lsh error");
        }
    }

    #[test]
    fn test_pipeline_error() {
        let err = ValknutError::pipeline("feature_extraction", "Timeout exceeded");

        if let ValknutError::Pipeline {
            stage,
            message,
            processed_count,
        } = err
        {
            assert_eq!(stage, "feature_extraction");
            assert_eq!(message, "Timeout exceeded");
            assert_eq!(processed_count, None);
        } else {
            panic!("Expected Pipeline error");
        }
    }

    #[test]
    fn test_validation_error() {
        let err = ValknutError::validation("Invalid range");

        if let ValknutError::Validation {
            message,
            field,
            expected,
            actual,
        } = err
        {
            assert_eq!(message, "Invalid range");
            assert_eq!(field, None);
            assert_eq!(expected, None);
            assert_eq!(actual, None);
        } else {
            panic!("Expected Validation error");
        }
    }

    #[test]
    fn test_feature_unavailable() {
        let err = ValknutError::feature_unavailable("SIMD operations", "CPU does not support AVX2");

        if let ValknutError::FeatureUnavailable { feature, reason } = err {
            assert_eq!(feature, "SIMD operations");
            assert_eq!(reason, Some("CPU does not support AVX2".to_string()));
        } else {
            panic!("Expected FeatureUnavailable error");
        }
    }

    #[test]
    fn test_unsupported_error() {
        let err = ValknutError::unsupported("Language not supported");

        if let ValknutError::Unsupported { message } = err {
            assert_eq!(message, "Language not supported");
        } else {
            panic!("Expected Unsupported error");
        }
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let valknut_err: ValknutError = io_err.into();

        assert!(matches!(valknut_err, ValknutError::Io { .. }));
    }

    #[test]
    fn test_from_json_error() {
        let json_err = serde_json::from_str::<i32>("invalid json").unwrap_err();
        let valknut_err: ValknutError = json_err.into();

        if let ValknutError::Serialization { data_type, .. } = valknut_err {
            assert_eq!(data_type, Some("JSON".to_string()));
        } else {
            panic!("Expected Serialization error");
        }
    }

    #[test]
    fn test_from_yaml_error() {
        let yaml_err = serde_yaml::from_str::<i32>("invalid: yaml: content").unwrap_err();
        let valknut_err: ValknutError = yaml_err.into();

        if let ValknutError::Serialization { data_type, .. } = valknut_err {
            assert_eq!(data_type, Some("YAML".to_string()));
        } else {
            panic!("Expected Serialization error");
        }
    }

    #[test]
    fn test_from_parse_int_error() {
        let parse_err = "not_a_number".parse::<i32>().unwrap_err();
        let valknut_err: ValknutError = parse_err.into();

        assert!(matches!(valknut_err, ValknutError::Validation { .. }));
    }

    #[test]
    fn test_from_parse_float_error() {
        let parse_err = "not_a_float".parse::<f64>().unwrap_err();
        let valknut_err: ValknutError = parse_err.into();

        assert!(matches!(valknut_err, ValknutError::Validation { .. }));
    }

    #[test]
    fn test_from_utf8_error() {
        let invalid_utf8 = vec![0, 159, 146, 150]; // Invalid UTF-8 sequence
        let utf8_err = std::str::from_utf8(&invalid_utf8).unwrap_err();
        let valknut_err: ValknutError = utf8_err.into();

        assert!(matches!(valknut_err, ValknutError::Parse { .. }));
    }

    #[test]
    fn test_with_context_math_error() {
        let mut err = ValknutError::math("Overflow occurred");
        err = err.with_context("In statistical calculation");

        if let ValknutError::Math { context, .. } = err {
            assert_eq!(context, Some("In statistical calculation".to_string()));
        } else {
            panic!("Expected Math error with context");
        }
    }

    #[test]
    fn test_with_context_non_contextual_error() {
        let err = ValknutError::config("Bad config");
        let err_with_context = err.with_context("Should not change");

        // Config errors don't support context, so it should remain unchanged
        if let ValknutError::Config { message, .. } = err_with_context {
            assert_eq!(message, "Bad config");
        } else {
            panic!("Expected Config error");
        }
    }

    #[test]
    fn test_result_ext_with_context() {
        let result: std::result::Result<i32, std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Bad input",
        ));

        let valknut_result = result.with_context(|| "Processing failed".to_string());
        assert!(valknut_result.is_err());

        // Verify the error was converted and context was added
        let err = valknut_result.unwrap_err();
        assert!(matches!(err, ValknutError::Io { .. }));
    }

    #[test]
    fn test_error_display_formatting() {
        let err = ValknutError::parse_with_location(
            "python",
            "Syntax error",
            "test.py",
            Some(10),
            Some(5),
        );
        let display = format!("{}", err);
        assert!(display.contains("Parse error in python"));
        assert!(display.contains("Syntax error"));
    }

    #[test]
    fn test_error_debug_formatting() {
        let err = ValknutError::config_field("Invalid threshold", "complexity_max");
        let debug = format!("{:?}", err);
        assert!(debug.contains("Config"));
        assert!(debug.contains("Invalid threshold"));
        assert!(debug.contains("complexity_max"));
    }
}
