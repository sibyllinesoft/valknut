//! Tests for semantic naming analysis system.
//!
//! This module tests the complete semantic naming analysis pipeline including:
//! - Behavior signature extraction
//! - Semantic mismatch detection  
//! - Name proposal generation
//! - Pack creation and prioritization
//! - Golden test cases from TODO.md

use std::collections::HashMap;

use valknut_rs::detectors::names::{
    BehaviorSignature, SideEffects, DatabaseOperations, FileOperations, MutationPattern,
    ExecutionPattern, ReturnTypeInfo, TypeCategory, ResourceHandling,
    SemanticNameAnalyzer, NamesConfig, FunctionInfo, ParameterInfo, CallSite,
    AnalysisResults, MismatchType, ContractSeverity,
};
use valknut_rs::detectors::embedding::EmbeddingBackend;
use valknut_rs::core::errors::Result;

/// Test behavior signature extraction
#[tokio::test]
async fn test_behavior_extraction() -> Result<()> {
    let config = NamesConfig::default();
    let analyzer = SemanticNameAnalyzer::new(config).await?;

    // Test function that should have database write effects
    let func = create_test_function("get_user", "public", "User");
    let behavior = analyzer.behavior_extractor.extract_behavior(&func)?;

    // Should detect read operation based on "get_" prefix
    assert!(behavior.side_effects.database_operations.reads);
    assert!(!behavior.side_effects.database_operations.writes);
    
    Ok(())
}

/// Test semantic mismatch detection - Effect Mismatch case
#[tokio::test] 
async fn test_effect_mismatch_detection() -> Result<()> {
    let config = NamesConfig::default();
    let mut analyzer = SemanticNameAnalyzer::new(config).await?;
    
    // Function named "get_user" but actually mutates database
    let mut func = create_test_function("get_user", "public", "User");
    func.call_sites = vec![
        CallSite { file_path: "module1.rs".to_string(), line_number: 42 },
        CallSite { file_path: "module2.rs".to_string(), line_number: 123 },
        CallSite { file_path: "module3.rs".to_string(), line_number: 456 },
        CallSite { file_path: "module4.rs".to_string(), line_number: 789 },
    ];
    
    // Mock behavior with write effects (contradicting "get_" name)
    let behavior = BehaviorSignature {
        side_effects: SideEffects {
            http_operations: false,
            database_operations: DatabaseOperations {
                reads: true,
                writes: true,  // This contradicts "get_" prefix
                creates: false,
                deletes: false,
            },
            file_operations: FileOperations {
                reads: false,
                writes: false,
                creates: false,
                deletes: false,
            },
            network_operations: false,
            console_output: false,
        },
        mutations: MutationPattern::GlobalMutation,
        execution_pattern: ExecutionPattern::Synchronous,
        return_type: ReturnTypeInfo {
            primary_type: Some("User".to_string()),
            optional: false,
            collection: false,
            lazy_evaluation: false,
            type_category: TypeCategory::Object,
        },
        resource_handling: ResourceHandling {
            acquires_resources: false,
            releases_resources: false,
            returns_handles: false,
        },
        confidence: 0.8,
    };

    let mismatch = analyzer.check_semantic_mismatch("get_user", &behavior).await?;
    
    // Should detect effect mismatch
    assert!(mismatch.mismatch_types.iter().any(|m| matches!(m, MismatchType::EffectMismatch { .. })));
    assert!(mismatch.mismatch_score >= 0.65); // Above threshold
    
    Ok(())
}

/// Test cardinality mismatch detection
#[tokio::test]
async fn test_cardinality_mismatch() -> Result<()> {
    let config = NamesConfig::default();
    let analyzer = SemanticNameAnalyzer::new(config).await?;
    
    // Function named "user" but returns collection
    let behavior = BehaviorSignature {
        side_effects: SideEffects {
            http_operations: false,
            database_operations: DatabaseOperations {
                reads: true,
                writes: false,
                creates: false,
                deletes: false,
            },
            file_operations: FileOperations {
                reads: false,
                writes: false,
                creates: false,
                deletes: false,
            },
            network_operations: false,
            console_output: false,
        },
        mutations: MutationPattern::Pure,
        execution_pattern: ExecutionPattern::Synchronous,
        return_type: ReturnTypeInfo {
            primary_type: Some("User".to_string()),
            optional: false,
            collection: true, // This contradicts singular "user" name
            lazy_evaluation: true,
            type_category: TypeCategory::Collection,
        },
        resource_handling: ResourceHandling {
            acquires_resources: false,
            releases_resources: false,
            returns_handles: false,
        },
        confidence: 0.9,
    };

    let mismatch = analyzer.check_semantic_mismatch("user", &behavior).await?;
    
    // Should detect cardinality mismatch
    assert!(mismatch.mismatch_types.iter().any(|m| matches!(m, MismatchType::CardinalityMismatch { .. })));
    
    Ok(())
}

