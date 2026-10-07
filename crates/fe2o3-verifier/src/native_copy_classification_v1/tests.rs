#![cfg(test)]
//! Synthetic structured-F shapes only, never a recovered V5 or machine capture.

use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    ComparePredicate, Constant, Function, FunctionRole, IntrinsicOperation, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Operation, OperationKind as Op, ScalarType, Signature, Terminator,
    Type, ValueDef, ValueId as V, WorkgroupSize, encode_module_v12,
};

const FLOOR: usize = 317;
const MATCH: NativeCopyProgramClassificationV1 = NativeCopyProgramClassificationV1::GuardedU32 {
    load: [17, 14],
    store: [17, 15],
};

fn module() -> Module {
    let mut module = Module::new("copy-shape-only");
    let mut kernel = Kernel::new(
        "copy_u32",
        "copy_u32",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    let u32_ty = Type::Scalar(ScalarType::U32);
    let input = Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output = Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::WriteOnly);
    let in_ptr = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let out_ptr = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::WriteOnly);
    let mut block = BasicBlock::new(BlockId(17));
    for (index, (ty, kind)) in [
        (
            Type::INDEX,
            Op::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        (Type::INDEX, Op::SliceLength { slice: V(1000) }),
        (Type::INDEX, Op::SliceLength { slice: V(1001) }),
        (
            Type::BOOL,
            Op::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: V(10),
                rhs: V(11),
            },
        ),
        (
            Type::BOOL,
            Op::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: V(10),
                rhs: V(12),
            },
        ),
        (
            Type::BOOL,
            Op::Binary {
                op: BinaryOp::BitAnd,
                lhs: V(13),
                rhs: V(14),
            },
        ),
        (Type::INDEX, Op::Constant(Constant::Index(0))),
        (
            Type::INDEX,
            Op::Select {
                condition: V(13),
                true_value: V(10),
                false_value: V(16),
            },
        ),
        (
            Type::INDEX,
            Op::Select {
                condition: V(14),
                true_value: V(10),
                false_value: V(16),
            },
        ),
        (in_ptr.clone(), Op::SliceData { slice: V(1000) }),
        (out_ptr.clone(), Op::SliceData { slice: V(1001) }),
        (
            in_ptr,
            Op::GetElementPointer {
                base: V(19),
                offset: V(17),
            },
        ),
        (
            out_ptr,
            Op::GetElementPointer {
                base: V(20),
                offset: V(18),
            },
        ),
        (u32_ty.clone(), Op::Constant(Constant::U32(0))),
        (
            u32_ty,
            Op::GuardedLoad {
                pointer: V(21),
                predicate: V(13),
                fallback: V(23),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        block.operations.push(Operation::new(
            vec![ValueDef::new(V(10 + index as u32), ty)],
            kind,
        ));
    }
    block.operations.push(Operation::new(
        vec![],
        Op::GuardedStore {
            pointer: V(22),
            predicate: V(15),
            value: V(24),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::definition(
        "copy_u32",
        Signature::new(vec![input, output], vec![]),
        vec![V(1000), V(1001)],
        vec![block],
    );
    function.role = FunctionRole::KernelEntry;
    module.functions.push(function);
    module
}

fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn check(
    module: &Module,
    budget: &mut Budget<'_>,
) -> Result<NativeCopyProgramClassificationV1, Resource> {
    let bytes = encode_module_v12(module).unwrap();
    classify_prepaid(
        Profile::Gfx942,
        1,
        &[0],
        true,
        module,
        bytes.len(),
        FLOOR,
        budget,
    )
}

#[test]
fn exact_whole_copy_shape_is_an_accounted_inert_observation() {
    let module = module();
    let bytes = encode_module_v12(&module).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert_eq!(check(&module, &mut budget), Ok(MATCH));
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.work(), bytes.len() + FIXED_WORK);
    assert_eq!(budget.peak_storage(), FLOOR + SCRATCH);
}

#[test]
fn missing_owner_payment_or_resources_never_becomes_unsupported_success() {
    let module = module();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
    budget.reserve_storage(FLOOR - 1).unwrap();
    assert_eq!(check(&module, &mut budget), Err(Resource::Accounting));
    assert_eq!(budget.storage(), FLOOR - 1);
    assert_eq!(budget.work(), ENTRY_WORK);

    let mut work = Work::new(ENTRY_WORK);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(check(&module, &mut budget).is_err());
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.storage(), FLOOR);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH - 1);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(check(&module, &mut budget).is_err());
    assert!(budget.failed_storage().is_some());
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn target_root_order_and_formula_are_not_inferred_from_a_matching_leaf() {
    let module = module();
    for (profile, roots, order, formula) in [
        (Profile::Gfx950, 1, &[0][..], true),
        (Profile::Gfx942, 2, &[0, 1][..], true),
        (Profile::Gfx942, 0, &[][..], true),
        (Profile::Gfx942, 1, &[1][..], true),
        (Profile::Gfx942, 1, &[0, 0][..], true),
        (Profile::Gfx942, 1, &[0][..], false),
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
        budget.reserve_storage(FLOOR).unwrap();
        assert_eq!(
            classify_prepaid(
                profile,
                roots,
                order,
                formula,
                &module,
                1,
                FLOOR,
                &mut budget
            ),
            Ok(NativeCopyProgramClassificationV1::Unsupported)
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn wrong_value_missing_bounds_and_unguarded_effects_are_not_copy() {
    let changes = [
        (
            15,
            Op::GuardedStore {
                pointer: V(22),
                predicate: V(15),
                value: V(23),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            15,
            Op::GuardedStore {
                pointer: V(22),
                predicate: V(13),
                value: V(24),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            15,
            Op::GuardedStore {
                pointer: V(22),
                predicate: V(14),
                value: V(24),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            15,
            Op::Store {
                pointer: V(22),
                value: V(24),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            14,
            Op::Load {
                pointer: V(21),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            14,
            Op::GuardedLoad {
                pointer: V(21),
                predicate: V(14),
                fallback: V(23),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            14,
            Op::GuardedLoad {
                pointer: V(22),
                predicate: V(13),
                fallback: V(23),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            14,
            Op::GuardedLoad {
                pointer: V(21),
                predicate: V(13),
                fallback: V(16),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        (
            14,
            Op::GuardedLoad {
                pointer: V(21),
                predicate: V(13),
                fallback: V(23),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
        (
            5,
            Op::Binary {
                op: BinaryOp::BitOr,
                lhs: V(13),
                rhs: V(14),
            },
        ),
        (
            5,
            Op::Binary {
                op: BinaryOp::BitAnd,
                lhs: V(13),
                rhs: V(13),
            },
        ),
        (
            3,
            Op::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: V(11),
                rhs: V(10),
            },
        ),
        (
            11,
            Op::GetElementPointer {
                base: V(19),
                offset: V(10),
            },
        ),
        (
            11,
            Op::GetElementPointer {
                base: V(19),
                offset: V(18),
            },
        ),
        (
            12,
            Op::GetElementPointer {
                base: V(20),
                offset: V(17),
            },
        ),
        (
            7,
            Op::Select {
                condition: V(13),
                true_value: V(16),
                false_value: V(10),
            },
        ),
    ];
    for (index, replacement) in changes {
        let mut candidate = module();
        operations(&mut candidate)[index].kind = replacement;
        assert_eq!(
            shape::classify(&candidate),
            NativeCopyProgramClassificationV1::Unsupported,
            "changed operation {index}"
        );
    }
}

#[test]
fn duplicate_effects_ssa_aliases_and_unknown_operands_refuse() {
    for mode in 0..6 {
        let mut candidate = module();
        let ops = operations(&mut candidate);
        match mode {
            0 => {
                let mut extra = ops[14].clone();
                extra.results[0].id = V(99);
                ops.insert(15, extra);
            }
            1 => {
                ops.push(ops[15].clone());
            }
            2 => {
                ops[0].results[0].id = V(1000);
            }
            3 => {
                ops[14].results[0].id = V(23);
            }
            4 => {
                ops[14].results[0].ty = Type::Scalar(ScalarType::U64);
            }
            _ => {
                ops[1].kind = Op::SliceLength { slice: V(999) };
            }
        }
        assert_eq!(
            shape::classify(&candidate),
            NativeCopyProgramClassificationV1::Unsupported
        );
    }
}

#[test]
fn only_the_exact_two_argument_u32_domain_is_recognized() {
    for mode in 0..8 {
        let mut candidate = module();
        match mode {
            0 => {
                candidate.functions[0].signature.parameters[0] = Type::slice(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                )
            }
            1 => {
                candidate.functions[0].signature.parameters[0] = Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )
            }
            2 => {
                candidate.functions[0].signature.parameters[1] = Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )
            }
            3 => candidate.functions[0]
                .signature
                .parameters
                .push(Type::INDEX),
            4 => candidate.functions[0].body.as_mut().unwrap().parameters[1] = V(1000),
            5 => candidate.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1)),
            6 => candidate.kernels.push(candidate.kernels[0].clone()),
            _ => candidate.functions.push(candidate.functions[0].clone()),
        }
        assert_eq!(
            shape::classify(&candidate),
            NativeCopyProgramClassificationV1::Unsupported
        );
    }
}

#[test]
fn complete_straight_line_cfg_is_checked_and_hidden_or_cyclic_blocks_refuse() {
    let mut candidate = module();
    let body = candidate.functions[0].body.as_mut().unwrap();
    let mut end = BasicBlock::new(BlockId(91));
    end.operations = body.blocks[0].operations.split_off(15);
    end.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(91),
        arguments: vec![],
    });
    body.blocks.push(end);
    assert_eq!(
        shape::classify(&candidate),
        NativeCopyProgramClassificationV1::GuardedU32 {
            load: [17, 14],
            store: [91, 0],
        }
    );
    let mut hidden = candidate.clone();
    hidden.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(BasicBlock::new(BlockId(92)));
    assert_eq!(
        shape::classify(&hidden),
        NativeCopyProgramClassificationV1::Unsupported
    );
    let body = candidate.functions[0].body.as_mut().unwrap();
    body.blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![],
    });
    assert_eq!(
        shape::classify(&candidate),
        NativeCopyProgramClassificationV1::Unsupported
    );
}

#[test]
fn complete_operation_and_block_caps_are_fail_closed() {
    let mut candidate = module();
    for index in 0..65 {
        operations(&mut candidate).push(Operation::new(
            vec![ValueDef::new(V(2000 + index), Type::INDEX)],
            Op::Constant(Constant::Index(0)),
        ));
    }
    assert_eq!(
        shape::classify(&candidate),
        NativeCopyProgramClassificationV1::Unsupported
    );
    let mut candidate = module();
    for index in 0..65 {
        candidate.functions[0]
            .body
            .as_mut()
            .unwrap()
            .blocks
            .push(BasicBlock::new(BlockId(2000 + index)));
    }
    assert_eq!(
        shape::classify(&candidate),
        NativeCopyProgramClassificationV1::Unsupported
    );
}

#[test]
fn absent_effects_and_extra_capability_declarations_are_not_silently_ignored() {
    use fe2o3_kernel_ir::TargetCapability;
    for mode in 0..5 {
        let mut candidate = module();
        match mode {
            0 => {
                operations(&mut candidate).pop();
            }
            1 => {
                operations(&mut candidate).remove(14);
            }
            2 => {
                candidate
                    .required_capabilities
                    .insert(TargetCapability::WorkgroupMemory);
            }
            3 => {
                candidate.kernels[0]
                    .required_capabilities
                    .insert(TargetCapability::WorkgroupMemory);
            }
            _ => {
                candidate.functions[0]
                    .required_capabilities
                    .insert(TargetCapability::WorkgroupMemory);
            }
        }
        assert_eq!(
            shape::classify(&candidate),
            NativeCopyProgramClassificationV1::Unsupported
        );
    }
}
