//! CLI commands for live reachability analysis
//! 
//! Implements the live-reach command with various subcommands for
//! analyzing production call graphs and detecting shadow islands

use crate::core::errors::{Result, ValknutError};
use crate::live::{
    storage::{LiveStorage, AggregationQuery},
    graph::CallGraph,
    community::LouvainDetector,
    scoring::{LiveReachScorer, ScoringConfig},
    reports::LiveReachReporter,
    types::{EdgeKind, LiveReachReport},
    // stacks::{StackProcessor, StackConfig, Language, TimestampSource},
};
// Temporarily duplicate config types until module organization is fixed
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiveReachConfig {
    pub enabled: bool,
    pub services: Vec<String>,
    pub sample_rate: f64,
    pub weight_static: f64,
    pub window_days: u32,
    pub island: IslandConfig,
    pub ci: CiConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IslandConfig {
    pub min_size: usize,
    pub min_score: f64,
    pub resolution: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CiConfig {
    pub warn: bool,
    pub hard_fail: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageConfig {
    pub bucket: String,
    pub layout: String,
}

impl Default for LiveReachConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            services: vec!["api".to_string(), "worker".to_string()],
            sample_rate: 0.02,
            weight_static: 0.1,
            window_days: 30,
            island: IslandConfig {
                min_size: 5,
                min_score: 0.6,
                resolution: 0.8,
            },
            ci: CiConfig {
                warn: true,
                hard_fail: false,
            },
            storage: StorageConfig {
                bucket: "s3://company-valknut/live".to_string(),
                layout: "edges/date={date}/svc={svc}/ver={ver}/part-*.parquet".to_string(),
            },
        }
    }
}

impl LiveReachConfig {
    pub fn validate(&self) -> crate::core::errors::Result<()> {
        use crate::core::errors::ValknutError;
        
        if self.sample_rate < 0.0 || self.sample_rate > 1.0 {
            return Err(ValknutError::validation(
                "Sample rate must be between 0.0 and 1.0"
            ));
        }
        
        if self.weight_static < 0.0 {
            return Err(ValknutError::validation(
                "Static weight must be non-negative"
            ));
        }
        
        if self.window_days == 0 {
            return Err(ValknutError::validation(
                "Window days must be greater than 0"
            ));
        }
        
        if self.island.min_size == 0 {
            return Err(ValknutError::validation(
                "Minimum island size must be greater than 0"
            ));
        }
        
        if self.island.min_score < 0.0 || self.island.min_score > 1.0 {
            return Err(ValknutError::validation(
                "Island score threshold must be between 0.0 and 1.0"
            ));
        }
        
        if self.island.resolution <= 0.0 {
            return Err(ValknutError::validation(
                "Louvain resolution must be positive"
            ));
        }
        
        Ok(())
    }
}

use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc, Duration};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use tokio::fs;

/// Live reachability analysis commands
#[derive(Debug, Args)]
pub struct LiveReachArgs {
    #[command(subcommand)]
    pub command: LiveReachCommand,
}

/// Live reachability subcommands
#[derive(Debug, Subcommand)]
pub enum LiveReachCommand {
    /// Build call graph from aggregated data and detect shadow islands
    Build(BuildArgs),
    
    /// Ingest NDJSON events and create aggregated parquet files
    Ingest(IngestArgs),
    
    /// Ingest collapsed stack files from profilers
    IngestStacks(IngestStacksArgs),
    
    /// Generate reports from existing analysis results
    Report(ReportArgs),
    
    /// Validate CI changes against shadow island rules
    CiCheck(CiCheckArgs),
    
    /// Show configuration and validate setup
    Config(ConfigArgs),
}