/// Test optionality mismatch detection
#[tokio::test]
async fn test_optionality_mismatch() -> Result<()> {
    let config = NamesConfig::default();
    let analyzer = SemanticNameAnalyzer::new(config).await?;
    
    // Function named "find_user" but returns non-optional
    let behavior = BehaviorSignature {
        side_effects: SideEffects {
            http_operations: false,
            database_operations: DatabaseOperations {
                reads: true,
                writes: false,
                creates: false,
                deletes: false,
            },
            file_operations: FileOperations {
                reads: false,
                writes: false,
                creates: false,
                deletes: false,
            },
            network_operations: false,
            console_output: false,
        },
        mutations: MutationPattern::Pure,
        execution_pattern: ExecutionPattern::Synchronous,
        return_type: ReturnTypeInfo {
            primary_type: Some("User".to_string()),
            optional: false, // This contradicts "find_" prefix expectation
            collection: false,
            lazy_evaluation: false,
            type_category: TypeCategory::Object,
        },
        resource_handling: ResourceHandling {
            acquires_resources: false,
            releases_resources: false,
            returns_handles: false,
        },
        confidence: 0.9,
    };

    let mismatch = analyzer.check_semantic_mismatch("find_user", &behavior).await?;
    
    // Should detect optionality mismatch
    assert!(mismatch.mismatch_types.iter().any(|m| matches!(m, MismatchType::OptionalityMismatch { .. })));
    
    Ok(())
}

/// Golden test case 1: get_user() mutates DB → expect EffectMismatch + rename to update_user/upsert_user
#[tokio::test]
async fn test_golden_case_get_user_mutation() -> Result<()> {
    let config = NamesConfig::default();
    let mut analyzer = SemanticNameAnalyzer::new(config).await?;
    
    let mut func = create_test_function("get_user", "public", "User");
    // Add enough call sites to meet impact threshold
    for i in 0..5 {
        func.call_sites.push(CallSite {
            file_path: format!("module{}.rs", i),
            line_number: 100 + i,
        });
    }
    
    let functions = vec![func];
    let results = analyzer.analyze_functions(&functions).await?;
    
    // Should generate rename pack due to effect mismatch
    assert!(!results.rename_packs.is_empty());
    
    let rename_pack = &results.rename_packs[0];
    assert_eq!(rename_pack.current_name, "get_user");
    
    // Should propose names like "update_user" or "upsert_user"
    let proposed_names: Vec<&String> = rename_pack.proposals.iter().map(|p| &p.name).collect();
    assert!(proposed_names.iter().any(|name| name.contains("update") || name.contains("upsert")));
    
    // Should detect effect mismatch
    assert!(rename_pack.mismatch.mismatch_types.iter().any(|m| {
        matches!(m, MismatchType::EffectMismatch { .. })
    }));
    
    Ok(())
}

/// Golden test case 2: find_user() returns User (non-Optional) → OptionalityMismatch
#[tokio::test]
async fn test_golden_case_find_user_non_optional() -> Result<()> {
    let config = NamesConfig::default();
    let mut analyzer = SemanticNameAnalyzer::new(config).await?;
    
    let mut func = create_test_function("find_user", "public", "User");
    // Add enough call sites to meet impact threshold  
    for i in 0..5 {
        func.call_sites.push(CallSite {
            file_path: format!("module{}.rs", i),
            line_number: 200 + i,
        });
    }
    
    let functions = vec![func];
    let results = analyzer.analyze_functions(&functions).await?;
    
    // Should generate contract mismatch pack due to optionality issue
    assert!(!results.contract_mismatch_packs.is_empty());
    
    let contract_pack = &results.contract_mismatch_packs[0];
    assert_eq!(contract_pack.current_name, "find_user");
    
    // Should detect optionality mismatch
    let has_optionality_issue = contract_pack.contract_issues.iter().any(|issue| {
        issue.description.contains("optionality") && 
        issue.name_implies.contains("optional")
    });
    assert!(has_optionality_issue);
    
    // Should suggest both rename and contract change solutions
    assert!(!contract_pack.solutions.is_empty());
    
    Ok(())
}

