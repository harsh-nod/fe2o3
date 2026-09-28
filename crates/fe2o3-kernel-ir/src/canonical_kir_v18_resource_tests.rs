use super::*;
use crate::{
    CanonicalKernelIrWorkBudgetV1 as Work, MeteredKernelIrVerificationErrorV1 as MeteredError,
    StorageLayoutV1, StructurallyCheckedModuleStorageV1 as CheckedModule,
    StructurallyCheckedStorageLayoutsV1 as CheckedRows, VerificationStorageContextV1 as Context,
};
use sha2::Sha256;
use std::{collections::BTreeSet, mem::size_of};

const FLOOR: usize = 19;
const PRIOR: usize = 7;
const WIRE: usize = 20 + 4 + 1 + 4 + 4 + 4 + 4;
const COUNT: usize = 5 + 2 + 4;
const EMIT: usize = WIRE + 4;
const PARSE: usize = WIRE + 2 + 1;
const COMPARE: usize = WIRE + 10 + 4 + 1 + 1;
const DECODE: usize = PARSE + COUNT + COMPARE;
const LAYOUT: usize = 1 + 1 + 1 + 5;
const VERIFY: usize = 1 + 1 + 5;

fn codec_outcomes<T>() -> usize {
    2 * size_of::<T>() + 2 * size_of::<Result<T, KernelIrDecodeError>>()
}
fn codec_headers() -> usize {
    use crate::*;
    // The three opaque header sizes are pinned-host layout premises only.
    // Every multiplicity/envelope here is independently stated, not queried.
    let [writer, reader, writer_result] = crate::wire::storage_v18_tests::header_layout_premises();
    writer
        + reader
        + 2 * writer_result
        + 2 * size_of::<Result<crate::wire::KernelIrV12WireExtentV1, KernelIrEncodeError>>()
        + 2 * size_of::<Result<bool, KernelIrEncodeError>>()
        + 2 * size_of::<Result<(), KernelIrEncodeError>>()
        + codec_outcomes::<Module>()
        + codec_outcomes::<Function>()
        + codec_outcomes::<Signature>()
        + codec_outcomes::<FunctionBody>()
        + codec_outcomes::<Kernel>()
        + codec_outcomes::<BasicBlock>()
        + codec_outcomes::<Operation>()
        + codec_outcomes::<OperationKind>()
        + codec_outcomes::<ValueDef>()
        + codec_outcomes::<Terminator>()
        + codec_outcomes::<String>()
        + codec_outcomes::<BTreeSet<TargetCapability>>()
        + codec_outcomes::<BTreeSet<AddressSpace>>()
        + codec_outcomes::<TargetCapability>()
        + codec_outcomes::<InlineAssembly>()
        + codec_outcomes::<MatrixOperation>()
        + codec_outcomes::<TensorLayoutContractV1>()
        + codec_outcomes::<TensorFragmentLayoutV1>()
        + codec_outcomes::<StorageLayoutV1>()
        + codec_outcomes::<StorageFieldV1>()
        + codec_outcomes::<StorageVariantV1>()
        + codec_outcomes::<Vec<StorageLayoutV1>>()
        + codec_outcomes::<Vec<StorageFieldV1>>()
        + codec_outcomes::<Box<[StorageFieldV1]>>()
        + codec_outcomes::<Vec<StorageVariantV1>>()
        + codec_outcomes::<Box<[StorageVariantV1]>>()
        + codec_outcomes::<Vec<Function>>()
        + codec_outcomes::<Vec<Kernel>>()
        + codec_outcomes::<Vec<BasicBlock>>()
        + codec_outcomes::<Vec<Operation>>()
        + codec_outcomes::<Vec<ValueDef>>()
        + codec_outcomes::<Vec<ValueId>>()
        + codec_outcomes::<Vec<Type>>()
        + (crate::MAX_TYPE_DEPTH_V1 + 2) * codec_outcomes::<Type>()
}
fn owner_headers() -> usize {
    codec_headers()+size_of::<resource::Scope<'_,'_>>()
        +2*size_of::<Result<resource::Scope<'_,'_>,ResourceError>>()
        // Facade/admission/closure/finish/transfer input+return/match/caller slots.
        +8*size_of::<resource::Outcome>()
        +size_of::<CheckedModule<'_>>()+size_of::<VerifiedStorageKernelIrModuleV1<'_>>()
        +2*size_of::<Result<CheckedModule<'_>,StorageLayoutErrorV1>>()
        +2*size_of::<Result<VerifiedStorageKernelIrModuleV1<'_>,BorrowedKernelIrVerificationErrorV1>>()
        +2*size_of::<Result<usize,ResourceError>>()
        +2*size_of::<Result<Module,KernelIrDecodeError>>()
        +2*size_of::<Result<Vec<u8>,KernelIrEncodeError>>()
        +2*size_of::<Result<(),CanonicalKernelIrReplayAdmissionErrorV18>>()
        +2*size_of::<Result<(),ResourceError>>()
        +2*size_of::<Result<VerifiedCanonicalKernelIrIdentityV18,ResourceError>>()
        +2*size_of::<Sha256>()+2*32
}
fn layout_headers() -> usize {
    let [scratch, scratch_result, vec_frame, allocation_frame, frame] =
        crate::verification_storage_v1::canonical_storage_header_layout_premises_v18();
    let allocation = size_of::<Result<Vec<u8>, StorageLayoutErrorV1>>()
        .max(size_of::<Result<Vec<usize>, StorageLayoutErrorV1>>())
        .max(size_of::<Result<Vec<(u64, u64)>, StorageLayoutErrorV1>>())
        .max(size_of::<Result<Vec<u128>, StorageLayoutErrorV1>>())
        .max(allocation_frame);
    let core = scratch
        + 2 * scratch_result
        + 2 * size_of::<Vec<u8>>()
        + 2 * vec_frame
        + 2 * size_of::<Vec<usize>>()
        + 2 * size_of::<Vec<(u64, u64)>>()
        + 2 * size_of::<Vec<u128>>()
        + 2 * allocation
        + 2 * size_of::<Result<(), StorageLayoutErrorV1>>()
        + 2 * size_of::<Result<(), ResourceError>>()
        + 2 * size_of::<Result<CheckedRows<'_>, StorageLayoutErrorV1>>()
        + 2 * frame;
    let module = crate::storage_module_v1::canonical_storage_header_layout_premise_v18()
        + size_of::<CheckedModule<'_>>()
        + 2 * size_of::<Result<CheckedModule<'_>, StorageLayoutErrorV1>>()
        + 2 * size_of::<Result<CheckedRows<'_>, StorageLayoutErrorV1>>()
        + 2 * size_of::<Result<(), ResourceError>>();
    module + core
}
fn verifier_headers() -> usize {
    crate::verification_storage_module_v1::canonical_storage_header_layout_premise_v18()
        + size_of::<CheckedModule<'_>>()
        + size_of::<VerifiedStorageKernelIrModuleV1<'_>>()
        + 6 * size_of::<Context<'_, '_>>()
        + 2 * size_of::<
            Result<VerifiedStorageKernelIrModuleV1<'_>, BorrowedKernelIrVerificationErrorV1>,
        >()
        + 2 * size_of::<Result<(), BorrowedKernelIrVerificationErrorV1>>()
        + 2 * size_of::<Result<(), MeteredError>>()
        + 2 * size_of::<Result<(), ResourceError>>()
}
fn hash_work() -> usize {
    4 + VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_DOMAIN_V1.len() + 2 + 8 + WIRE
}
fn exact_work(bytes: bool) -> usize {
    1 + DECODE + LAYOUT + VERIFY + hash_work() + if bytes { WIRE } else { COUNT + EMIT }
}
fn retained() -> usize {
    size_of::<VerifiedCanonicalKernelIrModuleV18>() + 1 + WIRE
}
fn exact_peak(bytes: bool) -> usize {
    let simultaneous = layout_headers().max(verifier_headers());
    owner_headers()
        + size_of::<VerifiedCanonicalKernelIrModuleV18>()
        + 1
        + if bytes {
            simultaneous.max(WIRE)
        } else {
            WIRE + simultaneous
        }
}
fn tiny_bytes() -> Vec<u8> {
    // Literal source-derived V18 empty Module("m"), not a successful receipt.
    let mut bytes = b"FE2O3KI\0".to_vec();
    bytes.extend_from_slice(&[
        18, 0, 0, 0, 41, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, b'm', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ]);
    assert_eq!(bytes.len(), WIRE);
    bytes
}
struct Observed {
    result: resource::Outcome,
    work: usize,
    storage: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn run(bytes: bool, work_limit: usize, storage_limit: usize) -> Observed {
    let mut work = Work::new(PRIOR + work_limit);
    work.charge_work(PRIOR).unwrap();
    let mut budget = Budget::new(&mut work, FLOOR + storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = if bytes {
        VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
            &tiny_bytes(),
            super::tests::LIMITS,
            &mut budget,
        )
    } else {
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &Module::new("m"),
            super::tests::LIMITS,
            &mut budget,
        )
    };
    Observed {
        result,
        work: budget.work(),
        storage: budget.storage(),
        peak: budget.peak_storage(),
        failed_work: budget.work_budget_v1().failed_work(),
        failed_storage: budget.failed_storage(),
    }
}
fn resource_error(error: &CanonicalKernelIrReplayAdmissionErrorV18) -> Option<ResourceError> {
    match error {
        CanonicalKernelIrReplayAdmissionErrorV18::Resource(error) => Some(*error),
        CanonicalKernelIrReplayAdmissionErrorV18::Layout(StorageLayoutErrorV1::Resource(error)) => {
            Some(*error)
        }
        CanonicalKernelIrReplayAdmissionErrorV18::Verification(
            BorrowedKernelIrVerificationErrorV1::Resource(error),
        ) => Some(*error),
        CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::Resource(error)) => {
            Some(*error)
        }
        CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::WorkLimit(error))
        | CanonicalKernelIrReplayAdmissionErrorV18::Encode(KernelIrEncodeError::WorkLimit(error)) => {
            Some(ResourceError::Work(*error))
        }
        _ => None,
    }
}

