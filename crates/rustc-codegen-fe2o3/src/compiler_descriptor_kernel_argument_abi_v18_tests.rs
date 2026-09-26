//! Tests the actual descriptor-to-input producer, not a KIR pointer cast.
use super::*;
use crate::production_pipeline::ProductionPipelineError as Error;
use fe2o3_kernel_descriptor::{
    PhysicalAbiComponentKind as Component, SourceTypeDescriptorV3 as Source,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_lower_mir_kernel::{
    ProductionKernelArgumentAbiKindV18 as Kind, ProductionSourceOwnedViewErrorV18 as ViewError,
};

const RESULT_HEADER: usize =
    std::mem::size_of::<fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiArgumentV18>();

fn argument(kind: DescriptorArgumentKindV1, offset: u32) -> TypedDescriptorArgumentV1 {
    TypedDescriptorArgumentV1 {
        name: "input".to_owned(),
        kind,
        access: if matches!(kind, DescriptorArgumentKindV1::SharedSlice(_)) {
            AccessMode::ReadOnly
        } else {
            AccessMode::ByValue
        },
        offset,
        layout: None,
        source_size: 16,
        source_alignment: 8,
        rustc_abi_class: fe2o3_artifacts::RustcAbiClassV1::Aggregate,
        semantic_type_identity: SemanticTypeIdentityV1::from_sha256([71; 32]),
    }
}

#[test]
fn original_descriptor_shared_slice_components_and_pending_byvalue_offsets_are_distinct() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let floor = 43 + 2 * RESULT_HEADER;
    budget.reserve_storage(floor).unwrap();
    let input = argument(DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32), 0);
    let shared = kernel_argument_abi_argument_v18(&input, 0, &mut budget).unwrap();
    assert_eq!(shared.semantic_type_identity, input.semantic_type_identity);
    let Kind::Descriptor { source, argument } = &shared.kind else {
        panic!("missing checked descriptor")
    };
    assert_eq!(*source, Source::SharedSlice(ScalarTypeV1::U32));
    assert_eq!(
        argument.physical_components().collect::<Vec<_>>(),
        [
            (Component::GlobalPointer, 0, 8, 8),
            (Component::SliceLengthU64, 8, 8, 8)
        ]
    );
    assert_eq!(argument.access(), AccessMode::ReadOnly);
    assert_eq!(
        argument.alias(),
        fe2o3_kernel_descriptor::AliasSemantics::SharedReadOnly
    );
    let held = budget.storage();
    let opaque = kernel_argument_abi_argument_v18(
        &self::argument(DescriptorArgumentKindV1::CompilerLaidOutByValue, 16),
        1,
        &mut budget,
    )
    .unwrap();
    assert!(matches!(
        opaque.kind,
        Kind::CompilerLaidOutByValue { offset: 16 }
    ));
    assert_eq!(
        budget.storage(),
        held,
        "opaque source ABI has no fabricated physical component allocation"
    );
    drop((shared, opaque));
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn pointer_sized_nominal_source_kind_is_not_lost_to_the_physical_scalar_encoding() {
    for (kind, source, scalar) in [
        (
            DescriptorArgumentKindV1::CompilerLaidOutUsize,
            Source::Usize,
            ScalarTypeV1::U64,
        ),
        (
            DescriptorArgumentKindV1::CompilerLaidOutIsize,
            Source::Isize,
            ScalarTypeV1::I64,
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(RESULT_HEADER).unwrap();
        let result = kernel_argument_abi_argument_v18(&argument(kind, 24), 3, &mut budget).unwrap();
        let Kind::Descriptor {
            source: actual,
            argument,
        } = &result.kind
        else {
            panic!("missing scalar")
        };
        assert_eq!(*actual, source);
        assert_eq!(argument.source_index(), 3);
        assert_eq!(
            argument.physical_components().collect::<Vec<_>>(),
            [(Component::ScalarByValue(scalar), 24, 8, 8)]
        );
        drop(result);
        budget
            .release_storage(budget.storage() - RESULT_HEADER)
            .unwrap();
        assert_eq!(budget.storage(), RESULT_HEADER);
    }
}

#[test]
fn descriptor_argument_capture_has_independent_exact_and_one_short_boundaries() {
    let input = argument(DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32), 0);
    // Four fixed construction units plus the checked constructor/hash envelope.
    let required_work = 4 + input.name.len() + 128;
    let retained =
        input.name.len() + 2 * std::mem::size_of::<fe2o3_kernel_descriptor::PhysicalComponentV3>();
    let floor = 43 + RESULT_HEADER;
    let peak = floor + retained + fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3;
    for (work_limit, storage_limit, expected) in [
        (required_work, peak, 0),
        (required_work - 1, peak, 1),
        (required_work, peak - 1, 2),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        match kernel_argument_abi_argument_v18(&input, 0, &mut budget) {
            Ok(value) => {
                assert_eq!(expected, 0);
                assert_eq!(budget.work(), required_work);
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.storage(), floor + retained);
                drop(value);
            }
            Err(Error::SourceOwnedEntrance(ViewError::Resource(Resource::Work(_)))) => {
                assert_eq!(expected, 1)
            }
            Err(Error::SourceOwnedEntrance(ViewError::Resource(Resource::Storage(_)))) => {
                assert_eq!(expected, 2)
            }
            Err(error) => panic!("unexpected ABI construction result: {error:?}"),
        }
        // This unit owns only concrete producer scratch, all dropped above.
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn invalid_descriptor_alignment_and_source_ordinal_are_not_repaired_by_capture() {
    for (offset, ordinal) in [(1, 0), (0, usize::from(u16::MAX) + 1)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(RESULT_HEADER).unwrap();
        assert!(matches!(
            kernel_argument_abi_argument_v18(
                &argument(
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
                    offset
                ),
                ordinal,
                &mut budget,
            ),
            Err(Error::DescriptorEvidence(_))
        ));
        budget
            .release_storage(budget.storage() - RESULT_HEADER)
            .unwrap();
        assert_eq!(budget.storage(), RESULT_HEADER);
    }
}
