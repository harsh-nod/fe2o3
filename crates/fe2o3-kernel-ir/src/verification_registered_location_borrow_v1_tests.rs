use super::*;
use crate::{
    CanonicalKernelIrWorkBudgetV1, Constant, FunctionId, IntrinsicOperation, ModuleId, Signature,
    ValueDef,
};

#[test]
fn borrowed_registered_location_has_exact_composed_work_and_storage_boundaries() {
    const FLOOR: usize = 7;
    const MODULE_INDEX_WORK: usize = 2;
    // One block, no edges: index29 + reach7 + RPO14 + idom9 + intervals24 + SCC20.
    const CFG_WORK: usize = 29 + 7 + 14 + 9 + 24 + 20;
    // Singleton block fill1 + B/O census5 + singleton definition fill1 + terminal1.
    const STATE_WORK: usize = 1 + 5 + 1 + 1;
    // Base location5; block/definition rosters1+1+5+1+1; block/op scans3;
    // operation location5 + terminal-position1 + operand visit1; dispatch below;
    // return location5 + operand visit1 + terminator check1.
    const COMMON_PASS_WORK: usize = 5 + (1 + 1 + 5 + 1 + 1) + 3 + 5 + 1 + 1 + 7;
    // Registered: selector1 + empty roster terminal1 + shape5 + payload2.
    // Legacy: selector1 + dispatcher1 + result-shape5+1 + scalar comparison1.
    const REGISTERED_WORK: usize = 1 + 1 + 5 + 2;
    const LEGACY_WORK: usize = 1 + 1 + 5 + 1 + 1;
    const EXACT_WORK: usize =
        MODULE_INDEX_WORK + CFG_WORK + STATE_WORK + COMMON_PASS_WORK + REGISTERED_WORK;
    const MODULE_CELLS: usize = 2;
    const CFG_RETAINED: usize = 11;
    const CFG_PEAK: usize = 16;
    const STATE_CELLS: usize = 3 + 6;
    const EXACT_STORAGE: usize = FLOOR + MODULE_CELLS + CFG_RETAINED + STATE_CELLS;

    assert_eq!(REGISTERED_WORK, LEGACY_WORK);
    assert_eq!(EXACT_WORK, 153);
    assert_eq!(EXACT_STORAGE, 29);
    for kind in [
        OperationKind::Constant(Constant::Index(0)),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    ] {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::INDEX),
            kind,
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::definition(
            "location-borrow",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let mut module = Module::new("location-borrow");
        module.functions.push(function);
        // Successful verification borrows IDs regardless of visible length or
        // caller-owned spare capacity; no diagnostic owners are materialized.
        for (identifier_bytes, spare_capacity) in
            [(1, 0), (14, 0), (14, 16_384), (16_384, 0), (16_384, 16_384)]
        {
            let mut module_name = "m".repeat(identifier_bytes);
            let mut function_name = "f".repeat(identifier_bytes);
            module_name.reserve(spare_capacity);
            function_name.reserve(spare_capacity);
            if spare_capacity != 0 {
                assert!(module_name.capacity() > identifier_bytes);
                assert!(function_name.capacity() > identifier_bytes);
            }
            module.id = ModuleId::from(module_name);
            module.functions[0].id = FunctionId::from(function_name);
            for (work_limit, storage_limit) in [
                (EXACT_WORK, EXACT_STORAGE),
                (EXACT_WORK - 1, EXACT_STORAGE),
                (EXACT_WORK, EXACT_STORAGE - 1),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
                budget.reserve_storage(FLOOR).unwrap();
                let module_state = VerificationModuleStateV1::build(&module, &mut budget).unwrap();
                assert_eq!(
                    (budget.work(), budget.storage()),
                    (MODULE_INDEX_WORK, FLOOR + MODULE_CELLS)
                );
                let mut diagnostics = VerificationDiagnosticCollectorV1::count();
                let result = run_verification_function_pass_v1(
                    &module,
                    &module.functions[0],
                    &module_state,
                    None,
                    &mut diagnostics,
                    &mut budget,
                );
                let failed_work = if work_limit < EXACT_WORK {
                    assert!(matches!(result,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == EXACT_WORK && error.limit() == work_limit));
                    assert_eq!(
                        (budget.work(), budget.peak_storage()),
                        (EXACT_WORK - 1, EXACT_STORAGE)
                    );
                    assert_eq!(budget.failed_storage(), None);
                    Some(EXACT_WORK)
                } else if storage_limit < EXACT_STORAGE {
                    assert!(matches!(result,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == EXACT_STORAGE && error.limit() == storage_limit));
                    // Definition reservation rejects after its fill charge, before terminal1.
                    assert_eq!(budget.work(), MODULE_INDEX_WORK + CFG_WORK + STATE_WORK - 1);
                    assert_eq!(budget.peak_storage(), FLOOR + MODULE_CELLS + CFG_PEAK);
                    assert_eq!(budget.failed_storage(), Some(EXACT_STORAGE));
                    None
                } else {
                    assert!(result.is_ok());
                    assert_eq!(
                        (budget.work(), budget.peak_storage()),
                        (EXACT_WORK, EXACT_STORAGE)
                    );
                    assert_eq!(budget.failed_storage(), None);
                    None
                };
                assert_eq!(diagnostics.counted(), Some(0));
                assert_eq!(budget.storage(), FLOOR + MODULE_CELLS);
                diagnostics.abandon(&mut budget).unwrap();
                module_state.release(&mut budget).unwrap();
                assert_eq!(budget.storage(), FLOOR);
                assert_eq!(work.failed_work(), failed_work);
            }
        }
    }
}