#[test]
fn canonical_storage_headers_match_independent_typed_slot_roster() {
    assert_eq!(resource::headers().unwrap(), owner_headers());
    assert_eq!(
        crate::wire::storage_codec_headers_v18().unwrap(),
        codec_headers()
    );
}
#[test]
fn canonical_storage_whole_empty_entries_have_exact_source_work_retained_and_live_peak() {
    assert_eq!(
        (WIRE, COUNT, EMIT, PARSE, COMPARE, DECODE, LAYOUT, VERIFY),
        (41, 11, 45, 44, 57, 112, 8, 7)
    );
    for bytes in [false, true] {
        let observed = run(bytes, exact_work(bytes), exact_peak(bytes));
        let (owner, receipt) = observed.result.unwrap();
        assert_eq!(owner.module(), &Module::new("m"));
        assert_eq!(owner.canonical_bytes(), tiny_bytes());
        assert_eq!(receipt.retained_storage(), retained());
        assert_eq!(
            (observed.work, observed.storage, observed.peak),
            (PRIOR + exact_work(bytes), FLOOR, FLOOR + exact_peak(bytes))
        );
        assert_eq!(
            (observed.failed_work, observed.failed_storage),
            (None, None)
        );
    }
}
#[test]
fn canonical_storage_whole_one_short_final_hash_keeps_atomic_work_prefix() {
    for bytes in [false, true] {
        let exact = exact_work(bytes);
        let observed = run(bytes, exact - 1, exact_peak(bytes));
        let error = resource_error(observed.result.as_ref().unwrap_err()).unwrap();
        assert!(
            matches!(error,ResourceError::Work(error) if error.actual()==PRIOR+exact && error.limit()==PRIOR+exact-1)
        );
        assert_eq!(observed.work, PRIOR + exact - hash_work());
        assert_eq!(observed.failed_work, Some(PRIOR + exact));
        assert_eq!(observed.failed_storage, None);
        assert_eq!(
            (observed.storage, observed.peak),
            (FLOOR, FLOOR + exact_peak(bytes))
        );
    }
}
#[test]
fn canonical_storage_whole_one_short_live_peak_refuses_before_hash_and_restores_floor() {
    for bytes in [false, true] {
        let peak = exact_peak(bytes);
        let observed = run(bytes, exact_work(bytes), peak - 1);
        let error = resource_error(observed.result.as_ref().unwrap_err()).unwrap();
        assert!(
            matches!(error,ResourceError::Storage(error) if error.actual()==FLOOR+peak && error.limit()==FLOOR+peak-1)
        );
        assert_eq!(observed.failed_storage, Some(FLOOR + peak));
        assert_eq!(observed.failed_work, None);
        let before_check = 1 + DECODE + if bytes { 0 } else { COUNT + EMIT };
        let accepted = if layout_headers() >= verifier_headers() {
            before_check + 2
        } else {
            before_check + LAYOUT + 1
        };
        assert_eq!(observed.work, PRIOR + accepted);
        assert_eq!(observed.storage, FLOOR);
        assert!(observed.peak < FLOOR + peak);
    }
}
#[test]
fn canonical_storage_entry_and_header_refusals_do_not_begin_codec() {
    for bytes in [false, true] {
        let denied = run(bytes, 0, exact_peak(bytes));
        assert_eq!(
            (denied.work, denied.storage, denied.peak),
            (PRIOR, FLOOR, FLOOR)
        );
        assert_eq!(denied.failed_work, Some(PRIOR + 1));
        let denied = run(bytes, exact_work(bytes), owner_headers() - 1);
        assert_eq!(
            (denied.work, denied.storage, denied.peak),
            (PRIOR + 1, FLOOR, FLOOR)
        );
        assert_eq!(denied.failed_storage, Some(FLOOR + owner_headers()));
        assert!(matches!(
            resource_error(denied.result.as_ref().unwrap_err()),
            Some(ResourceError::Storage(_))
        ));
    }
}
#[test]
fn canonical_storage_success_preserves_first_prior_denials_and_sibling_floor() {
    for bytes in [false, true] {
        let exact = exact_work(bytes);
        let peak = exact_peak(bytes);
        let mut work = Work::new(PRIOR + exact);
        work.charge_work(PRIOR).unwrap();
        assert!(work.charge_work(exact + 33).is_err());
        let mut budget = Budget::new(&mut work, FLOOR + peak);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.reserve_storage(peak + 29).is_err());
        let result = if bytes {
            VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
                &tiny_bytes(),
                super::tests::LIMITS,
                &mut budget,
            )
        } else {
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &Module::new("m"),
                super::tests::LIMITS,
                &mut budget,
            )
        };
        assert!(result.is_ok());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (PRIOR + exact, FLOOR, FLOOR + peak)
        );
        assert_eq!(
            budget.work_budget_v1().failed_work(),
            Some(PRIOR + exact + 33)
        );
        assert_eq!(budget.failed_storage(), Some(FLOOR + peak + 29));
    }
}
#[test]
fn canonical_storage_resource_overflow_retains_real_maximum_floor() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(usize::MAX).unwrap();
    let result = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &Module::new("m"),
        super::tests::LIMITS,
        &mut budget,
    );
    assert!(matches!(
        resource_error(result.as_ref().unwrap_err()),
        Some(ResourceError::Storage(_))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, usize::MAX, usize::MAX)
    );
}

