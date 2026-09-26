use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Function, MemoryAccess,
    Operation, ScalarType, Signature, StorageLayoutIdV1, StorageLayoutKindV1,
    StorageLayoutV1, StorageOperationV1, Terminator, ValueDef,
};
use crate::PlironOptimizationPlanV1;

const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 8, edges: 16, containment_depth: 8, object_bytes: 1024,
};
const AMPLE: usize = 1_000_000_000;

fn fixture() -> Module {
    let concrete = Type::pointer(Type::StorageObject(StorageLayoutIdV1(0)), AddressSpace::Private, AccessMode::ReadWrite);
    let generic = Type::pointer(Type::StorageObject(StorageLayoutIdV1(0)), AddressSpace::Generic, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    for (id, source) in [(10, 0), (11, 0), (12, 1)] {
        block.operations.push(Operation::effect_free(ValueDef::new(ValueId(id), generic.clone()),
            OperationKind::Cast { kind: CastKind::PointerToGeneric, value: ValueId(source), to: generic.clone() }));
    }
    block.operations.push(Operation::effect_free(ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(7))));
    for address in [10, 11] {
        block.operations.push(Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(address), value: ValueId(20), access: MemoryAccess::new(AddressSpace::Generic, 4),
        })));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("generic-pliron");
    module.storage_layouts.push(StorageLayoutV1 { size: 4, alignment: 4, kind: StorageLayoutKindV1::Scalar(ScalarType::U32) });
    module.functions.push(Function::internal_helper("write",
        Signature::new(vec![concrete.clone(), concrete], vec![]), vec![ValueId(0), ValueId(1)], vec![block]));
    module
}

fn casts(module: &Module) -> usize {
    module.functions.iter().filter_map(|f| f.body.as_ref()).flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations).filter(|op| matches!(op.kind,
            OperationKind::Cast { kind: CastKind::PointerToGeneric, .. })).count()
}

#[test]
fn actual_v18_owner_roundtrips_and_cse_dce_preserve_live_exposure_and_table() {
    let module = fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (input, input_storage) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &module, LIMITS, &mut budget).unwrap();
    budget.reserve_storage(input_storage.retained_storage()).unwrap();
    let (mut graph, graph_storage) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(graph_storage.retained_storage()).unwrap();
    let floor = budget.storage();
    let (original, _, original_storage) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
    budget.reserve_storage(original_storage.retained_storage()).unwrap();
    assert_eq!(original.canonical_bytes(), input.canonical_bytes());
    assert_eq!(casts(original.module()), 3);
    drop(original);
    budget.release_storage(original_storage.retained_storage()).unwrap();
    graph.session.execute_optimization_v1(&graph.root, &PlironOptimizationPlanV1::standard()).unwrap();
    graph.epoch = graph.session.operation_graph_epochs[&graph.root.identity];
    let (output, report, output_storage) = graph.extract_canonical_v18(LIMITS, &mut budget).unwrap();
    budget.reserve_storage(output_storage.retained_storage()).unwrap();
    assert_eq!(casts(output.module()), 1);
    assert_eq!(output.module().storage_layouts, input.module().storage_layouts);
    assert_eq!(report.table, graph.table_identity());
    assert_eq!(&report.input, input.identity());
    assert_eq!(&report.output, output.identity());
    assert_eq!(input.module(), &module);
    assert_ne!(output.identity(), input.identity());
    drop((output, report));
    budget.release_storage(output_storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(graph);
    budget.release_storage(graph_storage.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn live_cast_mutation_cannot_change_pointee_or_reverse_the_space_direction() {
    let module = fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (input, credit) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &module, LIMITS, &mut budget).unwrap();
    budget.reserve_storage(credit.retained_storage()).unwrap();
    let (mut graph, graph_credit) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(graph_credit.retained_storage()).unwrap();
    let (control, _, control_credit) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
    budget.reserve_storage(control_credit.retained_storage()).unwrap();
    assert_eq!(control.module(), input.module());
    drop(control);
    budget.release_storage(control_credit.retained_storage()).unwrap();
    let transaction = graph.session.begin_checked_operation_graph_mutation_v1(&graph.root).unwrap();
    let live = graph.origins.values.iter().find_map(|(value, source)|
        (*source == ValueId(10)).then_some(*value)).unwrap();
    let cast = pliron::operation::Operation::get_op::<CastOp>(
        live.defining_op().unwrap(), &graph.session.context).unwrap();
    cast.set_attr_gpu_cast_kind(&graph.session.context, CastKindAttr::Bitcast);
    assert!(graph.session.commit_checked_operation_graph_mutation_v1(transaction, true).is_err());
    assert!(graph.extract_canonical_v18(LIMITS, &mut budget).is_err());
    assert_eq!(input.module(), &module);
}

#[test]
fn live_v18_bridge_exposure_uses_exact_metered_import_and_output_boundaries() {
    let module = fixture();
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(19).unwrap();
        let result = (|| -> Result<(), KirBridgeErrorV18> {
            let (input, input_credit) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(&module, LIMITS, &mut budget)?;
            budget.reserve_storage(input_credit.retained_storage())?;
            let nested = (|| -> Result<(), KirBridgeErrorV18> {
                let (mut graph, graph_credit) = KirPlironGraphV18::import(&input, &mut budget)?;
                budget.reserve_storage(graph_credit.retained_storage())?;
                let extracted = graph.extract_canonical_v18_o0(LIMITS, &mut budget);
                let nested = match extracted {
                    Ok((output, report, credit)) => {
                        let paid = budget.reserve_storage(credit.retained_storage());
                        assert_eq!(output.canonical_bytes(), input.canonical_bytes());
                        drop((output, report));
                        match paid {
                            Ok(()) => { budget.release_storage(credit.retained_storage())?; Ok(()) }
                            Err(error) => Err(error.into()),
                        }
                    }
                    Err(error) => Err(error),
                };
                drop(graph);
                budget.release_storage(graph_credit.retained_storage())?;
                nested
            })();
            drop(input);
            budget.release_storage(input_credit.retained_storage())?;
            nested
        })();
        assert_eq!(budget.storage(), 19);
        let used = budget.work();
        let peak = budget.peak_storage();
        let denied_storage = budget.failed_storage().is_some();
        (result.is_ok(), used, peak, work.failed_work().is_some(), denied_storage)
    };
    let baseline = run(AMPLE, AMPLE);
    assert!(baseline.0);
    let exact = run(baseline.1, baseline.2);
    assert!(exact.0);
    assert_eq!((exact.1, exact.2), (baseline.1, baseline.2));
    let work_short = run(baseline.1 - 1, baseline.2);
    assert!(!work_short.0 && work_short.3);
    let storage_short = run(baseline.1, baseline.2 - 1);
    assert!(!storage_short.0 && storage_short.4);
}
