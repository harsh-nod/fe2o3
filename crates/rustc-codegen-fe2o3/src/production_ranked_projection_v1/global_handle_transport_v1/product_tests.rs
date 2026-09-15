use super::*;
#[path = "product_boundaries_tests.rs"]
mod boundaries;
#[path = "product_scalar_owner_tests.rs"]
mod scalar_owner;
#[path = "exclusive_tests.rs"]
mod exclusive_tests;
use fe2o3_mir_model::semantic_mir_v1::*;
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const GLOBAL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const WRAPPER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const WRAPPER_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const OPTION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const MUT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const TWO: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const MUT_ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const MIXED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const TWO_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const THREE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
const FOUR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
const FIVE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
const PRODUCT_OPTION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
const TWO_AND_SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);
const BORROWED_THREE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(17);
const BORROWED_TWO_SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(18);
const RAW_TWO: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(19);
const MUT_TWO: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(20);

fn pointer(pointee: SemanticTypeIdV1, mutable: bool) -> SemanticTypeShapeV1 {
    SemanticTypeShapeV1::Pointer(
        SemanticPointerTypeV1::new_with_kind(
            pointee,
            SemanticPointerKindV1::Reference,
            if mutable {
                SemanticMutabilityV1::Mutable
            } else {
                SemanticMutabilityV1::Immutable
            },
            0,
            64,
            SemanticPointerMetadataV1::None,
        )
        .unwrap(),
    )
}

fn aggregate(fields: Vec<SemanticTypeIdV1>) -> SemanticTypeShapeV1 {
    SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap())
}

