use super::*;
use fe2o3_kernel_ir::{
    AccessMode, CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
    IntrinsicOperation, Operation, ScalarType, ValueDef, ValueId,
};
use fe2o3_pliron::{
    ProductionRankedBlockV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
};

// An independent mirror of the pre-change error payload: diagnostics must fit
// its existing fixed envelope rather than add a hidden retained allocation.
#[allow(dead_code)]
enum PreviousAdmissionErrorV1 {
    Resource(AssertOriginResourceV1),
    Source(ProductionSemanticKirErrorV1),
    SourceOutput(ProductionSourceOutputErrorV1),
    Coordinates(fe2o3_kernel_analysis::CanonicalKirCoordinatePreservationErrorV1),
    Formal(crate::ProductionFormalMemoryErrorV1),
    PrivateAddressR2,
    Unsupported {
        phase: &'static str,
        detail: &'static str,
    },
}

#[test]
fn census_context_fits_existing_error_and_result_envelopes() {
    use std::mem::{align_of, needs_drop, size_of};
    assert_eq!(size_of::<E>(), size_of::<PreviousAdmissionErrorV1>());
    assert_eq!(align_of::<E>(), align_of::<PreviousAdmissionErrorV1>());
    assert_eq!(
        size_of::<Result<(), E>>(),
        size_of::<Result<(), PreviousAdmissionErrorV1>>()
    );
    assert!(!needs_drop::<ProductionCheckedOutputCensusContextV1>());
    assert!(!needs_drop::<ProductionCheckedOutputRankedCensusKindV1>());
    assert!(!needs_drop::<ProductionCheckedOutputNativeCensusKindV1>());
    assert!(!needs_drop::<ProductionCheckedOutputCensusTypeV1>());
}

fn ranked_space_blocks(space: dialect_kernel::MemorySpaceAttr) -> Vec<ProductionRankedBlockV1> {
    vec![
        ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::IndexUnknown {
                    result: ProductionRankedValueIdV1::new(0),
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: ProductionRankedValueIdV1::new(1),
                    element_width: 4,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    memory_space: space,
                    allocation_origin: 7,
                    noalias_class: 9,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        ),
    ]
}

#[test]
fn census_context_ranked_refusal_keeps_exact_coordinate_space_and_positive_neighbors() {
    for space in [
        dialect_kernel::MemorySpaceAttr::Global,
        dialect_kernel::MemorySpaceAttr::Private,
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
        ranked_blocks(&ranked_space_blocks(space), &mut budget).unwrap();
        assert_eq!((budget.work(), budget.storage()), (4, 0));
    }
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
    let error = ranked_blocks(
        &ranked_space_blocks(dialect_kernel::MemorySpaceAttr::Workgroup),
        &mut budget,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        E::UnsupportedOperation {
            phase: "ranked",
            detail: "closed global effects and source scalar recipes",
            context: ProductionCheckedOutputCensusContextV1::Ranked {
                block: 1,
                operation: 1,
                kind: ProductionCheckedOutputRankedCensusKindV1::ViewInSpace(
                    dialect_kernel::MemorySpaceAttr::Workgroup
                ),
            },
        }
    ));
    assert!(
        error
            .to_string()
            .contains("closed global effects and source scalar recipes")
    );
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!((budget.work(), budget.storage()), (4, 0));
}

#[test]
fn census_context_ranked_work_refusal_precedes_diagnostic_and_preserves_cause() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
    let error = ranked_blocks(
        &ranked_space_blocks(dialect_kernel::MemorySpaceAttr::Workgroup),
        &mut budget,
    )
    .unwrap_err();
    assert!(
        matches!(error, E::Resource(AssertOriginResourceV1::Work(error))
        if error.limit() == 3 && error.actual() == 4)
    );
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!((budget.work(), budget.storage()), (3, 0));
}

fn coordinate() -> CanonicalKirOperationCoordinateV1 {
    CanonicalKirOperationCoordinateV1 {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(5),
            block: 7,
        },
        operation: 11,
    }
}

#[test]
fn census_context_native_intrinsic_keeps_hierarchy_axis_scalar_and_coordinate() {
    let intrinsic = IntrinsicKind::InvocationIndex {
        kind: IndexKind::Local,
        axis: Axis::Y,
    };
    let operation = Operation {
        results: vec![ValueDef::new(ValueId(912), Type::INDEX)],
        kind: OperationKind::Intrinsic(IntrinsicOperation::new(intrinsic, Type::INDEX)),
    };
    assert_eq!(
        ProductionCheckedOutputCensusContextV1::native(coordinate(), &operation),
        ProductionCheckedOutputCensusContextV1::Native {
            coordinate: coordinate(),
            kind: ProductionCheckedOutputNativeCensusKindV1::Intrinsic(intrinsic),
            results: 1,
            first_result: Some(ProductionCheckedOutputCensusTypeV1::Scalar(
                ScalarType::Index
            )),
        }
    );
}

#[test]
fn census_context_native_type_is_shallow_and_empty_results_are_explicit() {
    let operation = Operation {
        results: vec![ValueDef::new(
            ValueId(44),
            Type::pointer(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
        )],
        kind: OperationKind::Alloca {
            element: Type::F32,
            count: None,
            address_space: AddressSpace::Workgroup,
            alignment: 4,
        },
    };
    assert!(matches!(
        ProductionCheckedOutputCensusContextV1::native(coordinate(), &operation),
        ProductionCheckedOutputCensusContextV1::Native {
            kind: ProductionCheckedOutputNativeCensusKindV1::Alloca(AddressSpace::Workgroup),
            results: 1,
            first_result: Some(ProductionCheckedOutputCensusTypeV1::Pointer(
                AddressSpace::Workgroup,
                AccessMode::ReadWrite
            )),
            ..
        }
    ));
    let empty = Operation {
        results: vec![],
        kind: OperationKind::Call {
            callee: "not-an-authority".into(),
            arguments: vec![],
        },
    };
    assert!(matches!(
        ProductionCheckedOutputCensusContextV1::native(coordinate(), &empty),
        ProductionCheckedOutputCensusContextV1::Native {
            kind: ProductionCheckedOutputNativeCensusKindV1::Call,
            results: 0,
            first_result: None,
            ..
        }
    ));
}

#[test]
fn census_context_does_not_decorate_original_source_or_unsupported_contracts() {
    let source = E::Source(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    assert!(std::error::Error::source(&source).is_some());
    let old = refused("source", "no atomic, volatile or assumed effects");
    assert!(matches!(
        old,
        E::Unsupported {
            phase: "source",
            detail: "no atomic, volatile or assumed effects"
        }
    ));
    assert!(std::error::Error::source(&old).is_none());
}
