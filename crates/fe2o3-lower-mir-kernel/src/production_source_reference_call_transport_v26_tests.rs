use super::*;

fn reference_types_v26(mutable: bool, raw: bool, bits: u16, space: u32) -> Vec<SemanticTypeDeclV1> {
    let mut rows = types(space);
    rows[POINTER.index() as usize] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([71; 32]),
        SemanticLayoutIdentityV1::from_sha256([72; 32]),
        SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8)).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                WORD,
                if raw {
                    SemanticPointerKindV1::Raw
                } else {
                    SemanticPointerKindV1::Reference
                },
                if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                space,
                bits,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    rows
}

fn widened_shape_v26(
    rows: &[SemanticTypeDeclV1],
    actual: Type,
    source: bool,
    work: usize,
    storage: usize,
) -> (
    Result<(), ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
) {
    let mut work_budget = Work::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut work_budget, storage);
    budget.reserve_storage(37).unwrap();
    let binding = SemanticValueBindingV1::Value {
        id: ValueId(7),
        ty: actual,
    };
    let result = execution_call_shape_with_representation_v29(
        rows,
        POINTER,
        &binding,
        if source {
            ExecutionCfgRepresentationV29::OriginalSource
        } else {
            ExecutionCfgRepresentationV29::LegacyAbi
        },
        &mut budget,
    )
    .map(|_| ());
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn source_reference_call_shape_transports_only_whole_thin_reference_representations() {
    for mutable in [false, true] {
        for space in [
            AddressSpace::Global,
            AddressSpace::Workgroup,
            AddressSpace::Private,
            AddressSpace::Constant,
        ] {
            let access = if mutable {
                AccessMode::ReadWrite
            } else {
                AccessMode::ReadOnly
            };
            let result = widened_shape_v26(
                &reference_types_v26(mutable, false, 64, 0),
                Type::pointer(Type::Scalar(ScalarType::U32), space, access),
                true,
                usize::MAX,
                usize::MAX,
            )
            .0;
            if mutable && space == AddressSpace::Constant {
                assert!(result.is_err(), "{space:?} {mutable}");
            } else {
                assert!(result.is_ok(), "{space:?} {mutable}: {result:?}");
            }
        }
    }
}

#[test]
fn source_reference_call_shape_preserves_raw_legacy_pointee_access_and_source_space_refusals() {
    for fault in 0..6 {
        let rows = reference_types_v26(
            false,
            fault == 0,
            if fault == 4 { 32 } else { 64 },
            if fault == 5 { 4 } else { 0 },
        );
        let actual = Type::pointer(
            Type::Scalar(if fault == 2 {
                ScalarType::U64
            } else {
                ScalarType::U32
            }),
            if fault == 1 {
                AddressSpace::Generic
            } else {
                AddressSpace::Global
            },
            if fault == 3 {
                AccessMode::ReadWrite
            } else {
                AccessMode::ReadOnly
            },
        );
        let result = widened_shape_v26(&rows, actual, fault != 1, usize::MAX, usize::MAX).0;
        assert!(result.is_err(), "fault {fault}: {result:?}");
    }
}

#[test]
fn source_reference_call_shape_replays_exact_and_one_short_cumulative_limits() {
    let rows = reference_types_v26(true, false, 64, 0);
    let actual = || {
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )
    };
    let (result, work, retained, peak) =
        widened_shape_v26(&rows, actual(), true, usize::MAX, usize::MAX);
    result.unwrap();
    let (exact, exact_work, exact_retained, exact_peak) =
        widened_shape_v26(&rows, actual(), true, work, peak);
    exact.unwrap();
    assert_eq!(
        (exact_work, exact_retained, exact_peak),
        (work, retained, peak)
    );
    let (short, _, _, _) = widened_shape_v26(&rows, actual(), true, work - 1, peak);
    assert!(matches!(
        short,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    let (short, _, _, _) = widened_shape_v26(&rows, actual(), true, work, peak - 1);
    assert!(matches!(
        short,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}
