//! CPU ownership/transaction controls. No device or copy queue is opened.
#![cfg(test)]

use super::*;

struct Fixture {
    owner: QueueKeyV1,
    root: Option<Custody>,
    generation: Option<u64>,
    count: usize,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    insertion: Option<usize>,
    outstanding: usize,
    mode: u8,
    validations: usize,
    currentness: usize,
    poisoned: bool,
}

impl Context for Fixture {
    fn owner(&self) -> QueueKeyV1 {
        self.owner
    }
    fn preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.mode == 5 {
            panic!("borrowed admission unwind");
        }
        if self.mode == 1 {
            Err(contract("borrowed preflight refusal"))
        } else {
            Ok(())
        }
    }
    fn root(&mut self) -> &mut Option<Custody> {
        &mut self.root
    }
    fn ledger(&mut self) -> Ledger<'_> {
        if self.mode == 4 && matches!(self.root, Some(Custody::Output(_))) {
            panic!("commit refusal after output rooted");
        }
        Ledger {
            generation: self.generation,
            count: &mut self.count,
            identities: &mut self.identities,
            next_insertion: &mut self.insertion,
            outstanding: &mut self.outstanding,
        }
    }
    fn validate_original(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.validations += 1;
        assert!(matches!(self.root, Some(Custody::Input(_, _))));
        assert_eq!(
            (self.count, self.identities.len(), self.outstanding),
            (1, 1, 3)
        );
        match self.mode {
            2 => Err(contract("original validation or retake refusal")),
            3 => panic!("original validation unwind"),
            _ => Ok(()),
        }
    }
    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.currentness += 1;
        assert!(matches!(self.root, Some(Custody::Input(_, _))));
        assert_eq!(
            (self.count, self.identities.len(), self.outstanding),
            (1, 1, 3)
        );
        let fail_at = if self.mode == 6 || self.mode == 8 {
            1
        } else {
            2
        };
        if self.mode >= 6 && self.currentness == fail_at {
            if self.mode >= 8 {
                panic!("original currentness unwind");
            }
            return Err(contract("original currentness refusal"));
        }
        Ok(())
    }
    fn poison(&mut self) {
        self.poisoned = true;
    }
}

fn originals(
    id: u64,
) -> (
    Gfx942DetachedFixedDispatchV1,
    Gfx942SdmaDispatchDataBridgeV1,
) {
    let owner = super::super::tests::test_queue_key(19, 23);
    let token = crate::shared_memory::mapped_host_for_persistent_sdma_test(id, 4096);
    let data = Gfx942FixedDispatchDataV1::host_visible_initialized(
        crate::shared_memory::Gfx942InitializedHostVisibleMemoryV1::from_completed_dispatch(token),
    );
    let bridge = Gfx942SdmaDispatchDataBridgeV1 {
        owner,
        pool_generation: 7,
        logical_bytes: 4096,
        physical_bytes: 4096,
        storage_identity: data.sdma_storage_identity(),
    };
    (
        Gfx942DetachedFixedDispatchV1 {
            generation: 5,
            data: vec![data],
        },
        bridge,
    )
}

fn fixture(
    detached: &Gfx942DetachedFixedDispatchV1,
    bridge: &Gfx942SdmaDispatchDataBridgeV1,
) -> Fixture {
    Fixture {
        owner: bridge.owner,
        root: None,
        generation: Some(detached.generation),
        count: 1,
        identities: detached
            .data
            .iter()
            .map(Gfx942FixedDispatchDataV1::storage_identity)
            .collect(),
        insertion: None,
        outstanding: 3,
        mode: 0,
        validations: 0,
        currentness: 0,
        poisoned: false,
    }
}

