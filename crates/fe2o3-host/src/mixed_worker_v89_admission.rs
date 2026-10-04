//! Exact V89 host readmission; signatures and content checks grant no execution authority.
use fe2o3_compiler_lineage::{
    MIXED_MIDDLE_END_WORKING_STORAGE_V90 as MIDDLE_END_WORKING_STORAGE,
    MixedMiddleEndRefV90 as MiddleEndRef, read_mixed_middle_end_v90 as read_middle_end,
};
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V89 as NOMINAL_DESCRIPTOR_SCRATCH_STORAGE,
    NominalDescriptorInspectionV89 as NominalDescriptorInspection,
    derive_unfinalized_nominal_hsaco_on_budget_v89 as derive_unfinalized_nominal_hsaco_on_budget,
    inspect_finalized_nominal_hsaco_v89 as inspect_finalized_nominal_hsaco,
};
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V89 as CANONICAL_CODE_OBJECT_DIGEST_OFFSET,
    CanonicalCodeObjectDigest,
    MIXED_DESCRIPTOR_READER_STORAGE_V89 as MIXED_DESCRIPTOR_READER_STORAGE,
    MixedDescriptorTableV89 as MixedDescriptorTable,
    decode_mixed_descriptor_v89 as decode_mixed_descriptor,
    mixed_conditional_v86::MIXED_CONTRACT_CODEC_STORAGE_V86 as CONTRACT_CODEC_STORAGE,
};
use fe2o3_verifier::check_mixed_native_correspondence_v89 as check_native_correspondence;
use fe2o3_verifier::{
    InertPredicatedTypedSourceReceiptV90 as TypedSourceReceipt,
    check_inert_predicated_typed_source_receipt_v90 as check_typed_source_receipt,
};
fn typed_receipt_error(error: fe2o3_verifier::MixedOptimizerRelocationErrorV28) -> AdmissionError {
    AdmissionError::MixedReceiptV90(error)
}
const DESCRIPTOR_SCHEMA: u16 = 89;
const ROSTER_DEBUG_NAME: &str = "RecoveredMixedWorkerV89PinnedRoster";
const LINEAGE_DOMAIN: &[u8] = b"fe2o3.host.worker-v3-mixed-descriptor-lineage.v89\0";
const SCHEMA_REFUSAL: &str = "descriptor schema must be V89; no legacy retry";
const TARGET_REFUSAL: &str = "unsupported inspected V89 target";
const ABI_REFUSAL: &str = "mandatory V89 ABI/formal-memory equality";
fn binding(detail: &'static str) -> AdmissionError {
    AdmissionError::MixedV89(detail)
}
fn codec_error(error: impl std::error::Error + Send + Sync + 'static) -> AdmissionError {
    AdmissionError::MixedCodecV89(Box::new(error))
}

#[path = "mixed_worker_v89_preparation.rs"]
mod preparation;
pub use preparation::{MixedWorkerV89PreparationError, PreparedMixedWorkerV89Invocation};
#[path = "mixed_worker_v89_target_readmission.rs"]
mod target_readmission;
include!("mixed_worker_admission_family.rs");
pub use MixedWorkerVerificationRequest as MixedWorkerV89VerificationRequest;
/// Move-only predicated publication; no clone, load, or legacy conversion.
/// ```compile_fail
/// use fe2o3_host::RecoveredMixedWorkerV89PinnedRoster;
/// fn clone<R>(v: RecoveredMixedWorkerV89PinnedRoster<R>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::RecoveredMixedWorkerV89PinnedRoster;
/// fn load<R>(v: RecoveredMixedWorkerV89PinnedRoster<R>) { v.load(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::{RecoveredMixedWorkerV89PinnedRoster, RecoveredMixedWorkerV53PinnedRoster};
/// fn convert<R>(v: RecoveredMixedWorkerV89PinnedRoster<R>) -> RecoveredMixedWorkerV53PinnedRoster<R> { v }
/// ```
pub use RecoveredMixedWorkerPinnedRoster as RecoveredMixedWorkerV89PinnedRoster;
pub use admit_recovered_mixed_worker_roster as admit_recovered_mixed_worker_v89_roster;

#[cfg(test)]
#[path = "mixed_worker_v89_native_lineage_v89_tests.rs"]
mod native_lineage_tests_v60;