/// Build call graph and analyze communities
#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Storage path (S3 URL or local path)
    #[arg(long, default_value = "s3://company-valknut/live")]
    pub from: String,
    
    /// Analysis window in days
    #[arg(long, default_value = "30")]
    pub since: u32,
    
    /// Services to include (comma-separated)
    #[arg(long, default_value = "api,worker")]
    pub svc: String,
    
    /// Output directory for results
    #[arg(long, default_value = ".valknut/live")]
    pub out: PathBuf,
    
    /// Static weight coefficient (alpha parameter)
    #[arg(long, default_value = "0.1")]
    pub static_weight: f64,
    
    /// Louvain resolution parameter
    #[arg(long, default_value = "0.8")]
    pub resolution: f64,
    
    /// Minimum community size for shadow island detection
    #[arg(long, default_value = "5")]
    pub min_size: usize,
    
    /// Minimum shadow island score threshold
    #[arg(long, default_value = "0.6")]
    pub min_score: f64,
    
    /// Generate HTML report in addition to JSON
    #[arg(long)]
    pub html: bool,
    
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Ingest events and create aggregated data
#[derive(Debug, Args)]
pub struct IngestArgs {
    /// Input NDJSON file path
    pub input: PathBuf,
    
    /// Storage path for output
    #[arg(long, default_value = ".valknut/live/storage")]
    pub storage: String,
    
    /// Service name
    #[arg(long, default_value = "api")]
    pub service: String,
    
    /// Version/SHA for this data
    #[arg(long, default_value = "unknown")]
    pub version: String,
    
    /// Date for partitioning (YYYY-MM-DD, defaults to today)
    #[arg(long)]
    pub date: Option<String>,
    
    /// Validate events but don't store them
    #[arg(long)]
    pub dry_run: bool,
}

/// Ingest collapsed stack files from profilers
#[derive(Debug, Args)]
pub struct IngestStacksArgs {
    /// Service name
    #[arg(long, default_value = "api")]
    pub svc: String,
    
    /// Version/SHA identifier
    #[arg(long, default_value = "unknown")]
    pub ver: String,
    
    /// Language for symbol normalization (auto|jvm|py|go|node|native)
    #[arg(long, default_value = "auto")]
    pub lang: String,
    
    /// Namespace allow-list (comma-separated prefixes)
    #[arg(long, value_delimiter = ',')]
    pub ns_allow: Option<Vec<String>>,
    
    /// Input file glob pattern
    #[arg(long, default_value = "stacks/*.txt")]
    pub from: String,
    
    /// Output directory
    #[arg(long, default_value = ".valknut/live/out")]
    pub out: PathBuf,
    
    /// Upload URI (S3 or cloud storage - stub for now)
    #[arg(long)]
    pub upload: Option<String>,
    
    /// Fail if no edges are extracted
    #[arg(long)]
    pub fail_if_empty: bool,
    
    /// Dry run - don't write output
    #[arg(long)]
    pub dry_run: bool,
    
    /// Timestamp source (filemtime|now|RFC3339)
    #[arg(long, default_value = "filemtime")]
    pub ts_source: String,
    
    /// Prefix to strip from symbols
    #[arg(long)]
    pub strip_prefix: Option<String>,
    
    /// Enable deduplication of identical edges
    #[arg(long)]
    pub dedupe: bool,
}

/// Generate reports from analysis results
#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Analysis results directory
    pub input: PathBuf,
    
    /// Output directory for reports
    #[arg(long, default_value = "./reports")]
    pub out: PathBuf,
    
    /// Report format
    #[arg(long, default_value = "html")]
    pub format: String,
    
    /// Service name for filtering
    #[arg(long)]
    pub service: Option<String>,
    
    /// Include detailed node information
    #[arg(long)]
    pub detailed: bool,
}

/// CI integration - check for shadow island violations
#[derive(Debug, Args)]
pub struct CiCheckArgs {
    /// Analysis results directory
    pub results_dir: PathBuf,
    
    /// Git diff or changed files (newline-separated)
    #[arg(long)]
    pub changed_files: Option<PathBuf>,
    
    /// Git diff command to run
    #[arg(long)]
    pub git_diff: Option<String>,
    
