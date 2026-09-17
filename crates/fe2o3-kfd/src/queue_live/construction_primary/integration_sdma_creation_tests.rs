//! Creation settlement on the original constructor and its model loan.

// Exercise the production inline failure-custody representation unchanged.
#![allow(clippy::result_large_err)]

use super::*;
use crate::queue::live::sdma_creation::{
    ReturnedSdmaCreationV1, SdmaCreationContextV1, create_with_custody_v1,
};
use crate::sdma::retained_release::fixture::{
    add_creation_attempt, cleanup_creation_set, creation_failure, creation_observation,
    creation_profile, directional_with_ids,
};
use std::cell::Cell;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct CreationParent {
    parent: Parent,
    trace: Rc<RefCell<Trace>>,
    skip_callback: bool,
    regress_reclaim: bool,
    final_poison_panic: bool,
    secondary_drops: Arc<AtomicUsize>,
    final_poison_roots: Vec<bool>,
}

impl CreationParent {
    fn new(dispatch: bool) -> Self {
        let (parent, trace, _) = constructed(dispatch);
        Self {
            parent,
            trace,
            skip_callback: false,
            regress_reclaim: false,
            final_poison_panic: false,
            secondary_drops: Arc::new(AtomicUsize::new(0)),
            final_poison_roots: Vec::new(),
        }
    }

    fn fault(&mut self, name: &'static str, panic: bool) {
        self.trace.borrow_mut().fault = Some((name, 1, panic));
    }

    fn assert_no_retry(&mut self) {
        let before = self.trace.borrow().calls.clone();
        let root = self.parent.sdma.as_ref().map(creation_observation);
        let engine = &self.parent.engine;
        let memory = engine
            .backend
            .session
            .retained_queue_memory_snapshot_v1(&engine.foundation);
        let loan = engine
            .backend
            .session
            .primary_loan_state_v1(&engine.foundation);
        let foundation_in_engine = engine.backend.foundation_in_engine;
        let poisoned = self.parent.poisoned;
        let poison_roots = self.final_poison_roots.clone();
        assert!(
            create_with_custody_v1(
                self,
                "retry",
                |_| -> Result<ReturnedSdmaCreationV1<()>, _> {
                    panic!("terminal creation cannot retry")
                }
            )
            .is_err()
        );
        assert_eq!(self.trace.borrow().calls, before);
        assert_eq!(self.parent.sdma.as_ref().map(creation_observation), root);
        let engine = &self.parent.engine;
        assert_eq!(
            engine
                .backend
                .session
                .retained_queue_memory_snapshot_v1(&engine.foundation),
            memory
        );
        assert_eq!(
            engine
                .backend
                .session
                .primary_loan_state_v1(&engine.foundation),
            loan
        );
        assert_eq!(engine.backend.foundation_in_engine, foundation_in_engine);
        assert_eq!(self.parent.poisoned, poisoned);
        assert_eq!(self.final_poison_roots, poison_roots);
    }
}

impl Drop for CreationParent {
    fn drop(&mut self) {
        if let Some(set) = &mut self.parent.sdma {
            cleanup_creation_set(set);
        }
    }
}

struct PanicDrop(Arc<AtomicUsize>);
impl Drop for PanicDrop {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("secondary payload destructor");
    }
}

impl SdmaCreationContextV1 for CreationParent {
    type Memory = Memory;

