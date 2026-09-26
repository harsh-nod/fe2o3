use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Constant, Function, MemoryAccess, Module, Operation, OperationKind as Op, ScalarType,
    Signature, StorageLayoutIdV1 as Id, StorageLayoutKindV1, StorageLayoutV1,
    StorageOperationV1 as Storage, Terminator, Type, ValueDef, ValueId,
};

pub(super) const WORK: usize = 10_000_000_000_000;
pub(super) const SPACE: usize = 2_000_000_000;
pub(super) const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};
pub(super) fn source() -> Module {
    let u32_type = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(42));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(0),
                Type::pointer(
                    Type::StorageObject(Id(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            Op::Alloca {
                element: Type::StorageObject(Id(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), u32_type.clone()),
            Op::Constant(Constant::U32(2)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), u32_type.clone()),
            Op::Constant(Constant::U32(3)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), u32_type),
            Op::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            Op::Storage(Storage::WriteValue {
                address: ValueId(0),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("kernel-opt-v18");
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}
pub(super) fn input(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v18(module, LIMITS, &mut budget).unwrap();
    (owner, storage.retained_storage())
}

#[test]
fn fixed_kernel_opt_orchestration_checks_actual_storage_aware_output() {
    let (input, storage) = input(&source());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(17 + storage).unwrap();
    let checked = optimize_checked_canonical_kernel_ir_v18(&input, LIMITS, &mut budget).unwrap();
    assert_eq!(budget.storage(), 17 + storage);
    assert_ne!(checked.owner().canonical_bytes(), input.canonical_bytes());
    assert_eq!(
        checked.owner().module().storage_layouts,
        input.module().storage_layouts
    );
    assert_eq!(checked.report().passes().len(), 8);
    assert_eq!(checked.input_audit_bytes(), input.canonical_bytes());
    let operations = &checked.owner().module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations;
    assert!(
        operations
            .iter()
            .any(|op| matches!(op.kind, Op::Constant(Constant::U32(5))))
    );
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, Op::Storage(Storage::WriteValue { .. })))
            .count(),
        1
    );
    assert!(!checked.grants_authority());
}
