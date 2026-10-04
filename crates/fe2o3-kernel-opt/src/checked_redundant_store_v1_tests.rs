use super::*;
#[path = "checked_optimization_policy7_semantic_v1_tests.rs"]
mod policy7_semantic;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function,
    MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Signature, Terminator,
    Type, UnaryOp, ValueDef, ValueId,
};
const WORK: usize = 100_000_000;
const STORAGE: usize = 64 << 20;
const FLOOR: usize = 43;
pub(super) fn fixture() -> Module {
    let mut block = BasicBlock::new(BlockId(9));
    let store = || {
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(10),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        )
    };
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            Kind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        store(),
        Operation::effect_free(
            ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U32)),
            Kind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(1),
            },
        ),
        store(),
        store(),
        Operation::effect_free(
            ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U32)),
            Kind::Load {
                pointer: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(12),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(12)],
    });
    let mut module = Module::new("checked-redundant-store");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
            ],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module
}
pub(super) fn with_owner(module: Module, next: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(module);
    let floor = budget.storage();
    next(&owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
// A deliberately closed test interpreter over the actual Module, independent
// of the plan/rows. It checks dynamic values and counts real memory operations.
pub(super) fn evaluate(module: &Module, x: u32, y: u32) -> (u32, Vec<u32>, usize) {
    let mut values = [0_u32; 16];
    values[0] = x;
    values[1] = y;
    let mut slot = None;
    let mut global = Vec::new();
    let mut writes = 0;
    let block = &module.functions[0].body.as_ref().unwrap().blocks[0];
    for operation in &block.operations {
        match operation.kind {
            Kind::Alloca { .. } => {
                assert!(slot.is_none());
            }
            Kind::Store {
                pointer: ValueId(10),
                value,
                ..
            } => {
                slot = Some(values[value.0 as usize]);
                writes += 1;
            }
            Kind::Store {
                pointer: ValueId(2),
                value,
                ..
            } => global.push(values[value.0 as usize]),
            Kind::Load {
                pointer: ValueId(10),
                ..
            } => {
                values[operation.results[0].id.0 as usize] = slot.expect("preserved initialization")
            }
            Kind::Unary {
                op: UnaryOp::Not,
                operand,
            } => values[operation.results[0].id.0 as usize] = !values[operand.0 as usize],
            _ => panic!("test interpreter shape"),
        }
    }
    let Some(Terminator::Return { values: returned }) = &block.terminator else {
        panic!()
    };
    (values[returned[0].0 as usize], global, writes)
}

#[test]
fn real_optimizer_preserves_dynamic_values_initialization_and_global_effects() {
    with_owner(fixture(), |input, budget| {
        let before = input.canonical().canonical_bytes().to_vec();
        let output = optimize_checked_redundant_store_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert_eq!(output.rows().len(), 2);
        assert!(!output.grants_authority());
        assert_ne!(
            input.canonical().canonical_bytes(),
            output.output().canonical().canonical_bytes()
        );
        for x in [0, 1, u32::MAX, 1 << 31, 0xa55a_19e7] {
            for y in [0, u32::MAX, 0x7654_3210] {
                assert_eq!(evaluate(input.module(), x, y), (x, vec![x], 3));
                assert_eq!(evaluate(output.output().module(), x, y), (x, vec![x], 1));
            }
        }
        let rs = {
            let (relation, rs) = output.replay(budget).unwrap();
            budget.reserve_storage(rs.retained_storage()).unwrap();
            assert_eq!(relation.retained_operations(), output.retained_operations());
            rs
        };
        budget.release_storage(rs.retained_storage()).unwrap();
        assert_eq!(input.canonical().canonical_bytes(), before);
        let retained = output.retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn already_single_store_is_an_honest_byte_identical_noop() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .drain(3..5);
    with_owner(module, |input, budget| {
        let output = optimize_checked_redundant_store_v1(input, budget).unwrap();
        assert!(output.rows().is_empty());
        assert_eq!(
            input.canonical().canonical_bytes(),
            output.output().canonical().canonical_bytes()
        );
    });
}

#[test]
fn fresh_sessions_have_identical_bytes_rows_maps_work_and_storage() {
    let mut expected = None;
    for _ in 0..2 {
        with_owner(fixture(), |input, outer| {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(outer.storage()).unwrap();
            let output = optimize_checked_redundant_store_v1(input, &mut budget).unwrap();
            let actual = (
                output.output().canonical().canonical_bytes().to_vec(),
                output.rows().to_vec(),
                output.retained_operations().to_vec(),
                output.retained_storage(),
                budget.work(),
                budget.peak_storage(),
            );
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected);
            } else {
                expected = Some(actual);
            }
            assert_eq!(budget.storage(), outer.storage());
        });
    }
}

#[test]
fn exact_complete_service_budgets_and_partial_failures_preserve_source_and_floor() {
    with_owner(fixture(), |input, outer| {
        let original = input.canonical().canonical_bytes().to_vec();
        let floor = outer.storage();
        let mut work = Work::new(WORK);
        let mut measured = Budget::new(&mut work, STORAGE);
        measured.reserve_storage(floor).unwrap();
        let result = optimize_checked_redundant_store_v1(input, &mut measured).unwrap();
        let cost = measured.work();
        let peak = measured.peak_storage();
        assert!(peak > floor + result.retained_storage());
        drop(result);
        for (work_limit, storage_limit, ok) in [
            (cost, peak, true),
            (cost - 1, peak, false),
            (cost, peak - 1, false),
            (0, peak, false),
            (cost, floor, false),
            (cost / 2, peak, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            assert_eq!(
                optimize_checked_redundant_store_v1(input, &mut budget).is_ok(),
                ok
            );
            assert_eq!(budget.storage(), floor);
            assert_eq!(input.canonical().canonical_bytes(), original);
            if ok {
                assert_eq!(budget.work(), cost);
                assert_eq!(budget.peak_storage(), peak);
            }
        }
    });
}

#[test]
fn pure_scope_cleanup_drops_owned_stages_before_releasing_on_error_or_panic() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let dropped = std::cell::Cell::new(false);
    struct Flag<'a>(&'a std::cell::Cell<bool>);
    impl Drop for Flag<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for panic in [false, true] {
        dropped.set(false);
        let result: Result<(), Error> = scoped(&mut budget, |budget| {
            let _stage = Flag(&dropped);
            budget.reserve_storage(31)?;
            budget.charge_work(3)?;
            if panic {
                panic!("test owned stage");
            }
            Err(Error::Resource(Resource::Accounting))
        });
        assert!(result.is_err());
        assert!(dropped.get());
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(budget.work(), 6);
}

#[test]
fn replaced_ledger_does_not_receive_cleanup_or_new_reservations() {
    let mut first = Work::new(WORK);
    let mut replacement = Work::new(WORK);
    let mut budget = Budget::new(&mut first, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: Result<(), Error> = scoped(&mut budget, |budget| {
        *budget = Budget::new(&mut replacement, STORAGE);
        budget.reserve_storage(17)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 17);
}
