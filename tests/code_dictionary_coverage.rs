use valknut_rs::core::pipeline::{
    issue_code_for_category, issue_definition_for_category, suggestion_code_for_kind,
    suggestion_definition_for_kind,
};

#[test]
fn issue_definitions_cover_all_known_categories() {
    let expectations = [
        ("complexity", "CMPLX"),
        ("cognitive", "COGNIT"),
        ("structure", "STRUCTR"),
        ("graph", "COUPLNG"),
        ("style", "STYLE"),
        ("coverage", "COVGAP"),
        ("debt", "TECHDEBT"),
        ("maintainability", "MAINTAIN"),
        ("readability", "READABL"),
        ("refactoring", "REFACTR"),
    ];

    for (category, expected_code) in expectations {
        let definition = issue_definition_for_category(category);
        assert_eq!(
            definition.code, expected_code,
            "unexpected code for category {category}"
        );
        assert_eq!(
            definition.category.as_deref(),
            Some(category),
            "category field should mirror input for {category}"
        );
    }

    let fallback = issue_definition_for_category("custom-signal");
    assert!(
        fallback.summary.contains("custom-signal"),
        "fallback summary should mention original category"
    );
    assert_eq!(
        issue_code_for_category("custom-signal"),
        fallback.code,
        "shortcut helper should align with definition"
    );
}

#[test]
fn suggestion_definitions_cover_branch_matrix() {
    let cases = [
        ("eliminate_duplication_block", "DEDUP"),
        ("extract_method_for_cleanup", "XTRMTH"),
        ("extract_class_controller", "XTRCLS"),
        ("simplify_nested_conditional_paths", "SIMPCND"),
        ("reduce_cyclomatic_complexity_in_loop", "RDCYCLEX"),
        ("reduce_cognitive_complexity_hotspot", "RDCOGN"),
        ("reduce_fan_in_hotspot", "RDFANIN"),
        ("reduce_fan_out_calls", "RDFANOUT"),
        ("reduce_centrality_module", "RDCNTRL"),
        ("reduce_chokepoint_service", "RDCHOKE"),
        ("address_nested_branching_issue", "RDNEST"),
        ("simplify_logic", "SMPLOGIC"),
        ("split_responsibilities_module", "SPLRESP"),
        ("move_method_to_helper", "MOVEMTH"),
        ("organize_imports_cleanup", "ORGIMPT"),
        ("introduce_facade_layer", "FACAD"),
        ("extract_interface_adapter", "XTRIFCE"),
        ("inline_temp_variable", "INLTEMP"),
        ("rename_class_handler", "RENCLSS"),
        ("rename_method_handler", "RENMTHD"),
        ("extract_variable_threshold", "XTRVAR"),
        ("add_comments_for_complex_flow", "ADDCMNT"),
        ("rename_variable_precisely", "RENVAR"),
        ("replace_magic_number_pi", "REPMAG"),
        ("format_code_style_update", "FMTSTYLE"),
        ("refactor_code_quality_sweep", "REFQLTY"),
    ];

    for (kind, expected_code) in cases {
        let definition = suggestion_definition_for_kind(kind);
        assert_eq!(
            definition.code, expected_code,
            "unexpected code for suggestion kind {kind}"
        );
        assert_eq!(
            definition.category.as_deref(),
            Some("refactoring"),
            "category should remain refactoring for {kind}"
        );
    }

    for (kind, expected_code) in cases {
        assert_eq!(
            suggestion_code_for_kind(kind),
            expected_code,
            "shortcut helper should mirror primary definition for {kind}"
        );
    }
}

#[test]
fn suggestion_definition_handles_fallback_paths() {
    let fallback = suggestion_definition_for_kind("rename@something");
    assert_eq!(fallback.title, "Refactor Rename@something");
    assert!(
        fallback.summary.contains("refactoring"),
        "fallback summary should mention refactoring"
    );

    let helper_code = suggestion_code_for_kind("rename@something");
    assert_eq!(
        helper_code, fallback.code,
        "code helper should align with fallback definition"
    );
}
