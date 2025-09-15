//! Core data types for live reachability analysis

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Kind of call edge
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// Runtime call edge sampled from production
    Runtime,
    /// Static call edge inferred from code analysis  
    Static,
}

/// A single call edge event (newline-delimited JSON format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdgeEvent {
    /// Unix timestamp
    pub ts: i64,
    
    /// Programming language
    pub lang: String,
    
    /// Service name
    pub svc: String,
    
    /// Version/SHA of the deployment
    pub ver: String,
    
    /// Calling function (fully qualified)
    pub caller: String,
    
    /// Called function (fully qualified)
    pub callee: String,
    
    /// Kind of edge (runtime or static)
    pub kind: EdgeKind,
    
    /// Sampled count/weight
    pub weight: u32,
    
    /// Optional HTTP route (for web services)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    
    /// Optional tenant identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    
    /// Optional host identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

/// Daily aggregated edge data for parquet storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedEdge {
    /// Calling function symbol ID
    pub caller: String,
    
    /// Called function symbol ID
    pub callee: String,
    
    /// Kind of edge
    pub kind: EdgeKind,
    
    /// Total call count
    pub calls: u64,
    
    /// Number of unique callers (for runtime edges)
    pub callers: u32,
    
    /// First timestamp seen
    pub first_ts: i64,
    
    /// Last timestamp seen
    pub last_ts: i64,
}

/// Canonical symbol identifier: "{lang}:{svc}:{fq_name}"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId {
    /// Programming language
    pub lang: String,
    /// Service name  
    pub svc: String,
    /// Fully qualified name
    pub fq_name: String,
}

impl SymbolId {
    pub fn new(lang: impl Into<String>, svc: impl Into<String>, fq_name: impl Into<String>) -> Self {
        Self {
            lang: lang.into(),
            svc: svc.into(),
            fq_name: fq_name.into(),
        }
    }
    
    /// Parse from string format "{lang}:{svc}:{fq_name}"
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(3, ':').collect();
        if parts.len() == 3 {
            Some(Self {
                lang: parts[0].to_string(),
                svc: parts[1].to_string(), 
                fq_name: parts[2].to_string(),
            })
        } else {
            None
        }
    }
    
    /// Convert to string format "{lang}:{svc}:{fq_name}"
    pub fn to_string(&self) -> String {
        format!("{}:{}:{}", self.lang, self.svc, self.fq_name)
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.lang, self.svc, self.fq_name)
    }
}

/// Node statistics for live reach scoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeStats {
    /// Number of live runtime callers
    pub live_callers: u32,
    
    /// Total weighted runtime calls received
    pub live_calls: u64,
    
    /// Last time this symbol was seen in runtime traces
    pub last_seen: Option<DateTime<Utc>>,
    
    /// First time this symbol was seen in traces
    pub first_seen: Option<DateTime<Utc>>,
    
    /// Whether reachable from entrypoint via static+runtime edges
    pub seed_reachable: bool,
}

/// Community detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    /// Community identifier
    pub id: String,
    
    /// Number of nodes in community
    pub size: usize,
    
    /// Shadow island score (0.0 to 1.0, higher = more isolated)
    pub score: f64,
    
    /// Ratio of edges crossing community boundary
    pub cut_ratio: f64,
    
    /// Fraction of internal edges that are runtime vs static
    pub runtime_internal: f64,
    
    /// Nodes in this community
    pub nodes: Vec<CommunityNode>,
    
    /// Top inbound edges from other communities
    pub top_inbound: Vec<CrossCommunityEdge>,
    
    /// Top outbound edges to other communities  
    pub top_outbound: Vec<CrossCommunityEdge>,
    
    /// Analysis notes for this community
    pub notes: Vec<String>,
}

/// Node within a community
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityNode {
    /// Symbol identifier
    pub id: String,
    
    /// Live reach score (0.0 to 1.0)
    pub live_reach: f64,
    
    /// Last time seen in runtime traces
    pub last_seen: Option<String>,
    
    /// Whether reachable from known entrypoints
    pub seed_reachable: bool,
}

