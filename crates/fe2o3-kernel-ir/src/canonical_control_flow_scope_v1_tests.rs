use super::*;
use crate::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function as KirFunction, Module,
    Signature, Terminator, Type, ValueId,
};
use std::{cell::Cell, rc::Rc};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 37;
fn block(block: u32) -> Block {
    Block {
        function: Function(0),
        block,
    }
}
fn owner() -> Owner {
    let mut module = Module::new("scoped-cfg");
    let make = |id, terminator| {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(terminator);
        block
    };
    module.functions.push(KirFunction::internal_helper(
        "f",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![
            make(
                90,
                Terminator::ConditionalBranch {
                    condition: ValueId(0),
                    then_target: BlockId(10),
                    then_arguments: vec![],
                    else_target: BlockId(30),
                    else_arguments: vec![],
                },
            ),
            make(
                10,
                Terminator::Branch {
                    target: BlockId(20),
                    arguments: vec![],
                },
            ),
            make(
                30,
                Terminator::Branch {
                    target: BlockId(20),
                    arguments: vec![],
                },
            ),
            make(20, Terminator::Return { values: vec![] }),
            make(
                99,
                Terminator::Branch {
                    target: BlockId(99),
                    arguments: vec![],
                },
            ),
        ],
    ));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget)
        .unwrap()
        .0
}
fn prepared(work: &mut Work, storage: usize) -> Budget<'_> {
    let mut budget = Budget::new(work, storage);
    budget.reserve_storage(FLOOR).unwrap();
    budget
}
fn run(owner: &Owner, work: usize, storage: usize) -> (Result<(), Error>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = prepared(&mut work, storage);
    let result = with_canonical_kir_control_flow_v1(
        owner,
        Function(0),
        Default::default(),
        &mut budget,
        |view, budget| {
            assert!(std::ptr::eq(view.owner(), owner));
            assert_eq!(view.function(), Function(0));
            assert!(view.dominates(block(0), block(3), budget)?);
            assert!(!view.dominates(block(1), block(3), budget)?);
            assert!(!view.is_reachable(block(4), budget)?);
            assert!(!view.dominates(block(4), block(4), budget)?);
            Ok(())
        },
    );
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn actual_stored_coordinates_and_disconnected_cycles_are_not_raw_id_or_reachability_authority() {
    let owner = owner();
    assert_eq!(run(&owner, LIMIT, LIMIT).0, Ok(()));
}
#[test]
fn exact_and_one_short_constructor_and_query_work_storage_preserve_the_floor() {
    let owner = owner();
    let (result, work, peak) = run(&owner, LIMIT, LIMIT);
    assert_eq!(result, Ok(()));
    assert_eq!(run(&owner, work, peak).0, Ok(()));
    assert!(matches!(
        run(&owner, work - 1, peak).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(&owner, work, peak - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    let mut work = Work::new(2);
    let mut budget = prepared(&mut work, LIMIT);
    assert!(matches!(
        with_canonical_kir_control_flow_v1(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |_, _| Ok::<_, Error>(())
        ),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn exact_constructor_only_limits_and_late_ignored_work_failure_are_preserved() {
    let owner = owner();
    let execute = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = prepared(&mut work, storage_limit);
        let result = with_canonical_kir_control_flow_v1(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |_, _| Ok::<_, Error>(()),
        );
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, peak) = execute(LIMIT, LIMIT);
    assert_eq!(result, Ok(()));
    assert_eq!(execute(work, peak).0, Ok(()));
    assert!(matches!(
        execute(work - 1, peak).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        execute(work, peak - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    let mut meter = Work::new(work);
    let mut budget = prepared(&mut meter, peak);
    let result = with_canonical_kir_control_flow_v1(
        &owner,
        Function(0),
        Default::default(),
        &mut budget,
        |view, budget| {
            assert!(matches!(
                view.is_reachable(block(0), budget),
                Err(Error::Resource(Resource::Work(_)))
            ));
            Ok::<_, Error>(())
        },
    );
    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn invalid_or_foreign_function_coordinates_latch_even_when_callback_ignores_them() {
    let owner = owner();
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let result = with_canonical_kir_control_flow_v1(
        &owner,
        Function(0),
        Default::default(),
        &mut budget,
        |view, budget| {
            let foreign = Block {
                function: Function(1),
                block: 0,
            };
            assert_eq!(
                view.is_reachable(foreign, budget),
                Err(Error::InvalidBlock(foreign))
            );
            let work = budget.work();
            assert_eq!(
                view.is_reachable(block(0), budget),
                Err(Error::InvalidBlock(foreign))
            );
            assert_eq!(budget.work(), work);
            Ok::<_, Error>(())
        },
    );
    assert!(matches!(result, Err(Error::InvalidBlock(_))));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(
        with_canonical_kir_control_flow_v1(
            &owner,
            Function(1),
            Default::default(),
            &mut budget,
            |_, _| Ok::<_, Error>(())
        ),
        Err(Error::InvalidFunction(Function(1)))
    );
}

struct Dropped(Rc<Cell<usize>>, bool);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        if self.1 {
            panic!("result drop");
        }
    }
}

#[test]
fn callback_errors_panics_and_rejected_result_destructors_do_not_skip_cleanup() {
    let owner = owner();
    for mode in 0..4 {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let dropped = Rc::new(Cell::new(0));
        let result = with_canonical_kir_control_flow_v1(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |_, budget| match mode {
                0 => Err(Error::InvalidFunction(Function(9))),
                1 => panic!("callback panic"),
                _ => {
                    budget.reserve_storage(17).unwrap();
                    Ok(Dropped(dropped.clone(), mode == 3))
                }
            },
        );
        assert!(result.is_err());
        assert_eq!(budget.storage(), FLOOR + if mode >= 2 { 17 } else { 0 });
        assert_eq!(dropped.get(), usize::from(mode >= 2));
        if mode >= 2 {
            budget.release_storage(17).unwrap();
        }
    }
}
#[test]
fn panic_payload_destructor_runs_only_after_owned_cfg_cleanup() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("payload destructor");
        }
    }
    let owner = owner();
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let dropped = Arc::new(AtomicUsize::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_canonical_kir_control_flow_v1(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |_, _| -> Result<(), Error> { std::panic::panic_any(Payload(dropped.clone())) },
        )
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
}
#[test]
fn original_payload_and_panicking_error_destructor_cannot_destroy_each_other_before_cleanup() {
    thread_local! {
        static ORIGINAL_DROPS: Cell<usize> = const { Cell::new(0) };
        static PANICKED_CONVERSIONS: Cell<usize> = const { Cell::new(0) };
        static SECONDARY_DROPS: Cell<usize> = const { Cell::new(0) };
    }
    struct OriginalPayload;
    impl Drop for OriginalPayload {
        fn drop(&mut self) {
            ORIGINAL_DROPS.with(|count| count.set(count.get() + 1));
            panic!("original payload destructor");
        }
    }
    struct SecondaryPayload;
    impl Drop for SecondaryPayload {
        fn drop(&mut self) {
            SECONDARY_DROPS.with(|count| count.set(count.get() + 1));
        }
    }
    struct HostileError(bool);
    impl From<Error> for HostileError {
        fn from(error: Error) -> Self {
            let panicked = matches!(error, Error::Panicked);
            if panicked {
                PANICKED_CONVERSIONS.with(|count| count.set(count.get() + 1));
            }
            Self(panicked)
        }
    }
    impl Drop for HostileError {
        fn drop(&mut self) {
            if self.0 {
                std::panic::panic_any(SecondaryPayload);
            }
        }
    }
    ORIGINAL_DROPS.with(|count| count.set(0));
    PANICKED_CONVERSIONS.with(|count| count.set(0));
    SECONDARY_DROPS.with(|count| count.set(0));
    let owner = owner();
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_canonical_kir_control_flow_v1(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |_, budget| -> Result<(), HostileError> {
                budget.reserve_storage(17).unwrap();
                std::panic::panic_any(OriginalPayload)
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), FLOOR + 17);
    assert_eq!(ORIGINAL_DROPS.with(Cell::get), 1);
    // Previously a premature From(Panicked) created an error whose rejected
    // destructor supplied a second payload, replacing/dropping the first early.
    // No generic error is now manufactured while either owned resource is live.
    assert_eq!(PANICKED_CONVERSIONS.with(Cell::get), 0);
    assert_eq!(SECONDARY_DROPS.with(Cell::get), 0);
    budget.release_storage(17).unwrap();
}

#[test]
fn caller_defined_error_conversion_may_panic_only_after_exact_cleanup() {
    struct PanickingConversion;
    impl From<Error> for PanickingConversion {
        fn from(_: Error) -> Self {
            panic!("caller-defined From");
        }
    }
    let owner = owner();
    for callback_panics in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_canonical_kir_control_flow_v1(
                &owner,
                Function(0),
                Default::default(),
                &mut budget,
                |_, budget| -> Result<(), PanickingConversion> {
                    if callback_panics {
                        panic!("original callback panic");
                    }
                    budget.reserve_storage(17).unwrap();
                    Ok(())
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(
            budget.storage(),
            FLOOR + if callback_panics { 0 } else { 17 }
        );
        if !callback_panics {
            budget.release_storage(17).unwrap();
        }
    }
}

#[test]
fn foreign_ledger_and_wrong_slot_are_never_charged_or_released() {
    let owner = owner();
    for wrong_slot in [false, true] {
        let mut original_work = Work::new(LIMIT);
        let mut foreign_work = Work::new(LIMIT);
        let mut budget = prepared(&mut original_work, LIMIT);
        let mut spare = prepared(&mut foreign_work, LIMIT);
        let mut retained = 0;
        let result = with_canonical_kir_control_flow_v1(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |view, budget| {
                retained = budget.storage() - FLOOR;
                std::mem::swap(budget, &mut spare);
                let target = if wrong_slot { &mut spare } else { &mut *budget };
                let before = (target.work(), target.storage());
                assert_eq!(
                    view.is_reachable(block(0), target),
                    Err(Resource::Accounting.into())
                );
                assert_eq!((target.work(), target.storage()), before);
                std::mem::swap(budget, &mut spare);
                Ok::<_, Error>(())
            },
        );
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), FLOOR + retained);
        assert_eq!(spare.storage(), FLOOR);
        budget.release_storage(retained).unwrap();
    }
}
#[test]
fn unreturned_foreign_ledger_on_success_error_and_panic_remains_untouched() {
    let owner = owner();
    for exit in 0..3 {
        let mut original_work = Work::new(LIMIT);
        let mut foreign_work = Work::new(LIMIT);
        let mut budget = prepared(&mut original_work, LIMIT);
        let mut spare = prepared(&mut foreign_work, LIMIT);
        let foreign_ledger = spare.work_ledger_identity_v1();
        let mut retained = 0;
        let result = with_canonical_kir_control_flow_v1(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |_, budget| {
                retained = budget.storage() - FLOOR;
                std::mem::swap(budget, &mut spare);
                match exit {
                    0 => Ok(()),
                    1 => Err(Error::InvalidFunction(Function(7))),
                    _ => panic!("foreign ledger callback"),
                }
            },
        );
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert_eq!((budget.work(), budget.storage()), (0, FLOOR));
        assert_eq!(spare.storage(), FLOOR + retained);
        spare.release_storage(retained).unwrap();
    }
}
#[test]
fn observed_undercut_remains_poisoned_after_restoration() {
    let owner = owner();
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let mut retained = 0;
    let result = with_canonical_kir_control_flow_v1(
        &owner,
        Function(0),
        Default::default(),
        &mut budget,
        |view, budget| {
            retained = budget.storage() - FLOOR;
            budget.release_storage(1).unwrap();
            assert_eq!(
                view.is_reachable(block(0), budget),
                Err(Resource::Accounting.into())
            );
            budget.reserve_storage(1).unwrap();
            Ok::<_, Error>(())
        },
    );
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert_eq!(budget.storage(), FLOOR + retained);
    budget.release_storage(retained).unwrap();
}
