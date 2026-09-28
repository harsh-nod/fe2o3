use super::*;
use crate::{
    AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function,
    FunctionRole, ModuleId, OperationKind, ScalarType, Signature, StorageLayoutKindV1,
    StorageLayoutV1, TargetCapability, Terminator, Type,
};
use std::{cell::Cell, collections::BTreeSet, mem::size_of};

thread_local! { static PANIC_STAGE: Cell<u8> = const { Cell::new(0) }; }
pub(super) fn checkpoint(stage: u8) {
    PANIC_STAGE.with(|slot| {
        if slot.get() == stage {
            slot.set(0);
            panic!("V18 copy checkpoint {stage}");
        }
    });
}

const FLOOR: usize = 19;
const PRIOR: usize = 7;
const WIRE: usize = 20 + 4 + 1 + 4 + 4 + 4 + 4;
const COUNT: usize = 5 + 2 + 4;
const PARSE: usize = WIRE + 2 + 1;
const COMPARE: usize = WIRE + 10 + 4 + 1 + 1;

fn codec_outcomes<T>() -> usize {
    2 * size_of::<T>() + 2 * size_of::<Result<T, KernelIrDecodeError>>()
}
fn codec_headers() -> usize {
    use crate::*;
    // Only opaque Rust layout sizes are premises; all slot counts are stated here.
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
fn headers() -> usize {
    type Outcome = Result<
        (Module, CanonicalKernelIrCandidateStorageV18),
        CanonicalKernelIrReplayAdmissionErrorV18,
    >;
    codec_headers()
        + size_of::<resource::Scope<'_, '_>>()
        + 2 * size_of::<Result<resource::Scope<'_, '_>, ResourceError>>()
        + 4 * size_of::<Outcome>()
        + 2 * size_of::<Result<Module, KernelIrDecodeError>>()
        + 2 * size_of::<Result<usize, ResourceError>>()
        + 2 * size_of::<Result<(), ResourceError>>()
}
fn wire_len(scalar: bool) -> usize {
    WIRE + if scalar { 8 + 4 + 1 + 1 } else { 0 }
}
fn literal_bytes(scalar: bool) -> Vec<u8> {
    let mut bytes = b"FE2O3KI\0".to_vec();
    bytes.extend_from_slice(&[
        18, 0, 0, 0, 41, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, b'm', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ]);
    if scalar {
        bytes[12..16].copy_from_slice(&55_u32.to_le_bytes());
        bytes[37..41].copy_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&8_u64.to_le_bytes());
        bytes.extend_from_slice(&8_u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 9]);
    }
    bytes
}
fn copy_work(scalar: bool) -> usize {
    let parse = PARSE + if scalar { 14 + 1 } else { 0 };
    let count = COUNT + if scalar { 1 + 4 } else { 0 };
    let compare = COMPARE + if scalar { 14 + 1 + 4 } else { 0 };
    1 + parse + count + compare
}
fn payload(scalar: bool) -> usize {
    1 + if scalar {
        size_of::<StorageLayoutV1>()
    } else {
        0
    }
}
fn retained(scalar: bool) -> usize {
    size_of::<Module>() + payload(scalar)
}
fn owner_retained(scalar: bool) -> usize {
    size_of::<VerifiedCanonicalKernelIrModuleV18>() + payload(scalar) + wire_len(scalar)
}
fn peak(scalar: bool) -> usize {
    headers() + retained(scalar)
}
fn fixture(scalar: bool) -> Module {
    if scalar {
        super::tests::small_module()
    } else {
        Module::new("m")
    }
}
fn source(
    module: &Module,
) -> (
    VerifiedCanonicalKernelIrModuleV18,
    CanonicalKernelIrReplayStorageV18,
) {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        module,
        super::tests::LIMITS,
        &mut budget,
    )
    .unwrap()
}
struct Observed {
    result: resource::CopyOutcome,
    work: usize,
    storage: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn run(scalar: bool, work_limit: usize, storage_limit: usize) -> Observed {
    let (owner, owner_receipt) = source(&fixture(scalar));
    assert_eq!(owner_receipt.retained_storage(), owner_retained(scalar));
    assert_eq!(owner.canonical_bytes().len(), wire_len(scalar));
    assert_eq!(owner.canonical_bytes(), literal_bytes(scalar));
    let floor = FLOOR + owner_retained(scalar);
    let mut work = Work::new(PRIOR + work_limit);
    work.charge_work(PRIOR).unwrap();
    let mut budget = Budget::new(&mut work, floor + storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = owner.copy_module_for_transformation_v18(&mut budget);
    assert_eq!(owner.module(), &fixture(scalar));
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
        CanonicalKernelIrReplayAdmissionErrorV18::Resource(error)
        | CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::Resource(error)) => {
            Some(*error)
        }
        CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::WorkLimit(error))
        | CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::Encode(
            KernelIrEncodeError::WorkLimit(error),
        )) => Some(ResourceError::Work(*error)),
        _ => None,
    }
}

