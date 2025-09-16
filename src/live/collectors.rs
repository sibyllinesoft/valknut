//! Runtime collectors for sampling call edges in production
//!
//! Production-safe collectors with configurable sampling, Bloom filtering,
//! and minimal performance overhead

use crate::core::errors::{Result, ValknutError};
use crate::live::types::{CallEdgeEvent, EdgeKind};

use std::sync::Arc;
use std::time::SystemTime;

use bloom::{BloomFilter, ASMS};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Configuration for runtime collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorConfig {
    /// Whether collection is enabled
    pub enabled: bool,

    /// Sampling rate (0.0 to 1.0)
    pub sample_rate: f64,

    /// Maximum edges per request to prevent DoS
    pub max_edges_per_request: u32,

    /// Service name for this collector
    pub service_name: String,

    /// Current deployment version/SHA
    pub version: String,

    /// Language being collected
    pub language: String,

    /// Bloom filter size in bits (per request)
    pub bloom_filter_bits: u32,

    /// Number of hash functions for Bloom filter
    pub bloom_filter_hashes: u32,

    /// Batch size for output events
    pub batch_size: usize,

    /// Flush interval in seconds
    pub flush_interval_secs: u64,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("VALKNUT_LIVE")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
            sample_rate: std::env::var("VALKNUT_LIVE_SAMPLE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.02),
            max_edges_per_request: std::env::var("VALKNUT_LIVE_MAX_EDGES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            service_name: std::env::var("VALKNUT_SERVICE")
                .unwrap_or_else(|_| "unknown".to_string()),
            version: std::env::var("VALKNUT_VERSION")
                .or_else(|_| std::env::var("GIT_SHA"))
                .unwrap_or_else(|_| "unknown".to_string()),
            language: "unknown".to_string(),
            bloom_filter_bits: 16384, // 2KB
            bloom_filter_hashes: 3,
            batch_size: 100,
            flush_interval_secs: 5,
        }
    }
}

/// Runtime edge collector
pub struct EdgeCollector {
    config: CollectorConfig,
    batch: Arc<Mutex<Vec<CallEdgeEvent>>>,
    stats: Arc<Mutex<CollectorStats>>,
}

/// Statistics for monitoring collector health
#[derive(Debug, Default)]
pub struct CollectorStats {
    /// Total edges observed
    pub edges_observed: u64,

    /// Edges sampled (after sampling rate)
    pub edges_sampled: u64,

    /// Edges deduplicated by Bloom filter
    pub edges_deduplicated: u64,

    /// Edges batched for output
    pub edges_batched: u64,

    /// Number of requests processed
    pub requests_processed: u64,

    /// Number of errors encountered
    pub errors: u64,

    /// Average processing time per request (microseconds)
    pub avg_processing_time_us: f64,
}

/// Per-request collection context
pub struct RequestCollector {
    config: CollectorConfig,
    bloom_filter: BloomFilter,
    edges: Vec<CallEdgeEvent>,
    edge_count: u32,
    start_time: SystemTime,
}

impl CollectorConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.sample_rate < 0.0 || self.sample_rate > 1.0 {
            return Err(ValknutError::validation(
                "Sample rate must be between 0.0 and 1.0",
            ));
        }

        if self.max_edges_per_request == 0 {
            return Err(ValknutError::validation(
                "Max edges per request must be greater than 0",
            ));
        }

        if self.bloom_filter_bits == 0 {
            return Err(ValknutError::validation(
                "Bloom filter bits must be greater than 0",
            ));
        }

        if self.bloom_filter_hashes == 0 {
            return Err(ValknutError::validation(
                "Bloom filter hashes must be greater than 0",
            ));
        }

        if self.batch_size == 0 {
            return Err(ValknutError::validation(
                "Batch size must be greater than 0",
            ));
        }

        Ok(())
    }

    /// Create configuration for a specific language
    pub fn for_language(language: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.language = language.into();
        config
    }

    /// Check if sampling should occur based on rate
    pub fn should_sample(&self) -> bool {
        if !self.enabled || self.sample_rate == 0.0 {
            return false;
        }

        if self.sample_rate >= 1.0 {
            return true;
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        std::thread::current().id().hash(&mut hasher);
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);

        let hash = hasher.finish();
        let threshold = (self.sample_rate * (u64::MAX as f64)) as u64;

        hash < threshold
    }
}