#[test]
fn canonical_storage_actual_facades_unwind_after_owned_stages_and_restore_floor() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for from_bytes in [false, true] {
        for stage in 1..=4 {
            let module = super::tests::fixture(crate::AddressSpace::Private);
            let bytes = super::tests::admit(&module).canonical_bytes().to_vec();
            let mut work = Work::new(1_000_000);
            assert!(work.charge_work(1_000_001).is_err());
            let mut budget = Budget::new(&mut work, 1_000_000);
            budget.reserve_storage(FLOOR).unwrap();
            assert!(budget.reserve_storage(1_000_001).is_err());
            admission::set_panic_stage(stage);
            let result = catch_unwind(AssertUnwindSafe(|| {
                if from_bytes {
                    VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
                        &bytes, super::tests::LIMITS, &mut budget)
                } else {
                    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                        &module,
                        super::tests::LIMITS,
                        &mut budget,
                    )
                }
            }));
            admission::set_panic_stage(0);
            assert!(result.is_err(), "entry {from_bytes}, stage {stage}");
            assert_eq!(budget.storage(), FLOOR);
            assert!(budget.peak_storage() > FLOOR);
            assert!(budget.work() > 1);
            assert_eq!(budget.work_budget_v1().failed_work(), Some(1_000_001));
            assert_eq!(budget.failed_storage(), Some(FLOOR + 1_000_001));
            assert_eq!(module, super::tests::fixture(crate::AddressSpace::Private));
        }
    }
}

