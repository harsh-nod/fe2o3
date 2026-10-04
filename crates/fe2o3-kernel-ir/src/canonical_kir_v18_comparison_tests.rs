use super::*;
use crate::*;
use std::{collections::BTreeSet, mem::size_of};

const PRIOR: usize = 7;
const FLOOR: usize = 19;
const LIMIT: usize = 10_000_000;

std::thread_local! {
    static PANIC_STAGE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

pub(super) fn checkpoint(stage: u8) {
    PANIC_STAGE.with(|value| {
        if value.get() == stage {
            value.set(0);
            panic!("V18 comparison checkpoint {stage}");
        }
    });
}

fn codec_outcomes<T>() -> usize {
    2 * size_of::<T>() + 2 * size_of::<Result<T, KernelIrDecodeError>>()
}

fn independent_headers() -> usize {
    // Opaque header sizes are host-layout premises, not production totals.
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
        + (MAX_TYPE_DEPTH_V1 + 2) * codec_outcomes::<Type>()
        + size_of::<ComparisonScopeV18<'_, '_>>()
        + 2 * size_of::<Result<ComparisonScopeV18<'_, '_>, ResourceError>>()
        // Facade, closure, saved outcome and caller result.
        + 4 * size_of::<Result<bool, CanonicalKernelIrReplayAdmissionErrorV18>>()
        + 2 * size_of::<Result<(), ResourceError>>()
        + 2 * size_of::<Result<usize, ResourceError>>()
}

struct Observed {
    result: Result<bool, CanonicalKernelIrReplayAdmissionErrorV18>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn compare(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    candidate: &Module,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    prior_denials: bool,
) -> Observed {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(PRIOR + work_limit);
    work.charge_work(PRIOR).unwrap();
    let mut budget = Budget::new(&mut work, floor + storage_limit);
    budget.reserve_storage(floor).unwrap();
    if prior_denials {
        assert!(budget.charge_work(work_limit + 33).is_err());
        assert!(budget.reserve_storage(storage_limit + 29).is_err());
    }
    let result = owner.matches_module_with_budget_v18(candidate, &mut budget);
    assert_eq!(budget.storage(), floor);
    Observed {
        result,
        work: budget.work(),
        peak: budget.peak_storage(),
        failed_work: budget.work_budget_v1().failed_work(),
        failed_storage: budget.failed_storage(),
    }
}

fn admitted(source: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            source,
            super::tests::LIMITS,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    (owner, FLOOR + receipt.retained_storage())
}

fn schedule(scalar: bool) -> (usize, usize, usize) {
    // Empty Module("m"): 20-byte header, ID length+byte, functions, kernels,
    // capabilities and storage-row counts. A U64 row adds size/alignment/tags.
    let wire = 20 + 4 + 1 + 4 + 4 + 4 + 4 + if scalar { 8 + 4 + 1 + 1 } else { 0 };
    let count = 5 + 2 + 4 + if scalar { 1 + 4 } else { 0 };
    // Compare charges bytes, each compared chunk, length patch and completion.
    let chunks = 10 + if scalar { 4 } else { 0 };
    let compare = wire + chunks + 4 + 1 + 1 + usize::from(scalar);
    (wire, count, compare)
}

#[test]
fn v18_comparison_empty_and_scalar_have_independent_exact_work_and_headers() {
    assert_eq!(
        ComparisonScopeV18::headers().unwrap(),
        independent_headers()
    );
    for scalar in [false, true] {
        let source = if scalar {
            super::tests::small_module()
        } else {
            Module::new("m")
        };
        let (owner, floor) = admitted(&source);
        let (wire, count, compared) = schedule(scalar);
        assert_eq!(
            (wire, count, compared),
            if scalar { (55, 16, 76) } else { (41, 11, 57) }
        );
        assert_eq!(owner.canonical_bytes().len(), wire);
        let exact = 1 + count + compared;
        let observed = compare(
            &owner,
            owner.module(),
            floor,
            exact,
            independent_headers(),
            false,
        );
        assert!(observed.result.unwrap());
        assert_eq!(
            (observed.work, observed.peak),
            (PRIOR + exact, floor + independent_headers())
        );
        assert_eq!(
            (observed.failed_work, observed.failed_storage),
            (None, None)
        );
        let mut different = source.clone();
        different.id = ModuleId::new("n");
        let mismatch = compare(
            &owner,
            &different,
            floor,
            exact,
            independent_headers(),
            false,
        );
        assert!(!mismatch.result.unwrap());
        assert_eq!(
            (mismatch.work, mismatch.peak),
            (PRIOR + exact, floor + independent_headers())
        );
        let denied_mismatch = compare(
            &owner,
            &different,
            floor,
            exact - 1,
            independent_headers(),
            false,
        );
        assert!(matches!(
            denied_mismatch.result,
            Err(CanonicalKernelIrReplayAdmissionErrorV18::Encode(
                KernelIrEncodeError::WorkLimit(_)
            ))
        ));
        assert_eq!(denied_mismatch.failed_work, Some(PRIOR + exact));
        let short_work = compare(
            &owner,
            owner.module(),
            floor,
            exact - 1,
            independent_headers(),
            false,
        );
        assert!(matches!(
            short_work.result,
            Err(CanonicalKernelIrReplayAdmissionErrorV18::Encode(
                KernelIrEncodeError::WorkLimit(_)
            ))
        ));
        assert_eq!(
            (short_work.work, short_work.failed_work),
            (PRIOR + exact - 1, Some(PRIOR + exact))
        );
        assert_eq!(short_work.peak, floor + independent_headers());
        let short_storage = compare(
            &owner,
            owner.module(),
            floor,
            exact,
            independent_headers() - 1,
            false,
        );
        assert!(matches!(
            short_storage.result,
            Err(CanonicalKernelIrReplayAdmissionErrorV18::Resource(
                ResourceError::Storage(_)
            ))
        ));
        assert_eq!((short_storage.work, short_storage.peak), (PRIOR + 1, floor));
        assert_eq!(
            short_storage.failed_storage,
            Some(floor + independent_headers())
        );
        let denied = compare(
            &owner,
            owner.module(),
            floor,
            0,
            independent_headers(),
            false,
        );
        assert!(matches!(
            denied.result,
            Err(CanonicalKernelIrReplayAdmissionErrorV18::Resource(
                ResourceError::Work(_)
            ))
        ));
        assert_eq!(
            (denied.work, denied.peak, denied.failed_work),
            (PRIOR, floor, Some(PRIOR + 1))
        );
        let inherited = compare(
            &owner,
            owner.module(),
            floor,
            exact,
            independent_headers(),
            true,
        );
        assert!(inherited.result.unwrap());
        assert_eq!(
            (inherited.work, inherited.peak),
            (PRIOR + exact, floor + independent_headers())
        );
        assert_eq!(inherited.failed_work, Some(PRIOR + exact + 33));
        assert_eq!(
            inherited.failed_storage,
            Some(floor + independent_headers() + 29)
        );
    }
}

#[test]
fn v18_comparison_auxiliary_payload_is_paid_without_wire_or_graph_storage() {
    let mut source = crate::verification_execution_lifecycle_v15::tests::fixture(1);
    source.storage_layouts.push(super::tests::scalar());
    let (owner, floor) = admitted(&source);
    // Largest execution payload: four bytes plus its 20+4 byte SO3 frame.
    let auxiliary = 4 + (20 + 4);
    let scratch = independent_headers() + auxiliary;
    let measured = compare(&owner, owner.module(), floor, LIMIT, LIMIT, false);
    assert!(measured.result.unwrap());
    assert_eq!(measured.peak, floor + scratch);
    let work = measured.work - PRIOR;
    let exact = compare(&owner, owner.module(), floor, work, scratch, true);
    assert!(exact.result.unwrap());
    assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
    assert_eq!(exact.failed_work, Some(PRIOR + work + 33));
    assert_eq!(exact.failed_storage, Some(floor + scratch + 29));
    let denied = compare(&owner, owner.module(), floor, work, scratch - 1, false);
    assert!(matches!(
        denied.result,
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Resource(
            ResourceError::Storage(_)
        ))
    ));
    assert_eq!(denied.peak, floor + independent_headers());
    assert_eq!(denied.failed_storage, Some(floor + scratch));
    assert!(denied.work > PRIOR + 1 && denied.work < measured.work);
    let denied = compare(&owner, owner.module(), floor, work - 1, scratch, false);
    assert!(matches!(
        denied.result,
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Encode(
            KernelIrEncodeError::WorkLimit(_)
        ))
    ));
    assert_eq!(denied.failed_work, Some(measured.work));
    assert_eq!(denied.peak, measured.peak);
}

