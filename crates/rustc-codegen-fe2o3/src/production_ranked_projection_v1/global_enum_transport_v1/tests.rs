use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirOwnerV1, ProductionSemanticSsaOwnerV1};

#[path = "scalar_enum_result_tests.rs"]
pub(in super::super) mod scalar_enum_result_tests;

#[path = "private_cost_tests.rs"]
mod private_cost_tests;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const STORAGE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const WRAPPER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const WRAPPER_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const ENUM: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U64,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
    ))
}
fn statement(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn aggregate(
    kind: SemanticAggregateKindV1,
    operands: Vec<SemanticOperandV1>,
) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::aggregate(kind, operands).unwrap()
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn switch(local: u32, zero: u32, other: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: copy(local, U64),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, zero),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, other),
        )
        .unwrap(),
    }
}
fn block(
    id: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([id + 1; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), terminator),
    )
    .unwrap()
}
fn projection(
    local: u32,
    fields: &[(SemanticProjectionKindV1, SemanticTypeIdV1)],
) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        fields
            .iter()
            .map(|&(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap())
            .collect(),
        fields.last().unwrap().1,
    )
    .unwrap()
}
fn declaration(
    id: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([id; 32]),
        SemanticLayoutIdentityV1::from_sha256([id; 32]),
        layout,
        shape,
    )
}
fn fields_type(id: u8, fields: Vec<SemanticTypeIdV1>) -> SemanticTypeDeclV1 {
    declaration(
        id,
        SemanticTypeLayoutV1::aggregate(
            Some(fields.len() as u64 * 8),
            8,
            SemanticAggregateLayoutV1::new(
                (0..fields.len()).map(|i| i as u64 * 8).collect(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}
fn reference_type(id: u8, pointee: SemanticTypeIdV1, size: u64) -> SemanticTypeDeclV1 {
    let kind = SemanticAbiPointeeKindV1::SharedReference { frozen: true };
    declaration(
        id,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(kind, size, 8).unwrap()),
            None,
        ),
    )
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let unit = declaration(
        1,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let scalar = declaration(
        2,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            bits: 64,
            signed: false,
        }),
    );
    let variants = (0..2)
        .map(|index| {
            let offsets = if index == 0 { vec![] } else { vec![8] };
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                if index == 0 { 8 } else { 24 },
                8,
                SemanticFieldsShapeV1::arbitrary(
                    offsets.clone(),
                    (0..offsets.len() as u32).collect(),
                )
                .unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let enumeration = declaration(
        7,
        SemanticTypeLayoutV1::enum_layout(
            24,
            8,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: U64,
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![WRAPPER]).unwrap()),
            ]
            .into_boxed_slice(),
        },
    );
    vec![
        unit,
        scalar,
        fields_type(3, vec![U64]),
        reference_type(4, STORAGE, 8),
        fields_type(5, vec![REF, U64]),
        reference_type(6, WRAPPER, 16),
        enumeration,
    ]
}

