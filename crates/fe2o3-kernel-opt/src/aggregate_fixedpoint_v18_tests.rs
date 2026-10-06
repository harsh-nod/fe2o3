use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Constant, Function, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType,
    Signature, StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1,
    StorageOperationV1 as Storage, Terminator, Type, ValueDef, ValueId,
};

const WORK: usize = 1_000_000_000;
const SPACE: usize = 512 * 1024 * 1024;
const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 4,
    edges: 8,
    containment_depth: 4,
    object_bytes: 64,
};
fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn op(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn alloca() -> Operation {
    op(
        10,
        Type::pointer(
            Type::StorageObject(StorageLayoutIdV1(0)),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
        Kind::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(0)),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    )
}
fn write(value: u32) -> Operation {
    Operation::new(
        vec![],
        Kind::Storage(Storage::WriteValue {
            address: ValueId(10),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        }),
    )
}
fn read() -> Operation {
    op(
        20,
        scalar(),
        Kind::Storage(Storage::ReadValue {
            address: ValueId(10),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        }),
    )
}
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut b = BasicBlock::new(BlockId(id));
    b.operations = operations;
    b.terminator = Some(terminator);
    b
}
fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}
fn ret(value: u32) -> Terminator {
    Terminator::Return {
        values: vec![ValueId(value)],
    }
}
fn conditional(a: u32, b: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(a),
        then_arguments: vec![],
        else_target: BlockId(b),
        else_arguments: vec![],
    }
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut m = Module::new("aggregate-policy12-cascade");
    m.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    m.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![scalar(), scalar(), Type::BOOL], vec![scalar()]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        blocks,
    ));
    m
}
fn fixture() -> Module {
    module(vec![block(
        100,
        vec![
            alloca(),
            write(0),
            read(),
            op(30, scalar(), Kind::Constant(Constant::U32(0))),
            op(
                21,
                scalar(),
                Kind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(20),
                    rhs: ValueId(30),
                },
            ),
        ],
        ret(21),
    )])
}
fn admit(m: &Module) -> (Owner, usize) {
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, SPACE);
    let (o, s) = Owner::from_module_ref_with_verification_budget_v18(m, LIMITS, &mut b).unwrap();
    assert_eq!(b.storage(), 0);
    (o, s.retained_storage())
}
fn exercise(m: Module, f: impl FnOnce(&Owner, &mut OwnedAggregateFixedpointV18, &mut Budget<'_>)) {
    let (input, bytes) = admit(&m);
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, SPACE);
    let floor = bytes + 37;
    b.reserve_storage(floor).unwrap();
    let mut output = optimize_owned_aggregate_fixedpoint_v18(&input, LIMITS, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    let retained = output.retained_storage();
    b.reserve_storage(retained).unwrap();
    output.replay_against(&input, &mut b).unwrap();
    f(&input, &mut output, &mut b);
    drop(output);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), floor);
}
#[test]
fn aggregate_policy12_runs_real_scalar_aggregate_scalar_cascade_to_terminal_round() {
    exercise(fixture(), |input, out, _| {
        assert_eq!(out.policy_version(), 12);
        assert_eq!(out.graph_schema(), 18);
        assert!(!out.grants_authority());
        assert_eq!(out.input_audit_bytes(), input.canonical_bytes());
        assert_eq!(out.rounds().len(), 3);
        assert_eq!(
            out.rounds().iter().map(|r| r.changed()).collect::<Vec<_>>(),
            [true, true, false]
        );
        assert_eq!(out.rounds()[0].aggregate().promoted_allocations(), 1);
        for r in out.rounds() {
            assert_eq!(r.scalar().execution().policy_version(), 11);
        }
        let b = &out.owner().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0];
        assert!(b.operations.is_empty());
        assert_eq!(b.terminator, Some(ret(0)));
    });
}
#[test]
fn aggregate_policy12_preserves_dynamic_diamond_phi_and_multiple_functions() {
    let mut m = module(vec![
        block(100, vec![alloca()], conditional(101, 102)),
        block(101, vec![write(0)], branch(103)),
        block(102, vec![write(1)], branch(103)),
        block(103, vec![read()], ret(20)),
    ]);
    let mut second = m.functions[0].clone();
    second.id = "second".into();
    m.functions.push(second);
    exercise(m, |_, out, _| {
        assert_eq!(out.rounds()[0].aggregate().promoted_allocations(), 2);
        assert_eq!(out.owner().module().functions.len(), 2);
        for f in &out.owner().module().functions {
            let b = &f.body.as_ref().unwrap().blocks;
            assert_eq!(b[3].parameters.len(), 1);
            assert!(
                matches!(&b[1].terminator, Some(Terminator::Branch { arguments, .. }) if arguments == &[ValueId(0)])
            );
            assert!(
                matches!(&b[2].terminator, Some(Terminator::Branch { arguments, .. }) if arguments == &[ValueId(1)])
            );
            assert_eq!(b[3].terminator, Some(ret(b[3].parameters[0].id.0)));
        }
    });
}
#[test]
fn aggregate_policy12_loop_local_allocation_kill_prevents_previous_iteration_initialization() {
    let m = module(vec![
        block(100, vec![], branch(101)),
        block(101, vec![alloca(), read()], conditional(102, 103)),
        block(102, vec![write(0)], branch(101)),
        block(103, vec![], ret(20)),
    ]);
    exercise(m, |_, out, _| {
        assert!(
            out.rounds()
                .iter()
                .all(|r| r.aggregate().promoted_allocations() == 0)
        );
        assert!(
            out.owner().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|b| &b.operations)
                .any(|o| matches!(o.kind, Kind::Alloca { .. }))
        );
    });
}
#[test]
fn aggregate_policy12_requires_complete_terminal_round_without_changing_policy11() {
    let (input, bytes) = admit(&fixture());
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, SPACE);
    let floor = bytes + 37;
    b.reserve_storage(floor).unwrap();
    assert!(matches!(
        run(&input, LIMITS, 1, &mut b),
        Err(Error::RoundLimit {
            completed: 1,
            limit: 1
        })
    ));
    assert_eq!(b.storage(), floor);
    let old = optimize_neutral_kernel_ir_mixed_fixedpoint_v18(&input, LIMITS, &mut b).unwrap();
    assert_eq!(old.execution().policy_version(), 11);
    assert!(
        old.owner().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations
            .iter()
            .any(|o| matches!(o.kind, Kind::Alloca { .. }))
    );
}
#[test]
fn aggregate_policy12_replay_rejects_full_witness_and_terminal_claim_tampering() {
    exercise(fixture(), |input, out, budget| {
        let last = out.canonical.len() - 1;
        out.canonical[last] ^= 1;
        assert!(matches!(
            out.replay_against(input, budget),
            Err(Error::Inconsistent("full actual Policy12 witness bytes"))
        ));
        out.canonical[last] ^= 1;
        out.replay_against(input, budget).unwrap();
        out.rounds.last_mut().unwrap().changed = true;
        assert!(matches!(
            out.replay_against(input, budget),
            Err(Error::Inconsistent(
                "every nonterminal round changed and full terminal round unchanged"
            ))
        ));
        out.rounds.last_mut().unwrap().changed = false;
        out.replay_against(input, budget).unwrap();
    });
}
fn resource(mut e: &(dyn std::error::Error + 'static)) -> Option<Resource> {
    loop {
        if let Some(e) = e.downcast_ref::<Resource>() {
            return Some(*e);
        }
        e = e.source()?;
    }
}
#[test]
fn aggregate_policy12_exact_and_one_short_work_storage_restore_caller_floor() {
    let (input, bytes) = admit(&fixture());
    let floor = bytes + 37;
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, SPACE);
    b.reserve_storage(floor).unwrap();
    let out = optimize_owned_aggregate_fixedpoint_v18(&input, LIMITS, &mut b).unwrap();
    let used = b.work();
    let peak = b.peak_storage();
    drop(out);
    assert_eq!(b.storage(), floor);
    for (work, storage, expected) in [(used, peak, 0), (used - 1, peak, 1), (used, peak - 1, 2)] {
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, storage);
        b.reserve_storage(floor).unwrap();
        match (
            expected,
            optimize_owned_aggregate_fixedpoint_v18(&input, LIMITS, &mut b),
        ) {
            (0, Ok(out)) => drop(out),
            (1, Err(e)) => assert!(matches!(resource(&e), Some(Resource::Work(_)))),
            (2, Err(e)) => assert!(matches!(resource(&e), Some(Resource::Storage(_)))),
            _ => panic!("exact closed-policy resource boundary"),
        }
        assert_eq!(b.storage(), floor);
    }
}
