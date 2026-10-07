//! CPU transaction controls; no device, native mapping or SDMA queue is opened.
#![cfg(test)]

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Success,
    Error,
    Panic,
}

fn outcome(value: Outcome, message: &'static str) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    match value {
        Outcome::Success => Ok(()),
        Outcome::Error => Err(contract(message)),
        Outcome::Panic => std::panic::panic_any(message),
    }
}

struct Fixture {
    owner: QueueKeyV1,
    root: Option<Custody>,
    outstanding: usize,
    initial_outstanding: usize,
    identity: Gfx942SdmaBufferStorageIdentityV1,
    preflight: Outcome,
    open: Outcome,
    read: Outcome,
    close: Outcome,
    bytes: Box<[u8]>,
    trace: Vec<&'static str>,
    poisoned: bool,
    commit_panic: bool,
    poison_panic: bool,
}

impl Fixture {
    fn assert_input(&self) {
        let Some(Custody::Input(buffer)) = self.root.as_ref() else {
            panic!("original input must precede all model/native operations");
        };
        assert_eq!(buffer.storage_identity(), self.identity);
        assert_eq!(self.outstanding, self.initial_outstanding);
    }
}

impl Context for Fixture {
    type Loan = u64;

    fn owner(&self) -> QueueKeyV1 {
        self.owner
    }
    fn preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        outcome(self.preflight, "preflight")
    }
    fn root(&mut self) -> &mut Option<Custody> {
        &mut self.root
    }
    fn outstanding(&mut self) -> &mut usize {
        if let Some(Custody::Output(data, bridge)) = self.root.as_ref() {
            assert_eq!(data.sdma_storage_identity(), self.identity);
            assert_eq!(bridge.storage_identity, self.identity);
            assert_eq!(self.outstanding, self.initial_outstanding);
            if self.commit_panic {
                std::panic::panic_any("commit");
            }
        }
        &mut self.outstanding
    }
    fn loan(&mut self) -> Result<Self::Loan, ComputeAqlQueueSessionErrorV1> {
        self.assert_input();
        self.trace.push("open");
        outcome(self.open, "open")?;
        Ok(31)
    }
    fn read(&mut self) -> Result<Box<[u8]>, ComputeAqlQueueSessionErrorV1> {
        self.assert_input();
        self.trace.push("read");
        outcome(self.read, "read")?;
        Ok(self.bytes.clone())
    }
    fn retake(&mut self, loan: Self::Loan) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.assert_input();
        assert_eq!(loan, 31);
        self.trace.push("close");
        outcome(self.close, "close")
    }
    fn poison(&mut self) {
        self.poisoned = true;
        if self.poison_panic {
            std::panic::panic_any("secondary poison");
        }
    }
}

fn original(owner: QueueKeyV1, id: u64, logical: u64) -> Gfx942SdmaBufferV1 {
    Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Host(
            crate::shared_memory::mapped_host_for_persistent_sdma_test(id, 4096),
        ),
        owner,
        7,
        logical,
    )
}

fn descriptor(bytes: &[u8]) -> Gfx942DeviceContentDescriptorV1 {
    Gfx942DeviceContentDescriptorV1::from_bytes(
        Gfx942DeviceContentRoleV1::new([0x5a; 32], 7).unwrap(),
        bytes,
    )
    .unwrap()
}

fn fixture() -> (Fixture, Gfx942SdmaBufferV1, Gfx942DeviceContentDescriptorV1) {
    let owner = super::super::tests::test_queue_key(19, 23);
    let buffer = original(owner, 100, 4096);
    let bytes: Box<[u8]> = Box::new([7; 4096]);
    let content = descriptor(&bytes);
    let fixture = Fixture {
        owner,
        root: None,
        outstanding: 3,
        initial_outstanding: 3,
        identity: buffer.storage_identity(),
        preflight: Outcome::Success,
        open: Outcome::Success,
        read: Outcome::Success,
        close: Outcome::Success,
        bytes,
        trace: Vec::new(),
        poisoned: false,
        commit_panic: false,
        poison_panic: false,
    };
    (fixture, buffer, content)
}

#[test]
fn original_storage_and_bridge_are_rooted_before_the_single_debit() {
    let (mut context, buffer, content) = fixture();
    assert!(!buffer.initialized_range_is_known(0, 4096));
    let (data, bridge) = promote(&mut context, buffer, content).unwrap();
    assert_eq!(data.sdma_storage_identity(), context.identity);
    assert!(data.is_fully_initialized());
    assert_eq!(bridge.owner, context.owner);
    assert_eq!(bridge.pool_generation, 7);
    assert_eq!((bridge.logical_bytes, bridge.physical_bytes), (4096, 4096));
    assert_eq!(bridge.storage_identity, context.identity);
    assert_eq!(context.outstanding, 2);
    assert_eq!(context.trace, ["open", "read", "close"]);
    assert!(context.root.is_none() && !context.poisoned);
}

