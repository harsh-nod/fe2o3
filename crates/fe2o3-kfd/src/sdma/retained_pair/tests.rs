use super::*;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Script {
    terminal: bool,
    trace: Vec<&'static str>,
    quarantined: [bool; 3],
}

impl Custody for Script {
    fn terminal(&self) -> bool { self.terminal }
    fn quarantine(&mut self) {
        self.terminal = true;
        self.quarantined = [true; 3];
        self.trace.push("quarantine");
    }
}

fn ticket() -> Gfx942SdmaCopyTicketV1 {
    Gfx942SdmaCopyTicketV1 {
        owner: super::super::tests::queue_key(7, 11, 13),
        queue_id: 17, slot: 3, generation: 9,
    }
}

fn completed() -> Gfx942XgmiCompletedCopyV1 {
    Gfx942XgmiCompletedCopyV1 {
        source: crate::shared_memory::xgmi_mapping_for_sdma_test(11),
        destination: crate::shared_memory::xgmi_mapping_for_sdma_test(12),
        copy_bytes: 4096,
    }
}

fn owner() -> Gfx942SdmaQueueOwnerV1 {
    let mut owner = id_only_sdma_owner_for_auxiliary_construction_test_v1(ticket().owner, 17, 2);
    owner.records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    owner.xgmi_records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    owner.persistent_window_slots.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    owner.persistent_window_records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    owner
}

#[test]
fn retained_pair_profile_policy_is_separate_and_digest_pinned() {
    use std::fmt::Write as _;
    let mut digest = String::new();
    for byte in Sha256::digest(GFX942_XGMI_RETAINED_PAIR_POLICY_V1) {
        write!(digest, "{byte:02x}").unwrap();
    }
    assert_eq!(digest, GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1);
    assert!(GFX942_XGMI_RETAINED_PAIR_POLICY_V1.starts_with(&format!("profile={}\n", GFX942_XGMI_RETAINED_PAIR_PROFILE_V1)));
    assert!(GFX942_XGMI_RETAINED_PAIR_POLICY_V1.contains("not-runtime-module-attestation"));
    assert!(!GFX942_SDMA_COPY_MANIFEST_V1.contains(GFX942_XGMI_RETAINED_PAIR_PROFILE_V1));
    let _: for<'a> fn(&'a mut Gfx942NativeXgmiSdmaQueueV1,
        &'a mut SharedGttMemorySessionV1, &'a mut SharedGttMemorySessionV1,
        Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
    ) -> Result<Gfx942NativeXgmiSdmaRetainedPairV1<'a>, Gfx942SdmaErrorV1> =
        Gfx942NativeXgmiSdmaQueueV1::begin_ordinary_retained_pair_v1;
}

#[test]
fn retained_pair_guard_terminalizes_each_live_stage_error_and_panic_before_escape() {
    let stages = ["source-open", "peer-open", "native", "source-close", "peer-close"];
    for (index, stage) in stages.iter().copied().enumerate() {
        for panic in [false, true] {
            let mut script = Script::default();
            let outcome = catch_unwind(AssertUnwindSafe(|| run_operation(&mut script, |script| {
                for current in stages {
                    script.trace.push(current);
                    if current == stage {
                        if panic { std::panic::panic_any(stage); }
                        script.terminal = true;
                        return Err(Gfx942SdmaErrorV1::Contract(stage));
                    }
                }
                Ok(())
            })));
            if panic {
                assert_eq!(outcome.unwrap_err().downcast_ref::<&str>(), Some(&stage));
            } else {
                assert!(matches!(outcome.unwrap(), Err(Gfx942SdmaErrorV1::Contract(found)) if found == stage));
            }
            assert_eq!(&script.trace[..index + 1], &stages[..index + 1]);
            assert_eq!(script.trace.last(), Some(&"quarantine"));
            assert_eq!(script.quarantined, [true; 3]);
        }
    }
}

#[test]
fn retained_pair_fenced_timeout_and_pure_refusal_preserve_retryable_custody() {
    let mut script = Script::default();
    let expected = ticket();
    let result = run_operation(&mut script, |_| Err::<Vec<Gfx942XgmiCompletedCopyV1>, _>(
        Gfx942XgmiBatchWaitFailureV1::Retained {
            error: Gfx942SdmaErrorV1::Timeout, tickets: vec![expected],
        },
    ));
    let failure = Gfx942XgmiRetainedPairWaitFailureV1 { inner: result.err().unwrap() };
    assert!(matches!(failure.error(), Gfx942SdmaErrorV1::Timeout));
    assert_eq!(failure.into_retained_tickets().unwrap(), vec![expected]);
    assert!(!script.terminal);
    assert!(script.trace.is_empty());
    let failure: Result<(), Gfx942SdmaErrorV1> = run_operation(&mut script, |_| Err(Gfx942SdmaErrorV1::QueueFull));
    assert!(matches!(failure, Err(Gfx942SdmaErrorV1::QueueFull)));
    assert!(!script.terminal);
    assert!(run_operation(&mut script, |_| Ok::<(), Gfx942SdmaErrorV1>(())).is_ok());
}