fn fixture() -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    ProjectedCapabilityStateV1,
) {
    let shapes = vec![
        SemanticTypeShapeV1::Unit,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            bits: 64,
            signed: false,
        }),
        aggregate(vec![SCALAR]),
        pointer(GLOBAL, false),
        aggregate(vec![REF, SCALAR]),
        pointer(WRAPPER, false),
        SemanticTypeShapeV1::Enum {
            discriminant: SCALAR,
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![WRAPPER]).unwrap()),
            ]
            .into(),
        },
        pointer(GLOBAL, true),
        aggregate(vec![REF, REF]),
        aggregate(vec![MUT_REF]),
        aggregate(vec![REF, MUT_REF]),
        pointer(TWO, false),
        aggregate(vec![REF, REF, WRAPPER]),
        aggregate(vec![REF; 4]),
        aggregate(vec![REF; 5]),
        SemanticTypeShapeV1::Enum {
            discriminant: SCALAR,
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![TWO]).unwrap()),
            ]
            .into(),
        },
        aggregate(vec![REF, REF, SCALAR]),
        pointer(THREE, false),
        pointer(TWO_AND_SCALAR, false),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                TWO,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        pointer(TWO, true),
    ];
    let types = shapes
        .into_iter()
        .enumerate()
        .map(|(i, shape)| {
            let size = [
                0, 8, 8, 8, 16, 8, 24, 8, 16, 8, 16, 8, 32, 32, 40, 24, 24, 8, 8, 8, 8,
            ][i];
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([i as u8 + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([i as u8 + 1; 32]),
                SemanticTypeLayoutV1::new(Some(size), if i == 0 { 1 } else { 8 }).unwrap(),
                shape,
            )
        })
        .collect();
    let local_types = [
        UNIT,
        REF,
        REF,
        WRAPPER,
        WRAPPER_REF,
        REF,
        SCALAR,
        OPTION,
        WRAPPER,
        MUT_REF,
        TWO,
        MUT_ENV,
        MIXED,
        TWO_REF,
        THREE,
        REF,
        REF,
        FOUR,
        FIVE,
        REF,
        PRODUCT_OPTION,
        TWO_AND_SCALAR,
        BORROWED_TWO_SCALAR,
        TWO,
        TWO_REF,
        BORROWED_THREE,
        RAW_TWO,
        MUT_TWO,
    ];
    let source = SemanticSourceProvenanceV1::unavailable();
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(i, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                ty,
                if i == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([1; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([1; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([1; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([1; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    // Deliberately inert unit-test custody records, not compiler-issued proof.
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([1; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
    )
    .unwrap();
    let value = |allocation_origin, mutable| {
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(
            ProjectedGlobalViewV1 {
                view: GLOBAL,
                physical: GLOBAL,
                element: SCALAR,
                provenance,
                contract: if mutable {
                    SemanticCapabilityMemoryContractV1::global_exclusive_read_write()
                } else {
                    SemanticCapabilityMemoryContractV1::global_read_only()
                },
                allocation: AllocationContractV1 {
                    allocation_origin,
                    noalias_class: allocation_origin,
                    writable: mutable,
                    singleton_object: false,
                },
                borrow: Some(if mutable {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                }),
            },
        ))
    };
    (
        types,
        function,
        HashMap::from([
            (1, value(1, false)),
            (2, value(2, false)),
            (9, value(3, true)),
            (15, value(4, false)),
            (16, value(5, false)),
            (19, value(6, false)),
        ]),
    )
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticAssignmentV1 {
    SemanticAssignmentV1::new(place(local, ty), SemanticRvalueV1::new(ty, kind))
}
fn construct(
    local: u32,
    ty: SemanticTypeIdV1,
    variant: Option<u32>,
    operands: Vec<SemanticOperandV1>,
) -> SemanticAssignmentV1 {
    assign(
        local,
        ty,
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                variant.map_or(
                    SemanticAggregateKindV1::Aggregate,
                    SemanticAggregateKindV1::EnumVariant,
                ),
                operands,
            )
            .unwrap(),
        ),
    )
}
fn use_path(local: u32, path: &[(SemanticProjectionKindV1, SemanticTypeIdV1)]) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        path.iter()
            .map(|&(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap())
            .collect(),
        path.last().unwrap().1,
    )
    .unwrap()
}
fn extract_ref() -> SemanticAssignmentV1 {
    assign(
        5,
        REF,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
            4,
            &[
                (SemanticProjectionKindV1::Dereference, WRAPPER),
                (SemanticProjectionKindV1::Field(0), REF),
            ],
        ))),
    )
}
fn wrapper(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &mut ProjectedCapabilityStateV1,
) {
    let a = construct(3, WRAPPER, None, vec![copy(1, REF), copy(6, SCALAR)]);
    let v = global_handle_transport_v1::assignment(types, function, state, &a).unwrap();
    state.insert(3, v);
    let a = assign(
        4,
        WRAPPER_REF,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(3, WRAPPER),
        },
    );
    let v = global_handle_transport_v1::assignment(types, function, state, &a).unwrap();
    state.insert(4, v);
}

#[test]
fn positive_repeated_shared_field_read_retains_exact_allocation() {
    let (types, function, mut state) = fixture();
    wrapper(&types, &function, &mut state);
    for _ in 0..2 {
        let a = extract_ref();
        assert_eq!(
            global_handle_transport_v1::assignment(&types, &function, &state, &a),
            Some(state[&1].clone())
        );
        let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
            unreachable!()
        };
        let initial = state.clone();
        consume_capability_operand_v1(&types, &function, &mut state, operand);
        assert_eq!(state, initial);
    }
}

#[test]
fn scalar_sibling_copy_alone_prevents_subsequent_global_field_read() {
    let (types, function, mut state) = fixture();
    wrapper(&types, &function, &mut state);
    assert!(
        global_handle_transport_v1::assignment(&types, &function, &state, &extract_ref()).is_some()
    );
    let scalar = SemanticOperandV1::Copy(use_path(
        4,
        &[
            (SemanticProjectionKindV1::Dereference, WRAPPER),
            (SemanticProjectionKindV1::Field(1), SCALAR),
        ],
    ));
    consume_capability_operand_v1(&types, &function, &mut state, &scalar);
    assert_eq!(state[&4], ProjectedCapabilityValueV1::Invalid);
    assert!(
        global_handle_transport_v1::assignment(&types, &function, &state, &extract_ref()).is_none()
    );
    assert!(matches!(
        state[&3],
        ProjectedCapabilityValueV1::CapturedGlobal(_)
    ));
}

#[test]
fn some_none_common_return_meet_loses_even_correctly_constructed_payload() {
    let (types, function, mut some) = fixture();
    wrapper(&types, &function, &mut some);
    let a = construct(7, OPTION, Some(1), vec![copy(3, WRAPPER)]);
    let capture = global_handle_transport_v1::assignment(&types, &function, &some, &a).unwrap();
    some.insert(7, capture.clone());
    let extract = assign(
        8,
        WRAPPER,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
            7,
            &[
                (SemanticProjectionKindV1::Downcast(1), OPTION),
                (SemanticProjectionKindV1::Field(0), WRAPPER),
            ],
        ))),
    );
    assert_eq!(
        global_handle_transport_v1::assignment(&types, &function, &some, &extract),
        Some(some[&3].clone())
    );
    let mut none = some.clone();
    assert!(
        global_handle_transport_v1::assignment(
            &types,
            &function,
            &none,
            &construct(7, OPTION, Some(0), vec![])
        )
        .is_none()
    );
    none.remove(&7); // Exact transfer behavior for an assignment with no origin.
    for reversed in [false, true] {
        let (mut current, incoming) = if reversed {
            (none.clone(), &some)
        } else {
            (some.clone(), &none)
        };
        merge_capability_states_v1(&mut current, incoming).unwrap();
        assert!(!current.contains_key(&7));
        assert!(
            global_handle_transport_v1::assignment(&types, &function, &current, &extract).is_none()
        );
    }
    let mut same = some.clone();
    assert!(!merge_capability_states_v1(&mut same, &some).unwrap());
    assert_eq!(same[&7], capture);
}

