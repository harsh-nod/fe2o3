#[derive(Clone, Copy)]
enum ScopeMutationV2 {
    None,
    HideReachable,
    ExposeDead,
    DifferentBlockCount,
}

fn exact_control_plan_v2(
    semantic: &AdmittedInertSemanticMirV1,
    mutation: ScopeMutationV2,
) -> crate::ssa::SsaConstructionPlanV1 {
    use crate::ssa::{
        SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, SsaEdgeInputV1, SsaEdgeRoleV1,
        plan_ssa_v1,
    };
    let function = &semantic.functions()[0];
    let mut blocks = function
        .blocks()
        .iter()
        .map(|block| {
            let mut edges = Vec::new();
            block
                .terminator()
                .kind()
                .try_for_each_edge(|edge| {
                    edges.push(SsaEdgeInputV1::new(
                        SsaEdgeRoleV1::new(u16::try_from(edges.len() + 1).unwrap()),
                        SsaBlockIdV1::new(edge.target().index()),
                        vec![],
                    ));
                    Ok::<(), ()>(())
                })
                .unwrap();
            SsaBlockInputV1::new(vec![], edges)
        })
        .collect::<Vec<_>>();
    match mutation {
        ScopeMutationV2::None => {}
        ScopeMutationV2::HideReachable => {
            blocks[0] = SsaBlockInputV1::new(vec![], vec![]);
        }
        ScopeMutationV2::ExposeDead => {
            let mut edges = blocks[0].edges().to_vec();
            edges.push(SsaEdgeInputV1::new(
                SsaEdgeRoleV1::new(2),
                SsaBlockIdV1::new(5),
                vec![],
            ));
            blocks[0] = SsaBlockInputV1::new(vec![], edges);
        }
        ScopeMutationV2::DifferentBlockCount => {
            blocks.push(SsaBlockInputV1::new(vec![], vec![]));
        }
    }
    // This is an ordinary checked neutral control-only plan, not a fabricated
    // production SSA owner. Backend tests exercise the retained production plan.
    plan_ssa_v1(&SsaConstructionInputV1::new(
        SsaBlockIdV1::new(function.entry().index()),
        0,
        vec![],
        vec![],
        blocks,
    ))
    .unwrap()
}

fn scoped_report_v2(
    semantic: &AdmittedInertSemanticMirV1,
) -> SemanticU32InductionNoOverflowReportV1 {
    analyze_semantic_u32_induction_no_overflow_with_ssa_plan_v2(
        semantic,
        SemanticFunctionIdV1::from_index(0),
        &exact_control_plan_v2(semantic, ScopeMutationV2::None),
        SemanticU32InductionAnalysisLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn reachable_scope_ignores_dead_predecessors_definitions_aliases_and_candidates() {
    for shape in [
        Shape {
            dead_predecessor: true,
            ..Shape::default()
        },
        Shape {
            dead_definitions: true,
            ..Shape::default()
        },
    ] {
        let semantic = admitted(shape);
        let scoped = scoped_report_v2(&semantic);
        assert_eq!(scoped.checked_additions_examined(), 1);
        let [certificate] = scoped.certificates() else {
            panic!("missing live induction fact")
        };
        assert_eq!(certificate.induction().local(), INDUCTION);
        assert_eq!(certificate.preheader().block().index(), 0);
        assert_eq!(certificate.header().block().index(), 1);
        assert_eq!(certificate.checked_addition().block().block().index(), 2);
        assert_eq!(certificate.update().block().block().index(), 3);
        assert_eq!(scoped.ssa_scope_work_units_v2(), 7);
        assert_eq!(scoped.total_work_units_v2(), scoped.work_units() + 7);
        assert!(scoped.uses_reachable_scope_v2());
        assert_eq!(scoped.reachable_block_count_v2(), Some(5));
        assert_eq!(scoped.reachable_statement_count_v2(), Some(4));
        assert!(
            !scoped
                .block_is_reachable_v2(SemanticBlockIdV1::from_index(5))
                .unwrap()
        );
        assert!(
            scoped
                .block_is_reachable_v2(SemanticBlockIdV1::from_index(6))
                .is_err()
        );
        assert!(!scoped.grants_authority());
        assert!(!scoped.authorizes_compiler_transform());
        assert!(matches!(
            analyze_semantic_u32_induction_no_overflow_v1(
                &semantic,
                SemanticFunctionIdV1::from_index(0)
            ),
            Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                "the semantic CFG contains an unreachable block"
            ))
        ));
    }
}

