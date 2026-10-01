//! Synthetic raw-constructor controls only; no admitted source owner is forged.
use super::super::canonical_assertion_facts_v1::ProjectedAssertionConditionV1 as Condition;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;
const LIMIT: usize = 16 * 1024 * 1024;
const FLOOR: usize = 37;
fn function(kinds: Vec<SemanticTerminatorKindV1>, entry: u32) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit = SemanticTypeIdV1::from_index(0);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([13; 32]),
        SemanticLayoutIdentityV1::from_sha256([14; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        Vec::new(),
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let blocks = kinds
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            let mut tag = [0; 32];
            tag[..8].copy_from_slice(&(i as u64).to_le_bytes());
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(tag),
                source,
                Vec::new(),
                SemanticTerminatorV1::new(source, kind),
            )
            .unwrap()
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([17; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([18; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([19; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([21; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([22; 32]),
            unit,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(entry),
        blocks,
    )
    .unwrap()
}
fn edge(role: SemanticEdgeRoleV1, block: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
}
fn place() -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(0),
        vec![],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap()
}
fn go(block: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, block))
}
fn switch(targets: &[u32], otherwise: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place()),
        targets: SemanticSwitchTargetsV1::new(
            targets
                .iter()
                .enumerate()
                .map(|(value, &target)| {
                    SemanticSwitchTargetV1::new(
                        value as u128,
                        edge(SemanticEdgeRoleV1::SwitchValue, target),
                    )
                })
                .collect(),
            edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
        )
        .unwrap(),
    }
}
fn call(target: Option<u32>) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            target.map(|block| {
                SemanticCallDestinationV1::new(place(), edge(SemanticEdgeRoleV1::CallReturn, block))
            }),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn assert_to(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Copy(place()),
        expected: true,
        message: SemanticAssertMessageV1::NullPointerDereference,
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}
fn drop_to(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Drop {
        place: place(),
        drop_glue: SemanticFunctionIdV1::from_index(0),
        target: edge(SemanticEdgeRoleV1::DropReturn, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}

struct Facts<'b, 'w> {
    budget: Option<&'b mut Budget<'w>>,
    condition: Condition,
    queries: usize,
}
impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
    fn charge_private_array_work(&mut self, n: usize) -> Result<()> {
        match self.budget.as_deref_mut() {
            Some(budget) => budget
                .charge_work(n)
                .map_err(ranked_projection_source_v1::resource),
            None => Ok(()),
        }
    }
    fn reserve_scalar_private_storage_v1(&mut self, n: usize) -> Result<()> {
        self.budget
            .as_deref_mut()
            .expect("strict test budget")
            .reserve_storage(n)
            .map_err(ranked_projection_source_v1::resource)
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> Result<Option<u64>> {
        Ok(None)
    }
    fn is_materialized_block(&mut self, _: usize) -> Result<bool> {
        Ok(true)
    }
    fn condition(&mut self, _: usize, _: bool, _: SemanticBlockIdV1) -> Result<Condition> {
        self.queries += 1;
        Ok(self.condition)
    }
}
fn shape(row: &ProjectedCfgTerminatorV1) -> (&'static str, Vec<usize>) {
    match row {
        ProjectedCfgTerminatorV1::AbsentMaterialized => ("absent", vec![]),
        ProjectedCfgTerminatorV1::Branch(n) => ("branch", vec![*n]),
        ProjectedCfgTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
        } => ("split", vec![*first_block, *second_block]),
        ProjectedCfgTerminatorV1::AnalysisMultiSplit { blocks } => ("multi", blocks.clone()),
        ProjectedCfgTerminatorV1::Return => ("return", vec![]),
        ProjectedCfgTerminatorV1::Trap => ("trap", vec![]),
        ProjectedCfgTerminatorV1::Predicate { .. } => ("predicate", vec![]),
        ProjectedCfgTerminatorV1::ExactSwitch(_) => ("exact", vec![]),
    }
}
fn reason(error: &ProductionRankedProjectionErrorV1) -> &'static str {
    match error {
        ProductionRankedProjectionErrorV1::Unsupported(reason)
        | ProductionRankedProjectionErrorV1::Incomplete(reason) => reason,
        ProductionRankedProjectionErrorV1::UnprovenAssert { .. } => "unproved-assert",
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(_),
        ) => "resource",
        _ => panic!("unexpected error"),
    }
}
fn run(
    f: &SemanticFunctionDeclV1,
    index: usize,
    proved: bool,
    condition: Condition,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<ProjectedCfgTerminatorV1>,
    usize,
    usize,
    usize,
    bool,
    bool,
) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let result = project(
        f,
        index,
        &[],
        proved,
        &mut Facts {
            budget: Some(&mut budget),
            condition,
            queries: 0,
        },
        &[],
        &[],
        Some(&mut owned),
    );
    assert_eq!(budget.storage(), FLOOR + owned);
    // The returned inert row can still own its Vec. Tests record the accounting
    // but do not claim this local budget's lifetime bounds a whole compiler.
    (
        result,
        budget.work(),
        budget.peak_storage(),
        owned,
        budget.failed_work().is_some(),
        budget.failed_storage().is_some(),
    )
}
fn legacy(
    f: &SemanticFunctionDeclV1,
    index: usize,
    proved: bool,
    condition: Condition,
) -> Result<ProjectedCfgTerminatorV1> {
    projected_cfg_terminator(
        f,
        index,
        &[],
        proved,
        &mut Facts {
            budget: None,
            condition,
            queries: 0,
        },
        &[],
        &[],
    )
}
#[test]
fn legacy_and_owned_match_original_supported_control_kinds() {
    use SemanticTerminatorKindV1 as T;
    let cases = [
        function(vec![T::Return], 0),
        function(vec![go(1), T::Return], 0),
        function(vec![call(Some(1)), T::Return], 0),
        function(vec![call(None)], 0),
        function(vec![assert_to(1), T::Return], 0),
        function(vec![drop_to(1), T::Return], 0),
        function(
            vec![
                T::Abort,
                T::UnwindResume,
                T::UnwindTerminate,
                T::Unreachable,
            ],
            0,
        ),
    ];
    for f in cases {
        for index in 0..f.blocks().len() {
            let a = legacy(&f, index, true, Condition::Bool(true)).unwrap();
            let b = run(&f, index, true, Condition::Bool(true), LIMIT, LIMIT)
                .0
                .unwrap();
            assert_eq!(shape(&a), shape(&b));
        }
    }
}
#[test]
fn unknown_switch_keeps_complete_first_seen_successors() {
    use SemanticTerminatorKindV1 as T;
    let f = function(
        vec![
            switch(&[3, 1, 3, 2], 4),
            T::Return,
            T::Return,
            T::Return,
            T::Return,
        ],
        0,
    );
    let a = legacy(&f, 0, false, Condition::Unknown).unwrap();
    let b = run(&f, 0, false, Condition::Unknown, LIMIT, LIMIT)
        .0
        .unwrap();
    assert_eq!(shape(&a), ("multi", vec![3, 1, 2, 4]));
    assert_eq!(shape(&a), shape(&b));
}
#[test]
fn duplicate_targets_collapse_only_identical_successor_ids() {
    use SemanticTerminatorKindV1 as T;
    for (targets, fallback, expected) in [
        (vec![1, 1, 1], 1, ("branch", vec![1])),
        (vec![2, 1, 2], 1, ("split", vec![2, 1])),
    ] {
        let f = function(vec![switch(&targets, fallback), T::Return, T::Return], 0);
        assert_eq!(
            shape(
                &run(&f, 0, false, Condition::Unknown, LIMIT, LIMIT)
                    .0
                    .unwrap()
            ),
            expected
        );
    }
}
#[test]
fn only_original_boolean_empty_unreachable_fallback_is_elided() {
    use SemanticTerminatorKindV1 as T;
    for (fallback, expected) in [(T::Unreachable, vec![1, 2]), (T::Return, vec![1, 2, 3])] {
        let f = function(vec![switch(&[1, 2], 3), T::Return, T::Return, fallback], 0);
        assert_eq!(
            shape(
                &run(&f, 0, false, Condition::Unknown, LIMIT, LIMIT)
                    .0
                    .unwrap()
            )
            .1,
            expected
        );
    }
}
#[test]
fn contradictory_graph_assertion_never_uses_source_proof_as_override() {
    use SemanticTerminatorKindV1 as T;
    let f = function(vec![assert_to(1), T::Return], 0);
    let a = legacy(&f, 0, true, Condition::Bool(false))
        .err()
        .expect("expected refusal");
    let b = run(&f, 0, true, Condition::Bool(false), LIMIT, LIMIT)
        .0
        .err()
        .expect("expected refusal");
    assert_eq!(reason(&a), "unproved-assert");
    assert_eq!(reason(&a), reason(&b));
}
#[test]
fn unknown_unproved_assertion_is_not_a_success_edge_only_graph() {
    use SemanticTerminatorKindV1 as T;
    let f = function(vec![assert_to(1), T::Return], 0);
    assert_eq!(
        reason(
            &run(&f, 0, false, Condition::Unknown, LIMIT, LIMIT)
                .0
                .err()
                .expect("expected refusal")
        ),
        "unproved-assert"
    );
    assert_eq!(
        shape(
            &run(&f, 0, true, Condition::Unknown, LIMIT, LIMIT)
                .0
                .unwrap()
        ),
        ("branch", vec![1])
    );
}
#[test]
fn cleanup_call_and_false_edge_preserve_exact_ordinary_refusals() {
    use SemanticTerminatorKindV1 as T;
    let cleanup = T::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 1)),
        )
        .unwrap(),
    );
    let false_edge = T::FalseEdge {
        real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
        imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 1),
    };
    for kind in [cleanup, false_edge] {
        let f = function(vec![kind, T::Return], 0);
        let a = legacy(&f, 0, false, Condition::Unknown)
            .err()
            .expect("expected refusal");
        let b = run(&f, 0, false, Condition::Unknown, LIMIT, LIMIT)
            .0
            .err()
            .expect("expected refusal");
        assert_eq!(reason(&a), reason(&b));
    }
}
#[test]
fn stale_block_and_successor_preserve_ordinary_refusal() {
    use SemanticTerminatorKindV1 as T;
    for (f, index) in [
        (function(vec![T::Return], 0), 99),
        (function(vec![go(99)], 0), 0),
        (function(vec![switch(&[1, 99], 1), T::Return], 0), 0),
    ] {
        let a = legacy(&f, index, false, Condition::Unknown)
            .err()
            .expect("expected refusal");
        let b = run(&f, index, false, Condition::Unknown, LIMIT, LIMIT)
            .0
            .err()
            .expect("expected refusal");
        assert_eq!(reason(&a), reason(&b));
    }
}
#[test]
fn exact_and_one_short_original_budget_boundaries() {
    use SemanticTerminatorKindV1 as T;
    let f = function(
        vec![
            switch(&[3, 1, 3, 2], 4),
            T::Return,
            T::Return,
            T::Return,
            T::Return,
        ],
        0,
    );
    let a = run(&f, 0, false, Condition::Unknown, LIMIT, LIMIT);
    assert!(a.0.is_ok());
    let exact = run(&f, 0, false, Condition::Unknown, a.1, a.2);
    assert!(exact.0.is_ok());
    assert_eq!((exact.1, exact.2, exact.3), (a.1, a.2, a.3));
    let w = run(&f, 0, false, Condition::Unknown, a.1 - 1, a.2);
    assert!(w.0.is_err() && w.4 && !w.5);
    let p = run(&f, 0, false, Condition::Unknown, a.1, a.2 - 1);
    assert!(p.0.is_err() && !p.4 && p.5);
}
#[test]
fn denial_precedes_source_walk_or_allocation() {
    let f = function(vec![SemanticTerminatorKindV1::Return], 0);
    let w = run(&f, 0, false, Condition::Unknown, 31, LIMIT);
    assert!(w.0.is_err() && w.4);
    assert_eq!((w.1, w.2, w.3), (0, FLOOR, 0));
    let p = run(&f, 0, false, Condition::Unknown, LIMIT, FLOOR);
    assert!(p.0.is_err() && p.5);
    assert_eq!((p.1, p.2, p.3), (32, FLOOR, 0));
}
#[test]
fn strict_mode_refuses_unowned_predicate_tables_before_clone() {
    let f = function(vec![SemanticTerminatorKindV1::Return], 0);
    for predicate in [true, false] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let error = project(
            &f,
            0,
            &[],
            false,
            &mut Facts {
                budget: Some(&mut budget),
                condition: Condition::Unknown,
                queries: 0,
            },
            if predicate { &[None] } else { &[] },
            if predicate { &[] } else { &[None] },
            Some(&mut owned),
        )
        .err()
        .expect("expected refusal");
        assert_eq!(
            reason(&error),
            "metered nominal CFG requires original conservative switch projection"
        );
        assert_eq!((owned, budget.storage()), (0, FLOOR));
    }
}
#[test]
fn arithmetic_refusal_precedes_storage_mutation() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = usize::MAX;
    let error = reserve_owned(
        &mut Facts {
            budget: Some(&mut budget),
            condition: Condition::Unknown,
            queries: 0,
        },
        &mut owned,
        1,
    )
    .err()
    .expect("expected refusal");
    assert!(matches!(
        error,
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
        )
    ));
    assert_eq!((owned, budget.storage()), (usize::MAX, FLOOR));
}
#[test]
fn accepted_row_storage_is_not_refunded_inside_shared_projector() {
    use SemanticTerminatorKindV1 as T;
    let f = function(
        vec![switch(&[1, 2, 3], 1), T::Return, T::Return, T::Return],
        0,
    );
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let row = project(
        &f,
        0,
        &[],
        false,
        &mut Facts {
            budget: Some(&mut budget),
            condition: Condition::Unknown,
            queries: 0,
        },
        &[],
        &[],
        Some(&mut owned),
    )
    .unwrap();
    assert!(owned > 0);
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(row);
    assert_eq!(budget.storage(), FLOOR + owned);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn strict_operand_lookup_does_not_walk_long_transparent_projections() {
    let ty = SemanticTypeIdV1::from_index(0);
    let projected = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            (0..4096)
                .map(|_| {
                    SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, ty).unwrap()
                })
                .collect(),
            ty,
        )
        .unwrap(),
    );
    assert_eq!(operand_local(&projected, true), None);
    assert_eq!(operand_local(&projected, false), None);
    assert_eq!(
        operand_local(&SemanticOperandV1::Copy(place()), true),
        Some(SemanticLocalIdV1::from_index(0))
    );

    let SemanticTerminatorKindV1::SwitchInt { targets, .. } = switch(&[1, 2, 1], 2) else {
        unreachable!()
    };
    let switched = function(
        vec![
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: projected.clone(),
                targets,
            },
            SemanticTerminatorKindV1::Return,
            SemanticTerminatorKindV1::Return,
        ],
        0,
    );
    let short = function(
        vec![
            switch(&[1, 2, 1], 2),
            SemanticTerminatorKindV1::Return,
            SemanticTerminatorKindV1::Return,
        ],
        0,
    );
    let baseline = run(&short, 0, false, Condition::Unknown, LIMIT, LIMIT);
    let actual = run(
        &switched,
        0,
        false,
        Condition::Unknown,
        baseline.1,
        baseline.2,
    );
    assert_eq!(shape(&baseline.0.unwrap()), shape(&actual.0.unwrap()));
    assert_eq!((baseline.1, baseline.2), (actual.1, actual.2));

    let asserted = function(
        vec![
            SemanticTerminatorKindV1::Assert {
                condition: projected,
                expected: true,
                message: SemanticAssertMessageV1::NullPointerDereference,
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            SemanticTerminatorKindV1::Return,
        ],
        0,
    );
    let outcome = run(&asserted, 0, false, Condition::Unknown, LIMIT, LIMIT);
    let error = outcome.0.err().expect("original unproved assertion");
    assert!(matches!(
        error,
        ProductionRankedProjectionErrorV1::UnprovenAssert {
            condition_local: None,
            ..
        }
    ));
    assert_eq!(outcome.1, 32);
}