    /// Minimum island size to warn about
    #[arg(long, default_value = "5")]
    pub warn_size: usize,
    
    /// Minimum island score to warn about
    #[arg(long, default_value = "0.6")]
    pub warn_score: f64,
    
    /// Exit with error code on warnings
    #[arg(long)]
    pub fail_on_warnings: bool,
}

/// Configuration management
#[derive(Debug, Args)]
pub struct ConfigArgs {
    /// Show current configuration
    #[arg(long)]
    pub show: bool,
    
    /// Validate configuration file
    #[arg(long)]
    pub validate: Option<PathBuf>,
    
    /// Generate default configuration file
    #[arg(long)]
    pub generate: Option<PathBuf>,
}

/// CLI command executor for live reachability
pub struct LiveReachCli {
    config: LiveReachConfig,
}

impl LiveReachCli {
    /// Create a new CLI executor with configuration
    pub fn new(config: LiveReachConfig) -> Self {
        Self { config }
    }
    
    /// Execute a live reachability command
    pub async fn execute(&self, args: LiveReachArgs) -> Result<()> {
        match args.command {
            LiveReachCommand::Build(args) => self.build_command(args).await,
            LiveReachCommand::Ingest(args) => self.ingest_command(args).await,
            LiveReachCommand::IngestStacks(args) => self.ingest_stacks_command(args).await,
            LiveReachCommand::Report(args) => self.report_command(args).await,
            LiveReachCommand::CiCheck(args) => self.ci_check_command(args).await,
            LiveReachCommand::Config(args) => self.config_command(args).await,
        }
    }
    
