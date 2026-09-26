//! Scaled admission through the same outer driver used by native construction.

use super::*;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};
use std::cell::Cell;

struct PanickingCapture<'a>(&'a Cell<usize>);

impl Drop for PanickingCapture<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        panic!("rejected initializer capture must remain retained");
    }
}

type ScaledScope = AuxiliaryConstructionScopeV1<1, Parent>;

fn scaled_scope(capacity: Gfx942FixedDispatchCapacityV1) -> (Box<ScaledScope>, Rc<RefCell<Trace>>) {
    let (memory, trace) = setup_memory_with_host_budget(2 << 20);
    let (primary, trace) = setup_with_memory(memory, trace);
    let (primary, result) = run(primary, QueueRingBackingV1::AqlSpecial, false);
    assert!(result.is_ok());
    let (_, [packet, _, _]) = recipe();
    let packets = [packet];
    let preparation = PrimaryPreparationSnapshotV1::packets(&packets);
    (
        Box::new(ScaledScope {
            parent: Parent {
                original: Some(Original {
                    primary,
                    lanes: Vec::new(),
                    sdma: None,
                    striped_sdma: None,
                    release: None,
                    data: Rc::new(RefCell::new(None)),
                    preparation: Rc::new(RefCell::new(preparation)),
                }),
                poisoned: false,
                faults: Faults::default(),
            },
            construction: AuxiliaryConstructionV1::with_capacity(packets, capacity),
            terminal_parent: None,
        }),
        trace,
    )
}

#[test]
fn scaled_auxiliary_preflight_retains_uninvoked_captures_without_parent_mutation() {
    for record_exhaustion in [false, true] {
        for retain_panics in [false, true] {
            let account = ResourceCreditAccountV1::new(
                ResourceVectorV1::ZERO.with(
                    ResourceKindV1::ControlResidentBytes,
                    if record_exhaustion { 4 << 20 } else { 0 },
                ),
                1,
            )
            .unwrap();
            let competing = record_exhaustion.then(|| {
                account
                    .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1))
                    .unwrap()
            });
            let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
            let (scope, trace) = scaled_scope(capacity);
            let (programs, _) = recipe();
            let before = scope
                .parent
                .original
                .as_ref()
                .unwrap()
                .primary
                .completed
                .as_ref()
                .unwrap()
                .engine
                .backend
                .session
                .observation();
            let usage = account.usage();
            let calls = trace.borrow().calls.clone();
            let dropped = Cell::new(0);
            let capture = PanickingCapture(&dropped);
            let retained = RefCell::new(None);
            let retain_count = Cell::new(0);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_auxiliary_construction_with_v1(
                    scope,
                    65_536,
                    &programs,
                    PreparedAuxiliaryComputeLaneSlotV1 {
                        index: 0,
                        generation: 1,
                        append: true,
                    },
                    move |_| {
                        let _ = &capture;
                        panic!("rejected initializer must not execute")
                    },
                    |scope| {
                        retain_count.set(retain_count.get() + 1);
                        *retained.borrow_mut() = Some(scope);
                        assert!(!retain_panics, "retention callback fault");
                    },
                )
            }));
            if retain_panics {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::HostAllocationCapacity { .. }
                    )))
                ));
            }
            assert_eq!(dropped.get(), 0);
            assert_eq!(retain_count.get(), 1);
            let retained = retained.into_inner().unwrap();
            assert!(retained.terminal_parent.is_none());
            assert!(!retained.parent.poisoned);
            assert!(retained.construction.preparation.is_none());
            assert!(retained.construction.data.is_none());
            assert_eq!(
                retained
                    .parent
                    .original
                    .as_ref()
                    .unwrap()
                    .primary
                    .completed
                    .as_ref()
                    .unwrap()
                    .engine
                    .backend
                    .session
                    .observation(),
                before
            );
            assert_eq!(trace.borrow().calls, calls);
            assert_eq!(account.usage(), usage);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            drop(retained);
            drop(competing);
            assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        }
    }
}

#[test]
fn scaled_auxiliary_post_entry_failures_retain_the_single_table_debit() {
    for generation_failure in [false, true] {
        for panic in [false, true] {
            let (capacity, account) = super::super::capacity_cases::capacity();
            let (mut scope, trace) = scaled_scope(capacity);
            if generation_failure {
                scope.construction.preparation_fault =
                    Some((PreparationStageV1::Generation, panic));
            } else {
                scope.parent.faults.loan = if panic {
                    Outcome::Panic
                } else {
                    Outcome::Error
                };
            }
            let (programs, _) = recipe();
            let retained = RefCell::new(None);
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_auxiliary_construction_with_v1(
                    scope,
                    65_536,
                    &programs,
                    PreparedAuxiliaryComputeLaneSlotV1 {
                        index: 0,
                        generation: 1,
                        append: true,
                    },
                    |memory| {
                        assert_eq!(account.usage().retained_records, 1);
                        Ok(memory.roster())
                    },
                    |scope| *retained.borrow_mut() = Some(scope),
                )
            }));
            assert_eq!(result.is_err(), panic);
            assert!(!matches!(result, Ok(Ok(_))));
            let retained = retained.into_inner().unwrap();
            assert!(retained.parent.poisoned && retained.parent.original.is_none());
            assert!(retained.terminal_parent.is_some());
            assert_eq!(
                retained.construction.preparation.is_some(),
                generation_failure
            );
            assert_eq!(account.usage().retained_records, 1);
            trace.borrow_mut().fault = None;
            drop(retained);
            assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        }
    }
}