#[test]
fn two_distinct_shared_global_fields_retain_exact_allocation_not_type_selection() {
    let (types, function, mut state) = fixture();
    assert_ne!(state[&1], state[&2]);
    let a = construct(10, TWO, None, vec![copy(1, REF), copy(2, REF)]);
    let capture = global_handle_transport_v1::assignment(&types, &function, &state, &a).unwrap();
    assert!(matches!(
        capture,
        ProjectedCapabilityValueV1::CapturedGlobalProduct(_)
    ));
    state.insert(10, capture);
    for (field, source) in [(0, 1), (1, 2)] {
        let selected = assign(
            5,
            REF,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
                10,
                &[(SemanticProjectionKindV1::Field(field), REF)],
            ))),
        );
        assert_eq!(
            assignment(&types, &function, &state, &selected),
            Some(state[&source].clone())
        );
        let before = state.clone();
        consume_capability_rvalue_operands_v1(
            &types,
            &function,
            selected.value().kind(),
            &mut state,
        );
        assert_eq!(state, before);
    }
    state.remove(&2);
    assert!(global_handle_transport_v1::assignment(&types, &function, &state, &a).is_some());
    // One known field can still be transported, but this cannot authorize the other.
    let capture = global_handle_transport_v1::assignment(&types, &function, &state, &a).unwrap();
    state.insert(10, capture);
    let second = assign(
        5,
        REF,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
            10,
            &[(SemanticProjectionKindV1::Field(1), REF)],
        ))),
    );
    assert!(global_handle_transport_v1::assignment(&types, &function, &state, &second).is_none());
}

#[test]
fn mutable_output_borrow_is_not_a_shared_global_capture() {
    let (types, function, state) = fixture();
    for moving in [false, true] {
        let operand = if moving {
            SemanticOperandV1::Move(place(9, MUT_REF))
        } else {
            copy(9, MUT_REF)
        };
        let a = construct(11, MUT_ENV, None, vec![operand]);
        assert!(global_handle_transport_v1::assignment(&types, &function, &state, &a).is_none());
    }
}