#[test]
fn reachable_scope_never_hides_hostile_live_definitions() {
    let semantic = admitted(Shape {
        dead_definitions: true,
        extra_induction_definition: true,
        ..Shape::default()
    });
    let scoped = scoped_report_v2(&semantic);
    assert_eq!(scoped.checked_additions_examined(), 1);
    assert!(scoped.certificates().is_empty());
}

#[test]
fn reachable_scope_cross_checks_every_actual_ssa_reachability_bit() {
    let semantic = admitted(Shape {
        dead_predecessor: true,
        ..Shape::default()
    });
    for (mutation, detail) in [
        (
            ScopeMutationV2::HideReachable,
            "the SSA plan differs from exact semantic reachability",
        ),
        (
            ScopeMutationV2::ExposeDead,
            "the SSA plan differs from exact semantic reachability",
        ),
        (
            ScopeMutationV2::DifferentBlockCount,
            "the SSA plan has a different source block count",
        ),
    ] {
        let plan = exact_control_plan_v2(&semantic, mutation);
        let error = analyze_semantic_u32_induction_no_overflow_with_ssa_plan_v2(
            &semantic,
            SemanticFunctionIdV1::from_index(0),
            &plan,
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(detail)
        );
    }
}

#[test]
fn reachable_cfg_literal_scope_and_filter_work_boundary() {
    let semantic = admitted(Shape {
        dead_predecessor: true,
        ..Shape::default()
    });
    let plan = exact_control_plan_v2(&semantic, ScopeMutationV2::None);
    // B=6, E=6; reachable traversal=5 blocks+5 edges. Filter=B+E.
    // Core=6+6+10+12=34. SSA cross-check=1+B=7. Total=41.
    let mut work = WorkBudgetV1::new(41);
    let mut scope_work = 0;
    let graph = SemanticCfgV1::analyze(
        &semantic.functions()[0],
        true,
        Some(&plan),
        &mut work,
        &mut scope_work,
    )
    .unwrap();
    assert_eq!(work.used, 41);
    assert_eq!(scope_work, 7);
    assert_eq!(graph.predecessors(1).unwrap(), &[0, 3]);
    assert!(graph.successors[5].is_empty());
    let mut no_work = WorkBudgetV1::new(0);
    assert!(!graph.dominates(5, 5, &mut no_work).unwrap());
    assert!(!graph.dominates(0, 5, &mut no_work).unwrap());
    assert!(!graph.dominates(5, 1, &mut no_work).unwrap());
    let mut insufficient = WorkBudgetV1::new(40);
    let mut failed_scope_work = 0;
    assert!(matches!(
        SemanticCfgV1::analyze(
            &semantic.functions()[0],
            true,
            Some(&plan),
            &mut insufficient,
            &mut failed_scope_work
        ),
        Err(SemanticU32InductionAnalysisErrorV1::WorkLimit {
            actual: 41,
            limit: 40
        })
    ));
    assert_eq!(failed_scope_work, 7);
}

#[test]
fn reachable_evidence_separates_ssa_checks_and_replays_the_same_semantic_work() {
    use crate::InertCanonicalSemanticU32InductionEvidenceV1 as Evidence;
    let semantic = admitted(Shape {
        dead_definitions: true,
        ..Shape::default()
    });
    let report = scoped_report_v2(&semantic);
    let evidence = Evidence::from_report(&report).unwrap();
    assert_eq!(evidence.version(), 2);
    assert_eq!(&evidence.canonical_bytes()[8..12], &[2, 0, 1, 0]);
    assert_eq!(evidence.work_units(), report.work_units() as u64);
    let replay = evidence
        .replay_report(&semantic, SemanticU32InductionAnalysisLimitsV1::default())
        .unwrap();
    assert_eq!(replay.ssa_scope_work_units_v2(), 0);
    assert_eq!(replay.work_units(), report.work_units());
    assert_eq!(replay.certificates(), report.certificates());
    assert_eq!(Evidence::from_report(&replay).unwrap(), evidence);
    evidence.revalidate().unwrap();
    assert!(!evidence.grants_authority());
}