#[test]
fn retained_diagnostic_errors_terminal_success_and_panics_never_publish_observations() {
    for scenario in ["error", "terminal-success", "panic", "success"] {
        let mut script = Script::default();
        let mut published = 0;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            run_diagnostic_operation(
                &mut script,
                17_u32,
                |script, measurement| {
                    assert_eq!(*measurement, 17);
                    match scenario {
                        "error" => Err(Gfx942SdmaErrorV1::Timeout),
                        "terminal-success" => {
                            script.terminal = true;
                            Ok(())
                        }
                        "panic" => std::panic::panic_any(53_u32),
                        _ => Ok(()),
                    }
                },
                |measurement| {
                    published += 1;
                    measurement
                },
            )
        }));
        match scenario {
            "success" => {
                assert_eq!(outcome.unwrap().unwrap(), ((), 17));
                assert_eq!(published, 1);
            }
            "error" => {
                assert!(matches!(outcome.unwrap(), Err(Gfx942SdmaErrorV1::Timeout)));
                assert_eq!(published, 0);
            }
            "terminal-success" => {
                assert!(outcome.unwrap().is_err());
                assert_eq!(published, 0);
                assert!(script.terminal);
            }
            "panic" => {
                assert_eq!(outcome.unwrap_err().downcast_ref::<u32>(), Some(&53));
                assert_eq!(published, 0);
                assert!(script.terminal);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn retained_pair_nonlossy_ticket_projection_preserves_exact_variant_custody() {
    let expected = ticket();
    let tickets = vec![expected];
    let ticket_storage = tickets.as_ptr();
    let failure = Gfx942XgmiRetainedPairWaitFailureV1 {
        inner: Gfx942XgmiBatchWaitFailureV1::Retained {
            error: Gfx942SdmaErrorV1::Timeout, tickets,
        },
    };
    let recovered = failure.try_into_retained_tickets().unwrap();
    assert_eq!(recovered, vec![expected]);
    assert_eq!(recovered.as_ptr(), ticket_storage);

    let copy = completed();
    let identities = (copy.source.lease().storage_identity(),
        copy.destination.lease().storage_identity());
    let completed = vec![copy];
    let completed_storage = completed.as_ptr();
    let message = String::from("original indeterminate fence");
    let message_storage = message.as_ptr();
    let failure = Gfx942XgmiRetainedPairWaitFailureV1 {
        inner: Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {
            error: Gfx942SdmaErrorV1::Doorbell(message), completed,
        },
    };
    let failure = failure.try_into_retained_tickets().unwrap_err();
    match &failure.inner {
        Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {
            error: Gfx942SdmaErrorV1::Doorbell(message), completed,
        } => {
            assert_eq!(message.as_ptr(), message_storage);
            assert_eq!(message, "original indeterminate fence");
            assert_eq!(completed.as_ptr(), completed_storage);
            assert_eq!(completed.len(), 1);
        }
        _ => panic!("projection lost the original failure variant"),
    }
    let (source, destination) = failure.into_indeterminate_mappings().unwrap().next().unwrap();
    assert_eq!((source.lease().storage_identity(), destination.lease().storage_identity()), identities);
}

#[test]
fn retained_pair_terminal_ok_never_mints_success_and_preserves_exact_owned_results() {
    let mut script = Script { terminal: true, ..Script::default() };
    let result = run_operation(&mut script, |_| Ok::<(), Gfx942SdmaErrorV1>(()));
    assert!(matches!(result, Err(Gfx942SdmaErrorV1::Contract(_))));
    let expected = ticket();
    let result = run_operation(&mut script, |_| Ok::<_, Gfx942XgmiBatchSubmissionFailureV1>(vec![expected]));
    match result {
        Err(Gfx942XgmiBatchSubmissionFailureV1::Retained { tickets, .. }) => assert_eq!(tickets, vec![expected]),
        _ => panic!("terminal submission did not retain tickets"),
    }
    let pair = completed();
    let identities = (pair.source.lease().storage_identity(), pair.destination.lease().storage_identity());
    let result = run_operation(&mut script, |_| Ok::<_, Gfx942XgmiBatchWaitFailureV1>(vec![pair]));
    let failure = Gfx942XgmiRetainedPairWaitFailureV1 { inner: result.err().unwrap() };
    let mut mappings = failure.into_indeterminate_mappings().unwrap();
    assert_eq!(mappings.len(), 1);
    let (source, destination) = mappings.next().unwrap();
    assert_eq!((source.lease().storage_identity(), destination.lease().storage_identity()), identities);
    assert_eq!(script.quarantined, [true; 3]);
}

#[test]
fn retained_pair_terminal_error_retains_first_cause() {
    let mut script = Script { terminal: true, ..Script::default() };
    let result = run_operation(&mut script, |_| Err::<(), _>(Gfx942SdmaErrorV1::Contract("first cause")));
    assert!(matches!(result, Err(Gfx942SdmaErrorV1::Contract("first cause"))));
    assert_eq!(script.trace, ["quarantine"]);
}

#[test]
fn retained_pair_conversion_panic_occurs_only_after_eager_quarantine() {
    struct PanickingConversion(Rc<RefCell<Vec<&'static str>>>);
    impl TerminalOutcome for PanickingConversion {
        fn refuse_terminal_success(self) -> Self {
            self.0.borrow_mut().push("conversion");
            panic!("conversion panic");
        }
    }
    struct SharedScript(Rc<RefCell<Vec<&'static str>>>);
    impl Custody for SharedScript {
        fn terminal(&self) -> bool { true }
        fn quarantine(&mut self) { self.0.borrow_mut().push("quarantine"); }
    }
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut script = SharedScript(trace.clone());
    let outcome = catch_unwind(AssertUnwindSafe(|| run_operation(&mut script, |_| PanickingConversion(trace.clone()))));
    assert!(outcome.is_err());
    assert_eq!(*trace.borrow(), ["quarantine", "conversion"]);
}

#[test]
fn retained_pair_drained_rejects_every_roster_and_uncertainty_without_mutating_owner() {
    for selected in 0..4 {
        let mut owner = owner();
        assert!(require_drained(&owner).is_ok());
        let (source, destination) = persistent_sdma_buffers_for_test(owner.owner, 100);
        match selected {
            0 => owner.records[0] = Some(SdmaCopyRecordV1 {
                directional_persistent: false, generation: 1, completion_value: 1,
                fence_header: 0, completion_observed: false, source, destination,
                copy_bytes: 4096, source_offset: 0, destination_offset: 0,
            }),
            1 => owner.xgmi_records[0] = Some(XgmiSdmaCopyRecordV1 {
                generation: 1, completion_value: 1,
                source: crate::shared_memory::xgmi_mapping_for_sdma_test(11),
                destination: crate::shared_memory::xgmi_mapping_for_sdma_test(12), copy_bytes: 4096,
            }),
            2 => owner.persistent_window_slots[0] = Some(PersistentSdmaWindowSlotV1 {
                anchor_slot: 0, generation: 1, completion_value: 1,
            }),
            _ => owner.persistent_window_records[0] = Some(PersistentSdmaWindowRecordV1 {
                request: Gfx942SdmaCopyRequestV1 { source, destination, source_offset: 0,
                    destination_offset: 0, copy_bytes: 4096 }, packet_count: 1,
            }),
        }
        assert!(matches!(require_drained(&owner), Err(Gfx942SdmaErrorV1::Pending)));
        assert!(!owner.poisoned);
    }
    let mut owner = owner();
    owner.uncertain_xgmi_ticket = Some(ticket());
    assert!(matches!(require_drained(&owner), Err(Gfx942SdmaErrorV1::Contract(_))));
    assert_eq!(owner.uncertain_xgmi_ticket, Some(ticket()));
    owner.uncertain_xgmi_ticket = None;
    owner.poisoned = true;
    assert!(require_drained(&owner).is_err());
    owner.poisoned = false;
    owner.destroyed = true;
    assert!(require_drained(&owner).is_err());
}

#[test]
fn retained_pair_drained_rejects_incomplete_rosters() {
    for selected in 0..4 {
        let mut owner = owner();
        match selected {
            0 => { owner.records.pop(); }
            1 => { owner.xgmi_records.pop(); }
            2 => { owner.persistent_window_slots.pop(); }
            _ => { owner.persistent_window_records.pop(); }
        }
        assert!(matches!(require_drained(&owner), Err(Gfx942SdmaErrorV1::Contract(_))));
        assert!(!owner.poisoned);
    }
}

#[test]
fn retained_pair_requires_every_genuine_control_owner_and_exact_engine() {
    use crate::sdma::retained_release::fixture;
    let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(false);
    let mut set = fixture::generic(&mut memory, ticket().owner, Some(2));
    let Gfx942SdmaQueueSetV1::Generic(owners) = &mut set else { panic!("fixture shape"); };
    let owner = &mut owners[0];
    assert!(require_drained(owner).is_ok());
    assert!(required_resources(owner, 2).is_ok());
    assert!(required_resources(owner, 3).is_err());
    let ring = owner.ring.take();
    assert!(required_resources(owner, 2).is_err());
    owner.ring = ring;
    let control = owner.control.take();
    assert!(required_resources(owner, 2).is_err());
    owner.control = control;
    let completions = owner.completions.take();
    assert!(required_resources(owner, 2).is_err());
    owner.completions = completions;
    let doorbell = owner.doorbell.take();
    assert!(required_resources(owner, 2).is_err());
    owner.doorbell = doorbell;
    assert!(required_resources(owner, 2).is_ok());
    assert!(!owner.poisoned);
    fixture::cleanup_set(&mut set);
}

#[test]
fn retained_pair_success_wrappers_preserve_mapping_ownership_without_old_receipt_conversion() {
    let mut script = Script::default();
    let completed = completed();
    let identities = (completed.source.lease().storage_identity(), completed.destination.lease().storage_identity());
    let inner = run_operation(&mut script, |_| Ok::<_, Gfx942XgmiBatchWaitFailureV1>(vec![completed])).unwrap();
    let batch = Gfx942XgmiRetainedPairCompletedBatchV1 { inner };
    assert_eq!(batch.profile(), GFX942_XGMI_RETAINED_PAIR_PROFILE_V1);
    assert_eq!(batch.len(), 1);
    assert!(!batch.is_empty());
    let copy = batch.into_copies().next().unwrap();
    assert_eq!(copy.profile(), GFX942_XGMI_RETAINED_PAIR_PROFILE_V1);
    assert_eq!(copy.copy_bytes(), 4096);
    let (source, destination) = copy.into_mappings();
    assert_eq!((source.lease().storage_identity(), destination.lease().storage_identity()), identities);
    assert!(!script.terminal);
}

struct BorrowedScript<'a> {
    state: &'a mut Script,
    pending: &'a mut Vec<Gfx942SdmaCopyTicketV1>,
}

impl Custody for BorrowedScript<'_> {
    fn terminal(&self) -> bool { self.state.terminal }
    fn quarantine(&mut self) { self.state.quarantine(); }
}

#[test]
fn retained_pair_consuming_pending_finish_quarantines_without_releasing_records() {
    for panic in [false, true] {
    let mut state = Script::default();
    let mut pending = vec![ticket()];
    let scope = Scope { context: BorrowedScript { state: &mut state, pending: &mut pending }, finished: false };
    let result = catch_unwind(AssertUnwindSafe(|| scope.finish(|context| {
        if panic {
            std::panic::panic_any("closing operational panic");
        }
        if context.pending.is_empty() { Ok(()) } else { Err(Gfx942SdmaErrorV1::Pending) }
    })));
    if panic {
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&"closing operational panic"));
    } else {
        assert!(matches!(result.unwrap(), Err(Gfx942SdmaErrorV1::Pending)));
    }
    assert_eq!(pending, vec![ticket()]);
    assert_eq!(state.quarantined, [true; 3]);
    }
}