#[test]
fn mixed_shared_input_and_mutable_output_capture_retains_only_input() {
    let (types, function, mut state) = fixture();
    let a = construct(
        12,
        MIXED,
        None,
        vec![copy(1, REF), SemanticOperandV1::Move(place(9, MUT_REF))],
    );
    let capture = global_handle_transport_v1::assignment(&types, &function, &state, &a).unwrap();
    state.insert(12, capture);
    let input = assign(
        5,
        REF,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
            12,
            &[(SemanticProjectionKindV1::Field(0), REF)],
        ))),
    );
    assert_eq!(
        global_handle_transport_v1::assignment(&types, &function, &state, &input),
        Some(state[&1].clone())
    );
    let output = assign(
        9,
        MUT_REF,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(use_path(
            12,
            &[(SemanticProjectionKindV1::Field(1), MUT_REF)],
        ))),
    );
    assert!(global_handle_transport_v1::assignment(&types, &function, &state, &output).is_none());
}

#[test]
fn changed_source_allocation_or_dead_storage_cannot_reauthenticate_capture() {
    let (types, function, mut initial) = fixture();
    wrapper(&types, &function, &mut initial);
    for dead in [false, true] {
        let mut state = initial.clone();
        if dead {
            state.remove(&3);
        } else {
            state.insert(1, state[&2].clone());
            let a = construct(3, WRAPPER, None, vec![copy(1, REF), copy(6, SCALAR)]);
            let changed =
                global_handle_transport_v1::assignment(&types, &function, &state, &a).unwrap();
            state.insert(3, changed);
        }
        assert!(
            global_handle_transport_v1::assignment(&types, &function, &state, &extract_ref())
                .is_none()
        );
    }
}

fn two(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &mut ProjectedCapabilityStateV1,
) {
    let value = assignment(
        types,
        function,
        state,
        &construct(10, TWO, None, vec![copy(1, REF), copy(2, REF)]),
    )
    .unwrap();
    state.insert(10, value);
    let value = assignment(
        types,
        function,
        state,
        &assign(
            13,
            TWO_REF,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(10, TWO),
            },
        ),
    )
    .unwrap();
    state.insert(13, value);
}

fn borrowed_field(field: u32) -> SemanticAssignmentV1 {
    assign(
        5,
        REF,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
            13,
            &[
                (SemanticProjectionKindV1::Dereference, TWO),
                (SemanticProjectionKindV1::Field(field), REF),
            ],
        ))),
    )
}

#[test]
fn product_borrow_reauthenticates_even_the_unselected_source_field() {
    let (types, function, mut initial) = fixture();
    two(&types, &function, &mut initial);
    assert_eq!(
        assignment(&types, &function, &initial, &borrowed_field(0)),
        Some(initial[&1].clone())
    );
    for mutation in 0..4 {
        let mut state = initial.clone();
        match mutation {
            0 => {
                state.remove(&10);
            }
            1 => {
                state.insert(10, ProjectedCapabilityValueV1::Invalid);
            }
            2 => {
                let changed = assignment(
                    &types,
                    &function,
                    &state,
                    &construct(10, TWO, None, vec![copy(1, REF), copy(15, REF)]),
                )
                .unwrap();
                state.insert(10, changed);
            }
            3 => {
                let changed = assignment(
                    &types,
                    &function,
                    &state,
                    &construct(10, TWO, None, vec![copy(2, REF), copy(1, REF)]),
                )
                .unwrap();
                state.insert(10, changed);
            }
            _ => unreachable!(),
        }
        assert!(
            assignment(&types, &function, &state, &borrowed_field(0)).is_none(),
            "mutation={mutation}"
        );
    }
}

