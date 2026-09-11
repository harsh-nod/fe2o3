use super::*;

mod readback_tests;

// Reuse the loader's structural builder without introducing executable or verifier evidence.
#[allow(dead_code)]
#[path = "../../../../fe2o3-runtime/src/kfd_backend/tests/synthetic_cov6.rs"]
mod synthetic_cov6;

fn geometry() -> AqlDispatchGeometryV1 {
    AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap()
}

struct DisposalWitness<P> {
    payload: Option<P>,
    budget: GeneratedRuntimeResultBudgetV1,
    disposed: Arc<AtomicBool>,
    expected_bytes: u64,
}

impl<P> Drop for DisposalWitness<P> {
    fn drop(&mut self) {
        drop(self.payload.take());
        assert_eq!(self.budget.usage().reserved_peak_bytes, self.expected_bytes);
        self.disposed.store(true, Ordering::SeqCst);
    }
}

fn input_parts(
    budget: &GeneratedRuntimeResultBudgetV1,
) -> (
    GeneratedRuntimeInvocationPartsV1,
    GeneratedRuntimeChargedResultV1<u32>,
) {
    let (output, observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![3u32, 7, 11, 19].into_boxed_slice());
    let packed = prepare(vec![output], budget).unwrap();
    let kernel_id = packed.kernel_id();
    let footprint = packed.footprint();
    let packing = packed.packing_observation().clone();
    let parts = packed.into_runtime_inputs(geometry(), 0, 1000);
    assert_eq!(parts.kernel_id, kernel_id);
    assert_eq!(parts.footprint, footprint);
    assert_eq!(parts.packing, packing);
    (parts, observer)
}