fn function() -> SemanticFunctionDeclV1 {
    let locals = [
        UNIT,
        STORAGE,
        REF,
        WRAPPER,
        ENUM,
        ENUM,
        U64,
        WRAPPER,
        WRAPPER_REF,
        U64,
        REF,
        U64,
    ]
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
            source(),
        )
    })
    .collect();
    let blocks = vec![
        block(
            0,
            vec![
                statement(
                    1,
                    STORAGE,
                    aggregate(SemanticAggregateKindV1::Aggregate, vec![constant(9)]),
                ),
                statement(
                    2,
                    REF,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(1, STORAGE),
                    },
                ),
                statement(
                    3,
                    WRAPPER,
                    aggregate(
                        SemanticAggregateKindV1::Aggregate,
                        vec![copy(2, REF), constant(5)],
                    ),
                ),
                statement(11, U64, SemanticRvalueKindV1::Use(constant(1))),
            ],
            switch(11, 2, 1),
        ),
        block(
            1,
            vec![statement(
                4,
                ENUM,
                aggregate(
                    SemanticAggregateKindV1::EnumVariant(1),
                    vec![SemanticOperandV1::Move(place(3, WRAPPER))],
                ),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            2,
            vec![statement(
                4,
                ENUM,
                aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![]),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            3,
            vec![
                statement(
                    5,
                    ENUM,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(4, ENUM))),
                ),
                statement(6, U64, SemanticRvalueKindV1::Discriminant(place(5, ENUM))),
            ],
            switch(6, 5, 4),
        ),
        block(
            4,
            vec![
                statement(
                    7,
                    WRAPPER,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(projection(
                        5,
                        &[
                            (SemanticProjectionKindV1::Downcast(1), ENUM),
                            (SemanticProjectionKindV1::Field(0), WRAPPER),
                        ],
                    ))),
                ),
                statement(
                    8,
                    WRAPPER_REF,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(7, WRAPPER),
                    },
                ),
                statement(
                    9,
                    U64,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projection(
                        8,
                        &[
                            (SemanticProjectionKindV1::Dereference, WRAPPER),
                            (SemanticProjectionKindV1::Field(1), U64),
                        ],
                    ))),
                ),
                statement(
                    10,
                    REF,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projection(
                        8,
                        &[
                            (SemanticProjectionKindV1::Dereference, WRAPPER),
                            (SemanticProjectionKindV1::Field(0), REF),
                        ],
                    ))),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        ),
        block(5, vec![], SemanticTerminatorKindV1::Return),
    ];
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([81; 32]),
        SemanticLayoutIdentityV1::from_sha256([82; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([83; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([84; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([85; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([86; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([87; 32]),
        source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"bounded_wrapper_transport_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([88; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ))
}

pub(in super::super) fn build_owner() -> ProductionSemanticSsaOwnerV1 {
    build_owner_from_function(function())
}

pub(in super::super) fn build_owner_from_function(
    function: SemanticFunctionDeclV1,
) -> ProductionSemanticSsaOwnerV1 {
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([89; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        request,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn at(function: &SemanticFunctionDeclV1, block: usize, statement: usize) -> &SemanticAssignmentV1 {
    let SemanticStatementKindV1::Assign(value) =
        function.blocks()[block].statements()[statement].kind()
    else {
        panic!()
    };
    value
}

fn with_blocks(
    original: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone())
}

fn replacing_assignment(
    original: &SemanticFunctionDeclV1,
    block: usize,
    assignment: SemanticAssignmentV1,
) -> SemanticFunctionDeclV1 {
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[block].statements().to_vec();
    statements[0] = SemanticStatementV1::new(source(), SemanticStatementKindV1::Assign(assignment));
    blocks[block] = SemanticBasicBlockV1::new(
        blocks[block].identity(),
        blocks[block].source(),
        statements,
        blocks[block].terminator().clone(),
    )
    .unwrap();
    with_blocks(original, blocks)
}

fn rejects_retained_assignment(
    types: &[SemanticTypeDeclV1],
    original: &SemanticFunctionDeclV1,
    block: usize,
    value: SemanticAssignmentV1,
) {
    let changed = replacing_assignment(original, block, value);
    let conditions = Conditions::new(types, &changed).unwrap();
    let initial = joined(types, original, &Conditions::new(types, original).unwrap());
    assert!(
        assignment(
            types,
            &changed,
            &initial,
            at(&changed, block, 0),
            block,
            0,
            Some(&conditions)
        )
        .is_none()
    );
}

// The semantic owner above proves private initialization only. The following
// inert Global record exercises the consumer; it is not a new issuer factory.
pub(in super::super) fn initial() -> ProjectedCapabilityStateV1 {
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([88; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([90; 32]),
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([92; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([93; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([94; 32]),
    )
    .unwrap();
    HashMap::from([(
        2,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(
            ProjectedGlobalViewV1 {
                view: STORAGE,
                physical: STORAGE,
                element: U64,
                contract: SemanticCapabilityMemoryContractV1::global_read_only(),
                provenance,
                allocation: AllocationContractV1 {
                    allocation_origin: 11,
                    noalias_class: 12,
                    writable: false,
                    singleton_object: false,
                },
                borrow: Some(SemanticBorrowKindV1::Shared),
            },
        )),
    )])
}

pub(in super::super) fn joined(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    conditions: &Conditions<'_>,
) -> ProjectedCapabilityStateV1 {
    let mut shared = initial();
    let wrapper =
        global_handle_transport_v1::assignment(types, function, &shared, at(function, 0, 2))
            .unwrap();
    shared.insert(3, wrapper);
    let mut some = shared.clone();
    some.insert(
        4,
        assignment(
            types,
            function,
            &shared,
            at(function, 1, 0),
            1,
            0,
            Some(conditions),
        )
        .unwrap(),
    );
    consume_capability_rvalue_operands_v1(
        types,
        function,
        at(function, 1, 0).value().kind(),
        &mut some,
    );
    let mut none = shared.clone();
    none.insert(
        4,
        assignment(
            types,
            function,
            &shared,
            at(function, 2, 0),
            2,
            0,
            Some(conditions),
        )
        .unwrap(),
    );
    merge_capability_states_v1(&mut some, &none).unwrap();
    let copied = assignment(
        types,
        function,
        &some,
        at(function, 3, 0),
        3,
        0,
        Some(conditions),
    )
    .unwrap();
    consume_capability_rvalue_operands_v1(
        types,
        function,
        at(function, 3, 0).value().kind(),
        &mut some,
    );
    some.insert(5, copied);
    some
}

#[test]
fn wrapper_enum_owner_proves_initialized_scalar_sibling_after_variant_join() {
    let owner = build_owner();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &owner,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    assert!(reads.contains(view.body(), &view.body().blocks()[4].statements()[2]));
}

#[test]
fn wrapper_enum_conditional_meet_requires_exact_payload_edge() {
    let types = types();
    let function = function();
    let conditions = Conditions::new(&types, &function).unwrap();
    let state = joined(&types, &function, &conditions);
    assert!(conditions.allows(&types, &function, SemanticLocalIdV1::from_index(5), 1, 4));
    assert!(
        assignment(
            &types,
            &function,
            &state,
            at(&function, 4, 0),
            4,
            0,
            Some(&conditions)
        )
        .is_some()
    );
    for block in [0, 1, 2, 3, 5] {
        assert!(!conditions.allows(
            &types,
            &function,
            SemanticLocalIdV1::from_index(5),
            1,
            block
        ));
        assert!(
            assignment(
                &types,
                &function,
                &state,
                at(&function, 4, 0),
                block,
                0,
                Some(&conditions)
            )
            .is_none()
        );
    }
    assert!(assignment(&types, &function, &state, at(&function, 4, 0), 4, 0, None).is_none());
    let foreign = function.clone();
    assert!(
        assignment(
            &types,
            &foreign,
            &state,
            at(&function, 4, 0),
            4,
            0,
            Some(&conditions)
        )
        .is_none()
    );
    let foreign_types = types.clone();
    assert!(
        assignment(
            &foreign_types,
            &function,
            &state,
            at(&function, 4, 0),
            4,
            0,
            Some(&conditions)
        )
        .is_none()
    );
}

#[test]
fn wrapper_enum_assignment_requires_exact_retained_statement() {
    let types = types();
    let function = function();
    let conditions = Conditions::new(&types, &function).unwrap();
    let state = joined(&types, &function, &conditions);
    for (block, state) in [(1, &state), (3, &state), (4, &state)] {
        let detached = at(&function, block, 0).clone();
        assert!(
            assignment(
                &types,
                &function,
                state,
                &detached,
                block,
                0,
                Some(&conditions)
            )
            .is_none()
        );
        assert!(
            assignment(
                &types,
                &function,
                state,
                at(&function, block, 0),
                block,
                usize::MAX,
                Some(&conditions)
            )
            .is_none()
        );
    }
    assert!(
        assignment(
            &types,
            &function,
            &state,
            at(&function, 4, 0),
            4,
            1,
            Some(&conditions)
        )
        .is_none()
    );
}

#[test]
fn wrapper_enum_retained_use_on_wrong_discriminant_edge_rejects() {
    let types = types();
    let original = function();
    let mut blocks = original.blocks().to_vec();
    blocks[3] = block(3, blocks[3].statements().to_vec(), switch(6, 4, 5));
    let changed = with_blocks(&original, blocks);
    let conditions = Conditions::new(&types, &changed).unwrap();
    let state = joined(&types, &changed, &conditions);
    assert!(!conditions.allows(&types, &changed, SemanticLocalIdV1::from_index(5), 1, 4));
    assert!(conditions.allows(&types, &changed, SemanticLocalIdV1::from_index(5), 1, 5));
    assert!(
        assignment(
            &types,
            &changed,
            &state,
            at(&changed, 4, 0),
            4,
            0,
            Some(&conditions)
        )
        .is_none()
    );
}

#[test]
fn wrapper_enum_and_initialized_scalar_copy_retain_exact_original_global() {
    let owner = build_owner();
    let types = owner.source_semantic().types();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let function = view.body();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &owner,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    let conditions = reads.conditions_for(types, function).unwrap();
    let mut state = joined(types, function, conditions);
    state.insert(
        7,
        assignment(
            types,
            function,
            &state,
            at(function, 4, 0),
            4,
            0,
            Some(conditions),
        )
        .unwrap(),
    );
    state.insert(
        8,
        global_handle_transport_v1::assignment(types, function, &state, at(function, 4, 1))
            .unwrap(),
    );
    let scalar = &function.blocks()[4].statements()[2];
    assert!(
        global_handle_transport_v1::preserves_initialized_scalar_copy(
            types,
            function,
            &state,
            scalar,
            Some(&reads)
        )
    );
    assert!(
        !global_handle_transport_v1::preserves_initialized_scalar_copy(
            types, function, &state, scalar, None
        )
    );
    assert_eq!(
        global_handle_transport_v1::assignment(types, function, &state, at(function, 4, 3)),
        Some(state[&2].clone())
    );
    state.remove(&7);
    assert!(
        !global_handle_transport_v1::preserves_initialized_scalar_copy(
            types,
            function,
            &state,
            scalar,
            Some(&reads)
        )
    );
    assert!(
        global_handle_transport_v1::assignment(types, function, &state, at(function, 4, 3))
            .is_none()
    );
}

#[test]
fn wrapper_enum_missing_unknown_and_conflicting_same_variant_cannot_join() {
    let types = types();
    let function = function();
    let conditions = Conditions::new(&types, &function).unwrap();
    let known = joined(&types, &function, &conditions);
    for missing in [false, true] {
        let mut other = known.clone();
        if missing {
            other.remove(&5);
        } else {
            other.insert(5, ProjectedCapabilityValueV1::Invalid);
        }
        for reversed in [false, true] {
            let (mut current, incoming) = if reversed {
                (other.clone(), &known)
            } else {
                (known.clone(), &other)
            };
            merge_capability_states_v1(&mut current, incoming).unwrap();
            assert!(!current.contains_key(&5));
        }
    }
    let ProjectedCapabilityValueV1::GlobalEnum(capture) = known[&5] else {
        panic!()
    };
    let unknown_same = Capture {
        ty: capture.ty,
        possible: 2,
        payload: None,
    };
    assert!(capture.merge(unknown_same).is_none());
    assert!(unknown_same.merge(capture).is_none());
    let foreign = Capture {
        ty: WRAPPER,
        ..capture
    };
    assert!(capture.merge(foreign).is_none());
    let mut other_initial = initial();
    let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(mut view)) =
        other_initial[&2]
    else {
        panic!()
    };
    view.allocation.allocation_origin += 1;
    other_initial.insert(
        2,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view)),
    );
    let other_wrapper = global_handle_transport_v1::assignment(
        &types,
        &function,
        &other_initial,
        at(&function, 0, 2),
    )
    .unwrap();
    other_initial.insert(3, other_wrapper);
    let ProjectedCapabilityValueV1::GlobalEnum(other) = assignment(
        &types,
        &function,
        &other_initial,
        at(&function, 1, 0),
        1,
        0,
        Some(&conditions),
    )
    .unwrap() else {
        panic!()
    };
    assert!(capture.merge(other).is_none());
    assert!(other.merge(capture).is_none());
}

#[test]
fn wrapper_enum_unknown_variant_wrong_field_cast_and_dereference_reject() {
    let types = types();
    let function = function();
    let conditions = Conditions::new(&types, &function).unwrap();
    let state = joined(&types, &function, &conditions);
    for path in [
        vec![
            (SemanticProjectionKindV1::Downcast(0), ENUM),
            (SemanticProjectionKindV1::Field(0), WRAPPER),
        ],
        vec![
            (SemanticProjectionKindV1::Downcast(2), ENUM),
            (SemanticProjectionKindV1::Field(0), WRAPPER),
        ],
        vec![
            (SemanticProjectionKindV1::Downcast(1), ENUM),
            (SemanticProjectionKindV1::Field(1), WRAPPER),
        ],
        vec![
            (SemanticProjectionKindV1::Downcast(1), WRAPPER),
            (SemanticProjectionKindV1::Field(0), WRAPPER),
        ],
        vec![
            (SemanticProjectionKindV1::Dereference, ENUM),
            (SemanticProjectionKindV1::Field(0), WRAPPER),
        ],
        vec![(SemanticProjectionKindV1::Field(0), WRAPPER)],
    ] {
        let bad = SemanticAssignmentV1::new(
            place(7, WRAPPER),
            SemanticRvalueV1::new(
                WRAPPER,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(projection(5, &path))),
            ),
        );
        assert!(assignment(&types, &function, &state, &bad, 4, 0, Some(&conditions)).is_none());
        assert!(
            global_handle_transport_v1::read_conditional_payload(
                &types,
                &function,
                &state,
                &projection(5, &path),
                &conditions,
                4
            )
            .is_none()
        );
        rejects_retained_assignment(&types, &function, 4, bad);
    }
    let bad_constructor = SemanticAssignmentV1::new(
        place(4, ENUM),
        SemanticRvalueV1::new(
            ENUM,
            aggregate(SemanticAggregateKindV1::EnumVariant(2), vec![]),
        ),
    );
    assert!(
        assignment(
            &types,
            &function,
            &state,
            &bad_constructor,
            1,
            0,
            Some(&conditions)
        )
        .is_none()
    );
    rejects_retained_assignment(&types, &function, 1, bad_constructor);
    for kind in [
        SemanticBorrowKindV1::Shared,
        SemanticBorrowKindV1::Mutable,
        SemanticBorrowKindV1::Fake,
    ] {
        let borrowed = SemanticAssignmentV1::new(
            place(8, WRAPPER_REF),
            SemanticRvalueV1::new(
                WRAPPER_REF,
                SemanticRvalueKindV1::Borrow {
                    kind,
                    place: place(5, ENUM),
                },
            ),
        );
        assert!(
            assignment(
                &types,
                &function,
                &state,
                &borrowed,
                4,
                0,
                Some(&conditions)
            )
            .is_none()
        );
        rejects_retained_assignment(&types, &function, 4, borrowed);
    }
    let cast = SemanticAssignmentV1::new(
        place(7, WRAPPER),
        SemanticRvalueV1::new(
            WRAPPER,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer,
                operand: copy(5, ENUM),
            },
        ),
    );
    assert!(assignment(&types, &function, &state, &cast, 4, 0, Some(&conditions)).is_none());
    rejects_retained_assignment(&types, &function, 4, cast);
}

#[test]
fn wrapper_enum_move_and_projected_write_still_invalidate() {
    let types = types();
    let function = function();
    let conditions = Conditions::new(&types, &function).unwrap();
    let initial = joined(&types, &function, &conditions);
    let mut copied = initial.clone();
    consume_capability_operand_v1(&types, &function, &mut copied, &copy(5, ENUM));
    assert_eq!(copied, initial);
    for operand in [
        SemanticOperandV1::Move(place(5, ENUM)),
        SemanticOperandV1::Move(projection(
            5,
            &[
                (SemanticProjectionKindV1::Downcast(1), ENUM),
                (SemanticProjectionKindV1::Field(0), WRAPPER),
            ],
        )),
    ] {
        let mut state = initial.clone();
        consume_capability_operand_v1(&types, &function, &mut state, &operand);
        assert_eq!(state[&5], ProjectedCapabilityValueV1::Invalid);
        assert!(
            assignment(
                &types,
                &function,
                &state,
                at(&function, 4, 0),
                4,
                0,
                Some(&conditions)
            )
            .is_none()
        );
    }
    let mut written = initial.clone();
    invalidate_capability_place_v1(
        &mut written,
        &projection(
            5,
            &[
                (SemanticProjectionKindV1::Downcast(1), ENUM),
                (SemanticProjectionKindV1::Field(0), WRAPPER),
            ],
        ),
    );
    assert!(
        assignment(
            &types,
            &function,
            &written,
            at(&function, 4, 0),
            4,
            0,
            Some(&conditions)
        )
        .is_none()
    );
    assert!(global_handle_transport_v1::invalidates_reference_storage(
        &initial,
        &place(5, ENUM)
    ));
}

#[test]
fn wrapper_scalar_copy_certificate_rejects_cloned_statement_move_path_and_alias() {
    let owner = build_owner();
    let types = owner.source_semantic().types();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let function = view.body();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &owner,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    let conditions = reads.conditions_for(types, function).unwrap();
    let mut state = joined(types, function, conditions);
    state.insert(
        7,
        assignment(
            types,
            function,
            &state,
            at(function, 4, 0),
            4,
            0,
            Some(conditions),
        )
        .unwrap(),
    );
    state.insert(
        8,
        global_handle_transport_v1::assignment(types, function, &state, at(function, 4, 1))
            .unwrap(),
    );
    let scalar = &function.blocks()[4].statements()[2];
    let cloned = scalar.clone();
    assert!(
        !global_handle_transport_v1::preserves_initialized_scalar_copy(
            types,
            function,
            &state,
            &cloned,
            Some(&reads)
        )
    );
    let moved = statement(
        9,
        U64,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(projection(
            8,
            &[
                (SemanticProjectionKindV1::Dereference, WRAPPER),
                (SemanticProjectionKindV1::Field(1), U64),
            ],
        ))),
    );
    assert!(
        !global_handle_transport_v1::preserves_initialized_scalar_copy(
            types,
            function,
            &state,
            &moved,
            Some(&reads)
        )
    );
    let changed_path = statement(
        9,
        U64,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projection(
            8,
            &[
                (SemanticProjectionKindV1::Dereference, WRAPPER),
                (SemanticProjectionKindV1::Field(0), U64),
            ],
        ))),
    );
    assert!(
        !global_handle_transport_v1::preserves_initialized_scalar_copy(
            types,
            function,
            &state,
            &changed_path,
            Some(&reads)
        )
    );
    for invalid in [false, true] {
        let mut alias = state.clone();
        if invalid {
            alias.insert(7, ProjectedCapabilityValueV1::Invalid);
        } else {
            alias.remove(&7);
        }
        assert!(
            !global_handle_transport_v1::preserves_initialized_scalar_copy(
                types,
                function,
                &alias,
                scalar,
                Some(&reads)
            )
        );
    }
    let foreign_owner = build_owner();
    let foreign_reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &foreign_owner,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    assert!(
        !global_handle_transport_v1::preserves_initialized_scalar_copy(
            types,
            function,
            &state,
            scalar,
            Some(&foreign_reads)
        )
    );
}

#[test]
fn wrapper_enum_shape_and_record_cost_remain_bounded() {
    assert!(
        std::mem::size_of::<Capture>()
            <= std::mem::size_of::<global_handle_transport_v1::Capture>() + 32
    );
    let mut types = types();
    assert!(variants(&types, ENUM).is_some());
    let mut remaining = 0;
    assert!(shared_copy_shape(&types, ENUM, 0, &mut remaining).is_none());
    let mut remaining = MAX_TYPE_NODES;
    assert!(shared_copy_shape(&types, ENUM, MAX_DEPTH, &mut remaining).is_none());
    let original = types[WRAPPER.index() as usize].clone();
    types[WRAPPER.index() as usize] = fields_type(5, vec![REF; MAX_FIELDS + 1]);
    assert!(variants(&types, ENUM).is_none());
    types[WRAPPER.index() as usize] = original;
    let original = types[REF.index() as usize].clone();
    types[REF.index() as usize] = declaration(
        4,
        original.layout().clone(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                STORAGE,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    assert!(variants(&types, ENUM).is_none());
}

#[test]
fn initialized_scalar_read_is_revoked_when_same_handle_wrapper_storage_is_rebound() {
    let original = function();
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[4].statements().to_vec();
    statements.insert(
        2,
        statement(
            7,
            WRAPPER,
            aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![copy(2, REF), constant(6)],
            ),
        ),
    );
    blocks[4] = SemanticBasicBlockV1::new(
        blocks[4].identity(),
        blocks[4].source(),
        statements,
        blocks[4].terminator().clone(),
    )
    .unwrap();
    let changed = with_blocks(&original, blocks);
    let owner = build_owner_from_function(changed);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &owner,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    assert!(!reads.contains(view.body(), &view.body().blocks()[4].statements()[3]));
}