/// Edge crossing community boundaries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossCommunityEdge {
    /// Source or target symbol (depending on context)
    pub symbol: String,
    
    /// Runtime call weight
    pub w_runtime: u64,
    
    /// Static call weight
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w_static: Option<u64>,
}

/// Complete live reachability analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveReachReport {
    /// When this report was generated
    pub generated_at: DateTime<Utc>,
    
    /// Service analyzed
    pub svc: String,
    
    /// Analysis window (start, end timestamps)
    pub window: (DateTime<Utc>, DateTime<Utc>),
    
    /// Detected communities sorted by shadow island score
    pub communities: Vec<Community>,
    
    /// Overall statistics
    pub stats: ReportStats,
}

/// Overall report statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportStats {
    /// Total nodes in call graph
    pub total_nodes: usize,
    
    /// Total edges in call graph
    pub total_edges: usize,
    
    /// Number of runtime edges
    pub runtime_edges: usize,
    
    /// Number of static edges
    pub static_edges: usize,
    
    /// Number of communities detected
    pub communities: usize,
    
    /// Number of shadow islands (score >= threshold)
    pub shadow_islands: usize,
    
    /// Median live reach score across all nodes
    pub median_live_reach: f64,
}

/// Live reach score components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveReachScore {
    /// Raw live reach score (0.0 to 1.0)
    pub score: f64,
    
    /// Component scores for debugging
    pub components: LiveReachComponents,
}

/// Components of live reach calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveReachComponents {
    /// Rank-normalized live callers component
    pub callers_component: f64,
    
    /// Rank-normalized live calls component  
    pub calls_component: f64,
    
    /// Seed reachability component (0 or 1)
    pub seed_component: f64,
    
    /// Recency component (based on last_seen)
    pub recency_component: f64,
}

impl CallEdgeEvent {
    /// Create symbol ID for caller
    pub fn caller_symbol(&self) -> SymbolId {
        SymbolId::new(&self.lang, &self.svc, &self.caller)
    }
    
    /// Create symbol ID for callee
    pub fn callee_symbol(&self) -> SymbolId {
        SymbolId::new(&self.lang, &self.svc, &self.callee)
    }
    
    /// Convert timestamp to DateTime
    pub fn timestamp(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.ts, 0).unwrap_or_else(Utc::now)
    }
}