#[test]
fn retained_pair_drop_and_clean_finish_have_distinct_custody_effects() {
    for action in 0..3 {
        let mut state = Script::default();
        let mut pending = Vec::new();
        let scope = Scope { context: BorrowedScript { state: &mut state, pending: &mut pending }, finished: false };
        match action {
            0 => scope.finish(|_| Ok(())).unwrap(),
            1 => scope.finish_terminal(),
            _ => drop(scope),
        }
        assert_eq!(state.terminal, action != 0);
        assert_eq!(state.quarantined, [action != 0; 3]);
    }
}

#[test]
fn retained_pair_forget_cannot_undo_terminalization_or_release_pending_custody() {
    for terminal in [false, true] {
        let mut state = Script::default();
        let mut pending = vec![ticket()];
        let mut scope = Scope { context: BorrowedScript { state: &mut state, pending: &mut pending }, finished: false };
        let result = run_operation(&mut scope.context, |context| {
            context.state.terminal = terminal;
            if terminal { Err(Gfx942SdmaErrorV1::Contract("closing failure")) } else { Ok(()) }
        });
        assert_eq!(result.is_err(), terminal);
        std::mem::forget(scope);
        assert_eq!(pending, vec![ticket()]);
        assert_eq!(state.terminal, terminal);
        assert_eq!(state.quarantined, [terminal; 3]);
    }
}
