use super::*;
use fe2o3_kernel_ir::{
    Atomic, AtomicKind, Barrier, BarrierSemantics, MemoryOrdering, SynchronizationScope,
};

fn noop(module: Module) {
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert!(owned.selected_allocations().is_empty());
        assert_eq!(
            owned.output().canonical().canonical_bytes(),
            input.canonical().canonical_bytes()
        );
        assert!(
            owned
                .origins()
                .iter()
                .all(|row| row.input == row.output && row.kind == OriginKind::Retained)
        );
        replay(&owned, input, budget);
        release(owned, budget);
    });
}
fn global(module: &mut Module) {
    module.functions[0].signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(4));
}

#[test]
fn division_and_external_store_remain_exact_and_in_original_order() {
    let mut module = fixture(ScalarType::U32, None);
    global(&mut module);
    ops(&mut module).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    );
    if let Kind::Store { value, .. } = &mut ops(&mut module)[5].kind {
        *value = ValueId(21);
    }
    ops(&mut module).push(Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(4),
            value: ValueId(14),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        let old = &input.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
        let new = &owned.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations;
        assert_eq!(new.len(), 5);
        assert_eq!(new[2], old[4]);
        assert_eq!(new[4], old[8]);
        assert_eq!(
            owned.origins()[2],
            Origin {
                input: coord(4),
                output: coord(2),
                kind: OriginKind::Retained
            }
        );
        assert_eq!(
            owned.origins()[4],
            Origin {
                input: coord(8),
                output: coord(4),
                kind: OriginKind::Retained
            }
        );
        replay(&owned, input, budget);
        release(owned, budget);
    });
}

#[test]
fn two_interleaved_allocations_are_selected_without_cell_confusion() {
    let mut module = fixture(ScalarType::U32, None);
    let ty = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    ops(&mut module).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(20), pointer),
            Kind::Alloca {
                element: ty.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    );
    ops(&mut module).insert(
        6,
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(20),
                value: ValueId(0),
                access,
            },
        ),
    );
    ops(&mut module).insert(
        8,
        Operation::effect_free(
            ValueDef::new(ValueId(21), ty),
            Kind::Load {
                pointer: ValueId(20),
                access,
            },
        ),
    );
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.selected_allocations(), [coord(1), coord(4)]);
        assert_eq!(owned.origins().len(), 4);
        assert!(
            matches!(owned.origins()[2].kind,OriginKind::LoadCopy {allocation,stored_value:ValueId(0),..}if allocation==coord(4))
        );
        assert!(
            matches!(owned.origins()[3].kind,OriginKind::LoadCopy {allocation,stored_value:ValueId(1),..}if allocation==coord(1))
        );
        replay(&owned, input, budget);
        release(owned, budget);
    });
}

#[test]
fn dynamic_nested_uninitialized_volatile_and_misaligned_cells_remain_unchanged() {
    let mut module = fixture(ScalarType::U32, Some(2));
    if let Kind::GetElementPointer { offset, .. } = &mut ops(&mut module)[3].kind {
        *offset = ValueId(2);
    }
    noop(module);
    let mut module = fixture(ScalarType::U32, Some(2));
    ops(&mut module)[2] = constant(12, 2);
    noop(module);
    let mut module = fixture(ScalarType::U32, None);
    ops(&mut module).drain(4..6);
    noop(module);
    for alignment in [1, 8] {
        let mut module = fixture(ScalarType::U32, None);
        if let Kind::Load { access, .. } = &mut ops(&mut module)[6].kind {
            access.alignment = alignment;
        }
        noop(module);
    }
    let mut module = fixture(ScalarType::U32, None);
    if let Kind::Load { access, .. } = &mut ops(&mut module)[6].kind {
        access.volatile = true;
    }
    noop(module);
    let mut module = fixture(ScalarType::U32, None);
    let pointer = ops(&mut module)[1].results[0].ty.clone();
    ops(&mut module).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(20), pointer),
            Kind::GetElementPointer {
                base: ValueId(13),
                offset: ValueId(12),
            },
        ),
    );
    noop(module);
    for ty in [
        ScalarType::Bool,
        ScalarType::Index,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        noop(fixture(ty, None));
    }
    noop(fixture(ScalarType::U32, Some(0)));
    noop(fixture(ScalarType::U64, Some(u64::MAX)));
}

#[test]
fn cross_block_guarded_calls_barriers_atomic_and_unknown_access_are_closed() {
    let mut module = fixture(ScalarType::U32, None);
    let load = ops(&mut module).pop().unwrap();
    let mut second = BasicBlock::new(BlockId(72));
    second.operations.push(load);
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(72),
        arguments: vec![],
    });
    body.blocks.push(second);
    noop(module);
    let mut module = fixture(ScalarType::U32, None);
    let Kind::Load { pointer, access } = ops(&mut module)[6].kind else {
        unreachable!()
    };
    ops(&mut module)[6].kind = Kind::GuardedLoad {
        pointer,
        predicate: ValueId(3),
        fallback: ValueId(0),
        access,
    };
    noop(module);
    for mode in 0..4 {
        let mut module = fixture(ScalarType::U32, None);
        global(&mut module);
        let operation = match mode {
            0 => Operation::new(
                vec![],
                Kind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                }),
            ),
            1 => Operation::new(
                vec![],
                Kind::Atomic(Atomic {
                    kind: AtomicKind::Store,
                    pointer: ValueId(4),
                    value: Some(ValueId(0)),
                    compare: None,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                    scope: SynchronizationScope::Device,
                    ordering: MemoryOrdering::Relaxed,
                    failure_ordering: None,
                }),
            ),
            2 => Operation::effect_free(
                ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                Kind::Load {
                    pointer: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            _ => {
                module.functions.push(Function::declaration(
                    "opaque",
                    Signature::new(vec![], vec![]),
                ));
                Operation::new(
                    vec![],
                    Kind::Call {
                        callee: "opaque".into(),
                        arguments: vec![],
                    },
                )
            }
        };
        ops(&mut module).insert(5, operation);
        noop(module);
    }
}

#[test]
fn escaped_allocation_is_preserved_but_independent_candidate_still_promotes() {
    let mut module = fixture(ScalarType::U32, None);
    let mut second = module.functions[0].clone();
    second.id = "other".into();
    let pointer = ops(&mut module)[1].results[0].ty.clone();
    ops(&mut module).push(Operation::new(
        vec![],
        Kind::Call {
            callee: "escape".into(),
            arguments: vec![ValueId(13)],
        },
    ));
    module.functions.push(second);
    module.functions.push(Function::declaration(
        "escape",
        Signature::new(vec![pointer], vec![]),
    ));
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.selected_allocations(), [location(1, 0, 1)]);
        assert_eq!(
            owned.output().module().functions[0],
            input.module().functions[0]
        );
        assert_eq!(
            owned.output().module().functions[2],
            input.module().functions[2]
        );
        replay(&owned, input, budget);
        release(owned, budget);
    });
}

#[test]
fn zero_operation_and_declaration_only_inputs_are_real_noops() {
    let mut empty_body = fixture(ScalarType::U32, None);
    ops(&mut empty_body).clear();
    empty_body.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::Return {
            values: vec![ValueId(0)],
        });
    noop(empty_body);
    let mut declarations = Module::new("declarations-only");
    declarations.functions.push(Function::declaration(
        "external",
        Signature::new(vec![], vec![]),
    ));
    noop(declarations);
}
