use super::*;
use std::cell::RefCell;
use std::rc::Rc;

struct Script {
    queue: Box<u64>,
    source: Box<u64>,
    destination: Box<u64>,
    terminal: bool,
    trace: Rc<RefCell<Vec<&'static str>>>,
}

impl Script {
    fn new(trace: Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            queue: Box::new(11),
            source: Box::new(23),
            destination: Box::new(37),
            terminal: false,
            trace,
        }
    }

    fn identities(&self) -> [*const u64; 3] {
        [
            self.queue.as_ref(),
            self.source.as_ref(),
            self.destination.as_ref(),
        ]
    }
}

impl Custody for Script {
    fn terminal(&self) -> bool {
        self.terminal
    }

    fn quarantine(&mut self) {
        self.terminal = true;
        self.trace.borrow_mut().push("quarantine");
    }
}

impl Drop for Script {
    fn drop(&mut self) {
        self.trace.borrow_mut().push("parts-drop");
    }
}

fn trace() -> Rc<RefCell<Vec<&'static str>>> {
    Rc::new(RefCell::new(Vec::new()))
}

fn error_pointer(error: &Gfx942SdmaErrorV1) -> *const u8 {
    let Gfx942SdmaErrorV1::Doorbell(text) = error else {
        panic!("expected move-only diagnostic");
    };
    text.as_ptr()
}

// Only CPU fixtures are extracted here to release test allocations. Production
// offers no corresponding extraction of a failed or terminal native owner.
fn dispose_fixture_failure(mut failure: Failure<Script>) {
    drop(failure.owner.take());
}

#[test]
fn owned_retained_pair_public_signatures_preserve_concrete_move_only_parts() {
    let _: fn(
        Gfx942NativeXgmiSdmaQueueV1,
        SharedGttMemorySessionV1,
        SharedGttMemorySessionV1,
        Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
    ) -> Result<
        Gfx942NativeXgmiSdmaOwnedRetainedPairV1,
        Gfx942XgmiOwnedRetainedPairFailureV1,
    > = Gfx942NativeXgmiSdmaOwnedRetainedPairV1::begin;
    let _: fn(
        Gfx942NativeXgmiSdmaOwnedRetainedPairV1,
    )
        -> Result<Gfx942XgmiOwnedRetainedPairPartsV1, Gfx942XgmiOwnedRetainedPairFailureV1> =
        Gfx942NativeXgmiSdmaOwnedRetainedPairV1::finish;
    let _: fn(&mut Gfx942NativeXgmiSdmaOwnedRetainedPairV1) =
        Gfx942NativeXgmiSdmaOwnedRetainedPairV1::quarantine;
    let _: fn(
        &mut Gfx942NativeXgmiSdmaOwnedRetainedPairV1,
        Vec<Gfx942SdmaCopyTicketV1>,
        Instant,
    ) -> Result<
        Gfx942XgmiRetainedPairCompletedBatchV1,
        Gfx942XgmiRetainedPairWaitFailureV1,
    > = Gfx942NativeXgmiSdmaOwnedRetainedPairV1::wait_batch_until;
}

#[test]
fn owned_retained_pair_success_returns_exact_owners_without_intermediate_drop() {
    let trace = trace();
    let script = Script::new(trace.clone());
    let identities = script.identities();
    let owner = Owned::new(script)
        .admit(|script| {
            script.trace.borrow_mut().push("admit");
            Ok(())
        })
        .unwrap_or_else(|_| panic!("unexpected admission failure"));
    assert_eq!(owner.context().identities(), identities);
    let parts = owner
        .finish(|script| {
            script.trace.borrow_mut().push("drained-close");
            Ok(())
        })
        .unwrap_or_else(|_| panic!("unexpected close failure"));
    assert_eq!(parts.identities(), identities);
    assert_eq!(*trace.borrow(), ["admit", "drained-close"]);
    drop(parts);
    assert_eq!(*trace.borrow(), ["admit", "drained-close", "parts-drop"]);
}

#[test]
fn owned_retained_pair_entry_refusal_recovers_only_original_unadmitted_parts() {
    let trace = trace();
    let script = Script::new(trace.clone());
    let identities = script.identities();
    let error = Gfx942SdmaErrorV1::Doorbell("original entry refusal".to_owned());
    let pointer = error_pointer(&error);
    let failure = Owned::new(script).admit(|_| Err(error)).err().unwrap();
    assert!(failure.entry_refusal);
    assert!(!failure.owner.context().terminal());
    assert_eq!(failure.owner.context().identities(), identities);
    assert!(trace.borrow().is_empty());
    let (error, parts) = failure
        .recover_unadmitted()
        .unwrap_or_else(|_| panic!("nonterminal entry refusal must recover its inputs"));
    assert_eq!(error_pointer(&error), pointer);
    assert_eq!(parts.identities(), identities);
    assert!(trace.borrow().is_empty());
    drop(parts);
    assert_eq!(*trace.borrow(), ["parts-drop"]);
}

