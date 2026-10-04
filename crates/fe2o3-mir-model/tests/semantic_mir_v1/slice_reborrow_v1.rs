use super::*;

const ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const POINTEE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

#[derive(Clone, Copy, Debug)]
enum Case {
    Shared,
    Mutable,
    SharedToMutable,
    MutableToShared,
    Fake,
    RawSource,
    RawResult,
    AddressOf,
    OtherAddressSpace,
    WrongPointerWidth,
    MissingResultLength,
    MissingSourceLength,
    VtableResult,
    NonSlicePointee,
    StrPointee,
    DifferentReferenceType,
    DifferentElement,
    ExtraProjection,
    IndexedProjection,
    NoProjection,
    WrongProjectionResult,
}

#[derive(Clone, Copy)]
struct ReferenceShape {
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    address_space: u32,
    width: u16,
    metadata: SemanticPointerMetadataV1,
}

impl ReferenceShape {
    fn shared() -> Self {
        Self {
            kind: SemanticPointerKindV1::Reference,
            mutability: SemanticMutabilityV1::Immutable,
            address_space: 0,
            width: 64,
            metadata: SemanticPointerMetadataV1::SliceLength,
        }
    }
}

fn unsized_layout(stride: u64, alignment: u64) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        0,
        alignment,
        SemanticFieldsShapeV1::array(stride, 0),
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::memory(false),
        None,
        false,
        None,
        alignment,
        0,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap()
}

fn slice_type(identity: u8, element: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        unsized_layout(4, 4),
        SemanticTypeShapeV1::Slice { element },
    )
}

fn reference_type(
    identity: u8,
    pointee: SemanticTypeIdV1,
    shape: ReferenceShape,
    pointee_alignment: u64,
) -> SemanticTypeDeclV1 {
    let data_size = u64::from(shape.width / 8);
    let first = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(shape.address_space, data_size, data_size),
        SemanticScalarValidityRangeV1::new(
            u128::from(shape.kind == SemanticPointerKindV1::Reference),
            (1_u128 << shape.width) - 1,
        ),
    );
    let (size, alignment, backend, second_pointee) = match shape.metadata {
        SemanticPointerMetadataV1::None => (
            data_size,
            data_size,
            SemanticBackendReprV1::scalar(first),
            None,
        ),
        SemanticPointerMetadataV1::SliceLength => (
            16,
            8,
            SemanticBackendReprV1::scalar_pair(
                first,
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                ),
            ),
            None,
        ),
        SemanticPointerMetadataV1::VTable => (
            16,
            8,
            SemanticBackendReprV1::scalar_pair(
                first,
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                ),
            ),
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        ),
    };
    let pointee_kind = match (shape.kind, shape.mutability) {
        (SemanticPointerKindV1::Raw, _) => SemanticAbiPointeeKindV1::Raw,
        (SemanticPointerKindV1::Reference, SemanticMutabilityV1::Immutable) => {
            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
        }
        (SemanticPointerKindV1::Reference, SemanticMutabilityV1::Mutable) => {
            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
        }
    };
    let reliable_alignment = if shape.kind == SemanticPointerKindV1::Raw {
        1
    } else {
        pointee_alignment
    };
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        SemanticTypeLayoutV1::new_with_backend_repr(Some(size), alignment, backend, false).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                shape.kind,
                shape.mutability,
                shape.address_space,
                shape.width,
                shape.metadata,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(pointee_kind, 0, reliable_alignment).unwrap()),
            second_pointee,
        ),
    )
}

fn slice_reborrow_request(case: Case) -> InertSemanticMirRequestV1 {
    let mut shape = ReferenceShape::shared();
    match case {
        Case::Mutable | Case::MutableToShared => shape.mutability = SemanticMutabilityV1::Mutable,
        Case::RawResult | Case::AddressOf => shape.kind = SemanticPointerKindV1::Raw,
        Case::OtherAddressSpace => shape.address_space = 1,
        Case::WrongPointerWidth => shape.width = 32,
        _ => {}
    }
    let pointee_alignment = if matches!(case, Case::StrPointee) {
        1
    } else {
        4
    };
    let pointee = match case {
        Case::NonSlicePointee => u32_type(2),
        Case::StrPointee => SemanticTypeDeclV1::new(
            type_identity(2),
            layout_identity(2),
            unsized_layout(1, 1),
            SemanticTypeShapeV1::Opaque,
        )
        .with_rust_type_kind(SemanticRustTypeKindV1::Str),
        _ => slice_type(2, ELEMENT),
    };
    let mut types = vec![
        u32_type(1),
        pointee,
        reference_type(3, POINTEE, shape, pointee_alignment),
    ];
    let mut source_type = REFERENCE;
    let mut result_type = REFERENCE;
    match case {
        Case::SharedToMutable | Case::MutableToShared => {
            shape.mutability = if matches!(case, Case::SharedToMutable) {
                SemanticMutabilityV1::Mutable
            } else {
                SemanticMutabilityV1::Immutable
            };
            result_type = SemanticTypeIdV1::from_index(3);
            types.push(reference_type(4, POINTEE, shape, pointee_alignment));
        }
        Case::RawSource => {
            shape.kind = SemanticPointerKindV1::Raw;
            source_type = SemanticTypeIdV1::from_index(3);
            types.push(reference_type(4, POINTEE, shape, pointee_alignment));
        }
        Case::MissingResultLength | Case::MissingSourceLength | Case::VtableResult => {
            shape.metadata = if matches!(case, Case::VtableResult) {
                SemanticPointerMetadataV1::VTable
            } else {
                SemanticPointerMetadataV1::None
            };
            if matches!(case, Case::MissingSourceLength) {
                source_type = SemanticTypeIdV1::from_index(3);
            } else {
                result_type = SemanticTypeIdV1::from_index(3);
            }
            types.push(reference_type(4, POINTEE, shape, pointee_alignment));
        }
        Case::DifferentReferenceType => {
            result_type = SemanticTypeIdV1::from_index(3);
            types.push(reference_type(4, POINTEE, shape, pointee_alignment));
        }
        Case::DifferentElement => {
            types.push(i32_type(4));
            types.push(slice_type(5, SemanticTypeIdV1::from_index(3)));
            result_type = SemanticTypeIdV1::from_index(5);
            types.push(reference_type(6, SemanticTypeIdV1::from_index(4), shape, 4));
        }
        Case::NoProjection => source_type = POINTEE,
        _ => {}
    }
    let mut place_type = POINTEE;
    let mut projections =
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, POINTEE).unwrap()];
    match case {
        Case::ExtraProjection => projections.push(
            SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, POINTEE).unwrap(),
        ),
        Case::IndexedProjection => {
            projections.push(
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(0)),
                    ELEMENT,
                )
                .unwrap(),
            );
            place_type = ELEMENT;
        }
        Case::NoProjection => projections.clear(),
        Case::WrongProjectionResult => {
            projections[0] =
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ELEMENT).unwrap();
            place_type = ELEMENT;
        }
        _ => {}
    }
    let place =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, place_type).unwrap();
    let value = if matches!(case, Case::AddressOf) {
        SemanticRvalueKindV1::AddressOf {
            mutability: shape.mutability,
            place,
        }
    } else {
        let kind = match case {
            Case::Mutable | Case::SharedToMutable => SemanticBorrowKindV1::Mutable,
            Case::Fake => SemanticBorrowKindV1::Fake,
            _ => SemanticBorrowKindV1::Shared,
        };
        SemanticRvalueKindV1::Borrow { kind, place }
    };
    request_with_statement(
        types,
        abi(1, vec![], ELEMENT),
        vec![
            local(1, ELEMENT, SemanticLocalRoleV1::Return),
            local(2, source_type, SemanticLocalRoleV1::Temporary),
            local(3, result_type, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], result_type).unwrap(),
            SemanticRvalueV1::new(result_type, value),
        )),
    )
}