#[test]
fn canonical_copy_empty_and_scalar_have_independent_exact_work_storage_and_receipts() {
    assert_eq!((WIRE, PARSE, COUNT, COMPARE), (41, 44, 11, 57));
    assert_eq!((copy_work(false), copy_work(true)), (113, 152));
    assert_eq!(headers(), resource::copy_headers().unwrap());
    assert_eq!(
        codec_headers(),
        crate::wire::storage_codec_headers_v18().unwrap()
    );
    for scalar in [false, true] {
        let observed = run(scalar, copy_work(scalar), peak(scalar));
        let (candidate, receipt) = observed.result.unwrap();
        assert_eq!(candidate, fixture(scalar));
        assert_eq!(receipt.retained_storage(), retained(scalar));
        let floor = FLOOR + owner_retained(scalar);
        assert_eq!(
            (observed.work, observed.storage, observed.peak),
            (PRIOR + copy_work(scalar), floor, floor + peak(scalar))
        );
        assert_eq!(
            (observed.failed_work, observed.failed_storage),
            (None, None)
        );
    }
}

#[test]
fn canonical_copy_one_short_work_denies_final_comparison_without_losing_history() {
    for scalar in [false, true] {
        let exact = copy_work(scalar);
        let observed = run(scalar, exact - 1, peak(scalar));
        assert!(matches!(
            observed.result.as_ref().unwrap_err(),
            CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::Encode(
                KernelIrEncodeError::WorkLimit(_)
            ))
        ));
        assert!(
            matches!(resource_error(observed.result.as_ref().unwrap_err()), Some(ResourceError::Work(error)) if error.actual() == PRIOR + exact && error.limit() == PRIOR + exact - 1)
        );
        assert_eq!(observed.work, PRIOR + exact - 1);
        assert_eq!(observed.failed_work, Some(PRIOR + exact));
        assert_eq!(observed.failed_storage, None);
        let floor = FLOOR + owner_retained(scalar);
        assert_eq!(
            (observed.storage, observed.peak),
            (floor, floor + peak(scalar))
        );
    }
}

#[test]
fn canonical_copy_one_short_storage_denies_exact_owned_payload_boundary() {
    for scalar in [false, true] {
        let observed = run(scalar, copy_work(scalar), peak(scalar) - 1);
        let floor = FLOOR + owner_retained(scalar);
        assert!(
            matches!(resource_error(observed.result.as_ref().unwrap_err()), Some(ResourceError::Storage(error)) if error.actual() == floor + peak(scalar) && error.limit() == floor + peak(scalar) - 1)
        );
        // ID allocation follows header20, count4, byte1 and UTF-8/copy2.
        // A row allocation additionally follows three count4s and row count4/check1.
        let accepted = 1 + 20 + 4 + 1 + 2 + if scalar { 3 * 4 + 4 + 1 } else { 0 };
        assert_eq!(observed.work, PRIOR + accepted);
        assert_eq!(observed.failed_storage, Some(floor + peak(scalar)));
        assert_eq!(observed.failed_work, None);
        assert_eq!(observed.storage, floor);
        assert_eq!(
            observed.peak,
            floor + headers() + size_of::<Module>() + usize::from(scalar)
        );
    }
}