#[test]
fn product_paths_reject_wrong_field_type_deref_and_foreign_local_type() {
    let (types, function, mut state) = fixture();
    two(&types, &function, &mut state);
    for path in [
        vec![
            (SemanticProjectionKindV1::Dereference, TWO),
            (SemanticProjectionKindV1::Field(2), REF),
        ],
        vec![
            (SemanticProjectionKindV1::Dereference, WRAPPER),
            (SemanticProjectionKindV1::Field(0), REF),
        ],
        vec![(SemanticProjectionKindV1::Field(0), REF)],
        vec![
            (SemanticProjectionKindV1::Dereference, TWO),
            (SemanticProjectionKindV1::Field(0), MUT_REF),
        ],
    ] {
        let ty = path.last().unwrap().1;
        assert!(
            assignment(
                &types,
                &function,
                &state,
                &assign(
                    if ty == REF { 5 } else { 9 },
                    ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(13, &path)))
                )
            )
            .is_none()
        );
    }
    // A state record cannot change the actual retained local's type.
    state.insert(4, state[&13].clone());
    assert!(assignment(&types, &function, &state, &extract_ref()).is_none());
}

#[test]
fn product_move_and_unknown_scalar_copy_revoke_carrier_not_extracted_handle() {
    let (types, function, mut initial) = fixture();
    two(&types, &function, &mut initial);
    let extracted = assignment(&types, &function, &initial, &borrowed_field(0)).unwrap();
    initial.insert(5, extracted.clone());
    for operand in [
        SemanticOperandV1::Move(place(13, TWO_REF)),
        SemanticOperandV1::Move(use_path(
            13,
            &[
                (SemanticProjectionKindV1::Dereference, TWO),
                (SemanticProjectionKindV1::Field(0), REF),
            ],
        )),
        SemanticOperandV1::Copy(use_path(
            13,
            &[
                (SemanticProjectionKindV1::Dereference, TWO),
                (SemanticProjectionKindV1::Field(0), SCALAR),
            ],
        )),
    ] {
        let mut state = initial.clone();
        consume_capability_operand_v1(&types, &function, &mut state, &operand);
        assert_eq!(state[&13], ProjectedCapabilityValueV1::Invalid);
        assert_eq!(state[&5], extracted);
        assert!(assignment(&types, &function, &state, &borrowed_field(0)).is_none());
    }
}

#[test]
fn product_cannot_become_known_origin_or_conditional_payload_authority() {
    let (types, function, mut state) = fixture();
    two(&types, &function, &mut state);
    assert!(capability_known_origin_v1(&state, &copy(10, TWO)).is_none());
    assert_eq!(
        wrap_capability_enum_value_v1(state[&10].clone(), 1).unwrap(),
        ProjectedCapabilityValueV1::Invalid
    );
    assert!(
        assignment(
            &types,
            &function,
            &state,
            &construct(20, PRODUCT_OPTION, Some(1), vec![copy(10, TWO)])
        )
        .is_none()
    );
    assert!(
        capability_borrow_origin_v1(&state, &place(10, TWO), SemanticBorrowKindV1::Mutable)
            .is_none()
    );
}

#[test]
fn product_copy_preservation_is_shared_reference_only_not_whole_owned_carrier() {
    let (types, function, mut initial) = fixture();
    two(&types, &function, &mut initial);
    let mut state = initial.clone();
    consume_capability_operand_v1(&types, &function, &mut state, &copy(13, TWO_REF));
    assert_eq!(state, initial);
    consume_capability_operand_v1(&types, &function, &mut state, &copy(10, TWO));
    assert_eq!(state[&10], ProjectedCapabilityValueV1::Invalid);
    assert!(assignment(&types, &function, &state, &borrowed_field(0)).is_none());
}

#[test]
fn product_does_not_extend_single_path_transport_to_writable_or_exclusive_views() {
    let (types, function, initial) = fixture();
    for writable in [false, true] {
        let mut state = initial.clone();
        let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view)) =
            state.get_mut(&2).unwrap()
        else {
            unreachable!()
        };
        if writable {
            view.allocation.writable = true;
        } else {
            view.contract = SemanticCapabilityMemoryContractV1::global_exclusive_read_write();
        }
        assert!(
            assignment(
                &types,
                &function,
                &state,
                &construct(10, TWO, None, vec![copy(1, REF), copy(2, REF)])
            )
            .is_none()
        );
    }
}