    /// Build call graph and analyze communities
    async fn build_command(&self, args: BuildArgs) -> Result<()> {
        if args.verbose {
            println!("🔍 Starting live reachability analysis");
            println!("   Storage: {}", args.from);
            println!("   Window: {} days", args.since);
            println!("   Services: {}", args.svc);
            println!("   Output: {}", args.out.display());
        }
        
        // Parse services
        let services: Vec<String> = args.svc.split(',')
            .map(|s| s.trim().to_string())
            .collect();
        
        // Create storage backend
        let storage = LiveStorage::new(&args.from)?;
        
        // Define analysis window
        let end_time = Utc::now();
        let start_time = end_time - Duration::days(args.since as i64);
        
        // Query aggregated data
        if args.verbose {
            println!("📊 Querying aggregated data from {} to {}", 
                start_time.format("%Y-%m-%d"), end_time.format("%Y-%m-%d"));
        }
        
        let query = AggregationQuery {
            services: services.clone(),
            start_date: start_time,
            end_date: end_time,
            versions: Vec::new(), // Include all versions
            edge_kinds: vec![EdgeKind::Runtime, EdgeKind::Static],
        };
        
        let edges = storage.query_aggregated(&query).await?;
        
        if edges.is_empty() {
            eprintln!("⚠️ No aggregated data found for the specified criteria");
            return Ok(());
        }
        
        if args.verbose {
            println!("📈 Found {} aggregated edges", edges.len());
        }
        
        // Build call graph
        let graph = CallGraph::from_aggregated_edges(&edges, start_time, end_time, args.static_weight)?;
        let graph_stats = graph.get_stats();
        
        if args.verbose {
            println!("🕸️ Built call graph: {} nodes, {} edges", 
                graph_stats.total_nodes, graph_stats.total_edges);
        }
        
        // Create undirected projection for community detection
        let undirected = graph.create_undirected_projection(args.static_weight);
        
        // Detect communities using Louvain algorithm
        let detector = LouvainDetector::new(args.resolution, 100, 1e-6);
        let detection = detector.detect_communities(&undirected)?;
        
        if args.verbose {
            println!("🏘️ Detected {} communities (modularity: {:.4})", 
                detection.communities.len(), detection.modularity);
        }
        
        // Calculate live reach scores
        let scoring_config = ScoringConfig::default();
        let scorer = LiveReachScorer::new(scoring_config)?;
        let live_reach_scores = scorer.calculate_live_reach_scores(&graph, end_time)?;
        
        // Calculate shadow island scores
        let shadow_scores = scorer.calculate_shadow_island_scores(
            &detection, 
            &live_reach_scores, 
            &graph
        )?;
        
        // Filter communities by thresholds
        let shadow_islands: Vec<_> = shadow_scores.iter()
            .filter(|(community_id, &score)| {
                if let Some(info) = detection.get_community_info(**community_id) {
                    info.size() >= args.min_size && score >= args.min_score
                } else {
                    false
                }
            })
            .collect();
        
        if args.verbose {
            println!("🏝️ Found {} shadow islands above thresholds", shadow_islands.len());
        }
        
        // Create output directory
        fs::create_dir_all(&args.out).await
            .map_err(|e| ValknutError::io("Failed to create output directory", e))?;
        
        // Generate and write reports
        let reporter = LiveReachReporter::new();
        let report = reporter.generate_report(
            &graph,
            &detection,
            &live_reach_scores,
            &shadow_scores,
            &services[0], // Primary service
            (start_time, end_time),
        )?;
        
        // Write JSON report
        let json_path = args.out.join("report.json");
        let json_content = serde_json::to_string_pretty(&report)
            .map_err(|e| ValknutError::io("Failed to serialize report", e.into()))?;
        
        fs::write(&json_path, json_content).await
            .map_err(|e| ValknutError::io("Failed to write JSON report", e))?;
        
        if args.verbose {
            println!("💾 Wrote JSON report to {}", json_path.display());
        }
        
        // Write HTML report if requested
        if args.html {
            let html_path = args.out.join("report.html");
            let html_content = reporter.generate_html_report(&report)?;
            
            fs::write(&html_path, html_content).await
                .map_err(|e| ValknutError::io("Failed to write HTML report", e))?;
                
            if args.verbose {
                println!("💾 Wrote HTML report to {}", html_path.display());
            }
        }
        
        // Print summary
        println!("✅ Analysis complete:");
        println!("   Nodes: {}", graph_stats.total_nodes);
        println!("   Edges: {} ({}% runtime)", 
            graph_stats.total_edges,
            if graph_stats.total_edges > 0 {
                (graph_stats.runtime_edges as f64 / graph_stats.total_edges as f64 * 100.0) as u32
            } else { 0 }
        );
        println!("   Communities: {}", detection.communities.len());
        println!("   Shadow Islands: {} (score >= {:.2}, size >= {})", 
            shadow_islands.len(), args.min_score, args.min_size);
        
        Ok(())
    }
    
    /// Ingest NDJSON events into aggregated storage
    async fn ingest_command(&self, args: IngestArgs) -> Result<()> {
        println!("📥 Ingesting events from {}", args.input.display());
        
        // Create storage backend
        let storage = LiveStorage::new(&args.storage)?;
        
        // Ingest events from file
        let bucket = storage.ingest_events(&args.input).await?;
        
        println!("📊 Processed {} unique edges", bucket.len());
        
        if args.dry_run {
            println!("🔍 Dry run - no data written to storage");
            return Ok(());
        }
        
        // Parse date or use today
        let date = if let Some(date_str) = &args.date {
            DateTime::parse_from_str(&format!("{}T00:00:00Z", date_str), "%Y-%m-%dT%H:%M:%SZ")
                .map_err(|e| ValknutError::validation(format!("Invalid date format: {}", e)))?
                .with_timezone(&Utc)
        } else {
            Utc::now()
        };
        
        // Write aggregated data
        storage.write_aggregation(&bucket, &args.service, &args.version, date).await?;
        
        println!("✅ Successfully stored aggregated data");
        Ok(())
    }
    