#[cfg(test)]
mod codec_tests_v53 {
    use super::*;

    fn resource(error: &AdmissionError) -> Resource {
        *std::error::Error::source(error)
            .unwrap()
            .downcast_ref::<Resource>()
            .unwrap()
    }

    #[test]
    fn mixed_v53_host_codec_scope_prepays_exact_coexisting_headers_and_preserves_denials() {
        fn inspect(budget: &mut Budget<'_>) -> Result<u32> {
            budget.charge_work(7)?;
            Ok(23)
        }
        type Callback = fn(&mut Budget<'_>) -> Result<u32>;
        let frame = 37
            + size_of::<Callback>()
            + size_of::<Budget<'_>>()
            + 3 * size_of::<usize>()
            + size_of::<u32>()
            + 2 * size_of::<Result<u32>>()
            + size_of::<std::result::Result<Result<u32>, Box<dyn std::any::Any + Send>>>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = Budget::new(&mut work, 19 + frame);
        budget.reserve_storage(19).unwrap();
        assert_eq!(
            codec_on_budget(&mut budget, 37, inspect as Callback).unwrap(),
            23
        );
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (8, 19, 19 + frame)
        );

        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = Budget::new(&mut work, 18 + frame);
        budget.reserve_storage(19).unwrap();
        let first = codec_on_budget(&mut budget, 37, inspect as Callback).unwrap_err();
        assert!(matches!(resource(&first), Resource::Storage(e)
            if e.actual() == 19 + frame && e.limit() == 18 + frame));
        let second = codec_on_budget(&mut budget, 37, inspect as Callback).unwrap_err();
        assert_eq!(resource(&first), resource(&second));
        assert_eq!((budget.work(), budget.storage()), (1, 19));

        let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
        let mut budget = Budget::new(&mut work, 19 + frame);
        budget.reserve_storage(19).unwrap();
        let error = codec_on_budget(&mut budget, 37, inspect as Callback).unwrap_err();
        assert!(matches!(resource(&error), Resource::Work(e)
            if e.actual() == 8 && e.limit() == 7));
        assert_eq!((budget.work(), budget.storage()), (1, 19));
    }

    #[test]
    fn mixed_v53_host_codec_scope_retires_scratch_on_error_and_unwind() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, HOST_CODEC_STORAGE_LIMIT);
        budget.reserve_storage(19).unwrap();
        let error = codec_on_budget::<(), _>(&mut budget, 37, |budget| {
            budget.charge_work(5)?;
            Err(binding("test codec refusal"))
        });
        assert!(matches!(
            error,
            Err(AdmissionError::MixedV89("test codec refusal"))
        ));
        assert_eq!((budget.work(), budget.storage()), (6, 19));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            codec_on_budget::<(), _>(&mut budget, 37, |budget| {
                budget.charge_work(5)?;
                panic!("test codec unwind")
            })
        }));
        assert!(panic.is_err());
        assert_eq!((budget.work(), budget.storage()), (12, 19));
    }

    #[test]
    fn mixed_v53_host_codec_work_is_cumulative_finite_and_overflow_checked() {
        let mut charge = codec_work();
        charge(HOST_CODEC_WORK_LIMIT - 1).unwrap();
        charge(1).unwrap();
        let error = charge(1).expect_err("no unbounded artifact codec work");
        assert_eq!(error.limit(), HOST_CODEC_WORK_LIMIT);
        assert_eq!(error.actual(), HOST_CODEC_WORK_LIMIT + 1);
        let error = charge(usize::MAX).expect_err("checked cumulative arithmetic");
        assert_eq!(error.actual(), usize::MAX);
    }

    #[test]
    fn mixed_v53_host_codec_refusal_retains_the_exact_resource_error() {
        fn require_send_sync<T: Send + Sync>() {}
        require_send_sync::<AdmissionError>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
        work.charge_work(2).unwrap();
        let expected = work.charge_work(2).unwrap_err();
        let error = codec_error(expected);
        let actual = std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<CanonicalKernelIrWorkLimitV1>()
            .unwrap();
        assert_eq!(*actual, expected);
        assert_eq!((actual.actual(), actual.limit()), (4, 3));
    }
}
