//! Integration tests for comprehensive clone detection system

use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio;

use valknut_rs::core::config::{AdaptiveDenoiseConfig, DedupeConfig, ValknutConfig};
use valknut_rs::core::featureset::{CodeEntity, ExtractionContext, FeatureExtractor};
use valknut_rs::detectors::clone_detection::{
    ComprehensiveCloneDetector, NormalizationConfig, PdgMotifAnalyzer, TfIdfAnalyzer,
    WeightedMinHash,
};
// use valknut_rs::detectors::boilerplate_learning::{
//     BoilerplateLearningSystem, BoilerplateLearningConfig
// };

/// Test TF-IDF analysis with language-agnostic normalization
#[tokio::test]
async fn test_tfidf_weighted_analysis() {
    let mut analyzer = TfIdfAnalyzer::new(NormalizationConfig::default());

    // Add documents with different rarity patterns
    analyzer.add_document(
        "common_doc".to_string(),
        vec![
            "println".to_string(),
            "debug".to_string(),
            "common".to_string(),
        ],
    );

    analyzer.add_document(
        "rare_doc".to_string(),
        vec![
            "complex_algorithm".to_string(),
            "unique_function".to_string(),
            "rare".to_string(),
        ],
    );

    analyzer.add_document(
        "mixed_doc".to_string(),
        vec![
            "println".to_string(),
            "complex_algorithm".to_string(),
            "mixed".to_string(),
        ],
    );

    // Test TF-IDF scoring - rare terms should have higher scores
    let common_tfidf = analyzer.tf_idf("common_doc", "println");
    let rare_tfidf = analyzer.tf_idf("rare_doc", "complex_algorithm");

    assert!(
        rare_tfidf > common_tfidf,
        "Rare terms should have higher TF-IDF scores: {} vs {}",
        rare_tfidf,
        common_tfidf
    );

    // Test vector generation
    let vector = analyzer.get_tfidf_vector("mixed_doc");
    assert!(!vector.is_empty());
    assert!(vector.contains_key("println"));
    assert!(vector.contains_key("complex_algorithm"));
}

/// Test PDG motif analysis with structural pattern detection
#[tokio::test]
async fn test_pdg_motif_analysis() {
    let mut analyzer = PdgMotifAnalyzer::new(3);

    let complex_code = r#"
        fn complex_function(x: i32) -> i32 {
            if x > 0 {
                for i in 0..x {
                    if i % 2 == 0 {
                        println!("Even: {}", i);
                    }
                }
            } else {
                while x < 0 {
                    x += 1;
                }
            }
            x
        }
    "#;

    let simple_code = r#"
        fn simple_function(x: i32) -> i32 {
            x + 1
        }
    "#;

    let complex_motifs = analyzer.extract_motifs(complex_code, "complex_entity");
    let simple_motifs = analyzer.extract_motifs(simple_code, "simple_entity");

    // Complex code should have more motifs
    assert!(
        complex_motifs.len() > simple_motifs.len(),
        "Complex code should have more motifs: {} vs {}",
        complex_motifs.len(),
        simple_motifs.len()
    );

    // Test rarity gain calculation
    let complex_rarity = analyzer.calculate_rarity_gain(&complex_motifs);
    let simple_rarity = analyzer.calculate_rarity_gain(&simple_motifs);

    assert!(complex_rarity > 0.0);
    assert!(simple_rarity > 0.0);
}

/// Test weighted MinHash with TF-IDF weighting
#[test]
fn test_weighted_minhash() {
    let mut weights = HashMap::new();
    weights.insert("rare_token".to_string(), 5.0);
    weights.insert("common_token".to_string(), 0.2);
    weights.insert("stop_motif".to_string(), 0.05); // Should be filtered

    let minhash = WeightedMinHash::new(64, weights);

    let tokens1 = vec![
        "rare_token".to_string(),
        "common_token".to_string(),
        "stop_motif".to_string(),
    ];

    let tokens2 = vec![
        "rare_token".to_string(),
        "different_common".to_string(),
        "stop_motif".to_string(),
    ];

    let sig1 = minhash.generate_signature(&tokens1);
    let sig2 = minhash.generate_signature(&tokens2);

    let similarity = sig1.jaccard_similarity(&sig2);

    // Should have some similarity due to shared rare token
    // Stop motifs should contribute minimally
    assert!(similarity >= 0.0 && similarity <= 1.0);
    assert_eq!(sig1.size, 64);
    assert_eq!(sig2.size, 64);
}