#[test]
fn canonical_copy_entry_headers_and_module_reservations_are_separate_denial_phases() {
    for scalar in [false, true] {
        let floor = FLOOR + owner_retained(scalar);
        let denied = run(scalar, 0, peak(scalar));
        assert_eq!(
            (denied.work, denied.storage, denied.peak),
            (PRIOR, floor, floor)
        );
        assert_eq!(denied.failed_work, Some(PRIOR + 1));
        assert!(matches!(
            resource_error(denied.result.as_ref().unwrap_err()),
            Some(ResourceError::Work(_))
        ));
        for (limit, attempted, old_peak) in [
            (headers() - 1, headers(), 0),
            (
                headers() + size_of::<Module>() - 1,
                headers() + size_of::<Module>(),
                headers(),
            ),
        ] {
            let denied = run(scalar, copy_work(scalar), limit);
            assert!(
                matches!(resource_error(denied.result.as_ref().unwrap_err()), Some(ResourceError::Storage(error)) if error.actual() == floor + attempted)
            );
            assert_eq!(
                (denied.work, denied.storage, denied.peak),
                (PRIOR + 1, floor, floor + old_peak)
            );
            assert_eq!(denied.failed_storage, Some(floor + attempted));
            assert_eq!(denied.failed_work, None);
        }
    }
}

#[test]
fn canonical_copy_paid_source_sibling_floor_and_prior_denials_survive_receipt_transfer() {
    for scalar in [false, true] {
        let (owner, owner_receipt) = source(&fixture(scalar));
        let floor = FLOOR + owner_receipt.retained_storage();
        let exact = copy_work(scalar);
        let mut work = Work::new(PRIOR + exact);
        work.charge_work(PRIOR).unwrap();
        assert!(work.charge_work(exact + 33).is_err());
        let mut budget = Budget::new(&mut work, floor + peak(scalar));
        budget.reserve_storage(FLOOR).unwrap();
        budget
            .reserve_storage(owner_receipt.retained_storage())
            .unwrap();
        assert!(budget.reserve_storage(peak(scalar) + 29).is_err());
        let (candidate, receipt) = owner
            .copy_module_for_transformation_v18(&mut budget)
            .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor + retained(scalar));
        assert_eq!(candidate, *owner.module());
        assert!(!std::ptr::eq(
            candidate.id.as_str().as_ptr(),
            owner.module().id.as_str().as_ptr()
        ));
        if scalar {
            assert_ne!(
                candidate.storage_layouts.as_ptr(),
                owner.module().storage_layouts.as_ptr()
            );
        }
        drop(owner);
        budget
            .release_storage(owner_receipt.retained_storage())
            .unwrap();
        assert_eq!(candidate, fixture(scalar));
        drop(candidate);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), PRIOR + exact);
        assert_eq!(budget.peak_storage(), floor + peak(scalar));
        assert_eq!(
            budget.work_budget_v1().failed_work(),
            Some(PRIOR + exact + 33)
        );
        assert_eq!(budget.failed_storage(), Some(floor + peak(scalar) + 29));
    }
}

#[test]
fn canonical_copy_actual_owned_stages_unwind_to_paid_source_floor() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for scalar in [false, true] {
        for stage in 1..=3 {
            let (owner, owner_receipt) = source(&fixture(scalar));
            let identity = *owner.identity();
            let floor = FLOOR + owner_receipt.retained_storage();
            let mut work = Work::new(PRIOR + copy_work(scalar));
            work.charge_work(PRIOR).unwrap();
            assert!(work.charge_work(copy_work(scalar) + 33).is_err());
            let mut budget = Budget::new(&mut work, floor + peak(scalar));
            budget.reserve_storage(floor).unwrap();
            assert!(budget.reserve_storage(peak(scalar) + 29).is_err());
            PANIC_STAGE.with(|slot| slot.set(stage));
            let result = catch_unwind(AssertUnwindSafe(|| {
                owner.copy_module_for_transformation_v18(&mut budget)
            }));
            PANIC_STAGE.with(|slot| slot.set(0));
            assert!(result.is_err(), "scalar {scalar}, stage {stage}");
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                budget.work(),
                PRIOR + if stage == 1 { 1 } else { copy_work(scalar) }
            );
            assert_eq!(
                budget.peak_storage(),
                floor
                    + headers()
                    + if stage == 1 {
                        size_of::<Module>()
                    } else {
                        retained(scalar)
                    }
            );
            assert_eq!(
                budget.work_budget_v1().failed_work(),
                Some(PRIOR + copy_work(scalar) + 33)
            );
            assert_eq!(budget.failed_storage(), Some(floor + peak(scalar) + 29));
            assert_eq!(owner.module(), &fixture(scalar));
            assert_eq!(*owner.identity(), identity);
        }
    }
}

