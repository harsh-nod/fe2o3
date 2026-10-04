use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const FLOOR: usize = 31;
fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn fixture() -> Module {
    let mut block = BasicBlock::new(BlockId(73));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(1),
                Type::pointer(scalar(), AddressSpace::Private, AccessMode::ReadWrite),
            ),
            Kind::Alloca {
                element: scalar(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar()),
            Kind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar()),
            Kind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut module = Module::new("checked-private-forwarding");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![scalar()], vec![scalar()]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}
fn with_owner(module: Module, body: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (input, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    drop(module);
    let floor = budget.storage();
    body(&input, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(input);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

// A deliberately closed test oracle, not a production interpreter or proof.
// It executes the real fixture's allocation, stores, loads and integer identity.
fn evaluate(module: &Module, argument: u32) -> (u32, u32, usize, usize) {
    let body = module.functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    let mut values = [None; 4];
    values[0] = Some(argument);
    let mut cell = None;
    let mut stores = 0;
    let mut loads = 0;
    for operation in &body.blocks[0].operations {
        match operation.kind {
            Kind::Alloca {
                count: None,
                address_space: AddressSpace::Private,
                ..
            } => {
                assert_eq!(operation.results[0].id, ValueId(1));
                assert!(cell.is_none());
            }
            Kind::Store {
                pointer: ValueId(1),
                value,
                ..
            } => {
                cell = Some(values[value.0 as usize].unwrap());
                stores += 1;
            }
            Kind::Load {
                pointer: ValueId(1),
                ..
            } => {
                values[operation.results[0].id.0 as usize] = Some(cell.expect("initialized read"));
                loads += 1;
            }
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs,
                rhs,
            } => {
                values[operation.results[0].id.0 as usize] =
                    Some(values[lhs.0 as usize].unwrap() | values[rhs.0 as usize].unwrap());
            }
            _ => panic!("outside the test oracle's exact closed subset"),
        }
    }
    let Some(Terminator::Return { values: returned }) = &body.blocks[0].terminator else {
        panic!("not a return")
    };
    (
        values[returned[0].0 as usize].unwrap(),
        cell.unwrap(),
        stores,
        loads,
    )
}

#[test]
fn actual_optimizer_preserves_values_and_store_effects_while_eliminating_two_private_loads() {
    with_owner(fixture(), |input, budget| {
        let output = optimize_checked_store_forwarding_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert!(std::ptr::eq(output.input(), input));
        assert_eq!(output.rows().len(), 2);
        assert!(!output.grants_authority());
        assert_ne!(
            input.canonical().canonical_bytes(),
            output.output().canonical().canonical_bytes()
        );
        for argument in [0, 1, 17, 0x8000_0000, 0xaaaa_5555, u32::MAX] {
            let before = evaluate(input.module(), argument);
            let after = evaluate(output.output().module(), argument);
            assert_eq!(before, (argument, argument, 1, 2));
            assert_eq!(after, (argument, argument, 1, 0));
        }
        let (checked, receipt) = output.replay(budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(std::ptr::eq(checked.output(), output.output()));
        assert_eq!(checked.rows(), output.rows());
        drop(checked);
        budget.release_storage(receipt.retained_storage()).unwrap();
        let storage = output.retained_storage();
        drop(output);
        budget.release_storage(storage).unwrap();
    });
}

#[test]
fn deterministic_output_records_work_and_storage_do_not_depend_on_a_previous_execution() {
    with_owner(fixture(), |input, parent| {
        let floor = parent.storage();
        let mut first_work = Work::new(WORK);
        let mut first = Budget::new(&mut first_work, STORAGE);
        first.reserve_storage(floor).unwrap();
        let a = optimize_checked_store_forwarding_v1(input, &mut first).unwrap();
        assert_eq!(first.storage(), floor);
        let mut second_work = Work::new(WORK);
        let mut second = Budget::new(&mut second_work, STORAGE);
        second.reserve_storage(floor).unwrap();
        let b = optimize_checked_store_forwarding_v1(input, &mut second).unwrap();
        assert_eq!(second.storage(), floor);
        assert_eq!(
            a.output().canonical().canonical_bytes(),
            b.output().canonical().canonical_bytes()
        );
        assert_eq!(a.rows(), b.rows());
        assert_eq!(a.retained_storage(), b.retained_storage());
        assert_eq!(first.work(), second.work());
        assert_eq!(first.peak_storage(), second.peak_storage());
        assert!(first.peak_storage() > floor + a.retained_storage());
    });
}

#[test]
fn unknown_call_barrier_remains_a_real_call_and_both_loads_survive() {
    let mut input = fixture();
    input.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            2,
            Operation::new(
                vec![],
                Kind::Call {
                    callee: "opaque".into(),
                    arguments: vec![],
                },
            ),
        );
    input.functions.push(Function::declaration(
        "opaque",
        Signature::new(vec![], vec![]),
    ));
    with_owner(input, |input, budget| {
        let output = optimize_checked_store_forwarding_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert!(output.rows().is_empty());
        assert_eq!(
            input.canonical().canonical_bytes(),
            output.output().canonical().canonical_bytes()
        );
        let storage = output.retained_storage();
        drop(output);
        budget.release_storage(storage).unwrap();
    });
}

#[test]
fn no_preceding_store_cannot_be_minted_into_an_initialized_read() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(1);
    with_owner(module, |input, budget| {
        let output = optimize_checked_store_forwarding_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert!(output.rows().is_empty());
        assert_eq!(
            input.canonical().canonical_bytes(),
            output.output().canonical().canonical_bytes()
        );
        let storage = output.retained_storage();
        drop(output);
        budget.release_storage(storage).unwrap();
    });
}

#[test]
fn exact_first_work_and_storage_denials_restore_floor_and_leave_the_source_untouched() {
    with_owner(fixture(), |input, parent| {
        let floor = parent.storage();
        let header = size_of::<CheckedStoreForwardingOutputV1<'_>>()
            - size_of::<Owner>()
            - size_of::<CanonicalKirAppliedStoreForwardingV1<'_>>();
        assert!(header > 0);
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            optimize_checked_store_forwarding_v1(input, &mut budget),
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.work(), 0);
        assert_eq!(work.failed_work(), Some(1));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor + header - 1);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            optimize_checked_store_forwarding_v1(input, &mut budget),
            Err(Error::Resource(Resource::Storage { .. }))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.work(), 1);
        assert!(matches!(
            input.module().functions[0].body.as_ref().unwrap().blocks[0].operations[2].kind,
            Kind::Load { .. }
        ));
    });
}

#[test]
fn independent_output_replay_rejects_width_changes_before_any_checked_result() {
    with_owner(fixture(), |input, budget| {
        let output = optimize_checked_store_forwarding_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        let (mut wrong, cs) = output
            .output()
            .copy_module_for_transformation_v12(budget)
            .unwrap();
        budget.reserve_storage(cs.retained_storage()).unwrap();
        wrong.functions[0].body.as_mut().unwrap().blocks[0].operations[2].results[0].ty =
            Type::Scalar(ScalarType::U64);
        // A wrong-width candidate fails fresh core verification, before a
        // replay can borrow it as an actual verified output owner.
        assert!(matches!(
            Owner::from_module_ref_with_verification_budget_v12(&wrong, budget),
            Err(CanonicalKernelIrReplayAdmissionErrorV12::Verification(_))
        ));
        drop(wrong);
        budget.release_storage(cs.retained_storage()).unwrap();
        let storage = output.retained_storage();
        drop(output);
        budget.release_storage(storage).unwrap();
    });
}