#[test]
fn product_terminal_copy_work_is_charged_before_consumption() {
    let (types, function, mut state) = fixture();
    two(&types, &function, &mut state);
    let terminator = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![copy(13, TWO_REF)],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let original = state.clone();
    let mut work = 0;
    charge_product_terminator(&state, &terminator, &mut work).unwrap();
    assert_eq!(work, 6);
    assert_eq!(state, original);
    let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    assert!(charge_product_terminator(&state, &terminator, &mut work).is_err());
    assert_eq!(state, original);
}

#[test]
fn product_meet_is_exact_and_missing_or_swapped_predecessor_loses_custody() {
    let (types, function, mut initial) = fixture();
    two(&types, &function, &mut initial);
    let mut same = initial.clone();
    assert!(!merge_capability_states_v1(&mut same, &initial).unwrap());
    for changed in [false, true] {
        let mut other = initial.clone();
        if changed {
            let value = assignment(
                &types,
                &function,
                &other,
                &construct(10, TWO, None, vec![copy(2, REF), copy(1, REF)]),
            )
            .unwrap();
            other.insert(10, value);
        } else {
            other.remove(&10);
        }
        for reverse in [false, true] {
            let (mut current, incoming) = if reverse {
                (other.clone(), &initial)
            } else {
                (initial.clone(), &other)
            };
            merge_capability_states_v1(&mut current, incoming).unwrap();
            assert!(!current.contains_key(&10));
            assert!(assignment(&types, &function, &current, &borrowed_field(0)).is_none());
        }
    }
}

#[test]
fn product_retains_three_nested_paths_and_rejects_duplicate_and_fifth_handle() {
    let (types, function, mut state) = fixture();
    // Three shared Globals, one nested in an independently initialized wrapper.
    let value = assignment(
        &types,
        &function,
        &state,
        &construct(3, WRAPPER, None, vec![copy(15, REF), copy(6, SCALAR)]),
    )
    .unwrap();
    state.insert(3, value);
    let value = assignment(
        &types,
        &function,
        &state,
        &construct(
            14,
            THREE,
            None,
            vec![copy(1, REF), copy(2, REF), copy(3, WRAPPER)],
        ),
    )
    .unwrap();
    state.insert(14, value);
    let value = assignment(
        &types,
        &function,
        &state,
        &assign(
            25,
            BORROWED_THREE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(14, THREE),
            },
        ),
    )
    .unwrap();
    state.insert(25, value);
    for (field, source) in [(0, 1), (1, 2), (2, 15)] {
        let mut path = vec![
            (SemanticProjectionKindV1::Dereference, THREE),
            (
                SemanticProjectionKindV1::Field(field),
                if field == 2 { WRAPPER } else { REF },
            ),
        ];
        if field == 2 {
            path.push((SemanticProjectionKindV1::Field(0), REF));
        }
        let value = assignment(
            &types,
            &function,
            &state,
            &assign(
                5,
                REF,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(25, &path))),
            ),
        );
        assert_eq!(value, Some(state[&source].clone()));
    }
    assert!(
        assignment(
            &types,
            &function,
            &state,
            &construct(10, TWO, None, vec![copy(1, REF), copy(1, REF)])
        )
        .is_none()
    );
    assert!(
        assignment(
            &types,
            &function,
            &state,
            &construct(
                17,
                FOUR,
                None,
                [1, 2, 15, 16].map(|local| copy(local, REF)).to_vec()
            )
        )
        .is_some()
    );
    assert!(
        assignment(
            &types,
            &function,
            &state,
            &construct(
                18,
                FIVE,
                None,
                [1, 2, 15, 16, 19].map(|local| copy(local, REF)).to_vec()
            )
        )
        .is_none()
    );
}

