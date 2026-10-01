use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, Constant, Function,
    IntrinsicOperation, LaunchDomain, LaunchExtent, MemoryAccess, Operation, OperationKind,
    ScalarType, Signature, Terminator, Type, ValueDef, derive_kernel_memory_obligations_for_launch,
};

pub(crate) fn fixture() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let op = |id, ty, kind| Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind);
    let mut entry = BasicBlock::new(BlockId(7));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(3, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(19),
        then_arguments: vec![],
        else_target: BlockId(23),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(19));
    yes.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(23));
    no.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("generic-exclusion");
    module.functions.push(Function::kernel_entry(
        "write_once_body",
        Signature::new(
            vec![
                Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        "write_once",
        "write_once_body",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

pub(crate) fn obligations(module: &Module) -> FormalMemoryObligations {
    let kernel = &module.kernels[0];
    derive_kernel_memory_obligations_for_launch(
        module,
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap()
    .obligations()
    .clone()
}

#[test]
fn exact_actual_access_exclusion_retains_complete_raw_conflict() {
    let module = fixture();
    let raw = obligations(&module);
    assert_eq!(raw.inter_invocation_conflicts().len(), 1);
    let original = raw.clone();
    let rows = derive_execution_discharges_v1(&module, &module.kernels[0], &raw).unwrap();
    assert_eq!(raw, original);
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    assert_eq!(row.conflict_ordinal(), 0);
    assert_eq!(row.allocation_parameter(), 0);
    assert_eq!(row.left(), FunctionOperationLocation::new(BlockId(19), 0));
    assert_eq!(row.left(), row.right());
    assert_eq!(row.left_witness(), row.right_witness());
    assert_eq!(row.left_witness().invocation(), 0);
    assert!(!row.grants_authority());
}

#[test]
fn non_singleton_and_false_edge_keep_original_conflict_refusal() {
    for false_edge in [false, true] {
        let mut module = fixture();
        if false_edge {
            let Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            } = module.functions[0].blocks[0].terminator.as_mut().unwrap()
            else {
                panic!()
            };
            std::mem::swap(then_target, else_target);
        } else {
            module.functions[0].blocks[0].operations[1].kind =
                OperationKind::Constant(Constant::Index(2));
        }
        let raw = obligations(&module);
        let error = derive_execution_discharges_v1(&module, &module.kernels[0], &raw).unwrap_err();
        let ProductionFormalMemoryErrorV1::InterInvocationConflicts { conflicts } = error else {
            panic!("wrong refusal")
        };
        assert_eq!(conflicts.as_ref(), raw.inter_invocation_conflicts());
    }
}

#[test]
fn unresolved_member_prevents_partial_discharge() {
    let mut module = fixture();
    let store = module.functions[0].blocks[1].operations[0].clone();
    module.functions[0].blocks[0].operations.push(store);
    let raw = obligations(&module);
    assert!(raw.inter_invocation_conflicts().len() > 1);
    assert!(matches!(
        derive_execution_discharges_v1(&module, &module.kernels[0], &raw),
        Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts { .. })
    ));
}

#[test]
fn empty_report_has_no_discharge_and_kernel_substitution_refuses() {
    let mut module = fixture();
    let raw = obligations(&module);
    let mut other = module.kernels[0].clone();
    other.id = fe2o3_kernel_ir::KernelId::new("other");
    assert!(derive_execution_discharges_v1(&module, &other, &raw).is_err());
    module.functions[0].blocks[1].operations.clear();
    let empty = obligations(&module);
    assert!(
        derive_execution_discharges_v1(&module, &module.kernels[0], &empty)
            .unwrap()
            .is_empty()
    );
}