fn all_rows_and_roles() -> Module {
    let mut module = Module::new("layouts");
    module.storage_layouts = crate::wire::storage_v18_tests::rows();
    let StorageLayoutKindV1::Variants { variants, .. } = &mut module.storage_layouts[8].kind else {
        unreachable!()
    };
    for (index, variant) in variants.iter_mut().enumerate() {
        variant.discriminant = index as u128;
    }
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::device_ffi_export(
        "export",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.functions.push(Function::external_import(
        "import",
        Signature::new(vec![], vec![]),
    ));
    module
}

#[test]
fn canonical_copy_preserves_all_storage_rows_explicit_roles_and_actual_graphs_independently() {
    let mut lifecycle = crate::verification_execution_lifecycle_v15::tests::fixture(1);
    lifecycle.storage_layouts.push(super::tests::scalar());
    for module in [
        all_rows_and_roles(),
        super::tests::fixture(AddressSpace::Private),
        super::tests::fixture(AddressSpace::Workgroup),
        lifecycle,
    ] {
        let (owner, owner_receipt) = source(&module);
        let identity = *owner.identity();
        let bytes = owner.canonical_bytes().to_vec();
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        let floor = FLOOR + owner_receipt.retained_storage();
        budget.reserve_storage(floor).unwrap();
        let (mut candidate, receipt) = owner
            .copy_module_for_transformation_v18(&mut budget)
            .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(candidate, module);
        assert_ne!(
            candidate.storage_layouts.as_ptr(),
            owner.module().storage_layouts.as_ptr()
        );
        assert_ne!(
            candidate.functions.as_ptr(),
            owner.module().functions.as_ptr()
        );
        for (left, right) in candidate
            .storage_layouts
            .iter()
            .zip(&owner.module().storage_layouts)
        {
            match (&left.kind, &right.kind) {
                (StorageLayoutKindV1::Record(a), StorageLayoutKindV1::Record(b))
                | (StorageLayoutKindV1::Union(a), StorageLayoutKindV1::Union(b))
                    if !a.is_empty() =>
                {
                    assert_ne!(a.as_ptr(), b.as_ptr())
                }
                (
                    StorageLayoutKindV1::Variants { variants: a, .. },
                    StorageLayoutKindV1::Variants { variants: b, .. },
                ) if !a.is_empty() => assert_ne!(a.as_ptr(), b.as_ptr()),
                _ => {}
            }
        }
        for (left, right) in candidate.functions.iter().zip(&owner.module().functions) {
            assert_eq!(left.role, right.role);
            if let (Some(a), Some(b)) = (&left.body, &right.body) {
                assert_ne!(a.blocks.as_ptr(), b.blocks.as_ptr());
                for (x, y) in a.blocks.iter().zip(&b.blocks) {
                    if !x.operations.is_empty() {
                        assert_ne!(x.operations.as_ptr(), y.operations.as_ptr());
                    }
                }
            }
        }
        if module.id.as_str() == "layouts" {
            assert_eq!(candidate.functions[0].role, FunctionRole::DeviceFfiExport);
            assert_eq!(candidate.functions[1].role, FunctionRole::ExternalImport);
        }
        assert!(
            owner
                .matches_module_with_budget_v18(&candidate, &mut budget)
                .unwrap()
        );
        // Same-size mutation needs no new payload and cannot mutate source custody.
        candidate.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I64);
        assert_eq!(owner.module(), &module);
        assert_eq!(owner.canonical_bytes(), bytes);
        assert_eq!(*owner.identity(), identity);
        assert!(
            !owner
                .matches_module_with_budget_v18(&candidate, &mut budget)
                .unwrap()
        );
        drop(candidate);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn canonical_copy_mutation_needs_fresh_admission_and_does_not_reuse_owner_identity() {
    let module = super::tests::fixture(AddressSpace::Private);
    let (owner, owner_receipt) = source(&module);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let floor = FLOOR + owner_receipt.retained_storage();
    budget.reserve_storage(floor).unwrap();
    let (mut candidate, receipt) = owner
        .copy_module_for_transformation_v18(&mut budget)
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let paid_candidate = budget.storage();
    candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[4].results[0].ty =
        Type::Scalar(ScalarType::U32);
    let invalid = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &candidate,
        super::tests::LIMITS,
        &mut budget,
    );
    assert!(matches!(
        invalid,
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Verification(_))
    ));
    assert_eq!(budget.storage(), paid_candidate);
    candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[4].results[0].ty =
        Type::Scalar(ScalarType::U64);
    candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        OperationKind::Constant(Constant::U64(12));
    let (successor, successor_receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &candidate,
            super::tests::LIMITS,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(successor_receipt.retained_storage())
        .unwrap();
    assert_eq!(successor.module(), &candidate);
    assert_ne!(successor.identity(), owner.identity());
    assert_ne!(
        successor.module().functions.as_ptr(),
        candidate.functions.as_ptr()
    );
    assert_eq!(owner.module(), &module);
    drop(candidate);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(successor);
    budget
        .release_storage(successor_receipt.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_copy_late_wire_errors_restore_owned_decode_state_and_preserve_denials() {
    // Only this private adversarial test can corrupt an admitted owner's bytes.
    // Public callers cannot detach or mutate them; this is not a source-validity test.
    let mut module = super::tests::fixture(AddressSpace::Private);
    module.required_capabilities =
        BTreeSet::from([TargetCapability::Float16, TargetCapability::BFloat16]);
    for mutation in 0..4 {
        let (mut owner, owner_receipt) = source(&module);
        match mutation {
            0 => {
                owner.canonical_bytes.pop();
                let length = u32::try_from(owner.canonical_bytes.len()).unwrap();
                owner.canonical_bytes[12..16].copy_from_slice(&length.to_le_bytes());
            }
            1 => {
                assert_eq!(&owner.canonical_bytes[33..39], &[2, 0, 0, 0, 1, 2]);
                owner.canonical_bytes[37..39].copy_from_slice(&[2, 1]);
            }
            2 => owner.canonical_bytes[8..10].copy_from_slice(&15_u16.to_le_bytes()),
            _ => {
                // Two module capability tags precede the first allocated row.
                assert_eq!(&owner.canonical_bytes[55..57], &[1, 9]);
                owner.canonical_bytes[55] = 255;
            }
        }
        let mut work = Work::new(10_000_000);
        assert!(work.charge_work(10_000_001).is_err());
        let mut budget = Budget::new(&mut work, 10_000_000);
        let floor = FLOOR + owner_receipt.retained_storage();
        budget.reserve_storage(floor).unwrap();
        assert!(budget.reserve_storage(10_000_001).is_err());
        let result = owner.copy_module_for_transformation_v18(&mut budget);
        match mutation {
            0 => assert!(matches!(
                result,
                Err(CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                    KernelIrDecodeError::Truncated
                ))
            )),
            1 => assert!(matches!(
                result,
                Err(CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                    KernelIrDecodeError::NonCanonical
                ))
            )),
            2 => assert!(matches!(
                result,
                Err(CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                    KernelIrDecodeError::UnknownVersion(15)
                ))
            )),
            _ => assert!(matches!(
                result,
                Err(CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                    KernelIrDecodeError::UnknownTag {
                        kind: "storage layout",
                        tag: 255
                    }
                ))
            )),
        }
        assert_eq!(budget.storage(), floor);
        if mutation == 0 || mutation == 3 {
            assert!(budget.work() > 55);
            assert!(
                budget.peak_storage()
                    > floor
                        + headers()
                        + size_of::<Module>()
                        + module.id.as_str().len()
                        + module.storage_layouts.len() * size_of::<StorageLayoutV1>()
            );
        } else if mutation == 2 {
            assert_eq!(budget.work(), 1 + 8 + 2);
            assert_eq!(
                budget.peak_storage(),
                floor + headers() + size_of::<Module>()
            );
        } else {
            // Header20 + text7 + counts12 + first tag/clone/insert3 + second tag/compare2.
            assert_eq!(budget.work(), 1 + 20 + 7 + 12 + 3 + 2);
            assert!(budget.peak_storage() > floor + headers() + size_of::<Module>() + 1);
        }
        assert_eq!(budget.work_budget_v1().failed_work(), Some(10_000_001));
        assert_eq!(budget.failed_storage(), Some(floor + 10_000_001));
        assert_eq!(owner.module(), &module);
    }
}

#[test]
fn canonical_copy_maximum_incoming_floor_is_not_reset_on_header_overflow() {
    let (owner, _) = source(&Module::new("m"));
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(usize::MAX).unwrap();
    let result = owner.copy_module_for_transformation_v18(&mut budget);
    assert!(matches!(
        resource_error(result.as_ref().unwrap_err()),
        Some(ResourceError::Storage(_))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, usize::MAX, usize::MAX)
    );
    assert_eq!(owner.module().id, ModuleId::new("m"));
}