/// Golden test case 3: users() returns iterator → CardinalityMismatch with iter_users
#[tokio::test]
async fn test_golden_case_users_iterator() -> Result<()> {
    let config = NamesConfig::default();
    let mut analyzer = SemanticNameAnalyzer::new(config).await?;
    
    let mut func = create_test_function("users", "public", "Iterator<User>");
    // Add enough call sites
    for i in 0..5 {
        func.call_sites.push(CallSite {
            file_path: format!("service{}.rs", i),
            line_number: 300 + i,
        });
    }
    
    let functions = vec![func];
    let results = analyzer.analyze_functions(&functions).await?;
    
    // Should generate rename pack
    assert!(!results.rename_packs.is_empty());
    
    let rename_pack = &results.rename_packs[0];
    assert_eq!(rename_pack.current_name, "users");
    
    // Should propose "iter_users" or "list_users"
    let proposed_names: Vec<&String> = rename_pack.proposals.iter().map(|p| &p.name).collect();
    assert!(proposed_names.iter().any(|name| name.contains("iter") || name.contains("list")));
    
    Ok(())
}

/// Test mismatch score calculation using TODO.md formula
#[tokio::test]
async fn test_mismatch_score_calculation() -> Result<()> {
    let config = NamesConfig::default();
    let analyzer = SemanticNameAnalyzer::new(config).await?;
    
    let mismatch_types = vec![
        MismatchType::EffectMismatch {
            expected: "read-only".to_string(),
            actual: "mutating".to_string(),
        }
    ];
    
    let behavior = BehaviorSignature {
        side_effects: SideEffects {
            http_operations: false,
            database_operations: DatabaseOperations {
                reads: true,
                writes: true,
                creates: false,
                deletes: false,
            },
            file_operations: FileOperations {
                reads: false,
                writes: false,
                creates: false,
                deletes: false,
            },
            network_operations: false,
            console_output: false,
        },
        mutations: MutationPattern::GlobalMutation,
        execution_pattern: ExecutionPattern::Synchronous,
        return_type: ReturnTypeInfo {
            primary_type: Some("User".to_string()),
            optional: false,
            collection: false,
            lazy_evaluation: false,
            type_category: TypeCategory::Object,
        },
        resource_handling: ResourceHandling {
            acquires_resources: false,
            releases_resources: false,
            returns_handles: false,
        },
        confidence: 0.8,
    };
    
    // Test formula: 0.5*(1 - cosine) + 0.2*effect + 0.1*cardinality + 0.1*optional + 0.1*async_or_idempotence
    let cosine_similarity = 0.3; // Low similarity
    let score = analyzer.calculate_mismatch_score(cosine_similarity, &mismatch_types, &behavior);
    
    let expected = 0.5 * (1.0 - cosine_similarity) + 0.2 * 1.0; // Effect mismatch
    assert!((score - expected).abs() < 0.01);
    
    Ok(())
}

/// Test priority calculation for rename packs
#[tokio::test] 
async fn test_rename_priority_calculation() -> Result<()> {
    let config = NamesConfig::default();
    let analyzer = SemanticNameAnalyzer::new(config).await?;
    
    let mismatch = create_test_mismatch(0.8, vec![MismatchType::EffectMismatch {
        expected: "read".to_string(),
        actual: "write".to_string(),
    }]);
    
    let impact = create_test_impact(10, 3, false, 4); // 10 refs, 3 files, not public, effort 4
    
    let priority = analyzer.calculate_rename_priority(&mismatch, &impact);
    
    // Priority should be value / (effort + ε)
    // value = mismatch_score * ln(1 + external_refs) = 0.8 * ln(11) ≈ 1.92
    // effort = 4.0
    let expected_priority = 1.92 / 4.1; // Adding epsilon of 0.1
    assert!((priority - expected_priority).abs() < 0.1);
    
    Ok(())
}

/// Test abbreviation expansion
#[tokio::test]
async fn test_abbreviation_expansion() -> Result<()> {
    let mut config = NamesConfig::default();
    config.abbrev_map.insert("usr".to_string(), "user".to_string());
    config.abbrev_map.insert("cfg".to_string(), "config".to_string());
    
    let analyzer = SemanticNameAnalyzer::new(config).await?;
    
    // Test name gloss generation with abbreviation expansion
    let gloss = analyzer.generate_name_gloss("get_usr_cfg")?;
    assert_eq!(gloss, "get user config");
    
    Ok(())
}

/// Test embedding backend functionality 
#[tokio::test]
async fn test_embedding_backend() -> Result<()> {
    // This test uses the dummy embedding implementation
    let backend = EmbeddingBackend::new("test-model").await;
    // Will fail since model isn't available, which is expected
    assert!(backend.is_err());
    
    Ok(())
}