    fn require_vacant(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.parent.poisoned || self.parent.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "fixture creation not vacant/live",
            ));
        }
        Ok(())
    }

    fn with_memory_custody(
        &mut self,
        work: impl FnOnce(&mut Memory),
    ) -> Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>
    {
        execute_live_model_custody_v1(
            self,
            |f| {
                step("creation-loan")?;
                let engine = &mut f.parent.engine;
                let loan = engine
                    .backend
                    .session
                    .primary_loan(&mut engine.foundation)?;
                engine.backend.foundation_in_engine = false;
                Ok(loan)
            },
            |f| {
                if !f.skip_callback {
                    work(&mut f.parent.engine.backend.session);
                }
            },
            |f, loan| {
                step("creation-retake-before")?;
                let engine = &mut f.parent.engine;
                if f.regress_reclaim {
                    engine
                        .backend
                        .session
                        .primary_regress_loan_revision_v1(&loan);
                }
                engine
                    .backend
                    .session
                    .primary_reclaim(&mut engine.foundation, loan)?;
                engine.backend.foundation_in_engine = true;
                step("creation-retake-after")
            },
            |_| {},
        )
    }

    fn retained_slot(&mut self) -> &mut Option<Gfx942SdmaQueueSetV1> {
        &mut self.parent.sdma
    }

    fn poison(&mut self) {
        self.final_poison_roots.push(self.parent.sdma.is_some());
        self.parent.poison_release();
        if self.final_poison_panic {
            std::panic::panic_any(PanicDrop(self.secondary_drops.clone()));
        }
    }
}

#[test]
fn constructed_sdma_creation_success_preserves_owner_and_normal_teardown() {
    for dispatch in [false, true] {
        let mut f = CreationParent::new(dispatch);
        let local = f.trace.borrow().local_resources.clone();
        let key = f.parent.key;
        let mut before = None;
        let created = create_with_custody_v1(&mut f, "fixture success", |memory| {
            let owner = directional_with_ids(memory, key, 100);
            before = Some(creation_observation(&owner));
            Ok(ReturnedSdmaCreationV1::single(owner, 17_u32))
        })
        .unwrap();
        let (owner, output) = created.into_single();
        assert_eq!(output, 17);
        assert_eq!(creation_observation(&owner), before.unwrap());
        assert!(!f.parent.poisoned && f.parent.sdma.is_none());
        assert!(f.final_poison_roots.is_empty());
        f.parent.sdma = Some(owner);
        let mut release = PrimaryReleaseStateV1::<Fixture>::new();
        release.release_in_place(&mut f.parent).unwrap();
        assert!(release.complete);
        release.sdma.as_mut().unwrap().cleanup_local_mappings();
        drop(release);
        drop(f);
        assert_eq!(local.live(), (0, 0, 0));
    }
}