fn differs(owner: &VerifiedCanonicalKernelIrModuleV18, candidate: &Module, floor: usize) {
    assert!(
        !compare(owner, candidate, floor, LIMIT, LIMIT, false)
            .result
            .unwrap()
    );
}

#[test]
fn v18_comparison_checks_all_table_rows_and_nested_storage_payloads() {
    let mut source = Module::new("all-layouts");
    source.storage_layouts = crate::wire::storage_v18_tests::rows();
    for row in &mut source.storage_layouts {
        if let StorageLayoutKindV1::Variants {
            encoding: StorageVariantEncodingV1::Niche { .. },
            variants,
        } = &mut row.kind
        {
            for (index, variant) in variants.iter_mut().enumerate() {
                variant.discriminant = index as u128;
            }
        }
    }
    let (owner, floor) = admitted(&source);
    assert!(
        compare(&owner, &source, floor, LIMIT, LIMIT, false)
            .result
            .unwrap()
    );
    for index in 0..source.storage_layouts.len() {
        for alignment in [false, true] {
            let mut changed = source.clone();
            if alignment {
                changed.storage_layouts[index].alignment ^= 1;
            } else {
                changed.storage_layouts[index].size ^= 1;
            }
            differs(&owner, &changed, floor);
        }
        let mut changed = source.clone();
        match &mut changed.storage_layouts[index].kind {
            StorageLayoutKindV1::Scalar(value) => *value = ScalarType::I128,
            kind @ StorageLayoutKindV1::Vector(_) => {
                *kind = StorageLayoutKindV1::Scalar(ScalarType::U8)
            }
            StorageLayoutKindV1::Pointer(value) => value.pointee = StorageLayoutIdV1(u32::MAX),
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                fields[0].offset ^= 1
            }
            StorageLayoutKindV1::Array { stride, .. } => *stride ^= 1,
            StorageLayoutKindV1::Slice { length, .. } => length.offset ^= 1,
            StorageLayoutKindV1::Variants { variants, .. } => variants[0].discriminant ^= 1,
        }
        differs(&owner, &changed, floor);
    }
    let mut changed = source.clone();
    changed.storage_layouts.swap(0, 1);
    differs(&owner, &changed, floor);
    changed.storage_layouts.clear();
    differs(&owner, &changed, floor);
    for change in 0..7 {
        let mut changed = source.clone();
        match change {
            0 => {
                let StorageLayoutKindV1::Pointer(pointer) = &mut changed.storage_layouts[2].kind
                else {
                    unreachable!()
                };
                pointer.encoded_space = AddressSpace::Global;
            }
            1 => {
                let StorageLayoutKindV1::Pointer(pointer) = &mut changed.storage_layouts[2].kind
                else {
                    unreachable!()
                };
                pointer.access = AccessMode::ReadOnly;
            }
            2 => {
                let StorageLayoutKindV1::Slice { data, .. } = &mut changed.storage_layouts[6].kind
                else {
                    unreachable!()
                };
                data.layout = StorageLayoutIdV1(u32::MAX);
            }
            3 | 4 => {
                let StorageLayoutKindV1::Variants { variants, .. } =
                    &mut changed.storage_layouts[7].kind
                else {
                    unreachable!()
                };
                if change == 3 {
                    variants[1].direct_tag_bits = Some(10);
                } else {
                    variants[1].uninhabited = true;
                }
            }
            5 => {
                let StorageLayoutKindV1::Variants {
                    encoding: StorageVariantEncodingV1::Niche { niche_start, .. },
                    ..
                } = &mut changed.storage_layouts[8].kind
                else {
                    unreachable!()
                };
                *niche_start ^= 1;
            }
            6 => {
                let StorageLayoutKindV1::Record(fields) = &mut changed.storage_layouts[3].kind
                else {
                    unreachable!()
                };
                fields.swap(0, 1);
            }
            _ => unreachable!(),
        }
        differs(&owner, &changed, floor);
    }
    assert_eq!(owner.module(), &source);
}