#[test]
fn complete_scope_preserves_v1_work_bytes_and_versioned_identity() {
    use crate::InertCanonicalSemanticU32InductionEvidenceV1 as Evidence;
    use sha2::{Digest, Sha256};
    let semantic = admitted(Shape::default());
    let legacy = report(&semantic);
    assert!(!legacy.uses_reachable_scope_v2());
    assert_eq!(legacy.reachable_block_count_v2(), None);
    assert!(
        legacy
            .block_is_reachable_v2(SemanticBlockIdV1::from_index(0))
            .is_err()
    );
    assert_eq!(legacy.ssa_scope_work_units_v2(), 0);
    assert_eq!(legacy.total_work_units_v2(), legacy.work_units());
    let old = Evidence::from_report(&legacy).unwrap();
    assert_eq!(old.version(), 1);
    assert_eq!(&old.canonical_bytes()[8..12], &[1, 0, 1, 0]);
    let replay = old
        .replay_report(&semantic, SemanticU32InductionAnalysisLimitsV1::default())
        .unwrap();
    assert_eq!(replay, legacy);
    assert_eq!(Evidence::from_report(&replay).unwrap(), old);
    let scoped = scoped_report_v2(&semantic);
    assert_eq!(scoped.work_units(), legacy.work_units() + 10); // B+E filter
    assert_eq!(scoped.ssa_scope_work_units_v2(), 6); // 1+B
    assert_eq!(scoped.certificates(), legacy.certificates());
    let new = Evidence::from_report(&scoped).unwrap();
    for (evidence, domain) in [
        (
            &old,
            b"FE2O3/SEMANTIC-U32-INDUCTION-EVIDENCE/V1\0".as_slice(),
        ),
        (
            &new,
            b"FE2O3/SEMANTIC-U32-INDUCTION-EVIDENCE/V2\0".as_slice(),
        ),
    ] {
        let mut hash = Sha256::new();
        hash.update(domain);
        hash.update((evidence.canonical_bytes().len() as u64).to_le_bytes());
        hash.update(evidence.canonical_bytes());
        assert_eq!(*evidence.identity(), <[u8; 32]>::from(hash.finalize()));
    }
    assert_ne!(new.identity(), old.identity());
}

#[test]
fn reachable_evidence_rejects_downgrade_unknown_version_work_and_owner_splices() {
    use crate::InertCanonicalSemanticU32InductionEvidenceV1 as Evidence;
    let semantic = admitted(Shape {
        dead_predecessor: true,
        ..Shape::default()
    });
    let evidence = Evidence::from_report(&scoped_report_v2(&semantic)).unwrap();
    for end in [0, 8, 9, 10, 12, 103, evidence.canonical_bytes().len() - 1] {
        assert!(Evidence::decode(&evidence.canonical_bytes()[..end]).is_err());
    }
    let mut trailing = evidence.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(Evidence::decode(&trailing).is_err());
    let mut reserved = evidence.canonical_bytes().to_vec();
    reserved[12] = 1;
    assert!(Evidence::decode(&reserved).is_err());
    let mut downgrade = evidence.canonical_bytes().to_vec();
    downgrade[8..10].copy_from_slice(&1_u16.to_le_bytes());
    let old = Evidence::decode(&downgrade).unwrap();
    assert!(matches!(
        old.replay_report(&semantic, SemanticU32InductionAnalysisLimitsV1::default()),
        Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(_))
    ));
    let mut unknown = evidence.canonical_bytes().to_vec();
    unknown[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert!(Evidence::decode(&unknown).is_err());
    let mut altered_work = evidence.canonical_bytes().to_vec();
    altered_work[92..100].copy_from_slice(&(evidence.work_units() + 1).to_le_bytes());
    let changed = Evidence::decode(&altered_work).unwrap();
    let replay = changed
        .replay_report(&semantic, SemanticU32InductionAnalysisLimitsV1::default())
        .unwrap();
    assert_ne!(
        Evidence::from_report(&replay).unwrap().canonical_bytes(),
        changed.canonical_bytes()
    );
    let complete = admitted(Shape::default());
    let legacy = Evidence::from_report(&report(&complete)).unwrap();
    let mut upgrade = legacy.canonical_bytes().to_vec();
    upgrade[8..10].copy_from_slice(&2_u16.to_le_bytes());
    let upgraded = Evidence::decode(&upgrade).unwrap();
    let replay = upgraded
        .replay_report(&complete, SemanticU32InductionAnalysisLimitsV1::default())
        .unwrap();
    assert_ne!(
        Evidence::from_report(&replay).unwrap().canonical_bytes(),
        upgraded.canonical_bytes()
    );
    let substituted = admitted(Shape {
        dead_predecessor: true,
        identity_seed: 1,
        ..Shape::default()
    });
    let replay = evidence
        .replay_report(
            &substituted,
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
        .unwrap();
    assert_ne!(
        Evidence::from_report(&replay).unwrap().canonical_bytes(),
        evidence.canonical_bytes()
    );
}
