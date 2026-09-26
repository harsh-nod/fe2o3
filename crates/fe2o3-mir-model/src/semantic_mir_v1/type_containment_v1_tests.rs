use super::*;

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn ordered_identity(domain: u8, index: usize) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[0] = domain;
    bytes[24..].copy_from_slice(&(index as u64).to_be_bytes());
    bytes
}

fn declaration(
    index: usize,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(ordered_identity(2, index)),
        SemanticLayoutIdentityV1(ordered_identity(3, index)),
        layout,
        shape,
    )
}

fn fields(indices: &[u32]) -> SemanticAggregateTypeV1 {
    SemanticAggregateTypeV1::new(indices.iter().copied().map(id).collect()).unwrap()
}

fn aggregate(
    index: usize,
    size: u64,
    alignment: u64,
    children: &[u32],
    offsets: &[u64],
) -> SemanticTypeDeclV1 {
    declaration(
        index,
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            alignment,
            SemanticAggregateLayoutV1::new(offsets.to_vec(), vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(fields(children)),
    )
}

fn array(index: usize, element: u32, length: u64, stride: u64) -> SemanticTypeDeclV1 {
    declaration(
        index,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            length * stride,
            stride.max(1),
            SemanticFieldsShapeV1::array(stride, length),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            stride.max(1),
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: id(element),
            length,
        },
    )
}

fn union(index: usize, size: u64, children: &[u32]) -> SemanticTypeDeclV1 {
    declaration(
        index,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            size,
            size.max(1),
            SemanticFieldsShapeV1::Union {
                field_count: children.len() as u64,
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            size.max(1),
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(fields(children)),
    )
}

fn single_enum(index: usize, size: u64, selected: &[u32], other: &[&[u32]]) -> SemanticTypeDeclV1 {
    let mut ty = aggregate(index, size, size.max(1), selected, &vec![0; selected.len()]);
    let mut variants = vec![SemanticEnumVariantV1::new(0, fields(selected))];
    for (ordinal, children) in other.iter().enumerate() {
        variants.push(SemanticEnumVariantV1::new_with_inhabitedness(
            (ordinal + 1) as u128,
            fields(children),
            true,
        ));
    }
    ty.shape = SemanticTypeShapeV1::enum_type(id(0), variants).unwrap();
    ty
}

fn pointer(index: usize, pointee: u32, reference: bool) -> SemanticTypeDeclV1 {
    declaration(
        index,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(u128::from(reference), u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                id(pointee),
                if reference {
                    SemanticPointerKindV1::Reference
                } else {
                    SemanticPointerKindV1::Raw
                },
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn request_with(extra: Vec<SemanticTypeDeclV1>) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let mut types = request.types.to_vec();
    types.extend(extra);
    request.types = types.into_boxed_slice();
    let mut locals = request.functions[0].locals.to_vec();
    for index in 1..request.types.len() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(ordered_identity(20, index)),
            id(index as u32),
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    request.functions[0].locals = locals.into_boxed_slice();
    request
}

fn context(request: &InertSemanticMirRequestV1, work: u64, max: u64) -> ValidationContextV1<'_> {
    ValidationContextV1 {
        request,
        limits: SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, max)
            .unwrap(),
        totals: ValidationTotalsV1::default(),
        work,
        owned_execution_roles: Vec::new(),
    }
}

fn local_types_are_valid(request: &InertSemanticMirRequestV1) {
    let mut context = context(request, 0, 1_000_000);
    for (index, ty) in request.types.iter().enumerate() {
        validate_type(&mut context, id(index as u32), ty).unwrap();
    }
}

fn rejects_cycle(request: InertSemanticMirRequestV1) {
    local_types_are_valid(&request);
    for result in [
        request.clone().admit(SemanticMirLimitsV1::default()),
        request
            .clone()
            .admit_exact_v15(SemanticMirLimitsV1::default()),
        request
            .clone()
            .admit_current_production(SemanticMirLimitsV1::default()),
    ] {
        assert_eq!(result.unwrap_err(), SemanticMirErrorV1::InvalidTypeLayout);
    }
    let bytes = encode_request(
        &request,
        SemanticMirWireVersionV1::V15,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    for result in [
        AdmittedInertSemanticMirV1::decode_exact_v15_canonical(
            &bytes,
            SemanticMirLimitsV1::default(),
        ),
        AdmittedInertSemanticMirV1::decode_minimal_compatible_canonical(
            &bytes,
            SemanticMirLimitsV1::default(),
        ),
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            &bytes,
            SemanticMirLimitsV1::default(),
        ),
    ] {
        assert!(matches!(
            result,
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidTypeLayout
            ))
        ));
    }
}