#[test]
fn v18_comparison_checks_graph_roles_operations_ids_and_metadata_without_admission() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let source = super::tests::fixture(space);
        let (owner, floor) = admitted(&source);
        let bytes = owner.canonical_bytes().as_ptr();
        let graph = owner.module() as *const Module;
        let identity = *owner.identity();
        for change in 0..6 {
            let mut candidate = source.clone();
            match change {
                0 => candidate.id = ModuleId::new("different"),
                1 => candidate.functions[0].role = FunctionRole::DeviceFfiExport,
                2 => candidate.functions[0].id = FunctionId::new("renamed"),
                3 => candidate.functions[0].signature.results[0] = Type::Scalar(ScalarType::I64),
                4 => {
                    candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind =
                        OperationKind::Storage(StorageOperationV1::Project {
                            base: ValueId(0),
                            step: StorageProjectionV1::Field(0),
                        })
                }
                5 => {
                    candidate.functions[0].body.as_mut().unwrap().blocks[0].terminator =
                        Some(Terminator::Return {
                            values: vec![ValueId(999)],
                        })
                }
                _ => unreachable!(),
            }
            // Some candidates deliberately fail structural verification. The
            // query compares their encodings; it must not admit them again.
            differs(&owner, &candidate, floor);
        }
        assert!(
            compare(&owner, &source, floor, LIMIT, LIMIT, false)
                .result
                .unwrap()
        );
        assert_eq!(owner.canonical_bytes().as_ptr(), bytes);
        assert_eq!(owner.module() as *const Module, graph);
        assert_eq!(*owner.identity(), identity);
        assert_eq!(owner.module(), &source);
    }
    let source = crate::verification_execution_lifecycle_v15::tests::fixture(1);
    let (owner, floor) = admitted(&source);
    let mut candidate = source;
    candidate.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(65),
    };
    differs(&owner, &candidate, floor);
    let empty = Module::new("m");
    let (owner, floor) = admitted(&empty);
    for name in ["", "n", "longer-module"] {
        differs(&owner, &Module::new(name), floor);
    }
}

