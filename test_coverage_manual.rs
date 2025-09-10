use std::path::PathBuf;
use valknut_rs::detectors::coverage::{CoverageExtractor, CoverageConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing Coverage Packs module manually");
    
    let config = CoverageConfig {
        enabled: true,
        report_paths: vec![PathBuf::from("coverage.lcov")],
        max_gaps_per_file: 5,
        min_gap_loc: 3,
        snippet_context_lines: 3,
        long_gap_head_tail: 5,
        group_cross_file: false,
        target_repo_gain: 0.10,
        weights: Default::default(),
        exclude_patterns: vec!["*/tests/*".to_string(), "*/test/*".to_string()],
    };
    
    println!("📊 Configuration: {:?}", config);
    
    let mut extractor = CoverageExtractor::new(config);
    let coverage_reports = vec![PathBuf::from("coverage.lcov")];
    
    println!("🔍 Building coverage packs from: {:?}", coverage_reports);
    
    let packs = extractor.build_coverage_packs(coverage_reports).await?;
    
    println!("✅ Generated {} coverage packs", packs.len());
    
    for (i, pack) in packs.iter().enumerate().take(3) {
        println!("\n📦 Coverage Pack #{}: {}", i + 1, pack.pack_id);
        println!("   📁 File: {:?}", pack.path);
        println!("   🎯 Gaps: {} gaps", pack.gaps.len());
        println!("   💾 File LOC: {}", pack.file_info.loc);
        println!("   📈 Coverage gain: {:.2}%", pack.value.file_cov_gain * 100.0);
        
        for (j, gap) in pack.gaps.iter().enumerate().take(2) {
            println!("   🔸 Gap #{}: lines {}-{} (score: {:.3})", 
                j + 1, gap.span.start, gap.span.end, gap.score);
            println!("      Language: {}, Gap LOC: {}", gap.language, gap.features.gap_loc);
            
            // Print snippet preview
            if !gap.preview.head.is_empty() {
                println!("      Preview (head):");
                for (line_idx, line) in gap.preview.head.iter().enumerate().take(2) {
                    let line_num = gap.span.start + line_idx;
                    println!("        {}: {}", line_num, line);
                }
            }
        }
    }
    
    println!("\n✅ Coverage Packs module is working correctly!");
    println!("🎉 Test completed successfully");
    
    Ok(())
}
