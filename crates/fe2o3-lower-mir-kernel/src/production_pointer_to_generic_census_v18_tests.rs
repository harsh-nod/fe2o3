use super::ty;
use fe2o3_kernel_ir::*;

#[test]
fn executable_v18_cast_does_not_fabricate_a_frozen_native_census_owner() {
    let concrete = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Global, AccessMode::ReadWrite);
    let generic = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadWrite);
    assert!(ty(&concrete));
    assert!(!ty(&generic));
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(vec![ValueDef::new(ValueId(1), generic.clone())], OperationKind::Cast {
        kind: CastKind::PointerToGeneric, value: ValueId(0), to: generic,
    }));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut graph = Module::new("actual-v18-exposure");
    graph.functions.push(Function::kernel_entry("entry", Signature::new(vec![concrete], vec![]), vec![ValueId(0)], vec![block]));
    graph.kernels.push(Kernel::new("entry", "entry", LaunchDomain::D1 { x: LaunchExtent::Static(1) }));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let (owner, receipt) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &graph, StorageLayoutLimitsV1 { rows: 8, edges: 16, containment_depth: 8, object_bytes: 4096 }, &mut budget,
    ).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(matches!(owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind,
        OperationKind::Cast { kind: CastKind::PointerToGeneric, .. }));
    assert!(matches!(
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &graph, &mut budget,
        ),
        Err(CanonicalKernelIrReplayAdmissionErrorV12::Encode(
            KernelIrEncodeError::UnsupportedInVersion {
                version: KERNEL_IR_VERSION_V12,
                feature: "pointer to generic cast",
            }
        ))
    ));
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    // The whitelist is not final-consumer activation: that consumer still
    // requires the separate source-owning V18 pipeline migration.
}