#[test]
fn v18_comparison_late_encoding_errors_precede_early_byte_mismatches() {
    let source = super::tests::fixture(AddressSpace::Private);
    let (owner, floor) = admitted(&source);
    let mut candidate = source.clone();
    candidate.id = ModuleId::new("different");
    let mut ty = Type::Scalar(ScalarType::U64);
    for _ in 0..=MAX_TYPE_DEPTH_V1 {
        ty = Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite);
    }
    candidate.functions[0].signature.results[0] = ty;
    let rejected = compare(&owner, &candidate, floor, LIMIT, LIMIT, true);
    assert!(matches!(
        rejected.result,
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Encode(
            KernelIrEncodeError::TypeNestingTooDeep {
                max: MAX_TYPE_DEPTH_V1
            }
        ))
    ));
    assert_eq!(rejected.peak, floor + independent_headers());
    assert_eq!(rejected.failed_work, Some(PRIOR + LIMIT + 33));
    assert_eq!(rejected.failed_storage, Some(floor + LIMIT + 29));
    let source = crate::verification_execution_lifecycle_v15::tests::fixture(1);
    let (owner, floor) = admitted(&source);
    let mut candidate = source;
    candidate.id = ModuleId::new("different");
    candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind =
        OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
            workgroup: ValueId(11),
            input: ValueId(0),
            base: ValueId(1),
            lanes: 0,
            elements: 1,
        });
    let rejected = compare(&owner, &candidate, floor, LIMIT, LIMIT, false);
    assert!(matches!(
        rejected.result,
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Encode(
            KernelIrEncodeError::NonCanonical {
                field: "execution payload"
            }
        ))
    ));
    assert!(
        compare(&owner, owner.module(), floor, LIMIT, LIMIT, false)
            .result
            .unwrap()
    );
}

#[test]
fn v18_comparison_real_facade_unwinds_restore_floor_and_prior_history() {
    for stage in 1..=3 {
        let source = crate::verification_execution_lifecycle_v15::tests::fixture(1);
        let (owner, floor) = admitted(&source);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(PRIOR + LIMIT);
        work.charge_work(PRIOR).unwrap();
        let mut budget = Budget::new(&mut work, floor + LIMIT);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.charge_work(LIMIT + 33).is_err());
        assert!(budget.reserve_storage(LIMIT + 29).is_err());
        PANIC_STAGE.with(|value| value.set(stage));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.matches_module_with_budget_v18(owner.module(), &mut budget)
        }));
        PANIC_STAGE.with(|value| value.set(0));
        assert!(result.is_err());
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            budget.peak_storage(),
            floor + independent_headers() + if stage == 1 { 0 } else { 28 }
        );
        assert!(budget.work() > PRIOR);
        assert_eq!(
            budget.work_budget_v1().failed_work(),
            Some(PRIOR + LIMIT + 33)
        );
        assert_eq!(budget.failed_storage(), Some(floor + LIMIT + 29));
        assert_eq!(owner.module(), &source);
        assert!(
            owner
                .matches_module_with_budget_v18(owner.module(), &mut budget)
                .unwrap()
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn v18_comparison_preserves_a_maximum_caller_floor_on_reservation_overflow() {
    let (owner, _) = admitted(&Module::new("m"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(usize::MAX).unwrap();
    let result = owner.matches_module_with_budget_v18(owner.module(), &mut budget);
    assert!(matches!(
        result,
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Resource(
            ResourceError::Storage(_)
        ))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, usize::MAX, usize::MAX)
    );
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
}