#[test]
fn constructed_sdma_creation_retake_faults_retain_exact_single_and_combined_owners() {
    for combined in [false, true] {
        for after in [false, true] {
            for panic in [false, true] {
                let mut f = CreationParent::new(true);
                let local = f.trace.borrow().local_resources.clone();
                let key = f.parent.key;
                let original = original_resource_ids(&f.parent);
                let signal = Memory::primary_token_identity(f.parent.signals.as_ref().unwrap());
                let dispatch = RetainedControlSnapshotV1::ordinary_owner_v1(
                    f.parent.dispatch.as_ref().unwrap(),
                );
                let drops = f.trace.borrow().drops;
                let point = if after {
                    "creation-retake-after"
                } else {
                    "creation-retake-before"
                };
                f.fault(point, panic);
                let mut primary = None;
                let mut secondary = None;
                let mut backing = None;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    create_with_custody_v1(&mut f, "fixture retake", |memory| {
                        let owner = directional_with_ids(memory, key, 100);
                        primary = Some(creation_observation(&owner).primary);
                        let created = if combined {
                            let second = directional_with_ids(memory, key, 200);
                            secondary = Some(creation_observation(&second).primary);
                            ReturnedSdmaCreationV1::combined(owner, second, ())
                        } else {
                            ReturnedSdmaCreationV1::single(owner, ())
                        };
                        backing = Some(memory.retained_cleanup_memory_snapshot_v1());
                        Ok(created)
                    })
                }));
                if panic {
                    assert_eq!(
                        result.err().unwrap().downcast_ref::<(&str, usize)>(),
                        Some(&(point, 1))
                    );
                } else {
                    let error = result.ok().unwrap().err().unwrap();
                    assert!(
                        matches!(error, ComputeAqlQueueSessionErrorV1::TerminalCreation { source, .. }
                        if matches!(*source, ComputeAqlQueueSessionErrorV1::Contract(name) if name == point))
                    );
                }
                let retained = f.parent.sdma.as_ref().unwrap();
                assert!(matches!(
                    retained,
                    Gfx942SdmaQueueSetV1::TerminalRetained { .. }
                ));
                let observed = creation_observation(retained);
                assert_eq!(observed.primary, primary.unwrap());
                if combined {
                    assert_eq!(observed.secondary, secondary);
                }
                assert!(observed.attempted.is_none());
                assert_eq!(
                    f.parent
                        .engine
                        .backend
                        .session
                        .retained_queue_memory_snapshot_v1(&f.parent.engine.foundation),
                    backing.unwrap()
                );
                assert_eq!(
                    f.parent
                        .engine
                        .backend
                        .session
                        .primary_loan_state_v1(&f.parent.engine.foundation)
                        .1
                        .is_some(),
                    !after
                );
                assert_eq!(f.parent.engine.backend.foundation_in_engine, after);
                assert_eq!(original_resource_ids(&f.parent), original);
                assert_eq!(
                    Memory::primary_token_identity(f.parent.signals.as_ref().unwrap()),
                    signal
                );
                dispatch.assert_restored_v1(
                    RetainedControlSnapshotV1::ordinary_owner_v1(
                        f.parent.dispatch.as_ref().unwrap(),
                    ),
                    true,
                );
                assert_eq!(f.trace.borrow().drops, drops);
                assert_eq!(f.final_poison_roots, [true]);
                f.assert_no_retry();
                drop(f);
                assert_eq!(local.live(), (0, 0, 0));
            }
        }
    }
}

#[test]
fn constructed_sdma_creation_real_reclaim_rejection_retains_exact_custody() {
    for combined in [false, true] {
        let mut f = CreationParent::new(true);
        let local = f.trace.borrow().local_resources.clone();
        let key = f.parent.key;
        let engine = &f.parent.engine;
        let opening = engine
            .backend
            .session
            .retained_queue_memory_snapshot_v1(&engine.foundation);
        let loan = engine
            .backend
            .session
            .primary_loan_state_v1(&engine.foundation);
        let resources = original_resource_ids(&f.parent);
        let mut primary = None;
        let mut secondary = None;
        let mut completed = None;
        f.regress_reclaim = true;
        let error = create_with_custody_v1(&mut f, "real reclaim rejection", |memory| {
            let owner = directional_with_ids(memory, key, 100);
            primary = Some(creation_observation(&owner).primary);
            let created = if combined {
                let second = directional_with_ids(memory, key, 200);
                secondary = Some(creation_observation(&second).primary);
                ReturnedSdmaCreationV1::combined(owner, second, ())
            } else {
                ReturnedSdmaCreationV1::single(owner, ())
            };
            completed = Some(memory.retained_cleanup_memory_snapshot_v1());
            Ok(created)
        })
        .err()
        .unwrap();
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::TerminalCreation { source, .. }
                if matches!(*source, ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::Model("fixture live foundation reclaim")
                ))
        ));
        let retained = creation_observation(f.parent.sdma.as_ref().unwrap());
        assert_eq!(retained.primary, primary.unwrap());
        if combined {
            assert_eq!(retained.secondary, secondary);
        }
        assert!(retained.attempted.is_none());
        let engine = &f.parent.engine;
        completed.unwrap().assert_after_retake_v1(
            engine
                .backend
                .session
                .retained_queue_memory_snapshot_v1(&engine.foundation),
            &opening,
            true,
        );
        assert_eq!(
            engine
                .backend
                .session
                .primary_loan_state_v1(&engine.foundation),
            (loan.0, Some(loan.2), loan.2 + 1)
        );
        assert!(!engine.backend.foundation_in_engine);
        assert_eq!(original_resource_ids(&f.parent), resources);
        assert!(f.parent.poisoned);
        assert_eq!(f.final_poison_roots, [true]);
        f.assert_no_retry();
        drop(f);
        assert_eq!(local.live(), (0, 0, 0));
    }
}

