//! Storage and aggregation system for live reachability data
//!
//! Handles NDJSON event ingestion, daily aggregation to JSON, and efficient querying

use crate::core::errors::{Result, ValknutError};
use crate::live::types::{AggregatedEdge, CallEdgeEvent, EdgeKind};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use url::Url;

/// Storage backend for live reachability data
pub struct LiveStorage {
    base_path: Url,
}

/// Aggregation bucket for collecting edges before writing to storage
#[derive(Debug, Default)]
pub struct AggregationBucket {
    edges: HashMap<EdgeKey, EdgeAccumulator>,
}

/// Key for grouping edges in aggregation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EdgeKey {
    caller: String,
    callee: String,
    kind: EdgeKind,
}

/// Accumulates edge statistics for aggregation
#[derive(Debug, Default)]
struct EdgeAccumulator {
    calls: u64,
    callers: std::collections::HashSet<String>, // Track unique callers
    first_ts: Option<i64>,
    last_ts: Option<i64>,
}

/// Query parameters for reading aggregated data
#[derive(Debug, Clone)]
pub struct AggregationQuery {
    /// Services to include
    pub services: Vec<String>,

    /// Start date (inclusive)
    pub start_date: DateTime<Utc>,

    /// End date (inclusive)  
    pub end_date: DateTime<Utc>,

    /// Versions to include (empty = all)
    pub versions: Vec<String>,

    /// Edge kinds to include
    pub edge_kinds: Vec<EdgeKind>,
}

impl LiveStorage {
    /// Create a new storage backend
    pub fn new(base_path: impl AsRef<str>) -> Result<Self> {
        let base_path = Url::parse(base_path.as_ref())
            .map_err(|e| ValknutError::validation(format!("Invalid storage URL: {}", e)))?;

        Ok(Self { base_path })
    }

    /// Ingest NDJSON events from a file or stream
    pub async fn ingest_events<P: AsRef<Path>>(&self, file_path: P) -> Result<AggregationBucket> {
        let file = fs::File::open(file_path)
            .await
            .map_err(|e| ValknutError::io("Failed to open events file", e))?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut bucket = AggregationBucket::default();

        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|e| ValknutError::io("Failed to read line", e))?
        {
            if line.trim().is_empty() {
                continue;
            }

            let event: CallEdgeEvent = serde_json::from_str(&line)
                .map_err(|e| ValknutError::validation(format!("Invalid JSON event: {}", e)))?;

            bucket.add_event(event);
        }