#[test]
fn owned_retained_pair_terminal_entry_retains_exact_parts_and_first_error() {
    let trace = trace();
    let script = Script::new(trace.clone());
    let identities = script.identities();
    let error = Gfx942SdmaErrorV1::Doorbell("first native error".to_owned());
    let pointer = error_pointer(&error);
    let failure = Owned::new(script)
        .admit(|script| {
            script.terminal = true;
            Err(error)
        })
        .err()
        .unwrap();
    let failure = failure.recover_unadmitted().err().unwrap();
    assert_eq!(failure.owner.context().identities(), identities);
    assert_eq!(error_pointer(&failure.error), pointer);
    assert_eq!(*trace.borrow(), ["quarantine"]);
    dispose_fixture_failure(failure);
    assert_eq!(*trace.borrow(), ["quarantine", "parts-drop"]);
}

#[test]
fn owned_retained_pair_later_terminalization_blocks_entry_recovery() {
    let trace = trace();
    let script = Script::new(trace.clone());
    let identities = script.identities();
    let mut failure = Owned::new(script)
        .admit(|_| Err(Gfx942SdmaErrorV1::QueueFull))
        .err()
        .unwrap();
    assert!(!failure.owner.context().terminal());
    failure.owner.context_mut().terminal = true;
    let failure = failure.recover_unadmitted().err().unwrap();
    assert_eq!(failure.owner.context().identities(), identities);
    assert!(trace.borrow().is_empty());
    dispose_fixture_failure(failure);
}

#[test]
fn owned_retained_pair_finish_refusal_quarantines_and_never_recovers_parts() {
    for error in [
        Gfx942SdmaErrorV1::Pending,
        Gfx942SdmaErrorV1::Timeout,
        Gfx942SdmaErrorV1::Contract("closing fence"),
    ] {
        let trace = trace();
        let script = Script::new(trace.clone());
        let identities = script.identities();
        let owner = Owned::new(script);
        let failure = owner.finish(|_| Err(error)).err().unwrap();
        assert!(!failure.entry_refusal);
        let failure = failure.recover_unadmitted().err().unwrap();
        assert_eq!(failure.owner.context().identities(), identities);
        assert!(failure.owner.context().terminal());
        assert_eq!(*trace.borrow(), ["quarantine"]);
        dispose_fixture_failure(failure);
    }
}

#[test]
fn owned_retained_pair_terminal_success_never_extracts_entry_or_closed_parts() {
    for closing in [false, true] {
        let trace = trace();
        let script = Script::new(trace.clone());
        let identities = script.identities();
        let operation = |script: &mut Script| {
            script.terminal = true;
            Ok(())
        };
        let failure = if closing {
            Owned::new(script).finish(operation).err().unwrap()
        } else {
            Owned::new(script).admit(operation).err().unwrap()
        };
        assert!(matches!(
            &failure.error,
            Gfx942SdmaErrorV1::Contract("retained XGMI success has terminal custody")
        ));
        let failure = failure.recover_unadmitted().err().unwrap();
        assert_eq!(failure.owner.context().identities(), identities);
        assert!(failure.owner.context().terminal());
        assert!(trace.borrow().iter().all(|item| *item == "quarantine"));
        dispose_fixture_failure(failure);
    }
}

#[test]
fn owned_retained_pair_operation_keeps_ticket_custody_on_fenced_timeout() {
    let trace = trace();
    let mut owner = Owned::new(Script::new(trace.clone()));
    let identities = owner.context().identities();
    let ticket = Gfx942SdmaCopyTicketV1 {
        owner: crate::sdma::tests::queue_key(7, 11, 13),
        queue_id: 17,
        slot: 3,
        generation: 9,
    };
    let outcome = run_operation(owner.context_mut(), |_| {
        Err::<Vec<Gfx942XgmiCompletedCopyV1>, _>(Gfx942XgmiBatchWaitFailureV1::Retained {
            error: Gfx942SdmaErrorV1::Timeout,
            tickets: vec![ticket],
        })
    });
    match outcome {
        Err(Gfx942XgmiBatchWaitFailureV1::Retained {
            error: Gfx942SdmaErrorV1::Timeout,
            tickets,
        }) => {
            assert_eq!(tickets, vec![ticket]);
        }
        _ => panic!("timeout changed exact native ticket custody"),
    }
    assert_eq!(owner.context().identities(), identities);
    assert!(trace.borrow().is_empty());
    let parts = owner
        .finish(|_| Ok(()))
        .unwrap_or_else(|_| panic!("fixture close"));
    drop(parts);
}