#[test]
fn actual_originals_transfer_once_with_initializedness_not_stale_content() {
    let (detached, bridge) = originals(100);
    let identity = detached.data[0].sdma_storage_identity();
    let old_generation = bridge.pool_generation;
    let mut context = fixture(&detached, &bridge);
    let mut buffer = transfer(&mut context, detached, bridge).unwrap();
    assert_eq!(buffer.storage_identity(), identity);
    assert_eq!(buffer.pool_generation(), old_generation + 1);
    assert!(buffer.initialized_range_is_known(0, 4096));
    assert!(!buffer.initialized_range_is_known(0, 4097));
    assert_eq!(buffer.certified_full_host_content_sha256(4096), None);
    buffer.inject_stale_detached_content_for_test_v1(old_generation);
    assert_eq!(buffer.certified_full_host_content_sha256(4096), None);
    assert_eq!(
        (context.count, context.identities.len(), context.outstanding),
        (0, 0, 4)
    );
    assert_eq!((context.generation, context.insertion), (Some(5), Some(0)));
    assert!(context.root.is_none() && !context.poisoned);
    assert_eq!(context.validations, 1);
    assert_eq!(context.currentness, 2);
    let (other, other_bridge) = originals(101);
    let failure = transfer(&mut context, other, other_bridge).unwrap_err();
    assert!(failure.into_recovered().is_some());
    assert_eq!(context.validations, 1);
}

#[test]
fn substitution_generation_extent_and_cardinality_refuse_before_validation() {
    for axis in 0..14 {
        let (mut detached, mut bridge) = originals(100);
        let mut context = fixture(&detached, &bridge);
        match axis {
            0 => bridge.owner = super::super::tests::test_queue_key(20, 23),
            1 => detached.generation += 1,
            2 => context.generation = None,
            3 => context.count = 2,
            4 => context.identities.clear(),
            5 => context.insertion = Some(0),
            6 => bridge.storage_identity = originals(101).1.storage_identity,
            7 => bridge.logical_bytes = 2048,
            8 => bridge.physical_bytes = 2048,
            9 => bridge.pool_generation = u64::MAX,
            10 => context.outstanding = usize::MAX,
            11 => detached.data.push(originals(101).0.data.pop().unwrap()),
            12 => bridge.pool_generation = 0,
            13 => detached.data.clear(),
            _ => unreachable!(),
        }
        let count = detached.data.len();
        let ids: Vec<_> = detached
            .data
            .iter()
            .map(Gfx942FixedDispatchDataV1::storage_identity)
            .collect();
        let old_count = context.count;
        let old_ids = context.identities.clone();
        let failure = transfer(&mut context, detached, bridge).unwrap_err();
        let (detached, _) = failure.into_recovered().unwrap();
        assert_eq!(detached.data.len(), count);
        assert_eq!(
            detached
                .data
                .iter()
                .map(Gfx942FixedDispatchDataV1::storage_identity)
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!((context.count, &context.identities), (old_count, &old_ids));
        assert_eq!(context.validations, 0);
        assert!(context.root.is_none() && !context.poisoned);
    }
}

#[test]
fn uninitialized_or_noncoherent_original_is_not_a_read_owner() {
    for device in [false, true] {
        let (mut detached, mut bridge) = originals(100);
        detached.data[0] = if device {
            Gfx942FixedDispatchDataV1::initialized_storage(
                crate::shared_memory::private_local_mapping_for_sdma_pool_test(200),
            )
        } else {
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(
                crate::shared_memory::mapped_host_for_persistent_sdma_test(200, 4096),
            )
        };
        bridge.storage_identity = detached.data[0].sdma_storage_identity();
        let mut context = fixture(&detached, &bridge);
        let failure = transfer(&mut context, detached, bridge).unwrap_err();
        assert!(failure.into_recovered().is_some());
        assert_eq!(context.validations, 0);
        assert!(context.root.is_none() && !context.poisoned);
    }
}

#[test]
fn admission_error_returns_originals_but_validation_error_keeps_terminal_root() {
    for mode in [1, 2] {
        let (detached, bridge) = originals(100);
        let mut context = fixture(&detached, &bridge);
        context.mode = mode;
        let failure = transfer(&mut context, detached, bridge).unwrap_err();
        assert_eq!(failure.into_recovered().is_some(), mode == 1);
        assert_eq!(context.poisoned, mode == 2);
        assert_eq!(
            matches!(context.root, Some(Custody::Input(_, _))),
            mode == 2
        );
        assert_eq!(
            (context.count, context.identities.len(), context.outstanding),
            (1, 1, 3)
        );
        assert_eq!(context.validations, usize::from(mode == 2));
    }
}

#[test]
fn unwind_retains_input_or_converted_output_before_any_ledger_debit() {
    for mode in [3, 4, 5] {
        let (detached, bridge) = originals(100);
        let identity = detached.data[0].sdma_storage_identity();
        let mut context = fixture(&detached, &bridge);
        context.mode = mode;
        let mut returned = None;
        let failure = catch_unwind(AssertUnwindSafe(|| {
            returned = Some(transfer(&mut context, detached, bridge));
        }));
        assert!(failure.is_err());
        assert!(returned.is_none());
        assert!(context.poisoned);
        match context.root.as_ref().unwrap() {
            Custody::Input(detached, _) => {
                assert!(mode == 3 || mode == 5);
                assert_eq!(detached.data[0].sdma_storage_identity(), identity);
            }
            Custody::Output(buffer) => {
                assert_eq!(mode, 4);
                assert_eq!(buffer.storage_identity(), identity);
                assert!(buffer.initialized_range_is_known(0, 4096));
            }
        }
        assert_eq!(
            (context.count, context.identities.len(), context.outstanding),
            (1, 1, 3)
        );
        assert_eq!((context.generation, context.insertion), (Some(5), None));
    }
}

#[test]
fn opening_and_closing_currentness_errors_or_unwind_retain_original_data() {
    for mode in 6..10 {
        let (detached, bridge) = originals(100);
        let original = detached.data[0].storage_identity();
        let mut context = fixture(&detached, &bridge);
        context.mode = mode;
        let mut returned = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            returned = Some(transfer(&mut context, detached, bridge));
        }));
        if mode >= 8 {
            assert!(result.is_err());
            assert!(returned.is_none());
        } else {
            result.unwrap();
            assert!(returned.unwrap().unwrap_err().into_recovered().is_none());
        }
        assert!(context.poisoned);
        let Some(Custody::Input(detached, _)) = context.root.as_ref() else {
            panic!("lost original input");
        };
        assert_eq!(detached.data[0].storage_identity(), original);
        assert_eq!(
            (context.count, context.identities.len(), context.outstanding),
            (1, 1, 3)
        );
        let closing = mode == 7 || mode == 9;
        assert_eq!(context.currentness, if closing { 2 } else { 1 });
        assert_eq!(context.validations, usize::from(closing));
    }
}