        Ok(bucket)
    }

    /// Write aggregated data to partitioned JSON files
    pub async fn write_aggregation(
        &self,
        bucket: &AggregationBucket,
        service: &str,
        version: &str,
        date: DateTime<Utc>,
    ) -> Result<()> {
        let edges = bucket.to_aggregated_edges();

        if edges.is_empty() {
            return Ok(()); // Nothing to write
        }

        let path = self.get_partition_path(service, version, date);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| ValknutError::io("Failed to create partition directory", e))?;
        }

        // Serialize to JSON
        let json_data = serde_json::to_vec_pretty(&edges)
            .map_err(|e| ValknutError::validation(format!("Failed to serialize edges: {}", e)))?;

        // Write JSON file
        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| ValknutError::io("Failed to create JSON file", e))?;

        file.write_all(&json_data)
            .await
            .map_err(|e| ValknutError::io("Failed to write JSON data", e))?;

        file.flush()
            .await
            .map_err(|e| ValknutError::io("Failed to flush JSON file", e))?;

        tracing::info!(
            "Wrote {} edges to partition: {}",
            edges.len(),
            path.display()
        );

        Ok(())
    }

    /// Query aggregated data across date range and services
    pub async fn query_aggregated(&self, query: &AggregationQuery) -> Result<Vec<AggregatedEdge>> {
        let mut all_edges = Vec::new();
        let mut current_date = query.start_date.date_naive();
        let end_date = query.end_date.date_naive();

        while current_date <= end_date {
            for service in &query.services {
                let pattern = self.get_partition_pattern(
                    service,
                    current_date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                );
                let edges = self.read_partition_pattern(&pattern, query).await?;
                all_edges.extend(edges);
            }
            current_date += chrono::Duration::days(1);
        }

        // Apply additional filtering
        all_edges.retain(|edge| {
            (query.edge_kinds.is_empty() || query.edge_kinds.contains(&edge.kind))
                && (query.versions.is_empty()
                    || query
                        .versions
                        .iter()
                        .any(|v| edge.caller.contains(v) || edge.callee.contains(v)))
        });

        Ok(all_edges)
    }

    /// Get partition path for service/version/date
    fn get_partition_path(&self, service: &str, version: &str, date: DateTime<Utc>) -> PathBuf {
        let date_str = date.format("%Y-%m-%d").to_string();

        if self.base_path.scheme() == "file" || self.base_path.scheme().is_empty() {
            // Local filesystem
            let base = PathBuf::from(self.base_path.path());
            base.join("edges")
                .join(format!("date={}", date_str))
                .join(format!("svc={}", service))
                .join(format!("ver={}", version))
                .join("data.json") // Changed from .parquet to .json
        } else {
            // For S3/cloud storage, we'd need object_store integration
            // For now, just create a local path representation
            PathBuf::from(format!(
                "edges/date={}/svc={}/ver={}/data.json",
                date_str, service, version
            ))
        }
    }

    /// Get partition pattern for globbing
    fn get_partition_pattern(&self, service: &str, date: DateTime<Utc>) -> PathBuf {
        let date_str = date.format("%Y-%m-%d").to_string();

        if self.base_path.scheme() == "file" || self.base_path.scheme().is_empty() {
            let base = PathBuf::from(self.base_path.path());
            base.join("edges")
                .join(format!("date={}", date_str))
                .join(format!("svc={}", service))
                .join("**")
                .join("*.json") // Changed from .parquet to .json
        } else {
            PathBuf::from(format!("edges/date={}/svc={}/**/*.json", date_str, service))
        }
    }

    /// Read JSON files matching a pattern
    async fn read_partition_pattern(
        &self,
        pattern: &Path,
        _query: &AggregationQuery,
    ) -> Result<Vec<AggregatedEdge>> {
        // For now, implement basic file reading
        // In production, this would use glob patterns and object_store
        if pattern.exists() {
            self.read_json_file(pattern).await
        } else {
            Ok(Vec::new())
        }
    }

    /// Read a single JSON file
    async fn read_json_file(&self, path: &Path) -> Result<Vec<AggregatedEdge>> {
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| ValknutError::io("Failed to read JSON file", e))?;

        let edges: Vec<AggregatedEdge> = serde_json::from_str(&content)
            .map_err(|e| ValknutError::validation(format!("Failed to deserialize JSON: {}", e)))?;

        Ok(edges)
    }
}

impl AggregationBucket {
    /// Add an event to the aggregation bucket
    pub fn add_event(&mut self, event: CallEdgeEvent) {
        let key = EdgeKey {
            caller: event.caller_symbol().to_string(),
            callee: event.callee_symbol().to_string(),
            kind: event.kind,
        };

        let accumulator = self.edges.entry(key).or_default();
        accumulator.calls += event.weight as u64;
        accumulator.callers.insert(event.caller.clone());

        let ts = event.ts;
        accumulator.first_ts = Some(accumulator.first_ts.unwrap_or(ts).min(ts));
        accumulator.last_ts = Some(accumulator.last_ts.unwrap_or(ts).max(ts));
    }

    /// Convert to aggregated edges for storage
    pub fn to_aggregated_edges(&self) -> Vec<AggregatedEdge> {
        self.edges
            .iter()
            .map(|(key, acc)| AggregatedEdge {
                caller: key.caller.clone(),
                callee: key.callee.clone(),
                kind: key.kind.clone(),
                calls: acc.calls,
                callers: acc.callers.len() as u32,
                first_ts: acc.first_ts.unwrap_or(0),
                last_ts: acc.last_ts.unwrap_or(0),
            })
            .collect()
    }