#[test]
fn constructed_sdma_creation_lower_failure_retains_attempted_owner_and_error_precedence() {
    for attempt in [None, Some(0), Some(1), Some(2)] {
        for closing in 0..3 {
            let mut f = CreationParent::new(false);
            let local = f.trace.borrow().local_resources.clone();
            let key = f.parent.key;
            if closing != 0 {
                f.fault("creation-retake-after", closing == 2);
            }
            let mut before = None;
            let mut backing = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                create_with_custody_v1(
                    &mut f,
                    "fixture lower",
                    |memory| -> Result<ReturnedSdmaCreationV1<()>, _> {
                        let owner = directional_with_ids(memory, key, 100);
                        let mut retained =
                            Gfx942SdmaQueueSetV1::retain_created_for_terminal(owner, None);
                        if let Some(attempt) = attempt {
                            add_creation_attempt(&mut retained, attempt);
                        }
                        before = Some(creation_observation(&retained));
                        backing = Some(memory.retained_cleanup_memory_snapshot_v1());
                        Err(creation_failure(Some(retained), true))
                    },
                )
            }));
            if closing == 2 {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, usize)>(),
                    Some(&("creation-retake-after", 1))
                );
            } else {
                let error = result.ok().unwrap().err().unwrap();
                assert!(
                    matches!(error, ComputeAqlQueueSessionErrorV1::TerminalCreation { source, .. }
                    if match *source {
                        ComputeAqlQueueSessionErrorV1::Contract("creation-retake-after") => closing == 1,
                        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Contract("injected creation failure")) => closing == 0,
                        _ => false,
                    })
                );
            }
            assert_eq!(
                creation_observation(f.parent.sdma.as_ref().unwrap()),
                before.unwrap()
            );
            assert_eq!(
                f.parent
                    .engine
                    .backend
                    .session
                    .retained_queue_memory_snapshot_v1(&f.parent.engine.foundation),
                backing.unwrap()
            );
            assert_eq!(f.final_poison_roots, [true]);
            f.assert_no_retry();
            drop(f);
            assert_eq!(local.live(), (0, 0, 0));
        }
    }
}

#[test]
fn constructed_sdma_creation_no_output_and_lower_panic_do_not_invent_custody() {
    for mode in 0..6 {
        let mut f = CreationParent::new(false);
        let local = f.trace.borrow().local_resources.clone();
        if mode < 2 {
            f.fault("creation-loan", mode == 1);
        }
        if mode == 2 {
            f.skip_callback = true;
        }
        if mode >= 4 {
            f.fault("creation-retake-before", mode == 5);
        }
        let calls = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            create_with_custody_v1(
                &mut f,
                "fixture no output",
                |_| -> Result<ReturnedSdmaCreationV1<()>, _> {
                    calls.set(calls.get() + 1);
                    std::panic::panic_any("creation-operation")
                },
            )
        }));
        assert_eq!(calls.get(), usize::from(mode >= 3));
        if mode == 1 {
            assert_eq!(
                result.err().unwrap().downcast_ref::<(&str, usize)>(),
                Some(&("creation-loan", 1))
            );
        } else if mode >= 3 {
            assert_eq!(
                result.err().unwrap().downcast_ref::<&str>(),
                Some(&"creation-operation")
            );
        } else {
            assert!(result.ok().unwrap().err().unwrap().is_terminal_creation());
        }
        assert!(f.parent.sdma.is_none());
        assert_eq!(f.final_poison_roots, [false]);
        f.assert_no_retry();
        drop(f);
        assert_eq!(local.live(), (0, 0, 0));
    }
}

