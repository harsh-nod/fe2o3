// Included in the ranked parent's test module after production_hook_tests.rs.

#[test]
fn empty_goto_census_both_live_raw_guards_keep_original_rejection_and_zero_proof_work() {
    let n = MAX_RANKED_BOUNDS_BLOCKS + 1;
    let function = source_cfg_diagnostic_function(n, |b| {
        if b + 1 == n {
            SemanticTerminatorKindV1::Return
        } else {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, (b + 1) as u32))
        }
    });
    let original = function.clone();
    let mut work = 0;
    let csr = LosslessCsrV1::build(&function, CsrWorkV1::new(&mut work, 0));
    let assertion = SemanticAssertProofsV1::new(&[], &function);
    let errors = [
        match csr {
            Err(e) => e,
            Ok(_) => panic!("raw CSR guard must reject"),
        },
        match assertion {
            Err(e) => e,
            Ok(_) => panic!("raw assert guard must reject"),
        },
    ];
    for error in errors {
        let ProductionRankedProjectionErrorV1::SourceCfgLimit(diagnostic) = error else {
            panic!("the original raw limit error must remain: {error:?}");
        };
        let text = diagnostic.to_string();
        let (original, census) = text.split_once("; empty-Goto census: ").unwrap();
        assert!(original.ends_with("1025 blocks, 1024 edge visits"));
        assert!(census.contains("literal_empty_gotos: 1024"), "{census}");
        assert!(
            census.contains("single_entry_acyclic_interiors: 1023"),
            "{census}"
        );
        assert!(census.contains("source_only_not_ranked_eligibility"));
    }
    assert_eq!(
        work, 0,
        "diagnostics must not replenish or spend proof fuel"
    );
    assert_eq!(function, original, "no source or source-coordinate rewrite");
}

#[test]
fn empty_goto_census_source_adapter_keeps_cleanup_and_parallel_entries() {
    let function = source_cfg_diagnostic_function(5, |b| match b {
        0 => source_cfg_cleanup_call(),
        1 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
        2 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
        3 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
        _ => SemanticTerminatorKindV1::Return,
    });
    let text = format!(
        "{:?}",
        source_cfg_diagnostic_v1::empty_goto_census_v1::capture(&function)
    );
    assert!(text.contains("visited_edges: 5"), "{text}");
    assert!(text.contains("literal_empty_gotos: 3"), "{text}");
    assert!(text.contains("single_entry_acyclic_interiors: 0"), "{text}");
    assert!(text.contains("blocks_after_structural_only: 5"), "{text}");
    assert!(text.contains("empty_by_kind: [3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0]"));
}

#[test]
fn empty_goto_census_source_adapter_never_calls_a_statement_bearing_goto_empty() {
    let original = source_cfg_diagnostic_function(4, |b| {
        if b == 3 {
            SemanticTerminatorKindV1::Return
        } else {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, (b + 1) as u32))
        }
    });
    let mut blocks = original.blocks().to_vec();
    let block = &blocks[1];
    blocks[1] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        vec![SemanticStatementV1::new(
            block.source(),
            SemanticStatementKindV1::Nop,
        )],
        block.terminator().clone(),
    )
    .unwrap();
    let changed = projection_function(blocks);
    let text = format!(
        "{:?}",
        source_cfg_diagnostic_v1::empty_goto_census_v1::capture(&changed)
    );
    assert!(text.contains("literal_empty_gotos: 2"), "{text}");
    assert!(text.contains("single_entry_acyclic_interiors: 1"), "{text}");
}