/// Test project lexicon building
#[tokio::test]
async fn test_lexicon_building() -> Result<()> {
    let config = NamesConfig::default();
    let mut analyzer = SemanticNameAnalyzer::new(config).await?;
    
    let functions = vec![
        create_test_function("get_user", "public", "User"),
        create_test_function("create_user", "public", "User"),
        create_test_function("update_user", "public", "User"),
        create_test_function("get_config", "private", "Config"),
        create_test_function("set_config", "private", "Config"),
    ];
    
    analyzer.build_lexicon(&functions)?;
    
    // Should extract domain nouns
    assert!(analyzer.lexicon.domain_nouns.contains_key("user"));
    assert!(analyzer.lexicon.domain_nouns.contains_key("config"));
    
    // Should extract verb patterns
    assert!(analyzer.lexicon.verb_patterns.contains_key("get"));
    assert!(analyzer.lexicon.verb_patterns.contains_key("create"));
    assert!(analyzer.lexicon.verb_patterns.contains_key("update"));
    assert!(analyzer.lexicon.verb_patterns.contains_key("set"));
    
    Ok(())
}

/// Test configuration validation
#[tokio::test]
async fn test_config_validation() {
    let mut config = NamesConfig::default();
    
    // Valid config should work
    assert!(config.min_mismatch >= 0.0 && config.min_mismatch <= 1.0);
    assert!(config.min_impact > 0);
    
    // Test edge values
    config.min_mismatch = 0.65; // From TODO.md spec
    config.min_impact = 3; // From TODO.md spec
    
    let analyzer = SemanticNameAnalyzer::new(config).await;
    assert!(analyzer.is_ok());
}

// Helper functions for creating test data

fn create_test_function(name: &str, visibility: &str, return_type: &str) -> FunctionInfo {
    FunctionInfo {
        id: format!("func_{}", name),
        name: name.to_string(),
        file_path: "test.rs".to_string(),
        line_number: 42,
        visibility: visibility.to_string(),
        parameters: vec![
            ParameterInfo {
                name: "self".to_string(),
                type_name: Some("&Self".to_string()),
            },
        ],
        return_type: Some(return_type.to_string()),
        body_ast: None,
        call_sites: vec![],
    }
}

fn create_test_mismatch(score: f64, mismatch_types: Vec<MismatchType>) -> valknut_rs::detectors::names::SemanticMismatch {
    valknut_rs::detectors::names::SemanticMismatch {
        cosine_similarity: 1.0 - score, // Inverse relationship
        mismatch_types,
        mismatch_score: score,
        confidence: 0.9,
    }
}

fn create_test_impact(external_refs: usize, affected_files: usize, public_api: bool, effort: u32) -> valknut_rs::detectors::names::ImpactAnalysis {
    valknut_rs::detectors::names::ImpactAnalysis {
        external_refs,
        affected_files,
        public_api,
        effort_estimate: effort,
        affected_locations: (0..external_refs).map(|i| format!("file{}:line{}", i % affected_files, i * 10)).collect(),
    }
}

/// Integration test - full analysis pipeline
#[tokio::test]
async fn test_full_analysis_pipeline() -> Result<()> {
    let config = NamesConfig::default();
    let mut analyzer = SemanticNameAnalyzer::new(config).await?;
    
    // Create a mix of functions with different issues
    let functions = vec![
        // Should trigger effect mismatch
        create_function_with_calls("get_user_and_update", "public", "User", 5),
        
        // Should trigger cardinality mismatch  
        create_function_with_calls("user", "public", "Vec<User>", 4),
        
        // Should trigger optionality mismatch
        create_function_with_calls("find_user", "public", "User", 6),
        
        // Good function - should not trigger anything
        create_function_with_calls("get_user", "private", "User", 2), // Below impact threshold
    ];
    
    let results = analyzer.analyze_functions(&functions).await?;
    
    // Should generate multiple packs
    let total_packs = results.rename_packs.len() + results.contract_mismatch_packs.len();
    assert!(total_packs >= 2); // At least some functions should trigger analysis
    
    // Verify priority ordering
    for i in 1..results.rename_packs.len() {
        assert!(results.rename_packs[i-1].priority >= results.rename_packs[i].priority);
    }
    
    for i in 1..results.contract_mismatch_packs.len() {
        assert!(results.contract_mismatch_packs[i-1].priority >= results.contract_mismatch_packs[i].priority);
    }
    
    Ok(())
}

fn create_function_with_calls(name: &str, visibility: &str, return_type: &str, num_calls: usize) -> FunctionInfo {
    let mut func = create_test_function(name, visibility, return_type);
    
    for i in 0..num_calls {
        func.call_sites.push(CallSite {
            file_path: format!("caller{}.rs", i % 3),
            line_number: 100 + i,
        });
    }
    
    func
}