//! Shared completion and disposal tests. No remote owner or executable authority is fabricated.
use super::*;
use crate::generated_runtime_carrier::{GeneratedRuntimeAuthorityV1, GeneratedRuntimeCarrierV1};
use fe2o3_runtime::{
    RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedCompletionCarrierV1,
    RuntimeGfx942ReadbackErrorV1, WorkerV3Gfx942ExecutionAuthorityV1,
};

struct SharedOwner(Arc<AtomicBool>);
impl Drop for SharedOwner {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

struct InertAuthority {
    _original: Arc<SharedOwner>,
    gate: Arc<ResultReadyGateV1>,
    budget: GeneratedRuntimeResultBudgetV1,
    expected_ready: bool,
    expected_bytes: u64,
}

impl Drop for InertAuthority {
    fn drop(&mut self) {
        assert_eq!(self.gate.ready(), self.expected_ready);
        assert_eq!(
            self.budget.usage().reserved_peak_bytes,
            self.expected_bytes,
            "authority dropped before storage disposal or decoder settlement"
        );
    }
}

// SAFETY: this test-only rejecting adapter cannot emit execution identity or pass currentness.
// Tests call only charged host completion/disposal, never a native transition.
unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for InertAuthority {
    type CurrentnessError = &'static str;
    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        panic!("inert authority")
    }
    fn finalized_hsaco_length(&self) -> u64 {
        panic!("inert authority")
    }
    fn kernel_name(&self) -> &str {
        panic!("inert authority")
    }
    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        panic!("inert authority")
    }
    fn device_unique_id(&self) -> u64 {
        panic!("inert authority")
    }
    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        Err("inert authority")
    }
}
impl GeneratedRuntimeAuthorityV1 for InertAuthority {
    fn artifact_bytes(&self) -> &[u8] {
        panic!("no native source")
    }
}

fn carrier(
    original: &Arc<SharedOwner>,
    budget: &GeneratedRuntimeResultBudgetV1,
    expected_ready: bool,
) -> (
    GeneratedRuntimeCarrierV1<InertAuthority>,
    GeneratedRuntimeChargedResultV1<u32>,
) {
    let (parts, observer) = input_parts(budget);
    let hsaco = synthetic_cov6::preparation_module();
    let storage = parts
        .storage
        .prepare(&hsaco, "vecadd")
        .unwrap()
        .project_persistent(&hsaco)
        .unwrap();
    let authority = InertAuthority {
        _original: Arc::clone(original),
        gate: Arc::clone(storage.decoder.result_gate.as_ref().unwrap()),
        budget: budget.clone(),
        expected_ready,
        expected_bytes: if expected_ready { 32 } else { 0 },
    };
    (
        GeneratedRuntimeCarrierV1 {
            storage,
            authority,
            footprint: parts.footprint,
            result_budget: budget.clone(),
        },
        observer,
    )
}

fn stage(carrier: &mut GeneratedRuntimeCarrierV1<InertAuthority>, malformed: bool) {
    let mut readback = carrier.prepare_readback().unwrap();
    readback.buffers_mut()[0]
        .1
        .copy_from_slice(&words(&[9u32, 13, 17, 21]));
    if malformed {
        readback.buffers_mut()[0].1.pop();
    }
    carrier.install_readback(readback);
}

#[test]
fn shared_carrier_keeps_original_owner_until_both_decoders_settle() {
    let disposed = Arc::new(AtomicBool::new(false));
    let original = Arc::new(SharedOwner(Arc::clone(&disposed)));
    let first_budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
    let second_budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
    let (mut first, mut first_observer) = carrier(&original, &first_budget, true);
    let (mut second, mut second_observer) = carrier(&original, &second_budget, true);
    drop(original);
    stage(&mut first, false);
    stage(&mut second, false);
    assert!(matches!(
        first.prepare_readback(),
        Err(RuntimeGfx942ReadbackErrorV1::AlreadyReserved)
    ));
    first.complete_readback_v1().unwrap();
    assert!(!disposed.load(Ordering::SeqCst));
    let result = first_observer.try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[9, 13, 17, 21]);
    drop(result);
    assert_empty(&first_budget);
    assert!(second_observer.try_take().unwrap().is_none());
    second.complete_readback_v1().unwrap();
    assert!(disposed.load(Ordering::SeqCst));
    drop(second_observer.try_take().unwrap().unwrap());
    assert_empty(&second_budget);
}

#[test]
fn shared_carrier_drop_and_failed_decode_dispose_storage_before_authority() {
    for case in ["drop", "no-readback", "malformed"] {
        let disposed = Arc::new(AtomicBool::new(false));
        let original = Arc::new(SharedOwner(Arc::clone(&disposed)));
        let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
        let (mut carrier, mut observer) = carrier(&original, &budget, false);
        drop(original);
        assert!(!disposed.load(Ordering::SeqCst));
        if case == "drop" {
            drop(carrier);
        } else {
            if case == "malformed" {
                stage(&mut carrier, true);
            }
            assert!(matches!(
                carrier.complete_readback_v1(),
                Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
            ));
        }
        assert!(disposed.load(Ordering::SeqCst));
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
        assert_empty(&budget);
    }
}