impl AggregatedEdge {
    /// First timestamp as DateTime
    pub fn first_timestamp(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.first_ts, 0).unwrap_or_else(Utc::now)
    }
    
    /// Last timestamp as DateTime
    pub fn last_timestamp(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.last_ts, 0).unwrap_or_else(Utc::now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_symbol_id_parsing() {
        let symbol = SymbolId::new("python", "api", "users.view:list_users");
        let string_repr = symbol.to_string();
        assert_eq!(string_repr, "python:api:users.view:list_users");
        
        let parsed = SymbolId::from_string(&string_repr).unwrap();
        assert_eq!(parsed.lang, "python");
        assert_eq!(parsed.svc, "api");
        assert_eq!(parsed.fq_name, "users.view:list_users");
        assert_eq!(parsed, symbol);
    }
    
    #[test]
    fn test_symbol_id_invalid_parsing() {
        assert!(SymbolId::from_string("invalid").is_none());
        assert!(SymbolId::from_string("lang:svc").is_none());
        
        // Should handle colons in fq_name
        let symbol = SymbolId::from_string("rust:api:module::struct::method").unwrap();
        assert_eq!(symbol.fq_name, "module::struct::method");
    }
    
    #[test] 
    fn test_edge_kind_serialization() {
        let runtime = EdgeKind::Runtime;
        let static_edge = EdgeKind::Static;
        
        let runtime_json = serde_json::to_string(&runtime).unwrap();
        let static_json = serde_json::to_string(&static_edge).unwrap();
        
        assert_eq!(runtime_json, "\"runtime\"");
        assert_eq!(static_json, "\"static\"");
        
        let runtime_parsed: EdgeKind = serde_json::from_str(&runtime_json).unwrap();
        let static_parsed: EdgeKind = serde_json::from_str(&static_json).unwrap();
        
        assert_eq!(runtime_parsed, EdgeKind::Runtime);
        assert_eq!(static_parsed, EdgeKind::Static);
    }
    
    #[test]
    fn test_call_edge_event_serialization() {
        let event = CallEdgeEvent {
            ts: 1699999999,
            lang: "py".to_string(),
            svc: "api".to_string(),
            ver: "2025.09.10".to_string(),
            caller: "users.view:list_users".to_string(),
            callee: "users.repo:get_all".to_string(),
            kind: EdgeKind::Runtime,
            weight: 1,
            route: Some("/users".to_string()),
            tenant: None,
            host: Some("api-1".to_string()),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        let parsed: CallEdgeEvent = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.ts, event.ts);
        assert_eq!(parsed.lang, event.lang);
        assert_eq!(parsed.caller, event.caller);
        assert_eq!(parsed.callee, event.callee);
        assert_eq!(parsed.kind, event.kind);
        assert_eq!(parsed.route, event.route);
        assert_eq!(parsed.host, event.host);
    }
    
    #[test]
    fn test_symbol_methods() {
        let event = CallEdgeEvent {
            ts: 1699999999,
            lang: "python".to_string(),
            svc: "api".to_string(),
            ver: "v1.0".to_string(),
            caller: "module.function".to_string(),
            callee: "other.function".to_string(),
            kind: EdgeKind::Runtime,
            weight: 5,
            route: None,
            tenant: None,
            host: None,
        };
        
        let caller_symbol = event.caller_symbol();
        let callee_symbol = event.callee_symbol();
        
        assert_eq!(caller_symbol.lang, "python");
        assert_eq!(caller_symbol.svc, "api");
        assert_eq!(caller_symbol.fq_name, "module.function");
        
        assert_eq!(callee_symbol.fq_name, "other.function");
    }
    
    #[test]
    fn test_aggregated_edge_timestamps() {
        let edge = AggregatedEdge {
            caller: "a".to_string(),
            callee: "b".to_string(),
            kind: EdgeKind::Runtime,
            calls: 100,
            callers: 10,
            first_ts: 1699999000,
            last_ts: 1699999999,
        };
        
        let first = edge.first_timestamp();
        let last = edge.last_timestamp();
        
        assert!(first < last);
        assert_eq!(first.timestamp(), 1699999000);
        assert_eq!(last.timestamp(), 1699999999);
    }

    #[test]
    fn test_node_stats_default() {
        let stats = NodeStats::default();
        assert_eq!(stats.live_callers, 0);
        assert_eq!(stats.live_calls, 0);
        assert!(stats.first_seen.is_none());
        assert!(stats.last_seen.is_none());
        assert!(!stats.seed_reachable);
    }

    #[test]
    fn test_community_scoring() {
        let community = Community {
            id: "community_1".to_string(),
            size: 5,
            score: 0.85,
            cut_ratio: 0.15,
            runtime_internal: 0.8,
            nodes: vec![
                CommunityNode {
                    id: "python:api:module1:func1".to_string(),
                    live_reach: 0.9,
                    last_seen: Some("2025-01-15T10:30:00Z".to_string()),
                    seed_reachable: true,
                },
                CommunityNode {
                    id: "python:api:module1:func2".to_string(),
                    live_reach: 0.8,
                    last_seen: None,
                    seed_reachable: false,
                }
            ],
            top_inbound: vec![],
            top_outbound: vec![],
            notes: vec!["High isolation detected".to_string()],
        };
        
        assert!(community.score > 0.8); // High shadow island score
        assert_eq!(community.nodes.len(), 2);
        assert_eq!(community.size, 5); // Size can be different from nodes.len()
    }

    #[test]
    fn test_cross_community_edge() {
        let edge = CrossCommunityEdge {
            symbol: "python:api:module2:func1".to_string(),
            w_runtime: 25,
            w_static: Some(5),
        };
        
        assert_eq!(edge.w_runtime, 25);
        assert_eq!(edge.w_static, Some(5));
        assert!(edge.symbol.contains("module2"));
    }

    #[test]
    fn test_live_reach_score_components() {
        let score = LiveReachScore {
            score: 0.75,
            components: LiveReachComponents {
                callers_component: 0.8,
                calls_component: 0.7,
                seed_component: 1.0,
                recency_component: 0.6,
            },
        };
        
        // All scores should be between 0 and 1
        assert!(score.score >= 0.0 && score.score <= 1.0);
        assert!(score.components.callers_component >= 0.0 && score.components.callers_component <= 1.0);
        assert!(score.components.calls_component >= 0.0 && score.components.calls_component <= 1.0);
        assert!(score.components.seed_component >= 0.0 && score.components.seed_component <= 1.0);
        assert!(score.components.recency_component >= 0.0 && score.components.recency_component <= 1.0);
    }

    #[test]
    fn test_report_stats_consistency() {
        let stats = ReportStats {
            total_nodes: 100,
            total_edges: 250,
            runtime_edges: 200,
            static_edges: 50,
            communities: 10,
            shadow_islands: 3,
            median_live_reach: 0.45,
        };
        
        // Consistency checks
        assert_eq!(stats.runtime_edges + stats.static_edges, stats.total_edges);
        assert!(stats.shadow_islands <= stats.communities);
        assert!(stats.median_live_reach >= 0.0 && stats.median_live_reach <= 1.0);
    }

    #[test]
    fn test_aggregated_edge_with_service_info() {
        let edge = AggregatedEdge {
            caller: "js:web:UserController".to_string(),
            callee: "js:web:UserService".to_string(),
            kind: EdgeKind::Runtime,
            calls: 150,
            callers: 5,
            first_ts: 1699999000,
            last_ts: 1699999999,
        };
        
        assert_eq!(edge.calls, 150);
        assert_eq!(edge.callers, 5);
        assert!(edge.first_ts <= edge.last_ts);
        
        // Test timestamp conversion
        let first = edge.first_timestamp();
        let last = edge.last_timestamp();
        assert!(first <= last);
    }

    #[test]
    fn test_live_reach_report_structure() {
        use chrono::Duration;
        let now = chrono::Utc::now();
        let report = LiveReachReport {
            generated_at: now,
            svc: "api".to_string(),
            window: (now - Duration::days(30), now),
            communities: vec![
                Community {
                    id: "comm_0".to_string(),
                    size: 10,
                    score: 0.8,
                    cut_ratio: 0.2,
                    runtime_internal: 0.9,
                    nodes: vec![],
                    top_inbound: vec![],
                    top_outbound: vec![],
                    notes: vec![],
                }
            ],
            stats: ReportStats {
                total_nodes: 100,
                total_edges: 200,
                runtime_edges: 150,
                static_edges: 50,
                communities: 5,
                shadow_islands: 2,
                median_live_reach: 0.6,
            },
        };
        
        assert_eq!(report.svc, "api");
        assert_eq!(report.communities.len(), 1);
        assert!(report.window.0 < report.window.1);
        assert_eq!(report.stats.communities, 5);
    }

    #[test]
    fn test_node_stats_evolution() {
        use chrono::Duration;
        let mut stats = NodeStats::default();
        
        // Simulate receiving calls over time
        let now = chrono::Utc::now();
        stats.live_callers = 5;
        stats.live_calls = 100;
        stats.first_seen = Some(now - Duration::days(10));
        stats.last_seen = Some(now);
        stats.seed_reachable = true;
        
        assert_eq!(stats.live_callers, 5);
        assert_eq!(stats.live_calls, 100);
        assert!(stats.first_seen.unwrap() < stats.last_seen.unwrap());
        assert!(stats.seed_reachable);
    }
}