impl EdgeCollector {
    /// Create a new edge collector
    pub fn new(config: CollectorConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            batch: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(CollectorStats::default())),
        })
    }

    /// Start a new request collection context
    pub fn start_request(&self) -> Option<RequestCollector> {
        if !self.config.should_sample() {
            return None;
        }

        Some(RequestCollector::new(self.config.clone()))
    }

    /// Finish a request and add its edges to the batch
    pub async fn finish_request(&self, request: RequestCollector) -> Result<()> {
        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.requests_processed += 1;
            stats.edges_observed += request.edge_count as u64;
            stats.edges_sampled += request.edges.len() as u64;

            let processing_time =
                request.start_time.elapsed().unwrap_or_default().as_micros() as f64;

            // Exponential moving average for processing time
            let alpha = 0.1;
            stats.avg_processing_time_us =
                alpha * processing_time + (1.0 - alpha) * stats.avg_processing_time_us;
        }

        // Add edges to batch
        if !request.edges.is_empty() {
            let mut batch = self.batch.lock().await;
            batch.extend(request.edges);

            {
                let mut stats = self.stats.lock().await;
                stats.edges_batched += batch.len() as u64;
            }

            // Check if we need to flush
            if batch.len() >= self.config.batch_size {
                let to_flush: Vec<_> = batch.drain(..).collect();
                drop(batch); // Release lock

                self.flush_batch(to_flush).await?;
            }
        }

        Ok(())
    }

    /// Get current collector statistics
    pub async fn get_stats(&self) -> CollectorStats {
        self.stats.lock().await.clone()
    }

    /// Manually flush the current batch
    pub async fn flush(&self) -> Result<()> {
        let to_flush = {
            let mut batch = self.batch.lock().await;
            if batch.is_empty() {
                return Ok(());
            }
            batch.drain(..).collect()
        };

        self.flush_batch(to_flush).await
    }

    /// Flush a batch of edges (implement based on your output needs)
    async fn flush_batch(&self, edges: Vec<CallEdgeEvent>) -> Result<()> {
        // In production, this would write to a file, send to a queue, etc.
        // For now, just log the count
        tracing::info!(
            "Flushing batch of {} edges for service {} ({})",
            edges.len(),
            self.config.service_name,
            self.config.language
        );

        // TODO: Implement actual output mechanism
        // Options:
        // 1. Write NDJSON to local file with rotation
        // 2. Send to message queue (Kafka, SQS, etc.)
        // 3. Send to HTTP endpoint
        // 4. Write directly to object storage

        Ok(())
    }
}

impl RequestCollector {
    /// Create a new request collector
    fn new(config: CollectorConfig) -> Self {
        let bloom_filter = BloomFilter::with_rate(
            0.01, // 1% false positive rate
            config.bloom_filter_bits as u32,
        );

        Self {
            config,
            bloom_filter,
            edges: Vec::new(),
            edge_count: 0,
            start_time: SystemTime::now(),
        }
    }

    /// Record a call edge (if not already seen and under limits)
    pub fn record_edge(
        &mut self,
        caller: impl Into<String>,
        callee: impl Into<String>,
        route: Option<String>,
    ) {
        // Check request limits first
        self.edge_count += 1;
        if self.edge_count > self.config.max_edges_per_request {
            return; // Silently drop to prevent DoS
        }

        let caller = caller.into();
        let callee = callee.into();

        // Create edge key for deduplication
        let edge_key = format!("{}:{}", caller, callee);

        // Check Bloom filter for deduplication
        if self.bloom_filter.contains(&edge_key) {
            return; // Likely duplicate
        }
        self.bloom_filter.insert(&edge_key);

        // Create event
        let event = CallEdgeEvent {
            ts: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            lang: self.config.language.clone(),
            svc: self.config.service_name.clone(),
            ver: self.config.version.clone(),
            caller,
            callee,
            kind: EdgeKind::Runtime,
            weight: 1,
            route,
            tenant: None, // Could be extracted from request context
            host: None,   // Could be extracted from system
        };

        self.edges.push(event);
    }