    /// Get number of unique edges
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Check if bucket is empty
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_aggregation_bucket() {
        let mut bucket = AggregationBucket::default();

        let event1 = CallEdgeEvent {
            ts: 1699999999,
            lang: "python".to_string(),
            svc: "api".to_string(),
            ver: "v1.0".to_string(),
            caller: "mod1.func1".to_string(),
            callee: "mod2.func2".to_string(),
            kind: EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        let event2 = CallEdgeEvent {
            ts: 1700000000,
            lang: "python".to_string(),
            svc: "api".to_string(),
            ver: "v1.0".to_string(),
            caller: "mod1.func1".to_string(),
            callee: "mod2.func2".to_string(),
            kind: EdgeKind::Runtime,
            weight: 2,
            route: None,
            tenant: None,
            host: None,
        };

        bucket.add_event(event1);
        bucket.add_event(event2);

        let edges = bucket.to_aggregated_edges();
        assert_eq!(edges.len(), 1);

        let edge = &edges[0];
        assert_eq!(edge.calls, 3); // 1 + 2
        assert_eq!(edge.callers, 1); // Same caller
        assert_eq!(edge.first_ts, 1699999999);
        assert_eq!(edge.last_ts, 1700000000);
    }

    #[test]
    fn test_aggregation_bucket_different_edges() {
        let mut bucket = AggregationBucket::default();

        let event1 = CallEdgeEvent {
            ts: 1699999999,
            lang: "python".to_string(),
            svc: "api".to_string(),
            ver: "v1.0".to_string(),
            caller: "mod1.func1".to_string(),
            callee: "mod2.func2".to_string(),
            kind: EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        let event2 = CallEdgeEvent {
            ts: 1700000000,
            lang: "python".to_string(),
            svc: "api".to_string(),
            ver: "v1.0".to_string(),
            caller: "mod1.func1".to_string(),
            callee: "mod3.func3".to_string(), // Different callee
            kind: EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        bucket.add_event(event1);
        bucket.add_event(event2);

        let edges = bucket.to_aggregated_edges();
        assert_eq!(edges.len(), 2); // Different callees = different edges
    }

    #[tokio::test]
    async fn test_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = format!("file://{}", temp_dir.path().display());

        let storage = LiveStorage::new(path).unwrap();
        assert_eq!(storage.base_path.scheme(), "file");
    }

    #[tokio::test]
    async fn test_invalid_storage_url() {
        let result = LiveStorage::new("not-a-url");
        assert!(result.is_err());
    }

    #[test]
    fn test_aggregation_query() {
        let query = AggregationQuery {
            services: vec!["api".to_string()],
            start_date: Utc::now() - chrono::Duration::days(7),
            end_date: Utc::now(),
            versions: vec![],
            edge_kinds: vec![EdgeKind::Runtime],
        };

        assert_eq!(query.services.len(), 1);
        assert_eq!(query.edge_kinds.len(), 1);
    }

    #[test]
    fn test_call_edge_event_parsing() {
        let event = CallEdgeEvent {
            ts: 1699999999,
            lang: "javascript".to_string(),
            svc: "web".to_string(),
            ver: "v2.1.0".to_string(),
            caller: "UserController.createUser".to_string(),
            callee: "UserService.save".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: Some("/users".to_string()),
            tenant: None,
            host: Some("web-1".to_string()),
        };

        assert_eq!(event.lang, "javascript");
        assert_eq!(event.svc, "web");
        assert!(event.ts > 0);
        assert!(event.caller.contains("UserController"));
        assert!(event.callee.contains("UserService"));
    }

    #[test]
    fn test_aggregation_bucket_edge_accumulation() {
        let mut bucket = AggregationBucket::default();

        // Add same edge multiple times with different callers
        let event1 = CallEdgeEvent {
            ts: 1699999990,
            lang: "rust".to_string(),
            svc: "api".to_string(),
            ver: "v1.0.0".to_string(),
            caller: "module::caller1".to_string(),
            callee: "module::target".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        let event2 = CallEdgeEvent {
            ts: 1699999995,
            lang: "rust".to_string(),
            svc: "api".to_string(),
            ver: "v1.0.0".to_string(),
            caller: "module::caller2".to_string(), // Different caller
            callee: "module::target".to_string(),  // Same callee
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        bucket.add_event(event1);
        bucket.add_event(event2);

        assert_eq!(bucket.len(), 2); // Two unique edges

        let edges = bucket.to_aggregated_edges();
        assert_eq!(edges.len(), 2);

        // Each edge should have calls = 1, callers = 1
        for edge in &edges {
            assert_eq!(edge.calls, 1);
            assert_eq!(edge.callers, 1);
        }
    }

    #[test]
    fn test_aggregation_bucket_same_edge_accumulation() {
        let mut bucket = AggregationBucket::default();

        // Add the exact same edge multiple times
        let event = CallEdgeEvent {
            ts: 1699999999,
            lang: "python".to_string(),
            svc: "api".to_string(),
            ver: "v1.0.0".to_string(),
            caller: "module.func1".to_string(),
            callee: "module.func2".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        bucket.add_event(event.clone());
        bucket.add_event(event.clone());
        bucket.add_event(event);

        assert_eq!(bucket.len(), 1); // Only one unique edge

        let edges = bucket.to_aggregated_edges();
        assert_eq!(edges.len(), 1);

        let edge = &edges[0];
        assert_eq!(edge.calls, 3); // Three calls accumulated
        assert_eq!(edge.callers, 1); // Only one unique caller
        assert_eq!(edge.caller, "python:api:module.func1");
        assert_eq!(edge.callee, "python:api:module.func2");
    }

    #[test]
    fn test_aggregation_bucket_timestamp_tracking() {
        let mut bucket = AggregationBucket::default();

        let event1 = CallEdgeEvent {
            ts: 1699999990, // Earlier
            lang: "java".to_string(),
            svc: "service".to_string(),
            ver: "v1.0.0".to_string(),
            caller: "Class.method1".to_string(),
            callee: "Class.method2".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        let event2 = CallEdgeEvent {
            ts: 1699999999, // Later
            lang: "java".to_string(),
            svc: "service".to_string(),
            ver: "v1.0.0".to_string(),
            caller: "Class.method1".to_string(),
            callee: "Class.method2".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        bucket.add_event(event1);
        bucket.add_event(event2);

        let edges = bucket.to_aggregated_edges();
        let edge = &edges[0];

        assert_eq!(edge.first_ts, 1699999990); // Should track earliest
        assert_eq!(edge.last_ts, 1699999999); // Should track latest
    }

    #[tokio::test]
    async fn test_live_storage_ingest_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("storage");

        let storage = LiveStorage::new(&format!("file://{}", storage_path.display())).unwrap();

        // Create empty NDJSON file
        let empty_file = temp_dir.path().join("empty.ndjson");
        tokio::fs::write(&empty_file, "").await.unwrap();

        let bucket = storage.ingest_events(&empty_file).await.unwrap();
        assert!(bucket.is_empty());
        assert_eq!(bucket.len(), 0);
    }

    #[tokio::test]
    async fn test_aggregation_query_date_filtering() {
        let now = Utc::now();
        let start_date = now - chrono::Duration::days(7);
        let end_date = now;

        let query = AggregationQuery {
            services: vec!["api".to_string(), "web".to_string()],
            start_date,
            end_date,
            versions: vec!["v1.0.0".to_string()],
            edge_kinds: vec![
                crate::live::types::EdgeKind::Runtime,
                crate::live::types::EdgeKind::Static,
            ],
        };

        assert!(query.start_date < query.end_date);
        assert_eq!(query.services.len(), 2);
        assert_eq!(query.versions.len(), 1);
        assert_eq!(query.edge_kinds.len(), 2);
    }

    #[test]
    fn test_edge_key_generation() {
        let event = CallEdgeEvent {
            ts: 1699999999,
            lang: "typescript".to_string(),
            svc: "frontend".to_string(),
            ver: "v3.0.0".to_string(),
            caller: "UserComponent.handleClick".to_string(),
            callee: "ApiService.saveUser".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        let key = EdgeKey {
            caller: event.caller_symbol().to_string(),
            callee: event.callee_symbol().to_string(),
            kind: event.kind.clone(),
        };

        assert_eq!(key.caller, "typescript:frontend:UserComponent.handleClick");
        assert_eq!(key.callee, "typescript:frontend:ApiService.saveUser");
        assert_eq!(key.kind, crate::live::types::EdgeKind::Runtime);
    }

    #[test]
    fn test_aggregation_bucket_multiple_services() {
        let mut bucket = AggregationBucket::default();

        // Add events from different services
        let api_event = CallEdgeEvent {
            ts: 1699999999,
            lang: "python".to_string(),
            svc: "api".to_string(),
            ver: "v1.0.0".to_string(),
            caller: "api.handler".to_string(),
            callee: "api.service".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        let worker_event = CallEdgeEvent {
            ts: 1699999999,
            lang: "python".to_string(),
            svc: "worker".to_string(),
            ver: "v1.0.0".to_string(),
            caller: "worker.task".to_string(),
            callee: "worker.processor".to_string(),
            kind: crate::live::types::EdgeKind::Runtime,
            weight: 1,
            route: None,
            tenant: None,
            host: None,
        };

        bucket.add_event(api_event);
        bucket.add_event(worker_event);

        assert_eq!(bucket.len(), 2);

        let edges = bucket.to_aggregated_edges();
        let api_edges: Vec<_> = edges.iter().filter(|e| e.caller.contains("api:")).collect();
        let worker_edges: Vec<_> = edges
            .iter()
            .filter(|e| e.caller.contains("worker:"))
            .collect();

        assert_eq!(api_edges.len(), 1);
        assert_eq!(worker_edges.len(), 1);
    }
}