fn statement_location() -> SemanticMirLocationV1 {
    SemanticMirLocationV1::Statement {
        function: SemanticFunctionIdV1::from_index(0),
        block: SemanticBlockIdV1::from_index(0),
        statement: 0,
    }
}

#[test]
fn exact_shared_and_mutable_slice_reborrows_preserve_the_semantic_carrier() {
    let limits = SemanticMirLimitsV1::default();
    for case in [Case::Shared, Case::Mutable] {
        let admitted = slice_reborrow_request(case)
            .admit_current_production(limits)
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            limits,
        )
        .unwrap();
        assert_eq!(decoded.functions(), admitted.functions());
        assert_eq!(decoded.types(), admitted.types());
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        let function = &admitted.functions()[0];
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[0].kind()
        else {
            panic!("expected the actual assignment");
        };
        assert_eq!(assignment.value().result_type(), REFERENCE);
        assert_eq!(assignment.destination().ty(), REFERENCE);
        assert_eq!(function.locals()[1].ty(), REFERENCE);
        let SemanticTypeShapeV1::Pointer(pointer) = admitted.types()[2].shape() else {
            panic!("expected the exact slice-reference type");
        };
        assert_eq!(pointer.metadata(), SemanticPointerMetadataV1::SliceLength);
        assert_eq!(pointer.pointee(), POINTEE);
        assert_eq!(
            pointer.mutability(),
            if matches!(case, Case::Shared) {
                SemanticMutabilityV1::Immutable
            } else {
                SemanticMutabilityV1::Mutable
            }
        );
    }
}

#[test]
fn slice_reborrow_does_not_change_access_or_invent_pointer_metadata() {
    for case in [
        Case::SharedToMutable,
        Case::MutableToShared,
        Case::Fake,
        Case::RawSource,
        Case::RawResult,
        Case::AddressOf,
        Case::OtherAddressSpace,
        Case::MissingResultLength,
        Case::MissingSourceLength,
        Case::VtableResult,
        Case::NonSlicePointee,
        Case::StrPointee,
        Case::DifferentReferenceType,
        Case::DifferentElement,
    ] {
        assert_eq!(
            slice_reborrow_request(case)
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::Borrow,
                location: statement_location(),
            },
            "{case:?}",
        );
    }
}

#[test]
fn whole_slice_reborrow_requires_one_exact_dereference() {
    for case in [
        Case::ExtraProjection,
        Case::IndexedProjection,
        Case::NoProjection,
    ] {
        assert_eq!(
            slice_reborrow_request(case)
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::Borrow,
                location: statement_location(),
            },
            "{case:?}",
        );
    }
    assert_eq!(
        slice_reborrow_request(Case::WrongProjectionResult)
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::TypeMismatch {
            expected: POINTEE,
            actual: ELEMENT,
            location: statement_location()
        },
    );
}

#[test]
fn slice_reborrow_keeps_target_layout_and_non_slice_thin_borrow_rules() {
    assert_eq!(
        slice_reborrow_request(Case::WrongPointerWidth)
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidTypeLayout,
    );
    for (kind, mutability) in [
        (
            SemanticBorrowKindV1::Shared,
            SemanticMutabilityV1::Immutable,
        ),
        (SemanticBorrowKindV1::Mutable, SemanticMutabilityV1::Mutable),
    ] {
        borrow_request(kind, SemanticPointerKindV1::Reference, mutability, false)
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
    }
    borrow_request(
        SemanticBorrowKindV1::Shared,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Immutable,
        true,
    )
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
}