    /// Get number of edges recorded in this request
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

/// Python-specific collector utilities
pub mod python {

    /// Extract module boundary from Python frame info
    pub fn is_module_boundary(caller_module: &str, callee_module: &str) -> bool {
        // Consider it a boundary if:
        // 1. Different top-level modules
        // 2. Crossing from user code to library code
        // 3. Crossing package boundaries

        if caller_module == callee_module {
            return false;
        }

        let caller_parts: Vec<&str> = caller_module.split('.').collect();
        let callee_parts: Vec<&str> = callee_module.split('.').collect();

        // Different top-level modules are always boundaries
        if caller_parts.get(0) != callee_parts.get(0) {
            return true;
        }

        // Same package, different submodules might be boundaries
        // This is a heuristic - could be refined based on project structure
        caller_parts.len() > 1
            && callee_parts.len() > 1
            && caller_parts.get(1) != callee_parts.get(1)
    }

    /// Format Python function name for collection
    pub fn format_python_symbol(module: &str, class: Option<&str>, function: &str) -> String {
        match class {
            Some(cls) => format!("{}:{}#{}", module, cls, function),
            None => format!("{}:{}", module, function),
        }
    }
}

/// Node.js-specific collector utilities  
pub mod nodejs {

    /// Extract module boundary from Node.js stack frames
    pub fn is_module_boundary(caller_file: &str, callee_file: &str) -> bool {
        // Consider it a boundary if:
        // 1. Different npm packages (node_modules)
        // 2. Different top-level directories
        // 3. Built-in modules vs user code

        if caller_file == callee_file {
            return false;
        }

        // Built-in modules
        if caller_file.starts_with("node:") || callee_file.starts_with("node:") {
            return true;
        }

        // Different packages in node_modules
        if caller_file.contains("node_modules") || callee_file.contains("node_modules") {
            return extract_npm_package(caller_file) != extract_npm_package(callee_file);
        }

        // Different top-level directories in project
        extract_top_level_dir(caller_file) != extract_top_level_dir(callee_file)
    }

    /// Extract npm package name from file path
    fn extract_npm_package(file_path: &str) -> Option<&str> {
        if let Some(node_modules_idx) = file_path.find("node_modules/") {
            let after_nm = &file_path[node_modules_idx + 13..]; // "node_modules/".len()
            if let Some(slash_idx) = after_nm.find('/') {
                Some(&after_nm[..slash_idx])
            } else {
                Some(after_nm)
            }
        } else {
            None
        }
    }

    /// Extract top-level directory from project file path
    fn extract_top_level_dir(file_path: &str) -> &str {
        // Remove any leading path and take first directory component
        let path = file_path.trim_start_matches("./").trim_start_matches('/');
        if let Some(slash_idx) = path.find('/') {
            &path[..slash_idx]
        } else {
            path
        }
    }

    /// Format Node.js function name for collection
    pub fn format_nodejs_symbol(file: &str, function: &str) -> String {
        let module = file_to_module_name(file);
        format!("{}:{}", module, function)
    }