#[test]
fn invocation_preparation_failure_disposes_inputs_and_credits() {
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
    let (parts, mut observer) = input_parts(&budget);
    assert!(matches!(
        parts.storage.prepare(b"invalid HSACO", "vecadd"),
        Err(Gfx942RuntimePreparationErrorV1::Envelope(_))
    ));
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn invocation_preparation_retains_exact_identity_and_disposes_without_outputs() {
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
    let (parts, mut observer) = input_parts(&budget);
    let hsaco = synthetic_cov6::preparation_module();
    let storage = parts.storage.prepare(&hsaco, "vecadd").unwrap();
    assert_eq!(storage.prepared().kernel_name(), "vecadd");
    assert_eq!(
        storage.prepared().finalized_hsaco_length(),
        hsaco.len() as u64
    );
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    assert!(observer.try_take().unwrap().is_none());
    drop(storage);
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn invocation_preparation_does_not_relax_private_segment_validation() {
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
    let (parts, mut observer) = input_parts(&budget);
    assert!(matches!(
        parts.storage.prepare(&synthetic_cov6::module(), "vecadd"),
        Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(_))
    ));
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn invocation_preparation_retains_credits_through_unwind() {
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
    let (parts, mut observer) = input_parts(&budget);
    let storage = parts
        .storage
        .prepare(&synthetic_cov6::preparation_module(), "vecadd")
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _storage = storage;
        assert_eq!(budget.usage().reserved_peak_bytes, 32);
        panic!("after real preparation, before invocation admission");
    }));
    assert!(result.is_err());
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn invocation_storage_disposes_payload_before_decoder_even_during_unwind() {
    for unwind in [false, true] {
        let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
        let (parts, mut observer) = input_parts(&budget);
        let storage = parts
            .storage
            .prepare(&synthetic_cov6::preparation_module(), "vecadd")
            .unwrap();
        let disposed = Arc::new(AtomicBool::new(false));
        let witnessed = GeneratedRuntimeStorageV1 {
            payload: DisposalWitness {
                payload: Some(storage.payload),
                budget: budget.clone(),
                disposed: Arc::clone(&disposed),
                expected_bytes: 32,
            },
            readback: storage.readback,
            decoder: storage.decoder,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _storage = witnessed;
            if unwind {
                panic!("test field-drop ordering during unwind");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert!(disposed.load(Ordering::SeqCst));
        assert_empty(&budget);
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    }
}

#[test]
fn invocation_read_only_preparation_retains_credit_with_policy_copies() {
    let plan = tests::plan::<u32>(&[Access::ReadOnly], None);
    let input = GeneratedRuntimeReadSlice::new(vec![1u32, 2, 3, 4].into_boxed_slice());
    let budget = GeneratedRuntimeResultBudgetV1::new(16, 1).unwrap();
    let packed = prepare_charged_with_plan(
        input,
        &plan,
        limits(),
        &budget,
        |input, account| input.account_storage(account),
        |input, account| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![input.bind_argument(&plan, 0, account)?],
                ),
            )
        },
    )
    .unwrap();
    let storage = packed
        .into_runtime_inputs(geometry(), 0, 1000)
        .storage
        .prepare(&synthetic_cov6::preparation_module(), "vecadd")
        .unwrap();
    assert_eq!(budget.usage().reserved_peak_bytes, 16);
    assert_eq!(budget.usage().retained_members, 1);
    let storage = storage
        .project_persistent(&synthetic_cov6::preparation_module())
        .unwrap();
    assert_eq!(budget.usage().reserved_peak_bytes, 16);
    assert_eq!(budget.usage().retained_members, 1);
    assert_eq!(
        storage.prepared().buffer_access(0),
        Some(Gfx942RuntimeBufferAccessV1::ReadOnly)
    );
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn invocation_persistent_projection_retains_original_peak_and_no_output_authority() {
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
    let (parts, mut observer) = input_parts(&budget);
    let hsaco = synthetic_cov6::preparation_module();
    let storage = parts.storage.prepare(&hsaco, "vecadd").unwrap();
    let digest = storage.prepared().dispatch_contract_sha256();
    let usage = budget.usage();
    let storage = storage.project_persistent(&hsaco).unwrap();
    assert_eq!(storage.prepared().dispatch_contract_sha256(), digest);
    assert_eq!(budget.usage(), usage);
    assert!(observer.try_take().unwrap().is_none());
    drop(storage);
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn invocation_persistent_projection_disposes_storage_and_credits_on_early_and_late_rejection() {
    for late in [false, true] {
        let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
        let (parts, mut observer) = input_parts(&budget);
        let hsaco = if late {
            synthetic_cov6::service_preparation_module()
        } else {
            synthetic_cov6::preparation_module()
        };
        let storage = parts.storage.prepare(&hsaco, "vecadd").unwrap();
        let result = storage.project_persistent(if late { &hsaco } else { b"wrong artifact" });
        if late {
            assert!(matches!(
                result,
                Err(Gfx942RuntimeProjectionErrorV1::FixedDispatch(_))
            ));
        } else {
            assert!(matches!(
                result,
                Err(Gfx942RuntimeProjectionErrorV1::Preparation(_))
            ));
        }
        assert_empty(&budget);
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    }
}

#[test]
fn invocation_persistent_projection_disposes_payload_before_decoder_on_drop_and_unwind() {
    for unwind in [false, true] {
        let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
        let (parts, mut observer) = input_parts(&budget);
        let hsaco = synthetic_cov6::preparation_module();
        let storage = parts
            .storage
            .prepare(&hsaco, "vecadd")
            .unwrap()
            .project_persistent(&hsaco)
            .unwrap();
        let disposed = Arc::new(AtomicBool::new(false));
        let witnessed = GeneratedRuntimeStorageV1 {
            payload: DisposalWitness {
                payload: Some(storage.payload),
                budget: budget.clone(),
                disposed: Arc::clone(&disposed),
                expected_bytes: 32,
            },
            readback: storage.readback,
            decoder: storage.decoder,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _storage = witnessed;
            if unwind {
                panic!("after checked projection, before native adoption");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert!(disposed.load(Ordering::SeqCst));
        assert_empty(&budget);
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    }
}