#[test]
fn actual_queue_refuses_reentry_and_drop_cannot_unwind_terminal_input() {
    const ENV: &str = "FE2O3_DETACHED_SDMA_ORIGINAL_ROOT_DROP_101";
    const TEST: &str = "queue::live::detached_sdma::tests::actual_queue_refuses_reentry_and_drop_cannot_unwind_terminal_input";
    if std::env::var_os(ENV).is_some() {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        struct MustNotUnwind;
        impl Drop for MustNotUnwind {
            fn drop(&mut self) {
                std::process::exit(91);
            }
        }
        let _originals = MustNotUnwind;
        let (detached, bridge) = originals(100);
        let mut session = super::super::tests::persistent_compute_cancellation_test_session(
            bridge.owner,
            None,
            None,
        );
        let (detached, bridge) = session
            .transfer_detached_fixed_dispatch_to_sdma_v1(detached, bridge)
            .unwrap_err()
            .into_recovered()
            .unwrap();
        assert!(session.detached_sdma.is_none());
        session.detached_sdma = Some(Custody::Input(detached, bridge));
        let (other, other_bridge) = originals(101);
        let failure = session
            .transfer_detached_fixed_dispatch_to_sdma_v1(other, other_bridge)
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::Contract("unfinished detached SDMA transfer")
        ));
        assert!(failure.into_recovered().is_some());
        assert!(session.require_no_sdma_owner_transition_v1().is_err());
        assert!(matches!(session.detached_sdma, Some(Custody::Input(_, _))));
        eprintln!("DETACHED_SDMA_ORIGINAL_ROOT_RETAINED");
        drop(session);
        panic!("returned after releasing original detached SDMA custody");
    }
    use std::io::Read;
    use std::os::unix::process::ExitStatusExt;
    struct Reap(std::process::Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let mut child = Reap(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(ENV, "1")
            .env("RUST_BACKTRACE", "0")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "detached SDMA child timeout"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let mut stderr = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .take(16 * 1024 + 1)
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stderr.len() <= 16 * 1024);
    assert_eq!(status.signal(), Some(libc::SIGABRT), "{stderr}");
    assert!(stderr.contains("DETACHED_SDMA_ORIGINAL_ROOT_RETAINED"));
    assert!(!stderr.contains("panicked at") && !stderr.contains("returned after releasing"));
}