fn admits_and_round_trips(request: InertSemanticMirRequestV1) {
    local_types_are_valid(&request);
    let expected = encode_request(
        &request,
        SemanticMirWireVersionV1::V15,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let admitted = request
        .clone()
        .admit_exact_v15(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(admitted.canonical_encoding(), expected);
    assert_eq!(admitted.types(), request.types.as_ref());
    for decoded in [
        AdmittedInertSemanticMirV1::decode_exact_v15_canonical(
            &expected,
            SemanticMirLimitsV1::default(),
        )
        .unwrap(),
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            &expected,
            SemanticMirLimitsV1::default(),
        )
        .unwrap(),
    ] {
        assert_eq!(decoded.canonical_encoding(), expected);
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
    }
    request
        .clone()
        .admit_minimal_compatible(SemanticMirLimitsV1::default())
        .unwrap();
    request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
}

#[test]
fn direct_recursive_fields_are_rejected_after_their_local_layout_checks_pass() {
    let mut tuple = aggregate(1, 8, 8, &[1], &[0]);
    tuple.shape = SemanticTypeShapeV1::Tuple(fields(&[1]));
    for ty in [
        aggregate(1, 8, 8, &[1], &[0]),
        tuple,
        union(1, 8, &[1]),
        array(1, 1, 1, 8),
        single_enum(1, 8, &[1], &[]),
    ] {
        rejects_cycle(request_with(vec![ty]));
    }
}

#[test]
fn mutual_and_mixed_containment_cycles_do_not_depend_on_identity_order() {
    rejects_cycle(request_with(vec![
        aggregate(1, 8, 8, &[2], &[0]),
        aggregate(2, 8, 8, &[1], &[0]),
    ]));
    for reverse in [false, true] {
        let children = if reverse { [4, 1, 2, 3] } else { [2, 3, 4, 1] };
        rejects_cycle(request_with(vec![
            aggregate(1, 8, 8, &[children[0]], &[0]),
            union(2, 8, &[children[1]]),
            array(3, children[2], 1, 8),
            single_enum(4, 8, &[children[3]], &[]),
        ]));
    }
}

#[test]
fn zero_length_and_zero_sized_shapes_do_not_hide_recursive_types() {
    for ty in [
        aggregate(1, 0, 1, &[1], &[0]),
        union(1, 0, &[1]),
        array(1, 1, 0, 0),
        array(1, 1, 1, 0),
        single_enum(1, 0, &[1], &[]),
    ] {
        rejects_cycle(request_with(vec![ty]));
    }
}

#[test]
fn every_enum_variant_is_traversed_including_uninhabited_payloads() {
    for before in [0, 1, 7] {
        let mut other = vec![&[][..]; before];
        other.push(&[1]);
        rejects_cycle(request_with(vec![single_enum(1, 4, &[0], &other)]));
    }
}

#[test]
fn pointers_references_and_function_pointer_signatures_break_containment_cycles() {
    for reference in [false, true] {
        admits_and_round_trips(request_with(vec![
            aggregate(1, 8, 8, &[2], &[0]),
            pointer(2, 1, reference),
        ]));
    }
    let mut function_pointer = pointer(2, 1, true);
    function_pointer.shape = SemanticTypeShapeV1::FunctionPointer {
        safety: SemanticFunctionSafetyV1::Safe,
        extern_abi: SemanticExternAbiV1::Rust,
        c_variadic: false,
        arguments: fields(&[1]),
        return_type: id(1),
    };
    admits_and_round_trips(request_with(vec![
        aggregate(1, 8, 8, &[2], &[0]),
        function_pointer,
    ]));
}

#[test]
fn pointers_do_not_hide_an_invalid_pointees_own_containment_cycle() {
    rejects_cycle(request_with(vec![
        pointer(1, 2, false),
        aggregate(2, 8, 8, &[2], &[0]),
    ]));
}

fn diamond() -> InertSemanticMirRequestV1 {
    let mut tuple = aggregate(2, 4, 4, &[4], &[0]);
    tuple.shape = SemanticTypeShapeV1::Tuple(fields(&[4]));
    request_with(vec![
        aggregate(1, 8, 4, &[2, 3], &[0, 4]),
        tuple,
        union(3, 4, &[4]),
        single_enum(4, 4, &[0], &[&[]]),
    ])
}

#[test]
fn shared_children_and_forward_references_are_not_cycles() {
    admits_and_round_trips(diamond());
    admits_and_round_trips(request_with(vec![
        aggregate(1, 4, 4, &[0], &[0]),
        array(2, 1, 1, 4),
        union(3, 4, &[1, 2]),
        single_enum(4, 4, &[3], &[&[]]),
    ]));
}

#[test]
fn acyclic_zero_sized_and_empty_shapes_remain_admissible() {
    let empty = aggregate(1, 0, 1, &[], &[]);
    let mut tuple = aggregate(3, 0, 1, &[1, 2, 1], &[0, 0, 0]);
    tuple.shape = SemanticTypeShapeV1::Tuple(fields(&[1, 2, 1]));
    admits_and_round_trips(request_with(vec![
        empty,
        array(2, 1, u64::MAX, 0),
        tuple,
        union(4, 0, &[1, 3]),
        single_enum(5, 0, &[4], &[&[], &[1]]),
        array(6, 0, 0, 4),
    ]));
}

#[test]
fn slice_elements_are_contained_but_a_slice_pointer_breaks_recursion() {
    let mut slice = array(2, 1, 0, 8);
    slice.layout.size_bytes = None;
    slice.layout.backend_repr = SemanticBackendReprV1::memory(false);
    slice.shape = SemanticTypeShapeV1::Slice { element: id(1) };
    let request = request_with(vec![aggregate(1, 8, 8, &[1], &[0]), slice.clone()]);
    rejects_cycle(request);
    // The direct domain checks a slice edge once, not by runtime extent.
    let mut request = request_with(vec![pointer(1, 2, false), slice]);
    let mut checked = context(&request, 0, 1_000);
    type_containment_v1::validate(&mut checked).unwrap();
    assert_eq!(checked.work, 4 * 3 + 1);
    request.types[1] = aggregate(1, 8, 8, &[2], &[0]);
    let mut context = context(&request, 0, 1_000);
    assert_eq!(
        type_containment_v1::validate(&mut context),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    );
}

#[test]
fn deep_graphs_use_a_bounded_explicit_stack_and_visit_each_type_once() {
    const COUNT: usize = 4096;
    for reversed in [false, true] {
        let extra = (1..COUNT)
            .map(|index| {
                let child = if reversed {
                    index - 1
                } else if index + 1 == COUNT {
                    0
                } else {
                    index + 1
                };
                aggregate(index, 4, 4, &[child as u32], &[0])
            })
            .collect();
        let request = request_with(extra);
        let expected = 4 * COUNT as u64 + (COUNT - 1) as u64;
        let mut context = context(&request, 0, expected);
        type_containment_v1::validate(&mut context).unwrap();
        assert_eq!(context.work, expected);
        assert_eq!(context.request.types.len(), COUNT);
    }
}

#[test]
fn containment_work_has_independent_exact_and_one_short_boundaries() {
    let request = diamond();
    // Five types, five inline edges and two enum variants. A pointer/signature
    // edge would not appear in this count; a shared child is not entered twice.
    const REQUIRED: u64 = 4 * 5 + 5 + 2;
    for floor in [0, 73] {
        let mut exact = context(&request, floor, floor + REQUIRED);
        type_containment_v1::validate(&mut exact).unwrap();
        assert_eq!(exact.work, floor + REQUIRED);
        let mut short = context(&request, floor, floor + REQUIRED - 1);
        assert_eq!(
            type_containment_v1::validate(&mut short),
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual: floor + REQUIRED,
                max: floor + REQUIRED - 1,
            })
        );
        assert_eq!(short.work, floor + REQUIRED);
        assert!(short.owned_execution_roles.is_empty());
        // Scratch is local and is discarded on failure; a fresh validation
        // against the same immutable request reaches the same exact boundary.
        let mut replay = context(&request, floor, floor + REQUIRED);
        type_containment_v1::validate(&mut replay).unwrap();
        assert_eq!(replay.work, exact.work);
    }
}

