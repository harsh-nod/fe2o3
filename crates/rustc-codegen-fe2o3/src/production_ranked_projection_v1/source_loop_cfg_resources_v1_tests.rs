//! Raw constructor-only graph algorithm fixtures, not admitted semantic owners.
//! Discriminants/types need not be meaningful here; only CFG structure is tested.
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
fn same(actual: &ProjectedLoopCfgV1, expected: &ProjectedLoopCfgV1) {
    assert_eq!(actual.successors, expected.successors);
    assert_eq!(actual.predecessors, expected.predecessors);
    assert_eq!(actual.reachable, expected.reachable);
    assert_eq!(actual.entry, expected.entry);
}

fn measure(
    function: &SemanticFunctionDeclV1,
    work_limit: usize,
    storage_limit: usize,
) -> (bool, usize, usize, bool, bool) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let result = projected_loop_cfg_graph_with_resources_v1(
        function,
        &mut PreparationResourcesV1::new(&mut budget, &mut owned),
    );
    let success = result.is_ok();
    drop(result);
    assert_eq!(budget.storage(), FLOOR + owned);
    budget.release_storage(owned).unwrap();
    (
        success,
        budget.work(),
        budget.peak_storage(),
        budget.failed_work().is_some(),
        budget.failed_storage().is_some(),
    )
}
#[test]
fn strict_and_legacy_graphs_match_across_supported_terminators_and_cycles() {
    use SemanticTerminatorKindV1 as T;
    let cases = [
        function(vec![T::Return], 0),
        function(vec![go(1), go(2), T::Return], 0),
        function(vec![switch(&[3, 1, 3, 2], 1), go(3), go(3), T::Return], 0),
        function(
            vec![switch(&[1, 2], 3), T::Return, T::Return, T::Unreachable],
            0,
        ),
        function(vec![call(Some(1)), assert_to(2), drop_to(3), go(0)], 0),
        function(
            vec![call(None), T::UnwindResume, T::UnwindTerminate, T::Abort],
            0,
        ),
        function(
            vec![T::TailCall(
                SemanticDirectTailCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )],
            0,
        ),
    ];
    for function in &cases {
        let expected = projected_loop_cfg_graph_v1(function).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let actual = projected_loop_cfg_graph_with_resources_v1(
            function,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        same(&actual, &expected);
        assert!(budget.work() > 0 && owned > 0);
        drop(actual);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
#[test]
fn graph_keeps_sorted_unique_rows_and_boolean_empty_fallback_rule() {
    use SemanticTerminatorKindV1 as T;
    let raw = function(vec![switch(&[3, 1, 3, 2], 1), go(3), go(3), T::Return], 0);
    let graph = projected_loop_cfg_graph_v1(&raw).unwrap();
    assert_eq!(
        graph.successors,
        vec![vec![1, 2, 3], vec![3], vec![3], vec![]]
    );
    assert_eq!(
        graph.predecessors,
        vec![vec![], vec![0], vec![0], vec![0, 1, 2]]
    );
    assert_eq!(graph.reachable, vec![true; 4]);
    let raw = function(
        vec![switch(&[1, 2], 3), T::Return, T::Return, T::Unreachable],
        0,
    );
    let graph = projected_loop_cfg_graph_v1(&raw).unwrap();
    assert_eq!(graph.successors[0], [1, 2]);
    assert_eq!(graph.reachable, [true, true, true, false]);
    // An otherwise Return is not the existing empty-unreachable exception.
    let raw = function(vec![switch(&[1, 2], 3), T::Return, T::Return, T::Return], 0);
    assert_eq!(
        projected_loop_cfg_graph_v1(&raw).unwrap().successors[0],
        [1, 2, 3]
    );
}
fn reason(error: &ProductionRankedProjectionErrorV1) -> &'static str {
    match error {
        ProductionRankedProjectionErrorV1::Unsupported(reason)
        | ProductionRankedProjectionErrorV1::Incomplete(reason) => *reason,
        _ => panic!("unexpected resource failure"),
    }
}
#[test]
fn legacy_refusal_text_and_order_survive_shared_core_extraction() {
    use SemanticTerminatorKindV1 as T;
    let cases = [
        function(vec![], 0),
        function(vec![go(99)], 0),
        function(vec![T::Return], 99),
        function(
            vec![
                T::FalseEdge {
                    real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal, 0),
                    imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 99),
                },
                go(99),
            ],
            0,
        ),
    ];
    for function in &cases {
        let legacy = projected_loop_cfg_graph_v1(function).unwrap_err();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let strict = projected_loop_cfg_graph_with_resources_v1(
            function,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap_err();
        assert_eq!(reason(&strict), reason(&legacy));
        drop(strict);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
#[test]
fn strict_exact_and_one_short_budgets_are_deterministic() {
    use SemanticTerminatorKindV1 as T;
    let f = function(vec![switch(&[3, 1, 3, 2], 1), go(3), go(3), T::Return], 0);
    let measured = measure(&f, LIMIT, LIMIT);
    assert!(measured.0);
    assert_eq!(measure(&f, measured.1, measured.2), measured);
    let work = measure(&f, measured.1 - 1, measured.2);
    assert!(!work.0 && work.3 && !work.4);
    let storage = measure(&f, measured.1, measured.2 - 1);
    assert!(!storage.0 && !storage.3 && storage.4);
}
#[test]
fn strict_denial_precedes_header_allocation_and_all_source_traversals() {
    let f = function(vec![SemanticTerminatorKindV1::Return], 0);
    let denied_work = measure(&f, 15, LIMIT);
    assert_eq!(denied_work, (false, 0, FLOOR, true, false));
    let denied_storage = measure(&f, LIMIT, FLOOR);
    assert_eq!(denied_storage, (false, 16, FLOOR, false, true));
}
#[test]
fn strict_partial_denials_refund_only_accepted_scratch_after_it_drops() {
    use SemanticTerminatorKindV1 as T;
    let f = function(vec![switch(&[3, 1, 3, 2], 1), go(3), go(3), T::Return], 0);
    let measured = measure(&f, LIMIT, LIMIT);
    for work_limit in [16, 17, measured.1 / 3, measured.1 / 2, measured.1 - 1] {
        let value = measure(&f, work_limit, LIMIT);
        assert!(!value.0 && value.3 && !value.4);
    }
    for storage_limit in [FLOOR, (FLOOR + measured.2) / 2, measured.2 - 1] {
        let value = measure(&f, LIMIT, storage_limit);
        assert!(!value.0 && !value.3 && value.4);
    }
}
#[test]
fn strict_adjacency_sort_and_duplicate_scans_are_not_free() {
    use SemanticTerminatorKindV1 as T;
    let ascending = function(
        vec![
            switch(&[1, 2, 3, 4], 4),
            T::Return,
            T::Return,
            T::Return,
            T::Return,
        ],
        0,
    );
    let descending = function(
        vec![
            switch(&[4, 3, 2, 1], 4),
            T::Return,
            T::Return,
            T::Return,
            T::Return,
        ],
        0,
    );
    let a = measure(&ascending, LIMIT, LIMIT);
    let b = measure(&descending, LIMIT, LIMIT);
    assert!(a.0 && b.0 && b.1 > a.1);
    assert_eq!(a.2, b.2);
    let duplicated = function(vec![switch(&[1, 1, 1, 1], 1), T::Return], 0);
    assert!(measure(&duplicated, LIMIT, LIMIT).0);
}
#[test]
fn reachability_pending_capacity_handles_reconvergence_without_growth() {
    use SemanticTerminatorKindV1 as T;
    let f = function(
        vec![
            switch(&[1, 2, 3], 4),
            switch(&[2, 3], 4),
            switch(&[3, 4], 4),
            go(4),
            T::Return,
        ],
        0,
    );
    let measured = measure(&f, LIMIT, LIMIT);
    assert!(measured.0);
    assert_eq!(measure(&f, measured.1, measured.2), measured);
}