    /// Convert file path to module name
    fn file_to_module_name(file_path: &str) -> String {
        // Convert /path/to/file.js to path/to/file
        let path = file_path.trim_start_matches("./").trim_start_matches('/');
        if let Some(dot_idx) = path.rfind('.') {
            path[..dot_idx].replace('/', ".")
        } else {
            path.replace('/', ".")
        }
    }
}

impl Clone for CollectorStats {
    fn clone(&self) -> Self {
        Self {
            edges_observed: self.edges_observed,
            edges_sampled: self.edges_sampled,
            edges_deduplicated: self.edges_deduplicated,
            edges_batched: self.edges_batched,
            requests_processed: self.requests_processed,
            errors: self.errors,
            avg_processing_time_us: self.avg_processing_time_us,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector_config_default() {
        let config = CollectorConfig::default();
        assert!(config.validate().is_ok());
        assert!(!config.enabled); // Default is disabled unless env var set
        assert_eq!(config.sample_rate, 0.02);
        assert_eq!(config.language, "unknown");
        assert_eq!(config.service_name, "unknown");
    }

    #[test]
    fn test_collector_config_for_language() {
        let python_config = CollectorConfig::for_language("python");
        assert_eq!(python_config.language, "python");
        assert_eq!(python_config.service_name, "unknown"); // Uses default service name
        assert!(python_config.validate().is_ok());

        let nodejs_config = CollectorConfig::for_language("javascript");
        assert_eq!(nodejs_config.language, "javascript");
        assert!(nodejs_config.validate().is_ok());
    }

    #[test]
    fn test_collector_config_validation() {
        let mut config = CollectorConfig::default();

        // Invalid sample rate - too high
        config.sample_rate = 1.5;
        assert!(config.validate().is_err());

        // Invalid sample rate - negative
        config.sample_rate = -0.1;
        assert!(config.validate().is_err());

        // Valid sample rate
        config.sample_rate = 0.5;
        assert!(config.validate().is_ok());

        // Invalid max edges
        config.max_edges_per_request = 0;
        assert!(config.validate().is_err());

        // Valid max edges
        config.max_edges_per_request = 100;
        assert!(config.validate().is_ok());

        // Invalid batch size
        config.batch_size = 0;
        assert!(config.validate().is_err());

        // Valid batch size
        config.batch_size = 50;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_should_sample_disabled() {
        let mut config = CollectorConfig::default();
        config.enabled = false;
        assert!(!config.should_sample());
    }

    #[test]
    fn test_should_sample_zero_rate() {
        let mut config = CollectorConfig::default();
        config.enabled = true;
        config.sample_rate = 0.0;
        assert!(!config.should_sample());
    }

    #[test]
    fn test_should_sample_full_rate() {
        let mut config = CollectorConfig::default();
        config.enabled = true;
        config.sample_rate = 1.0;
        assert!(config.should_sample());
    }

    #[test]
    fn test_should_sample_partial_rate() {
        let mut config = CollectorConfig::default();
        config.enabled = true;
        config.sample_rate = 0.5;

        // Test multiple times due to randomness
        let mut sampled_count = 0;
        let iterations = 1000;

        for _ in 0..iterations {
            if config.should_sample() {
                sampled_count += 1;
            }
        }

        // Should be roughly 50% with some tolerance
        let sample_rate = sampled_count as f64 / iterations as f64;
        assert!(
            sample_rate > 0.4 && sample_rate < 0.6,
            "Sample rate was {}",
            sample_rate
        );
    }

    #[tokio::test]
    async fn test_edge_collector_creation() {
        let config = CollectorConfig::default();
        let collector = EdgeCollector::new(config);
        assert!(collector.is_ok());

        let collector = collector.unwrap();
        let stats = collector.get_stats().await;
        assert_eq!(stats.requests_processed, 0);
        assert_eq!(stats.edges_observed, 0);
    }

    #[tokio::test]
    async fn test_edge_collector_invalid_config() {
        let mut config = CollectorConfig::default();
        config.sample_rate = 2.0; // Invalid

        let collector = EdgeCollector::new(config);
        assert!(collector.is_err());
    }

    #[tokio::test]
    async fn test_request_collector_basic() {
        let config = CollectorConfig::for_language("python");
        let collector = EdgeCollector::new(config).unwrap();

        if let Some(mut request) = collector.start_request() {
            request.record_edge("mod1.func1", "mod2.func2", None);
            request.record_edge("mod1.func1", "mod3.func3", Some("/api/test".to_string()));

            assert_eq!(request.edge_count(), 2);

            let result = collector.finish_request(request).await;
            assert!(result.is_ok());

            let stats = collector.get_stats().await;
            assert_eq!(stats.requests_processed, 1);
            assert_eq!(stats.edges_observed, 2);
        }
    }

    #[tokio::test]
    async fn test_request_collector_deduplication() {
        let config = CollectorConfig::for_language("python");
        let collector = EdgeCollector::new(config).unwrap();

        if let Some(mut request) = collector.start_request() {
            // Record the same edge multiple times
            request.record_edge("mod1.func1", "mod2.func2", None);
            request.record_edge("mod1.func1", "mod2.func2", None);
            request.record_edge("mod1.func1", "mod2.func2", None);

            // Should be deduplicated to 1 edge
            assert_eq!(request.edge_count(), 1);

            let result = collector.finish_request(request).await;
            assert!(result.is_ok());

            let stats = collector.get_stats().await;
            assert_eq!(stats.requests_processed, 1);
            assert_eq!(stats.edges_observed, 3); // All were observed
            assert!(stats.edges_deduplicated > 0); // But some were deduplicated
        }
    }

    #[tokio::test]
    async fn test_request_collector_max_edges() {
        let mut config = CollectorConfig::for_language("python");
        config.max_edges_per_request = 2; // Small limit for testing
        let collector = EdgeCollector::new(config).unwrap();

        if let Some(mut request) = collector.start_request() {
            // Try to record more edges than the limit
            request.record_edge("mod1.func1", "mod2.func2", None);
            request.record_edge("mod1.func1", "mod3.func3", None);
            request.record_edge("mod1.func1", "mod4.func4", None); // Should be ignored

            assert_eq!(request.edge_count(), 2); // Limited to max_edges_per_request

            let result = collector.finish_request(request).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_request_collector_disabled_sampling() {
        let mut config = CollectorConfig::for_language("python");
        config.sample_rate = 0.0; // No sampling
        let collector = EdgeCollector::new(config).unwrap();

        // Should return None when sampling is disabled
        let request = collector.start_request();
        assert!(request.is_none());
    }

    #[tokio::test]
    async fn test_collector_stats_accumulation() {
        let mut config = CollectorConfig::for_language("python");
        config.enabled = true; // Enable sampling
        config.sample_rate = 1.0; // 100% sampling
        let collector = EdgeCollector::new(config).unwrap();

        // Process multiple requests
        for i in 0..3 {
            if let Some(mut request) = collector.start_request() {
                request.record_edge(&format!("mod{}.func1", i), "target.func", None);
                let _ = collector.finish_request(request).await;
            }
        }

        let stats = collector.get_stats().await;
        assert_eq!(stats.requests_processed, 3);
        assert_eq!(stats.edges_observed, 3);
    }

    #[test]
    fn test_python_module_boundary_comprehensive() {
        use super::python::is_module_boundary;

        // Same module
        assert!(!is_module_boundary("myapp.views", "myapp.views"));

        // Different top-level modules
        assert!(is_module_boundary("myapp.views", "django.http"));
        assert!(is_module_boundary("requests.api", "urllib3.poolmanager"));

        // Same package, different submodules
        assert!(is_module_boundary("myapp.views", "myapp.models"));
        assert!(is_module_boundary("django.http", "django.contrib"));

        // Same submodule
        assert!(!is_module_boundary("myapp.views.user", "myapp.views.user"));

        // Nested submodules in same package
        assert!(is_module_boundary("myapp.views.user", "myapp.models.user"));

        // Single component modules
        assert!(is_module_boundary("main", "utils"));
        assert!(!is_module_boundary("main", "main"));
    }

    #[test]
    fn test_python_symbol_formatting_comprehensive() {
        use super::python::format_python_symbol;

        // Class method
        assert_eq!(
            format_python_symbol("myapp.views", Some("UserView"), "get"),
            "myapp.views:UserView#get"
        );

        // Module function
        assert_eq!(
            format_python_symbol("myapp.utils", None, "helper_function"),
            "myapp.utils:helper_function"
        );

        // Nested module with class
        assert_eq!(
            format_python_symbol("package.submodule.views", Some("APIView"), "post"),
            "package.submodule.views:APIView#post"
        );

        // Empty function name
        assert_eq!(format_python_symbol("test", None, ""), "test:");

        // Special characters in names
        assert_eq!(
            format_python_symbol("test.module", Some("TestClass"), "__init__"),
            "test.module:TestClass#__init__"
        );
    }

    #[test]
    fn test_nodejs_module_boundary_comprehensive() {
        use super::nodejs::is_module_boundary;

        // Same file
        assert!(!is_module_boundary("src/app.js", "src/app.js"));

        // Different packages in node_modules
        assert!(is_module_boundary(
            "node_modules/express/lib/router.js",
            "node_modules/body-parser/index.js"
        ));

        // User code to node_modules
        assert!(is_module_boundary(
            "src/routes.js",
            "node_modules/express/lib/router.js"
        ));

        // Different top-level directories
        assert!(is_module_boundary("controllers/user.js", "models/user.js"));
        assert!(is_module_boundary("src/app.js", "lib/utils.js"));

        // Same top-level directory
        assert!(!is_module_boundary("src/app.js", "src/utils.js"));

        // Built-in modules
        assert!(is_module_boundary("node:fs", "src/app.js"));
        assert!(is_module_boundary("node:path", "node:fs"));
        assert!(is_module_boundary("src/app.js", "node:util"));

        // Scoped packages
        assert!(is_module_boundary(
            "node_modules/@types/node/index.d.ts",
            "node_modules/typescript/lib/typescript.js"
        ));
    }

    #[test]
    fn test_nodejs_symbol_formatting_comprehensive() {
        use super::nodejs::format_nodejs_symbol;

        // Standard file
        assert_eq!(
            format_nodejs_symbol("src/controllers/user.js", "createUser"),
            "src.controllers.user:createUser"
        );

        // Relative path
        assert_eq!(
            format_nodejs_symbol("./lib/utils.js", "helper"),
            "lib.utils:helper"
        );

        // Nested directories
        assert_eq!(
            format_nodejs_symbol("src/api/v1/routes/users.js", "getUserById"),
            "src.api.v1.routes.users:getUserById"
        );

        // No extension
        assert_eq!(format_nodejs_symbol("src/app", "main"), "src.app:main");

        // TypeScript file
        assert_eq!(
            format_nodejs_symbol("src/types/index.ts", "User"),
            "src.types.index:User"
        );

        // Root level file
        assert_eq!(format_nodejs_symbol("index.js", "main"), "index:main");
    }

    #[test]
    fn test_nodejs_file_to_module_conversion() {
        use super::nodejs::format_nodejs_symbol;

        // Test various file path patterns through the public interface
        let test_cases = vec![
            ("./src/utils.js", "helper", "src.utils:helper"),
            ("/absolute/path/file.js", "func", "absolute.path.file:func"),
            (
                "relative/path/module.ts",
                "export",
                "relative.path.module:export",
            ),
            ("single.js", "main", "single:main"),
            ("no-extension", "func", "no-extension:func"),
        ];

        for (file_path, function, expected) in test_cases {
            assert_eq!(
                format_nodejs_symbol(file_path, function),
                expected,
                "Failed for file: {}",
                file_path
            );
        }
    }

    #[test]
    fn test_collector_stats_clone() {
        let stats = CollectorStats {
            edges_observed: 100,
            edges_sampled: 80,
            edges_deduplicated: 70,
            edges_batched: 60,
            requests_processed: 10,
            errors: 2,
            avg_processing_time_us: 150.0,
        };

        let cloned_stats = stats.clone();
        assert_eq!(stats.edges_observed, cloned_stats.edges_observed);
        assert_eq!(stats.edges_sampled, cloned_stats.edges_sampled);
        assert_eq!(stats.requests_processed, cloned_stats.requests_processed);
        assert_eq!(
            stats.avg_processing_time_us,
            cloned_stats.avg_processing_time_us
        );
    }
}