#[test]
fn cycle_detection_and_early_work_denial_have_stable_priority() {
    let request = request_with(vec![aggregate(1, 8, 8, &[1], &[0])]);
    // Initialize two colors; inspect/enter/finish scalar 0; inspect/enter type 1;
    // examine its gray self edge. The cycle is found at the eighth work unit.
    for floor in [0, 61] {
        let mut exact = context(&request, floor, floor + 8);
        assert_eq!(
            type_containment_v1::validate(&mut exact),
            Err(SemanticMirErrorV1::InvalidTypeLayout)
        );
        assert_eq!(exact.work, floor + 8);
        for allowance in [0, 1, 7] {
            let mut denied = context(&request, floor, floor + allowance);
            assert_eq!(
                type_containment_v1::validate(&mut denied),
                Err(SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    actual: floor + allowance + 1,
                    max: floor + allowance,
                })
            );
            assert_eq!(denied.work, floor + allowance + 1);
        }
    }
}

#[test]
fn scratch_accounting_is_linear_checked_and_bounded_by_the_existing_type_limit() {
    use std::mem::size_of;
    for count in [0, 1, 5, 4096] {
        let independent =
            2 * size_of::<Vec<usize>>() + count * (size_of::<u8>() + 3 * size_of::<usize>());
        assert_eq!(
            type_containment_v1::scratch_bytes(count).unwrap(),
            independent
        );
    }
    assert_eq!(
        type_containment_v1::scratch_bytes(usize::MAX),
        Err(SemanticMirErrorV1::ArithmeticOverflow {
            resource: SemanticMirResourceV1::Types,
        })
    );
    let request = diamond();
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Types, 5)
        .unwrap();
    request.clone().admit_current_production(exact).unwrap();
    let short = exact.with_limit(SemanticMirResourceV1::Types, 4).unwrap();
    assert_eq!(
        request.admit_current_production(short).unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Types,
            actual: 5,
            max: 4,
        }
    );
}

#[test]
fn malformed_type_references_still_fail_before_the_containment_pass() {
    let mut request = request_with(vec![
        aggregate(1, 8, 8, &[1], &[0]),
        aggregate(2, 8, 8, &[3], &[0]),
    ]);
    let expected = SemanticMirErrorV1::InvalidReference {
        reference: SemanticMirReferenceV1::Type,
        index: 3,
        bound: 3,
        location: SemanticMirLocationV1::Type(id(2)),
    };
    assert_eq!(
        request
            .clone()
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
    request.types[2] = pointer(2, 3, false);
    assert_eq!(
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
}
