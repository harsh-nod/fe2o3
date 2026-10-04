//! Private scan/predicate controls, not a source-owned inspection fixture.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const RESULTS: [ValueId; 4] = [ValueId(10), ValueId(11), ValueId(12), ValueId(13)];
fn block() -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(7));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}
fn store(value: ValueId) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(90),
            value,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}
fn paid<T>(run: impl FnOnce(&mut Budget<'_>) -> T) -> T {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = run(&mut budget);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}
fn rows(recorder: &Recorder) -> Vec<(u32, Option<u32>, u32, u8)> {
    recorder.uses[..recorder.use_count]
        .iter()
        .map(|row| {
            let row = row.unwrap();
            (row.block.0, row.operation, row.operand, row.component)
        })
        .collect()
}
#[test]
fn one_consumed_result_does_not_fabricate_uses_for_three_unused_definitions() {
    let mut block = block();
    block.operations.push(store(RESULTS[0]));
    let mut recorder = Recorder::blank(ROOT);
    paid(|budget| recorder.capture_result_uses(&[block], RESULTS, budget)).unwrap();
    assert_eq!(rows(&recorder), [(7, Some(0), 1, 0)]);
}
#[test]
fn no_actual_result_use_is_an_empty_roster_not_a_missing_definition_proof() {
    let mut recorder = Recorder::blank(ROOT);
    paid(|budget| recorder.capture_result_uses(&[block()], RESULTS, budget)).unwrap();
    assert_eq!(recorder.use_count, 0);
    assert!(recorder.uses.iter().all(Option::is_none));
    // Only the scan is tested here. seal() still requires one real Matrix,
    // its source span, and all four exact definitions before calling it.
}
#[test]
fn all_four_results_and_repeated_operands_keep_exact_operation_and_edge_rows() {
    let mut block = block();
    block.operations.extend(RESULTS.map(store));
    block.terminator = Some(Terminator::Branch {
        target: BlockId(8),
        arguments: vec![RESULTS[2], RESULTS[2], ValueId(99)],
    });
    let mut recorder = Recorder::blank(ROOT);
    paid(|budget| recorder.capture_result_uses(&[block], RESULTS, budget)).unwrap();
    assert_eq!(
        rows(&recorder),
        [
            (7, Some(0), 1, 0),
            (7, Some(1), 1, 1),
            (7, Some(2), 1, 2),
            (7, Some(3), 1, 3),
            (7, None, 0, 2),
            (7, None, 1, 2),
        ]
    );
}
#[test]
fn changing_a_real_operand_changes_only_its_actual_recorded_component() {
    for component in 0..4 {
        let mut block = block();
        block.operations.push(store(RESULTS[component]));
        let mut recorder = Recorder::blank(ROOT);
        paid(|budget| recorder.capture_result_uses(&[block], RESULTS, budget)).unwrap();
        assert_eq!(rows(&recorder), [(7, Some(0), 1, component as u8)]);
    }
    let mut block = block();
    block.operations.push(store(ValueId(99)));
    let mut recorder = Recorder::blank(ROOT);
    paid(|budget| recorder.capture_result_uses(&[block], RESULTS, budget)).unwrap();
    assert_eq!(recorder.use_count, 0);
}
#[test]
fn result_definition_arity_identity_order_and_type_stay_mandatory() {
    let operation = Operation::new(
        RESULTS
            .map(|id| ValueDef::new(id, Type::Scalar(ScalarType::F32)))
            .to_vec(),
        OperationKind::Matrix(MatrixOperation::multiply_accumulate(
            [ValueId(0); 4],
            [ValueId(1); 4],
            [ValueId(2); 4],
        )),
    );
    assert!(matrix_results(&operation, RESULTS).is_ok());
    for mutation in 0..5 {
        let mut changed = operation.clone();
        match mutation {
            0 => {
                changed.results.pop();
            }
            1 => changed
                .results
                .push(ValueDef::new(ValueId(14), Type::Scalar(ScalarType::F32))),
            2 => changed.results.swap(0, 1),
            3 => changed.results[2].id = ValueId(99),
            _ => changed.results[3].ty = Type::Scalar(ScalarType::U32),
        }
        assert!(matrix_results(&changed, RESULTS).is_err());
    }
}
#[test]
fn use_capacity_is_exact_and_one_over_refuses_without_truncation_success() {
    for count in [USES, USES + 1] {
        let mut block = block();
        block.operations = (0..count).map(|_| store(RESULTS[0])).collect();
        let mut recorder = Recorder::blank(ROOT);
        let result = paid(|budget| recorder.capture_result_uses(&[block], RESULTS, budget));
        assert_eq!(result.is_ok(), count == USES);
        assert_eq!(recorder.use_count, USES);
    }
}
#[test]
fn use_scan_zero_and_exact_work_keep_original_floor_and_sticky_denial() {
    // One bounded block header, one scan block header, one operation and
    // two actual Store operands. Empty Return has no operands.
    for limit in [0, 4, 5] {
        let mut block = block();
        block.operations.push(store(RESULTS[0]));
        let mut recorder = Recorder::blank(ROOT);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = recorder.capture_result_uses(&[block], RESULTS, &mut budget);
        assert_eq!(result.is_ok(), limit == 5);
        assert_eq!(recorder.use_count, usize::from(limit == 5));
        assert_eq!(budget.work(), limit);
        assert_eq!(budget.failed_work().is_some(), limit != 5);
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.peak_storage(), 17);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}
#[test]
fn use_scan_still_rejects_missing_terminator_and_overwide_graph() {
    let mut missing = block();
    missing.terminator = None;
    let mut recorder = Recorder::blank(ROOT);
    assert!(paid(|budget| recorder.capture_result_uses(&[missing], RESULTS, budget)).is_err());
    let mut oversized = block();
    oversized.operations = (0..=OPERATIONS).map(|_| store(RESULTS[0])).collect();
    let mut recorder = Recorder::blank(ROOT);
    assert!(paid(|budget| recorder.capture_result_uses(&[oversized], RESULTS, budget)).is_err());
    assert_eq!(recorder.use_count, 0);
}