#[test]
fn constructed_sdma_creation_retryable_failure_and_occupied_preflight_are_inert() {
    let mut f = CreationParent::new(false);
    let local = f.trace.borrow().local_resources.clone();
    let result = create_with_custody_v1(
        &mut f,
        "retryable",
        |_| -> Result<ReturnedSdmaCreationV1<()>, _> { Err(creation_failure(None, false)) },
    );
    assert!(matches!(
        result,
        Err(ComputeAqlQueueSessionErrorV1::Sdma(_))
    ));
    assert!(!f.parent.poisoned && f.parent.sdma.is_none());
    assert!(f.final_poison_roots.is_empty());
    let key = f.parent.key;
    let created = create_with_custody_v1(&mut f, "success", |memory| {
        Ok(ReturnedSdmaCreationV1::single(
            directional_with_ids(memory, key, 100),
            (),
        ))
    })
    .unwrap();
    f.parent.sdma = Some(created.into_single().0);
    f.assert_no_retry();
    assert!(!f.parent.poisoned);
    drop(f);
    assert_eq!(local.live(), (0, 0, 0));
}

#[test]
fn constructed_sdma_creation_every_profile_retains_its_original_roster() {
    for profile in 0..5 {
        let mut f = CreationParent::new(false);
        let local = f.trace.borrow().local_resources.clone();
        f.fault("creation-retake-after", false);
        let key = f.parent.key;
        let mut original = None;
        let result = create_with_custody_v1(&mut f, "profile retention", |memory| {
            let owner = creation_profile(memory, key, profile);
            original = Some(creation_observation(&owner).primary);
            Ok(ReturnedSdmaCreationV1::single(owner, ()))
        });
        assert!(result.err().unwrap().is_terminal_creation());
        assert_eq!(
            creation_observation(f.parent.sdma.as_ref().unwrap()).primary,
            original.unwrap()
        );
        assert_eq!(f.final_poison_roots, [true]);
        f.assert_no_retry();
        drop(f);
        assert_eq!(local.live(), (0, 0, 0));
    }
}

#[test]
fn constructed_sdma_creation_ownerless_failure_distinguishes_retryable_from_transport() {
    for terminal in [false, true] {
        for closing in 0..3 {
            let mut f = CreationParent::new(false);
            let local = f.trace.borrow().local_resources.clone();
            if closing != 0 {
                f.fault("creation-retake-before", closing == 2);
            }
            let calls = Cell::new(0);
            let result = catch_unwind(AssertUnwindSafe(|| {
                create_with_custody_v1(
                    &mut f,
                    "ownerless failure",
                    |_| -> Result<ReturnedSdmaCreationV1<()>, _> {
                        calls.set(calls.get() + 1);
                        Err(creation_failure(None, terminal))
                    },
                )
            }));
            assert_eq!(calls.get(), 1);
            let poisoned = terminal || closing != 0;
            assert_eq!(f.parent.poisoned, poisoned);
            assert!(f.parent.sdma.is_none());
            assert_eq!(
                f.final_poison_roots,
                if poisoned { vec![false] } else { vec![] }
            );
            if closing == 2 {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, usize)>(),
                    Some(&("creation-retake-before", 1))
                );
            } else {
                assert_eq!(
                    result.ok().unwrap().err().unwrap().is_terminal_creation(),
                    poisoned
                );
            }
            drop(f);
            assert_eq!(local.live(), (0, 0, 0));
        }
    }
}

#[test]
fn constructed_sdma_creation_first_panic_survives_retake_and_final_poison_panics() {
    for retake_panic in [false, true] {
        let mut f = CreationParent::new(false);
        f.final_poison_panic = true;
        f.fault("creation-retake-before", retake_panic);
        let result = catch_unwind(AssertUnwindSafe(|| {
            create_with_custody_v1(
                &mut f,
                "panic",
                |_| -> Result<ReturnedSdmaCreationV1<()>, _> {
                    std::panic::panic_any("original lower panic")
                },
            )
        }));
        assert_eq!(
            result.err().unwrap().downcast_ref::<&str>(),
            Some(&"original lower panic")
        );
        assert_eq!(f.secondary_drops.load(Ordering::SeqCst), 0);
        assert_eq!(f.final_poison_roots, [false]);
        assert!(f.parent.poisoned);
    }
}