#[test]
fn borrowed_refusal_order_returns_the_exact_original_before_any_read() {
    for axis in 0..5 {
        let (mut context, mut buffer, mut content) = fixture();
        let expected = match axis {
            0 => {
                context.owner = super::super::tests::test_queue_key(20, 23);
                context.preflight = Outcome::Panic;
                "foreign SDMA buffer owner"
            }
            1 => {
                context.preflight = Outcome::Error;
                content = descriptor(&[1]);
                "preflight"
            }
            2 => {
                buffer = original(context.owner, 101, 2048);
                "SDMA host promotion requires one exact full physical extent"
            }
            3 => {
                content = descriptor(&[1]);
                "SDMA host promotion requires one exact full physical extent"
            }
            4 => {
                buffer = crate::sdma::persistent_sdma_buffers_for_test(context.owner, 300).0;
                "SDMA host promotion requires one exact full physical extent"
            }
            _ => unreachable!(),
        };
        let identity = buffer.storage_identity();
        let failure = promote(&mut context, buffer, content).err().unwrap();
        assert!(
            matches!(failure.error, ComputeAqlQueueSessionErrorV1::Contract(s) if s == expected)
        );
        assert_eq!(failure.recovered.unwrap().storage_identity(), identity);
        assert!(context.trace.is_empty() && context.root.is_none() && !context.poisoned);
        assert_eq!(context.outstanding, 3);
    }
}

#[test]
fn settled_digest_mismatch_returns_original_without_trusting_cached_content() {
    let (mut context, mut buffer, content) = fixture();
    buffer.inject_stale_detached_content_for_test_v1(6);
    let initialized = buffer.initialized_range_is_known(0, 4096);
    let certificate = buffer.certified_full_host_content_sha256(4096);
    context.bytes[0] ^= 1;
    // Even an underflow cannot precede the original read/hash refusal.
    context.outstanding = 0;
    context.initial_outstanding = 0;
    let failure = promote(&mut context, buffer, content).err().unwrap();
    assert!(matches!(
        failure.error,
        ComputeAqlQueueSessionErrorV1::Contract("SDMA host promotion content descriptor mismatch")
    ));
    let recovered = failure.recovered.unwrap();
    assert_eq!(recovered.storage_identity(), context.identity);
    assert_eq!(recovered.pool_generation(), 7);
    assert_eq!(recovered.initialized_range_is_known(0, 4096), initialized);
    assert_eq!(
        recovered.certified_full_host_content_sha256(4096),
        certificate
    );
    assert_eq!(context.trace, ["open", "read", "close"]);
    assert!(context.root.is_none() && !context.poisoned);
    assert_eq!(context.outstanding, 0);
}

#[test]
fn opening_read_and_retake_matrix_retains_input_and_preserves_failure_order() {
    for open in [Outcome::Success, Outcome::Error, Outcome::Panic] {
        for read in [Outcome::Success, Outcome::Error, Outcome::Panic] {
            for close in [Outcome::Success, Outcome::Error, Outcome::Panic] {
                if [open, read, close] == [Outcome::Success; 3] {
                    continue;
                }
                let (mut context, buffer, content) = fixture();
                context.open = open;
                context.read = read;
                context.close = close;
                let mut returned = None;
                let caught = catch_unwind(AssertUnwindSafe(|| {
                    returned = Some(promote(&mut context, buffer, content));
                }));
                let panic = if open == Outcome::Panic {
                    Some("open")
                } else if open == Outcome::Error {
                    None
                } else if read == Outcome::Panic {
                    Some("read")
                } else if close == Outcome::Panic {
                    Some("close")
                } else {
                    None
                };
                if let Some(expected) = panic {
                    assert!(returned.is_none());
                    assert_eq!(caught.unwrap_err().downcast_ref::<&str>(), Some(&expected));
                } else {
                    caught.unwrap();
                    let failure = returned.unwrap().err().unwrap();
                    let expected = if open == Outcome::Error {
                        "open"
                    } else if close == Outcome::Error {
                        "close"
                    } else {
                        "read"
                    };
                    assert!(
                        matches!(failure.error, ComputeAqlQueueSessionErrorV1::Contract(s) if s == expected)
                    );
                    assert!(failure.recovered.is_none());
                }
                context.assert_input();
                assert!(context.poisoned);
                assert_eq!(
                    context.trace,
                    if open == Outcome::Success {
                        vec!["open", "read", "close"]
                    } else {
                        vec!["open"]
                    }
                );
            }
        }
    }
}

#[test]
fn underflow_is_terminal_only_after_read_and_retake_and_keeps_input() {
    let (mut context, buffer, content) = fixture();
    context.outstanding = 0;
    context.initial_outstanding = 0;
    let failure = promote(&mut context, buffer, content).err().unwrap();
    assert!(matches!(
        failure.error,
        ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger underflow")
    ));
    assert!(failure.recovered.is_none() && context.poisoned);
    context.assert_input();
    assert_eq!(context.trace, ["open", "read", "close"]);
}

#[test]
fn preflight_or_commit_unwind_roots_original_input_or_output() {
    for commit in [false, true] {
        let (mut context, buffer, content) = fixture();
        context.commit_panic = commit;
        context.poison_panic = true;
        if !commit {
            context.preflight = Outcome::Panic;
        }
        let mut returned = None;
        let caught = catch_unwind(AssertUnwindSafe(|| {
            returned = Some(promote(&mut context, buffer, content));
        }));
        assert!(returned.is_none());
        assert_eq!(
            caught.unwrap_err().downcast_ref::<&str>(),
            Some(&if commit { "commit" } else { "preflight" })
        );
        assert!(context.poisoned);
        assert_eq!(context.outstanding, 3);
        if commit {
            let Some(Custody::Output(data, bridge)) = context.root.as_ref() else {
                panic!("converted original must precede debit");
            };
            assert_eq!(data.sdma_storage_identity(), context.identity);
            assert_eq!(bridge.storage_identity, context.identity);
            assert!(data.is_fully_initialized());
        } else {
            context.assert_input();
            assert!(context.trace.is_empty());
        }
    }
}

#[path = "death.rs"]
mod death;