#[test]
fn owned_retained_pair_terminal_completion_retains_actual_mapping_identities() {
    let trace = trace();
    let mut script = Script::new(trace.clone());
    script.terminal = true;
    let mut owner = Owned::new(script);
    let parts = owner.context().identities();
    let completed = Gfx942XgmiCompletedCopyV1 {
        source: crate::shared_memory::xgmi_mapping_for_sdma_test(101),
        destination: crate::shared_memory::xgmi_mapping_for_sdma_test(102),
        copy_bytes: 4096,
    };
    let mappings = (
        completed.source.lease().storage_identity(),
        completed.destination.lease().storage_identity(),
    );
    let result = run_operation(owner.context_mut(), |_| {
        Ok::<_, Gfx942XgmiBatchWaitFailureV1>(vec![completed])
    });
    let failure = Gfx942XgmiRetainedPairWaitFailureV1 {
        inner: result.err().unwrap(),
    };
    let mut retained = failure.into_indeterminate_mappings().unwrap();
    assert_eq!(retained.len(), 1);
    let (source, destination) = retained.next().unwrap();
    assert_eq!(
        (
            source.lease().storage_identity(),
            destination.lease().storage_identity()
        ),
        mappings
    );
    assert_eq!(owner.context().identities(), parts);
    assert_eq!(*trace.borrow(), ["quarantine"]);
    drop(owner.take());
}

#[test]
fn owned_retained_pair_operation_panic_quarantines_before_unwind_escapes() {
    let trace = trace();
    let mut owner = Owned::new(Script::new(trace.clone()));
    let identities = owner.context().identities();
    let error = Box::new(0x1234_u64);
    let pointer = error.as_ref() as *const u64;
    let result = catch_unwind(AssertUnwindSafe(|| {
        run_operation(owner.context_mut(), |_| -> Result<(), Gfx942SdmaErrorV1> {
            std::panic::panic_any(error)
        })
    }));
    let payload = result.unwrap_err().downcast::<Box<u64>>().unwrap();
    assert_eq!(payload.as_ref().as_ref() as *const u64, pointer);
    assert!(owner.context().terminal());
    assert_eq!(owner.context().identities(), identities);
    assert_eq!(*trace.borrow(), ["quarantine"]);
    // The fixture has no native owners; release its test-only probes explicitly.
    drop(owner.take());
}

#[test]
fn owned_retained_pair_occupied_drop_aborts_before_any_parts_drop() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_OWNED_RETAINED_PAIR_DROP";
    const TEST: &str = "sdma::retained_pair::owned::tests::owned_retained_pair_occupied_drop_aborts_before_any_parts_drop";
    let Some(mode) = std::env::var_os(CHILD) else {
        for mode in [
            "occupied",
            "quarantine-panic",
            "entry-unwind",
            "close-unwind",
            "failure",
        ] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, mode)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
            assert!(
                stderr.contains("owned-pair-quarantined"),
                "{mode}: {stderr}"
            );
            assert!(
                !stderr.contains("owned-pair-parts-dropped"),
                "{mode}: {stderr}"
            );
        }
        return;
    };
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .unwrap();
    struct AbortProbe(bool);
    impl Custody for AbortProbe {
        fn terminal(&self) -> bool {
            false
        }
        fn quarantine(&mut self) {
            eprintln!("owned-pair-quarantined");
            if self.0 {
                panic!("quarantine panic");
            }
        }
    }
    impl Drop for AbortProbe {
        fn drop(&mut self) {
            eprintln!("owned-pair-parts-dropped");
        }
    }
    let owner = Owned::new(AbortProbe(mode == "quarantine-panic"));
    match mode.to_str().unwrap() {
        "entry-unwind" => {
            let _ = owner.admit(|_| panic!("entry panic"));
        }
        "close-unwind" => {
            let _ = owner.finish(|_| panic!("close panic"));
        }
        "failure" => {
            let _ = owner.finish(|_| Err(Gfx942SdmaErrorV1::Pending));
        }
        "occupied" | "quarantine-panic" => drop(owner),
        _ => panic!("unexpected child mode"),
    }
    panic!("occupied owner unexpectedly returned from Drop");
}