/// Test comprehensive clone detector integration
#[tokio::test]
async fn test_comprehensive_clone_detector() {
    let config = DedupeConfig {
        adaptive: AdaptiveDenoiseConfig {
            auto_denoise: true,
            rarity_weighting: true,
            structural_validation: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let detector = ComprehensiveCloneDetector::new(config);

    // Test with structural clone
    let entity1 = CodeEntity::new("entity1", "function", "complex_func", "test.rs")
        .with_source_code(
            r#"
        fn process_data(items: Vec<Item>) -> Vec<ProcessedItem> {
            let mut results = Vec::new();
            for item in items {
                if item.is_valid() {
                    let processed = transform_item(item);
                    results.push(processed);
                }
            }
            results
        }
    "#,
        );

    let entity2 = CodeEntity::new("entity2", "function", "similar_func", "test.rs")
        .with_source_code(
            r#"
        fn handle_records(records: Vec<Record>) -> Vec<ProcessedRecord> {
            let mut output = Vec::new();
            for record in records {
                if record.validate() {
                    let transformed = process_record(record);
                    output.push(transformed);
                }
            }
            output
        }
    "#,
        );

    let valknut_config = Arc::new(ValknutConfig::default());
    let mut context = ExtractionContext::new(valknut_config, "rust");
    context
        .entity_index
        .insert("entity1".to_string(), entity1.clone());
    context
        .entity_index
        .insert("entity2".to_string(), entity2.clone());

    // Extract features for first entity
    let features = detector.extract(&entity1, &context).await.unwrap();

    assert!(features.contains_key("saved_tokens_score"));
    assert!(features.contains_key("rarity_gain"));
    assert!(features.contains_key("structural_evidence"));
    assert!(features.contains_key("live_reach_boost"));
    assert!(features.contains_key("final_clone_score"));

    // Structural evidence should be detected
    let structural_evidence = features.get("structural_evidence").unwrap();
    assert!(
        *structural_evidence > 0.0,
        "Should detect structural similarity"
    );

    // Final score should be calculated
    let final_score = features.get("final_clone_score").unwrap();
    assert!(
        *final_score > 0.0,
        "Should calculate meaningful clone score"
    );
}

// Test boilerplate learning system with adaptive patterns
// #[tokio::test]
// async fn test_boilerplate_learning_system() {
//     let temp_dir = TempDir::new().unwrap();
//     let temp_path = temp_dir.path();
//
//     // Create test files with common and rare patterns
//     let test_files = [
//         ("common1.rs", r#"
//             fn test1() {
//                 println!("Debug message");
//                 log::info!("Processing");
//                 assert_eq!(x, y);
//             }
//         "#),
//         ("common2.rs", r#"
//             fn test2() {
//                 println!("Another debug");
//                 log::info!("Still processing");
//                 assert_ne!(a, b);
//             }
//         "#),
//         ("unique.rs", r#"
//             fn unique_algorithm() {
//                 let result = complex_calculation(x, y, z);
//                 advanced_processing(result);
//             }
//         "#),
//     ];
//
//     for (filename, content) in &test_files {
//         let file_path = temp_path.join(filename);
//         tokio::fs::write(&file_path, content).await.unwrap();
//     }
//
//     // Test boilerplate learning
//     let config = BoilerplateLearningConfig::default();
//     let mut learning_system = BoilerplateLearningSystem::new(config);
//
//     let report = learning_system.learn_from_codebase(temp_path).await.unwrap();
//
//     assert!(report.shingles_analyzed > 0);
//     assert!(report.learning_duration.num_milliseconds() >= 0);
//
//     // Test pattern weighting
//     let debug_weight = learning_system.get_shingle_weight("println!");
//     let unique_weight = learning_system.get_shingle_weight("complex_calculation");
//
//     // Common patterns should be down-weighted
//     assert!(unique_weight >= debug_weight,
//         "Unique patterns should have higher weights: {} vs {}", unique_weight, debug_weight);
//
//     // Test hub pattern detection
//     assert!(learning_system.is_hub_pattern("log.info(\"test\");"));
//     assert!(!learning_system.is_hub_pattern("complex_calculation(x, y);"));
// }

/// Test adaptive ranking system with SavedTokens, RarityGain, and LiveReachBoost
#[tokio::test]
async fn test_adaptive_ranking_system() {
    let config = DedupeConfig {
        adaptive: AdaptiveDenoiseConfig {
            min_rarity_gain: 1.2,
            ..Default::default()
        },
        min_saved_tokens: 50,
        ..Default::default()
    };

    let detector = ComprehensiveCloneDetector::new(config);

    // Create entities with different characteristics
    let high_value_entity = CodeEntity::new(
        "high_value",
        "function",
        "complex_business_logic",
        "business.rs"
    ).with_source_code(r#"
        fn calculate_risk_assessment(portfolio: &Portfolio, market_data: &MarketData) -> RiskReport {
            let mut risk_factors = Vec::new();
            
            for position in &portfolio.positions {
                let volatility = market_data.get_volatility(&position.symbol);
                let correlation = market_data.get_correlation_matrix();
                
                if volatility > RISK_THRESHOLD {
                    let risk_factor = RiskFactor {
                        symbol: position.symbol.clone(),
                        exposure: position.value * volatility,
                        correlation_risk: calculate_correlation_risk(position, correlation),
                    };
                    risk_factors.push(risk_factor);
                }
            }
            
            RiskReport::new(risk_factors)
        }
    "#);

    let low_value_entity = CodeEntity::new("low_value", "function", "simple_getter", "utils.rs")
        .with_source_code(
            r#"
        fn get_name(&self) -> &str {
            &self.name
        }
    "#,
        );

    let valknut_config = Arc::new(ValknutConfig::default());
    let mut context = ExtractionContext::new(valknut_config, "rust");
    context
        .entity_index
        .insert("high_value".to_string(), high_value_entity.clone());
    context
        .entity_index
        .insert("low_value".to_string(), low_value_entity.clone());

    // Test ranking - high-value entity should score higher
    let high_features = detector
        .extract(&high_value_entity, &context)
        .await
        .unwrap();
    let low_features = detector.extract(&low_value_entity, &context).await.unwrap();

    let high_score = high_features.get("final_clone_score").unwrap();
    let low_score = low_features.get("final_clone_score").unwrap();

    // Complex business logic should have higher potential value
    assert!(
        *high_score >= *low_score,
        "Complex code should have higher clone detection score: {} vs {}",
        high_score,
        low_score
    );

    // Test rarity gain
    let high_rarity = high_features.get("rarity_gain").unwrap();
    let low_rarity = low_features.get("rarity_gain").unwrap();

    assert!(
        *high_rarity >= *low_rarity,
        "Complex code should have higher rarity gain: {} vs {}",
        high_rarity,
        low_rarity
    );
}

/// Test structural evidence validation requirements
#[tokio::test]
async fn test_structural_evidence_requirements() {
    let config = DedupeConfig {
        require_distinct_blocks: 2,
        adaptive: AdaptiveDenoiseConfig {
            structural_validation: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let detector = ComprehensiveCloneDetector::new(config);

    // Entity with insufficient structure (should be filtered)
    let simple_entity = CodeEntity::new("simple", "function", "simple", "test.rs")
        .with_source_code("fn simple() { println!(\"hello\"); }");

    // Entity with sufficient structure (should pass)
    let complex_entity = CodeEntity::new("complex", "function", "complex", "test.rs")
        .with_source_code(
            r#"
        fn complex(x: i32) -> i32 {
            if x > 0 {
                for i in 0..x {
                    if i % 2 == 0 {
                        process_even(i);
                    } else {
                        process_odd(i);
                    }
                }
            }
            x
        }
    "#,
        );

    let valknut_config = Arc::new(ValknutConfig::default());
    let context = ExtractionContext::new(valknut_config, "rust");

    let simple_features = detector.extract(&simple_entity, &context).await.unwrap();
    let complex_features = detector.extract(&complex_entity, &context).await.unwrap();

    let simple_evidence = simple_features.get("structural_evidence").unwrap();
    let complex_evidence = complex_features.get("structural_evidence").unwrap();

    // Complex entity should have higher structural evidence
    assert!(
        *complex_evidence > *simple_evidence,
        "Complex code should have more structural evidence: {} vs {}",
        complex_evidence,
        simple_evidence
    );
}

// Integration test for end-to-end adaptive clone detection
// #[tokio::test]
// async fn test_end_to_end_adaptive_detection() {
//     let temp_dir = TempDir::new().unwrap();
//     let temp_path = temp_dir.path();
//
//     // Set up comprehensive configuration
//     let config = DedupeConfig {
//         min_saved_tokens: 100,
//         require_distinct_blocks: 2,
//         adaptive: AdaptiveDenoiseConfig {
//             auto_denoise: true,
//             adaptive_learning: true,
//             rarity_weighting: true,
//             structural_validation: true,
//             live_reach_integration: true,
//             min_rarity_gain: 1.2,
//             quality_gate_percentage: 0.8,
//             ..Default::default()
//         },
//         ..Default::default()
//     };
//
//     // Initialize both systems
//     let clone_detector = ComprehensiveCloneDetector::new(config.clone());
//     let learning_config = BoilerplateLearningConfig::default();
//     let mut learning_system = BoilerplateLearningSystem::new(learning_config);
//
//     // Create test codebase with various patterns
//     let test_files = [
//         ("business_logic.rs", r#"
//             fn calculate_portfolio_risk(portfolio: Portfolio) -> RiskMetrics {
//                 let mut total_risk = 0.0;
//                 for position in portfolio.positions {
//                     let asset_risk = calculate_asset_risk(&position);
//                     let correlation_adjustment = get_correlation_factor(&position);
//                     total_risk += asset_risk * correlation_adjustment;
//                 }
//                 RiskMetrics::new(total_risk)
//             }
//         "#),
//         ("similar_logic.rs", r#"
//             fn compute_investment_risk(investment: Investment) -> RiskAnalysis {
//                 let mut risk_score = 0.0;
//                 for holding in investment.holdings {
//                     let holding_risk = assess_holding_risk(&holding);
//                     let correlation_factor = derive_correlation_multiplier(&holding);
//                     risk_score += holding_risk * correlation_factor;
//                 }
//                 RiskAnalysis::from_score(risk_score)
//             }
//         "#),
//         ("boilerplate.rs", r#"
//             fn test_function() {
//                 log::info!("Starting test");
//                 println!("Debug output");
//                 assert_eq!(1, 1);
//                 log::info!("Test complete");
//             }
//         "#),
//     ];
//
//     for (filename, content) in &test_files {
//         let file_path = temp_path.join(filename);
//         tokio::fs::write(&file_path, content).await.unwrap();
//     }
//
//     // Step 1: Learn boilerplate patterns
//     let learning_report = learning_system.learn_from_codebase(temp_path).await.unwrap();
//     assert!(learning_report.stop_shingles_identified > 0);
//
//     // Step 2: Detect clones with adaptive system
//     let business_entity = CodeEntity::new(
//         "business_logic",
//         "function",
//         "calculate_portfolio_risk",
//         "business_logic.rs"
//     ).with_source_code(test_files[0].1);
//
//     let similar_entity = CodeEntity::new(
//         "similar_logic",
//         "function",
//         "compute_investment_risk",
//         "similar_logic.rs"
//     ).with_source_code(test_files[1].1);
//
//     let boilerplate_entity = CodeEntity::new(
//         "boilerplate",
//         "function",
//         "test_function",
//         "boilerplate.rs"
//     ).with_source_code(test_files[2].1);
//
//     let valknut_config = Arc::new(ValknutConfig::default());
//     let mut context = ExtractionContext::new(valknut_config, "rust");
//     context.entity_index.insert("business_logic".to_string(), business_entity.clone());
//     context.entity_index.insert("similar_logic".to_string(), similar_entity.clone());
//     context.entity_index.insert("boilerplate".to_string(), boilerplate_entity.clone());
//
//     // Analyze each entity
//     let business_features = clone_detector.extract(&business_entity, &context).await.unwrap();
//     let similar_features = clone_detector.extract(&similar_entity, &context).await.unwrap();
//     let boilerplate_features = clone_detector.extract(&boilerplate_entity, &context).await.unwrap();
//
//     // Verify adaptive system behavior
//     let business_score = business_features.get("final_clone_score").unwrap();
//     let similar_score = similar_features.get("final_clone_score").unwrap();
//     let boilerplate_score = boilerplate_features.get("final_clone_score").unwrap();
//
//     // Business logic should be prioritized over boilerplate
//     assert!(*business_score >= *boilerplate_score,
//         "Business logic should score higher than boilerplate: {} vs {}",
//         business_score, boilerplate_score);
//
//     assert!(*similar_score >= *boilerplate_score,
//         "Similar business logic should score higher than boilerplate: {} vs {}",
//         similar_score, boilerplate_score);
//
//     // Verify structural evidence
//     let business_evidence = business_features.get("structural_evidence").unwrap();
//     let similar_evidence = similar_features.get("similar_evidence").unwrap();
//     let boilerplate_evidence = boilerplate_features.get("boilerplate_evidence").unwrap();
//
//     assert!(*business_evidence > *boilerplate_evidence,
//         "Complex business logic should have more structural evidence than simple boilerplate");
//
//     // Verify rarity gain is calculated
//     let business_rarity = business_features.get("rarity_gain").unwrap();
//     let similar_rarity = similar_features.get("rarity_gain").unwrap();
//
//     assert!(*business_rarity >= 1.0, "Rarity gain should be meaningful");
//     assert!(*similar_rarity >= 1.0, "Rarity gain should be meaningful");
// }
