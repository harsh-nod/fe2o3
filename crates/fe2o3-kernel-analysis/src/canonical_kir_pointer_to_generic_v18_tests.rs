use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrWorkBudgetV1, Function, Module, Signature, StorageLayoutLimitsV1,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV18,
};

#[test]
fn pointer_exposure_never_uses_numeric_constant_or_range_information() {
    let kind = OperationKind::Cast {
        kind: CastKind::PointerToGeneric, value: ValueId(0),
        to: Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadOnly),
    };
    for value in [Value::Unknown, Value::Dynamic, constant(ScalarType::U32, 17)] {
        for input_type in [None, Some(ScalarType::U32)] {
            for result_type in [None, Some(ScalarType::U64)] {
                let result = transfer(&kind, result_type, [Input { value, ty: input_type }; 3]);
                assert_eq!(result.values, [Value::Dynamic; 2]);
                assert!(!result.exceptional);
            }
        }
    }
}

#[test]
fn ordinary_integer_transfer_still_folds_after_the_pointer_kind_extension() {
    let result = transfer(
        &OperationKind::Cast { kind: CastKind::ZeroExtend, value: ValueId(0), to: Type::Scalar(ScalarType::U64) },
        Some(ScalarType::U64),
        [Input { value: constant(ScalarType::U32, 17), ty: Some(ScalarType::U32) }; 3],
    );
    assert_eq!(result.values[0], constant(ScalarType::U64, 17));
}

#[test]
fn actual_v18_pointer_exposure_does_not_invent_uniformity() {
    let from = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let to = Type::pointer(Type::F32, AddressSpace::Generic, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(fe2o3_kernel_ir::Operation::effect_free(
        ValueDef::new(ValueId(1), to.clone()),
        OperationKind::Cast { kind: CastKind::PointerToGeneric, value: ValueId(0), to },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("generic-analysis");
    module.functions.push(Function::internal_helper(
        "expose", Signature::new(vec![from], vec![]), vec![ValueId(0)], vec![block],
    ));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 10_000_000);
    let (owner, _) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &module, StorageLayoutLimitsV1 { rows: 8, edges: 16, containment_depth: 8, object_bytes: 1024 }, &mut budget,
    ).unwrap();
    let report = crate::analyze_function(&owner.module().functions[0]);
    assert_eq!(report.value(ValueId(0)), crate::Variation::Varying);
    assert_eq!(report.value(ValueId(1)), crate::Variation::Varying);
}