fn with_statements(
    function: &SemanticFunctionDeclV1,
    statements: Vec<SemanticStatementKindV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        vec![
            SemanticBasicBlockV1::new(
                function.blocks()[0].identity(),
                function.source(),
                statements
                    .into_iter()
                    .map(|kind| SemanticStatementV1::new(function.source(), kind))
                    .collect(),
                function.blocks()[0].terminator().clone(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn product_real_statement_hook_moves_parameter_value_and_charges_shared_budget() {
    let (types, function, initial) = fixture();
    let function = with_statements(
        &function,
        vec![
            SemanticStatementKindV1::Assign(construct(
                10,
                TWO,
                None,
                vec![copy(1, REF), copy(2, REF)],
            )),
            SemanticStatementKindV1::Assign(assign(
                23,
                TWO,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(10, TWO))),
            )),
            SemanticStatementKindV1::Assign(assign(
                24,
                TWO_REF,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(23, TWO),
                },
            )),
            SemanticStatementKindV1::Assign(assign(
                5,
                REF,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
                    24,
                    &[
                        (SemanticProjectionKindV1::Dereference, TWO),
                        (SemanticProjectionKindV1::Field(1), REF),
                    ],
                ))),
            )),
        ],
    );
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let mut state = initial.clone();
    let mut work = 0;
    transfer_capability_statements_metered_v1(
        &types, &function, 0, &mut state, &dominance, None, None, None, &mut work,
    )
    .unwrap();
    assert!(work > 0);
    assert_eq!(state[&5], initial[&2]);
    assert_eq!(state[&10], ProjectedCapabilityValueV1::Invalid);
    for local in [23, 24] {
        assert!(matches!(
            state[&local],
            ProjectedCapabilityValueV1::CapturedGlobalProduct(_)
        ));
    }
    let mut exhausted = initial.clone();
    let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    assert!(
        transfer_capability_statements_metered_v1(
            &types,
            &function,
            0,
            &mut exhausted,
            &dominance,
            None,
            None,
            None,
            &mut work
        )
        .is_err()
    );
    assert_eq!(exhausted, initial);
}

#[test]
fn product_real_statement_hook_keeps_storage_death_and_projected_write_kills() {
    let (types, function, mut initial) = fixture();
    two(&types, &function, &mut initial);
    for kill in [
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(10)),
        SemanticStatementKindV1::Deinitialize(place(10, TWO)),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            use_path(10, &[(SemanticProjectionKindV1::Field(1), REF)]),
            SemanticRvalueV1::new(REF, SemanticRvalueKindV1::Use(copy(15, REF))),
        )),
        SemanticStatementKindV1::Assign(assign(
            26,
            RAW_TWO,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(10, TWO),
            },
        )),
        SemanticStatementKindV1::Assign(assign(
            27,
            MUT_TWO,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(10, TWO),
            },
        )),
    ] {
        let function = with_statements(
            &function,
            vec![kill, SemanticStatementKindV1::Assign(borrowed_field(0))],
        );
        let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        let mut state = initial.clone();
        transfer_capability_statements_metered_v1(
            &types, &function, 0, &mut state, &dominance, None, None, None, &mut 0,
        )
        .unwrap();
        assert!(!state.contains_key(&5));
    }
}

#[test]
fn product_storage_counts_shared_payload_in_each_retained_entry_without_ceiling_raise() {
    let (types, function, mut state) = fixture();
    two(&types, &function, &mut state);
    let value = state[&10].clone();
    let single = HashMap::from([(10, value.clone())]);
    assert_eq!(capability_state_storage_units_v1(&single).unwrap(), 4);
    assert_eq!(
        capability_state_storage_units_v1(&try_clone_capability_state_v1(&single).unwrap())
            .unwrap(),
        4
    );
    let mut full: ProjectedCapabilityStateV1 = (0..MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 / 4)
        .map(|local| (local, value.clone()))
        .collect();
    let units = capability_state_storage_units_v1(&full).unwrap();
    assert_eq!(units, full.len() * 4);
    let remainder = MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 - units;
    for _ in 0..remainder {
        full.insert(full.len(), ProjectedCapabilityValueV1::Invalid);
    }
    assert_eq!(
        capability_state_storage_units_v1(&full).unwrap(),
        MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1
    );
    full.insert(full.len(), ProjectedCapabilityValueV1::Invalid);
    assert!(capability_state_storage_units_v1(&full).is_err());
    assert!(try_clone_capability_state_v1(&full).is_err());
}