#[test]
fn constructed_sdma_creation_roots_before_metadata_drop_and_preserves_retake_panic() {
    for retake_panic in [false, true] {
        let mut f = CreationParent::new(false);
        f.fault("creation-retake-before", retake_panic);
        f.final_poison_panic = true;
        let key = f.parent.key;
        let mut before = None;
        let metadata_drops = Arc::new(AtomicUsize::new(0));
        let result = catch_unwind(AssertUnwindSafe(|| {
            create_with_custody_v1(&mut f, "metadata", |memory| {
                let owner = directional_with_ids(memory, key, 100);
                before = Some(creation_observation(&owner).primary);
                Ok(ReturnedSdmaCreationV1::single(
                    owner,
                    PanicDrop(metadata_drops.clone()),
                ))
            })
        }));
        let payload = result.err().unwrap();
        if retake_panic {
            assert_eq!(
                payload.downcast_ref::<(&str, usize)>(),
                Some(&("creation-retake-before", 1))
            );
        } else {
            assert_eq!(
                payload.downcast_ref::<&str>(),
                Some(&"secondary payload destructor")
            );
        }
        assert_eq!(metadata_drops.load(Ordering::SeqCst), 1);
        assert_eq!(f.secondary_drops.load(Ordering::SeqCst), 0);
        assert_eq!(
            creation_observation(f.parent.sdma.as_ref().unwrap()).primary,
            before.unwrap()
        );
        assert_eq!(f.final_poison_roots, [true]);
        f.assert_no_retry();
    }
}

#[test]
fn sdma_creation_all_six_public_profiles_use_rooted_production_settlement() {
    let live = include_str!("../../queue_live.rs");
    let live = live.split("#[cfg(test)]\nmod tests").next().unwrap();
    let profiles = live
        .split("pub fn enable_sdma_copy_engine(")
        .nth(1)
        .unwrap()
        .split("pub fn allocate_sdma_host_buffer(")
        .next()
        .unwrap();
    assert_eq!(
        profiles
            .matches("with_sdma_queue_creation_custody_v1(")
            .count(),
        6
    );
    assert_eq!(
        profiles.matches("ReturnedSdmaCreationV1::single(").count(),
        5
    );
    assert_eq!(
        profiles
            .matches("ReturnedSdmaCreationV1::combined(")
            .count(),
        1
    );
    for name in [
        "enable_gfx942_directional_sdma_copy_engines(",
        "enable_gfx942_sdma_copy_engine_on_engine_index(",
    ] {
        let body = profiles
            .split(name)
            .nth(1)
            .unwrap()
            .split("\n    pub fn ")
            .next()
            .unwrap();
        assert!(body.find("self.sdma = Some(").unwrap() < body.find(".expect(\"created").unwrap());
    }
    let adapter = live
        .split("fn with_sdma_queue_creation_custody_v1<R>")
        .nth(1)
        .unwrap()
        .split("fn detach_persistent_dispatch_data_retaining_control_v1")
        .next()
        .unwrap();
    assert!(adapter.contains("sdma_creation::create_with_custody_v1(self, stage, operation)"));
    let implementation = include_str!("../sdma_creation.rs")
        .split("impl SdmaCreationContextV1 for ComputeAqlQueueSessionV1")
        .nth(1)
        .unwrap();
    assert!(
        implementation
            .contains("self.with_live_queue_memory_model_custody_with_poison(operation, |_| {})")
    );
    assert!(implementation.contains("&mut self.sdma"));
    assert!(implementation.contains("self.striped_sdma.is_some()"));
    assert!(implementation.contains("self.poison_terminal()"));
    assert!(implementation.contains("permanently_poison_process_global_kfd_runtime_gate_v1()"));
}
