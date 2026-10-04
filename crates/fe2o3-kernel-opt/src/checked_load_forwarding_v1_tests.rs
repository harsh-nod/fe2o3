use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId,
};

pub(crate) const WORK: usize = 1_000_000_000;
pub(crate) const STORAGE: usize = 256 * 1024 * 1024;
const FLOOR: usize = 41;

#[path = "checked_load_forwarding_golden_v1_tests.rs"]
mod observation_goldens;

pub(crate) fn fixture() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(73));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(2),
                Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            ),
            Kind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        // Real unrelated effect: Policy4 deliberately forgets its Store seed.
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            Kind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), scalar.clone()),
            Kind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), scalar.clone()),
            Kind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let mut module = Module::new("checked-load-forwarding");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                scalar.clone(),
                Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![scalar],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

pub(crate) fn with_owner(module: Module, body: impl FnOnce(&Owner, &mut Budget<'_>)) {
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

// Closed fixture interpreter, independently observing both memory effects and
// the return value. This is not a compiler admission premise or general engine.
pub(crate) fn evaluate(owner: &Owner, argument: u32) -> (u32, u32, usize, usize) {
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let mut values = [None; 6];
    values[0] = Some(argument);
    let mut private = None;
    let mut global = None;
    let mut stores = 0;
    let mut loads = 0;
    for operation in &body.blocks[0].operations {
        match operation.kind {
            Kind::Alloca { .. } => assert_eq!(operation.results[0].id, ValueId(2)),
            Kind::Store { pointer, value, .. } => {
                let cell = if pointer == ValueId(2) {
                    &mut private
                } else {
                    assert_eq!(pointer, ValueId(1));
                    &mut global
                };
                *cell = Some(values[value.0 as usize].unwrap());
                stores += 1;
            }
            Kind::Load {
                pointer: ValueId(2),
                ..
            } => {
                values[operation.results[0].id.0 as usize] =
                    Some(private.expect("initialized scalar"));
                loads += 1;
            }
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs,
                rhs,
            } => {
                values[operation.results[0].id.0 as usize] =
                    Some(values[lhs.0 as usize].unwrap() | values[rhs.0 as usize].unwrap())
            }
            _ => panic!("outside fixture grammar"),
        }
    }
    let Some(Terminator::Return { values: returned }) = &body.blocks[0].terminator else {
        panic!("Return");
    };
    (
        values[returned[0].0 as usize].unwrap(),
        global.unwrap(),
        stores,
        loads,
    )
}

#[test]
fn owning_continuation_keeps_initialization_global_effect_and_first_read() {
    with_owner(fixture(), |input, budget| {
        let output = optimize_checked_load_forwarding_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert!(std::ptr::eq(input, output.input()));
        assert_eq!(output.rows().len(), 2);
        assert!(!output.grants_authority());
        let before = &input.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
        let after = &output.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations;
        assert_eq!(&before[..4], &after[..4]);
        for value in [0, 1, 17, 0x8000_0000, 0xaaaa_5555, u32::MAX] {
            assert_eq!(evaluate(input, value), (value, value, 2, 3));
            assert_eq!(evaluate(output.output(), value), (value, value, 2, 1));
        }
        let receipt = {
            let (checked, receipt) = output.replay(budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(std::ptr::eq(checked.output(), output.output()));
            assert_eq!(checked.rows(), output.rows());
            receipt
        };
        budget.release_storage(receipt.retained_storage()).unwrap();
        let retained = output.retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn owning_rule_is_noop_without_initialized_valid_read_premise() {
    for mode in 0..3 {
        let mut module = fixture();
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        match mode {
            0 => {
                operations.remove(1);
            }
            1 => {
                if let Kind::Load { access, .. } = &mut operations[3].kind {
                    access.alignment = 8;
                }
            }
            2 => {
                if let Kind::Load { access, .. } = &mut operations[3].kind {
                    access.volatile = true;
                }
            }
            _ => unreachable!(),
        }
        with_owner(module, |input, budget| {
            let output = optimize_checked_load_forwarding_v1(input, budget).unwrap();
            assert!(output.rows().is_empty());
            assert_eq!(
                input.canonical().canonical_bytes(),
                output.output().canonical().canonical_bytes()
            );
        });
    }
}

#[test]
fn owning_service_replays_allocation_overlap_at_exact_and_one_short_budgets() {
    with_owner(fixture(), |input, parent| {
        let floor = parent.storage();
        let mut work = Work::new(WORK);
        let mut measured = Budget::new(&mut work, STORAGE);
        measured.reserve_storage(floor).unwrap();
        let baseline = optimize_checked_load_forwarding_v1(input, &mut measured).unwrap();
        assert_eq!(measured.storage(), floor);
        let spent = measured.work();
        let peak = measured.peak_storage();
        assert!(peak > floor + baseline.retained_storage());
        for (work_limit, storage_limit, success) in [
            (spent, peak, true),
            (spent - 1, peak, false),
            (spent, peak - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = optimize_checked_load_forwarding_v1(input, &mut budget);
            assert_eq!(result.is_ok(), success);
            assert_eq!(budget.storage(), floor);
            if let Ok(output) = result {
                assert_eq!(
                    output.output().canonical().canonical_bytes(),
                    baseline.output().canonical().canonical_bytes()
                );
                assert_eq!(output.rows(), baseline.rows());
                assert_eq!(output.retained_storage(), baseline.retained_storage());
            }
        }
    });
}
