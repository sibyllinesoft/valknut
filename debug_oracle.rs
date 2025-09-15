// Quick debugging script to check oracle response

use std::env;
use tokio;
use valknut_rs::oracle::{RefactoringOracle, OracleConfig};
use valknut_rs::api::results::AnalysisResults;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a minimal mock analysis result
    let mock_analysis = AnalysisResults {
        summary: valknut_rs::api::results::AnalysisSummary {
            files_processed: 1,
            entities_analyzed: 5,
            refactoring_needed: 2,
            high_priority: 1,
            critical: 0,
            avg_refactoring_score: 5.0,
            code_health_score: 0.75,
        },
        refactoring_candidates: vec![],
        refactoring_candidates_by_file: vec![],
        statistics: valknut_rs::api::results::AnalysisStatistics {
            total_duration: std::time::Duration::from_secs(1),
            avg_file_processing_time: std::time::Duration::from_millis(100),
            avg_entity_processing_time: std::time::Duration::from_millis(20),
            features_per_entity: std::collections::HashMap::new(),
            priority_distribution: std::collections::HashMap::new(),
            issue_distribution: std::collections::HashMap::new(),
            memory_stats: valknut_rs::api::results::MemoryStats {
                peak_memory_bytes: 1000,
                final_memory_bytes: 500,
                efficiency_score: 0.9,
            },
        },
        directory_health_tree: None,
        clone_analysis: None,
        coverage_packs: vec![],
        warnings: vec![],
    };

    // Check if we have an API key
    if let Ok(api_key) = env::var("GEMINI_API_KEY") {
        println!("API key found, testing oracle...");
        
        let config = OracleConfig::from_env()?;
        let oracle = RefactoringOracle::new(config);
        
        let response = oracle.analyze(&std::path::Path::new("."), &mock_analysis).await?;
        
        // Write response to file
        let json_response = serde_json::to_string_pretty(&response)?;
        tokio::fs::write("oracle_debug_response.json", json_response).await?;
        
        println!("Oracle response written to oracle_debug_response.json");
        println!("Assessment: {:?}", response.assessment);
    } else {
        println!("No GEMINI_API_KEY found");
    }
    
    Ok(())
}