    /// Ingest collapsed stack files from profilers
    async fn ingest_stacks_command(&self, args: IngestStacksArgs) -> Result<()> {
        println!("📥 Ingesting stack traces (placeholder implementation)");
        println!("   Service: {}", args.svc);
        println!("   Version: {}", args.ver);
        println!("   Language: {}", args.lang);
        println!("   Pattern: {}", args.from);
        println!("   Output: {}", args.out.display());
        
        println!("📊 Processing complete (placeholder):");
        println!("   Files processed: 0");
        println!("   Stack samples: 0");
        println!("   Edges before filtering: 0");
        println!("   Edges after filtering: 0");
        println!("   Final aggregated edges: 0");
        
        println!("⚠️ Warnings:");
        println!("   Stack processor integration is not yet complete");
        println!("   This command provides a placeholder implementation");
        
        if args.dry_run {
            println!("🔍 Dry run - no data written");
        } else {
            println!("🚧 Stack profiler integration framework created, full implementation pending");
        }
        
        if let Some(upload_uri) = args.upload {
            println!("📤 Upload URI (not implemented yet): {}", upload_uri);
        }
        
        Ok(())
    }
    
    /// Generate reports from existing analysis
    async fn report_command(&self, args: ReportArgs) -> Result<()> {
        println!("📊 Generating reports from {}", args.input.display());
        
        // Load analysis results
        let report_path = args.input.join("report.json");
        let content = fs::read_to_string(&report_path).await
            .map_err(|e| ValknutError::io("Failed to read analysis results", e))?;
        
        let report: LiveReachReport = serde_json::from_str(&content)
            .map_err(|e| ValknutError::io("Failed to parse analysis results", e.into()))?;
        
        // Create output directory
        fs::create_dir_all(&args.out).await
            .map_err(|e| ValknutError::io("Failed to create output directory", e))?;
        
        let reporter = LiveReachReporter::new();
        
        match args.format.as_str() {
            "json" => {
                let json_path = args.out.join("formatted_report.json");
                let json_content = serde_json::to_string_pretty(&report)
                    .map_err(|e| ValknutError::io("Failed to serialize report", e.into()))?;
                fs::write(&json_path, json_content).await
                    .map_err(|e| ValknutError::io("Failed to write JSON report", e))?;
                println!("💾 Wrote JSON report to {}", json_path.display());
            },
            "html" => {
                let html_path = args.out.join("report.html");
                let html_content = reporter.generate_html_report(&report)?;
                fs::write(&html_path, html_content).await
                    .map_err(|e| ValknutError::io("Failed to write HTML report", e))?;
                println!("💾 Wrote HTML report to {}", html_path.display());
            },
            "markdown" | "md" => {
                let md_path = args.out.join("report.md");
                let md_content = reporter.generate_markdown_report(&report)?;
                fs::write(&md_path, md_content).await
                    .map_err(|e| ValknutError::io("Failed to write Markdown report", e))?;
                println!("💾 Wrote Markdown report to {}", md_path.display());
            },
            _ => {
                return Err(ValknutError::validation(
                    format!("Unsupported report format: {}", args.format)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Check CI changes against shadow island rules
    async fn ci_check_command(&self, args: CiCheckArgs) -> Result<()> {
        println!("🔍 Checking CI changes for shadow island violations");
        
        // Load analysis results
        let report_path = args.results_dir.join("report.json");
        let content = fs::read_to_string(&report_path).await
            .map_err(|e| ValknutError::io("Failed to read analysis results", e))?;
        
        let report: LiveReachReport = serde_json::from_str(&content)
            .map_err(|e| ValknutError::io("Failed to parse analysis results", e.into()))?;
        
        // Get changed files
        let changed_files = if let Some(diff_cmd) = &args.git_diff {
            self.get_changed_files_from_git(diff_cmd).await?
        } else if let Some(file_path) = &args.changed_files {
            self.get_changed_files_from_file(file_path).await?
        } else {
            Vec::new()
        };
        
        if changed_files.is_empty() {
            println!("ℹ️ No changed files to analyze");
            return Ok(());
        }
        
        println!("📝 Analyzing {} changed files", changed_files.len());
        
        // Check for violations
        let mut warnings = Vec::new();
        
        for community in &report.communities {
            if community.size >= args.warn_size && community.score >= args.warn_score {
                // Check if any changed files affect this community
                for node in &community.nodes {
                    // Extract file path from symbol ID (heuristic)
                    if let Some(file_path) = self.extract_file_path(&node.id) {
                        if changed_files.iter().any(|cf| cf.contains(&file_path) || file_path.contains(cf)) {
                            warnings.push(format!(
                                "Shadow island violation: {} (score {:.2}, size {}) affects changed code in {}",
                                community.id, community.score, community.size, file_path
                            ));
                        }
                    }
                }
            }
        }
        
        // Report violations
        if warnings.is_empty() {
            println!("✅ No shadow island violations detected");
        } else {
            println!("⚠️ Found {} shadow island violations:", warnings.len());
            for warning in &warnings {
                println!("   {}", warning);
            }
            
            if args.fail_on_warnings {
                eprintln!("❌ CI check failed due to shadow island violations");
                std::process::exit(1);
            }
        }
        
        Ok(())
    }
    
    /// Configuration management
    async fn config_command(&self, args: ConfigArgs) -> Result<()> {
        if args.show {
            println!("Live Reachability Configuration:");
            println!("{}", serde_yaml::to_string(&self.config)
                .map_err(|e| ValknutError::validation(format!("Failed to serialize config: {}", e)))?);
        }
        
        if let Some(config_path) = args.validate {
            println!("🔍 Validating configuration: {}", config_path.display());
            
            let content = fs::read_to_string(&config_path).await
                .map_err(|e| ValknutError::io("Failed to read config file", e))?;
            
            let config: LiveReachConfig = serde_yaml::from_str(&content)
                .map_err(|e| ValknutError::validation(format!("Failed to parse config file: {}", e)))?;
            
            config.validate()?;
            println!("✅ Configuration is valid");
        }
        
        if let Some(output_path) = args.generate {
            println!("📝 Generating default configuration: {}", output_path.display());
            
            let default_config = LiveReachConfig::default();
            let yaml_content = serde_yaml::to_string(&default_config)
                .map_err(|e| ValknutError::validation(format!("Failed to serialize default config: {}", e)))?;
            
            fs::write(&output_path, yaml_content).await
                .map_err(|e| ValknutError::io("Failed to write config file", e))?;
            
            println!("✅ Default configuration written");
        }
        
        Ok(())
    }
    
    /// Extract changed files from git diff command
    async fn get_changed_files_from_git(&self, git_cmd: &str) -> Result<Vec<String>> {
        use tokio::process::Command;
        
        let output = Command::new("sh")
            .arg("-c")
            .arg(git_cmd)
            .output()
            .await
            .map_err(|e| ValknutError::io("Failed to run git command", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ValknutError::io("Git command failed", 
                std::io::Error::new(std::io::ErrorKind::Other, stderr.to_string())));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|line| line.trim().to_string()).collect())
    }
    
    /// Extract changed files from a file
    async fn get_changed_files_from_file(&self, file_path: &Path) -> Result<Vec<String>> {
        let content = fs::read_to_string(file_path).await
            .map_err(|e| ValknutError::io("Failed to read changed files", e))?;
        
        Ok(content.lines().map(|line| line.trim().to_string()).collect())
    }
    
    /// Extract file path from symbol ID (heuristic)
    fn extract_file_path(&self, symbol_id: &str) -> Option<String> {
        // Parse symbol format: "{lang}:{svc}:{fq_name}"
        let parts: Vec<&str> = symbol_id.splitn(3, ':').collect();
        if parts.len() == 3 {
            let fq_name = parts[2];
            
            // Convert module-style names to file paths
            match parts[0] {
                "python" | "py" => {
                    // python:api:myapp.views:list_users -> myapp/views.py
                    let module_parts: Vec<&str> = fq_name.split(':').next()?.split('.').collect();
                    Some(format!("{}.py", module_parts.join("/")))
                },
                "javascript" | "js" | "typescript" | "ts" => {
                    // js:api:src.controllers.user:createUser -> src/controllers/user.js
                    let module_parts: Vec<&str> = fq_name.split(':').next()?.split('.').collect();
                    Some(format!("{}.js", module_parts.join("/")))
                },
                "rust" => {
                    // rust:api:myapp::controllers::user::create -> src/controllers/user.rs
                    let module_parts: Vec<&str> = fq_name.split("::").collect();
                    if module_parts.len() > 1 {
                        Some(format!("src/{}.rs", module_parts[..module_parts.len()-1].join("/")))
                    } else {
                        Some("src/main.rs".to_string())
                    }
                },
                _ => None,
            }
        } else {
            None
        }
    }
}

/// Helper function to print CLI help and examples
pub fn print_live_reach_help() {
    println!(r#"
Live Reachability Analysis Commands

EXAMPLES:

  # Build call graph from last 30 days of data
  valknut live-reach build --from s3://bucket/live --since 30 --svc api,worker --out .valknut/live

  # Ingest NDJSON events into storage  
  valknut live-reach ingest events.ndjson --storage ./storage --service api --version v1.2.3

  # Ingest collapsed stack traces from profilers
  valknut live-reach ingest-stacks --svc api --ver v1.2.3 --lang auto --ns-allow myco.,internal. --from "stacks/*.txt" --out .valknut/live/out

  # Generate HTML report from analysis results
  valknut live-reach report .valknut/live --format html --out ./reports

  # Check CI changes for shadow island violations
  valknut live-reach ci-check .valknut/live --git-diff "git diff --name-only HEAD~1" --fail-on-warnings

  # Show current configuration
  valknut live-reach config --show

  # Generate default config file
  valknut live-reach config --generate .valknut-live.yml

CONFIGURATION:
  
  Live reachability can be configured via YAML file or environment variables:
  
  live_reach:
    enabled: true
    sample_rate: 0.02
    services: ["api", "worker"]
    storage:
      bucket: "s3://company-valknut/live"
      
  Environment variables:
    VALKNUT_LIVE=1                    # Enable collection
    VALKNUT_LIVE_SAMPLE=0.02         # 2% sampling rate  
    VALKNUT_LIVE_MAX_EDGES=200       # Max edges per request
    VALKNUT_SERVICE=api              # Service name
    VALKNUT_VERSION=v1.2.3           # Deployment version

"#);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_file_path_extraction() {
        let cli = LiveReachCli::new(LiveReachConfig::default());
        
        // Python module
        let py_path = cli.extract_file_path("python:api:myapp.views:list_users");
        assert_eq!(py_path, Some("myapp/views.py".to_string()));
        
        // JavaScript module
        let js_path = cli.extract_file_path("js:api:src.controllers.user:createUser");
        assert_eq!(js_path, Some("src/controllers/user.js".to_string()));
        
        // Rust module
        let rs_path = cli.extract_file_path("rust:api:myapp::controllers::user::create");
        assert_eq!(rs_path, Some("src/myapp/controllers/user.rs".to_string()));
        
        // Invalid format
        let invalid_path = cli.extract_file_path("invalid-symbol");
        assert_eq!(invalid_path, None);
    }
    
    #[tokio::test]
    async fn test_config_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.yml");
        
        let cli = LiveReachCli::new(LiveReachConfig::default());
        let args = ConfigArgs {
            show: false,
            validate: None,
            generate: Some(config_path.clone()),
        };
        
        let result = cli.config_command(args).await;
        assert!(result.is_ok());
        assert!(config_path.exists());
        
        // Validate the generated config
        let content = fs::read_to_string(&config_path).await.unwrap();
        let config: LiveReachConfig = serde_yaml::from_str(&content).unwrap();
        assert!(config.validate().is_ok());
    }
}