fn scalar_bytes() -> Vec<u8> {
    let mut bytes = tiny_bytes();
    bytes[12..16].copy_from_slice(&55_u32.to_le_bytes());
    bytes[37..41].copy_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes.extend_from_slice(&8_u32.to_le_bytes());
    bytes.extend_from_slice(&[1, 9]); // Scalar row, U64.
    assert_eq!(bytes.len(), 55);
    bytes
}
fn scalar_work(bytes: bool) -> usize {
    let wire = WIRE + 14;
    let count = COUNT + 1 + 4;
    let emit = wire + 1 + 4;
    let parse = wire + 2 + 1 + 1;
    let compare = wire + (10 + 4) + 1 + 4 + 1 + 1;
    let decode = parse + count + compare;
    let layout = 1 + 15; // Wrapper plus independently declared single-scalar core schedule.
    let hash = 4 + VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_DOMAIN_V1.len() + 2 + 8 + wire;
    assert_eq!(
        (wire, count, emit, parse, compare, decode, layout),
        (55, 16, 60, 59, 76, 151, 16)
    );
    1 + decode + layout + VERIFY + hash + if bytes { wire } else { count + emit }
}
fn scalar_peak(bytes: bool) -> usize {
    let frame = crate::verification_storage_v1::canonical_storage_header_layout_premises_v18()[4];
    let scratch = size_of::<u8>() + frame + size_of::<usize>();
    let simultaneous = (layout_headers() + scratch).max(verifier_headers());
    owner_headers()
        + size_of::<VerifiedCanonicalKernelIrModuleV18>()
        + 1
        + size_of::<StorageLayoutV1>()
        + if bytes {
            simultaneous.max(55)
        } else {
            55 + simultaneous
        }
}
fn run_scalar(bytes: bool, work_limit: usize, storage_limit: usize) -> Observed {
    let mut work = Work::new(PRIOR + work_limit);
    work.charge_work(PRIOR).unwrap();
    let mut budget = Budget::new(&mut work, FLOOR + storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = if bytes {
        VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
            &scalar_bytes(),
            super::tests::LIMITS,
            &mut budget,
        )
    } else {
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &super::tests::small_module(),
            super::tests::LIMITS,
            &mut budget,
        )
    };
    Observed {
        result,
        work: budget.work(),
        storage: budget.storage(),
        peak: budget.peak_storage(),
        failed_work: budget.work_budget_v1().failed_work(),
        failed_storage: budget.failed_storage(),
    }
}
#[test]
fn canonical_storage_nonempty_table_whole_entries_have_independent_exact_budgets() {
    // Exact-capacity one-element Vec premises match the separately tested core
    // allocator contract. These are logical payload bytes, not allocator RSS.
    for bytes in [false, true] {
        let observed = run_scalar(bytes, scalar_work(bytes), scalar_peak(bytes));
        let (owner, receipt) = observed.result.unwrap();
        assert_eq!(owner.module(), &super::tests::small_module());
        assert_eq!(owner.canonical_bytes(), scalar_bytes());
        assert_eq!(
            receipt.retained_storage(),
            size_of::<VerifiedCanonicalKernelIrModuleV18>() + 1 + size_of::<StorageLayoutV1>() + 55
        );
        assert_eq!(
            (observed.work, observed.storage, observed.peak),
            (
                PRIOR + scalar_work(bytes),
                FLOOR,
                FLOOR + scalar_peak(bytes)
            )
        );
        assert_eq!(
            (observed.failed_work, observed.failed_storage),
            (None, None)
        );
        let denied = run_scalar(bytes, scalar_work(bytes) - 1, scalar_peak(bytes));
        let hash = 4 + VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_DOMAIN_V1.len() + 2 + 8 + 55;
        assert_eq!(denied.work, PRIOR + scalar_work(bytes) - hash);
        assert_eq!(denied.failed_work, Some(PRIOR + scalar_work(bytes)));
        assert_eq!(denied.storage, FLOOR);
        assert!(matches!(
            resource_error(denied.result.as_ref().unwrap_err()),
            Some(ResourceError::Work(_))
        ));
    }
}
#[test]
fn canonical_storage_nonempty_table_one_short_peak_is_the_source_derived_live_owner_boundary() {
    let frame = crate::verification_storage_v1::canonical_storage_header_layout_premises_v18()[4];
    let scratch = 1 + frame + size_of::<usize>();
    for bytes in [false, true] {
        let peak = scalar_peak(bytes);
        let denied = run_scalar(bytes, scalar_work(bytes), peak - 1);
        assert!(
            matches!(resource_error(denied.result.as_ref().unwrap_err()),Some(ResourceError::Storage(error)) if error.actual()==FLOOR+peak)
        );
        assert_eq!(denied.failed_storage, Some(FLOOR + peak));
        let before_check = 1 + 151 + if bytes { 0 } else { 16 + 60 };
        let accepted = if layout_headers() + scratch >= verifier_headers() {
            before_check + 1 + 12 // Last heights allocation: wrapper1, core accepted12.
        } else {
            before_check + 16 + 1
        };
        assert_eq!(denied.work, PRIOR + accepted);
        assert_eq!(denied.storage, FLOOR);
        assert_eq!(denied.failed_work, None);
        assert!(denied.peak < FLOOR + peak);
    }
}