// The source41 compressed roster has aggregate -> parameter Move -> closure
// Copy -> field Copy. Local numbers here are fixture locals, not source evidence.
fn closure_transfer_sequence() -> Vec<SemanticStatementKindV1> {
    vec![
        SemanticStatementKindV1::Assign(construct(
            10,
            TWO,
            None,
            vec![
                SemanticOperandV1::Move(place(1, REF)),
                SemanticOperandV1::Move(place(2, REF)),
            ],
        )),
        SemanticStatementKindV1::Assign(assign(
            23,
            TWO,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(10, TWO))),
        )),
        SemanticStatementKindV1::Assign(assign(10, TWO, SemanticRvalueKindV1::Use(copy(23, TWO)))),
        SemanticStatementKindV1::Assign(assign(
            5,
            REF,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
                10,
                &[(SemanticProjectionKindV1::Field(1), REF)],
            ))),
        )),
        SemanticStatementKindV1::Assign(assign(
            19,
            REF,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
                10,
                &[(SemanticProjectionKindV1::Field(0), REF)],
            ))),
        )),
    ]
}

#[test]
fn product_closure_copy_transfers_each_field_without_preserving_the_whole_owner() {
    let (types, original, initial) = fixture();
    let function = with_statements(&original, closure_transfer_sequence());
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let mut state = initial.clone();
    transfer_capability_statements_metered_v1(
        &types, &function, 0, &mut state, &dominance, None, None, None, &mut 0,
    )
    .unwrap();
    assert_eq!(state[&5], initial[&2]);
    assert_eq!(state[&19], initial[&1]);
    assert_ne!(state[&5], state[&19]);
    for consumed in [1, 2, 23] {
        assert_eq!(state[&consumed], ProjectedCapabilityValueV1::Invalid);
    }
    assert!(matches!(
        state[&10],
        ProjectedCapabilityValueV1::CapturedGlobalProduct(_)
    ));
    // A new borrowed field is preserved, but the copied owned environment is not.
    assert!(!preserves_projected_copy(
        &types,
        &function,
        &state,
        &place(10, TWO)
    ));
}

#[test]
fn product_closure_transfer_rejects_missing_current_input_dead_and_overwritten_owner() {
    let (types, original, initial) = fixture();
    for mutation in 0..4 {
        let mut state = initial.clone();
        let mut statements = closure_transfer_sequence();
        match mutation {
            0 => {
                state.remove(&2);
            }
            1 => {
                statements.insert(
                    3,
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(10)),
                );
            }
            2 => {
                statements.insert(
                    3,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        use_path(10, &[(SemanticProjectionKindV1::Field(1), REF)]),
                        SemanticRvalueV1::new(REF, SemanticRvalueKindV1::Use(copy(15, REF))),
                    )),
                );
            }
            3 => {
                statements.insert(
                    3,
                    SemanticStatementKindV1::Assign(assign(
                        27,
                        MUT_TWO,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: place(10, TWO),
                        },
                    )),
                );
            }
            _ => unreachable!(),
        }
        let function = with_statements(&original, statements);
        let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        transfer_capability_statements_metered_v1(
            &types, &function, 0, &mut state, &dominance, None, None, None, &mut 0,
        )
        .unwrap();
        assert!(
            !state.contains_key(&5),
            "must reject selected field after mutation {mutation}"
        );
    }
}
