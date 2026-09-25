use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "partial_move_discriminant_resources_v1_tests.rs"]
mod resources;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn id(index: usize) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(u32::try_from(index).unwrap())
}

fn declaration(
    index: usize,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    let mut bytes = [0; 32];
    bytes[24..].copy_from_slice(&(index as u64 + 1).to_be_bytes());
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes),
        SemanticLayoutIdentityV1::from_sha256(bytes),
        layout,
        shape,
    )
}

fn scalar(
    primitive: SemanticBackendPrimitiveV1,
    start: u128,
    end: u128,
) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(start, end))
}

fn base_types() -> Vec<SemanticTypeDeclV1> {
    vec![
        declaration(
            0,
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
        ),
        declaration(
            1,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(scalar(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    0,
                    u128::from(u64::MAX),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
    ]
}

fn variant(
    index: u32,
    size: u64,
    offsets: Vec<u64>,
    padding: Vec<SemanticPaddingV1>,
) -> SemanticEnumVariantLayoutV1 {
    let mut order: Vec<u32> = (0..u32::try_from(offsets.len()).unwrap()).collect();
    order.sort_by_key(|&index| (offsets[index as usize], index));
    SemanticEnumVariantLayoutV1::from_rustc(
        index,
        size,
        8,
        SemanticFieldsShapeV1::arbitrary(offsets.clone(), order).unwrap(),
        SemanticBackendReprV1::memory(true),
        None,
        false,
        None,
        8,
        0,
        SemanticAggregateLayoutV1::new(offsets, padding).unwrap(),
    )
    .unwrap()
}

fn add_direct(
    types: &mut Vec<SemanticTypeDeclV1>,
    fields: Vec<SemanticTypeIdV1>,
    offsets: Vec<u64>,
    size: u64,
) -> SemanticTypeIdV1 {
    let ty = id(types.len());
    let tag = scalar(SemanticBackendPrimitiveV1::integer(false, 8, 1), 0, 255);
    types.push(declaration(
        types.len(),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            size,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                (0..2)
                    .map(|index| {
                        variant(
                            index,
                            size,
                            offsets.clone(),
                            vec![SemanticPaddingV1::new(1, 7).unwrap()],
                        )
                    })
                    .collect(),
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            WORD,
            (0..2)
                .map(|index| {
                    SemanticEnumVariantV1::new(
                        index,
                        SemanticAggregateTypeV1::new(fields.clone()).unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    ));
    ty
}

fn add_niche(types: &mut Vec<SemanticTypeDeclV1>, pointer: bool) -> SemanticTypeIdV1 {
    let primitive = if pointer {
        SemanticBackendPrimitiveV1::pointer(0, 8, 8)
    } else {
        SemanticBackendPrimitiveV1::integer(false, 64, 8)
    };
    let valid = SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX));
    let payload = id(types.len());
    let shape = if pointer {
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                WORD,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        )
    } else {
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
                vec![valid],
            )
            .unwrap(),
        )
    };
    let payload_decl = declaration(
        types.len(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(primitive, valid)),
            false,
        )
        .unwrap(),
        shape,
    );
    types.push(if pointer {
        payload_decl.with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        8,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        )
    } else {
        payload_decl
    });
    let niche = SemanticLayoutNicheV1::new(0, primitive, valid).unwrap();
    let payload_layout = SemanticEnumVariantLayoutV1::from_rustc(
        1,
        16,
        8,
        SemanticFieldsShapeV1::arbitrary(vec![0, 8], vec![0, 1]).unwrap(),
        SemanticBackendReprV1::memory(true),
        Some(niche),
        false,
        None,
        8,
        0,
        SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
    )
    .unwrap();
    let ty = id(types.len());
    types.push(declaration(
        types.len(),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            16,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                vec![variant(0, 16, vec![], vec![]), payload_layout],
                SemanticEnumEncodingV1::Niche(
                    SemanticNicheEnumEncodingV1::new(
                        0,
                        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0)
                            .unwrap(),
                        niche,
                        scalar(primitive, 1, 0),
                        1,
                        0,
                        0,
                        0,
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            WORD,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![payload, WORD]).unwrap(),
                ),
            ],
        )
        .unwrap(),
    ));
    ty
}

fn add_record(
    types: &mut Vec<SemanticTypeDeclV1>,
    fields: Vec<SemanticTypeIdV1>,
    offsets: Vec<u64>,
    size: u64,
) -> SemanticTypeIdV1 {
    let ty = id(types.len());
    types.push(declaration(
        types.len(),
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            8,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    ));
    ty
}

fn add_array(
    types: &mut Vec<SemanticTypeDeclV1>,
    element: SemanticTypeIdV1,
    length: u64,
) -> SemanticTypeIdV1 {
    let stride = types[element.index() as usize]
        .layout()
        .size_bytes()
        .unwrap();
    let ty = id(types.len());
    types.push(declaration(
        types.len(),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            stride.checked_mul(length).unwrap(),
            8,
            SemanticFieldsShapeV1::Array {
                stride_bytes: stride,
                count: length,
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array { element, length },
    ));
    ty
}

fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), terminator),
    )
    .unwrap()
}

fn function(root: SemanticTypeIdV1, blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    function_with_scratch(root, WORD, blocks)
}

fn function_with_scratch(
    root: SemanticTypeIdV1,
    scratch: SemanticTypeIdV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
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
        [UNIT, root, scratch, WORD, root]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([90 + index as u8; 32]),
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"partial_move_discriminant".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([88; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ))
}

fn admit(
    types: Vec<SemanticTypeDeclV1>,
    function: SemanticFunctionDeclV1,
) -> AdmittedInertSemanticMirV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([89; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    function: SemanticFunctionDeclV1,
    root: SemanticTypeIdV1,
}

impl Fixture {
    fn new(types: Vec<SemanticTypeDeclV1>, root: SemanticTypeIdV1) -> Self {
        let function = function(
            root,
            vec![block(100, vec![], SemanticTerminatorKindV1::Return)],
        );
        let admitted = admit(types, function);
        Self {
            types: admitted.types().to_vec(),
            function: admitted.functions()[0].clone(),
            root,
        }
    }
    fn direct() -> Self {
        let mut types = base_types();
        let root = add_direct(&mut types, vec![WORD, WORD], vec![8, 16], 24);
        Self::new(types, root)
    }
    fn niche(pointer: bool) -> Self {
        let mut types = base_types();
        let root = add_niche(&mut types, pointer);
        Self::new(types, root)
    }
    fn query(&self) -> SemanticPlaceV1 {
        place(1, self.root, vec![])
    }
    fn check(
        &self,
        query: &SemanticPlaceV1,
        paths: &[Vec<SemanticMovePathElementV1>],
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        validate_partial_move_discriminant_read_v1(
            &self.function,
            Some(&self.types),
            query,
            location(),
            &state(paths),
            &mut budget(100_000),
        )
    }
}

fn location() -> SemanticPartialMoveLocationV1 {
    SemanticPartialMoveLocationV1 {
        function: SemanticFunctionIdV1::from_index(0),
        block: 0,
        statement: Some(3),
    }
}
fn budget(remaining: usize) -> SemanticPartialMoveBudgetV1 {
    let limits = SsaPlannerLimitsV1::default();
    SemanticPartialMoveBudgetV1 {
        function: SemanticFunctionIdV1::from_index(0),
        base_storage_words: 19,
        base_work_units: limits.max_work_units() - remaining,
        state_entries: 7,
        work_units: 0,
        limits,
    }
}
fn state(paths: &[Vec<SemanticMovePathElementV1>]) -> SemanticPartialMoveStateV1 {
    BTreeMap::from([(1, paths.iter().cloned().collect())])
}
fn payload(variant: u32, field: u32) -> Vec<SemanticMovePathElementV1> {
    vec![
        SemanticMovePathElementV1::Downcast(variant),
        SemanticMovePathElementV1::Field(field),
    ]
}
fn projection(kind: SemanticProjectionKindV1, ty: SemanticTypeIdV1) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(kind, ty).unwrap()
}
fn place(
    local: u32,
    ty: SemanticTypeIdV1,
    projections: Vec<SemanticProjectionV1>,
) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), projections, ty).unwrap()
}
fn field_place(
    fixture: &Fixture,
    variant: u32,
    field: u32,
    ty: SemanticTypeIdV1,
) -> SemanticPlaceV1 {
    place(
        1,
        ty,
        vec![
            projection(SemanticProjectionKindV1::Downcast(variant), fixture.root),
            projection(SemanticProjectionKindV1::Field(field), ty),
        ],
    )
}
fn moved(result: Result<(), ProductionSemanticSsaErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn exact_direct_tag_read_preserves_siblings_but_not_payload_or_whole_availability() {
    let fixture = Fixture::direct();
    let paths = [payload(0, 0)];
    fixture.check(&fixture.query(), &paths).unwrap();
    let mut state = state(&paths);
    let original = state.clone();
    let mut budget = budget(1000);
    let sibling = field_place(&fixture, 0, 1, WORD);
    validate_partial_move_place_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &sibling,
        location(),
        &state,
        &mut budget,
    )
    .unwrap();
    for operand in [
        SemanticOperandV1::Copy(fixture.query()),
        SemanticOperandV1::Move(fixture.query()),
    ] {
        moved(validate_partial_move_operand_v1(
            &fixture.function,
            Some(&fixture.types),
            &operand,
            location(),
            &mut state,
            &mut budget,
        ));
    }
    moved(validate_partial_move_place_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &field_place(&fixture, 0, 0, WORD),
        location(),
        &state,
        &mut budget,
    ));
    for value in [
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: fixture.query(),
        },
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable,
            place: fixture.query(),
        },
        SemanticRvalueKindV1::Length(fixture.query()),
    ] {
        moved(validate_partial_move_rvalue_v1(
            &fixture.function,
            Some(&fixture.types),
            &value,
            location(),
            &mut state,
            &mut budget,
        ));
    }
    moved(validate_partial_move_statement_v1(
        &fixture.function,
        Some(&fixture.types),
        &SemanticStatementKindV1::SetDiscriminant {
            place: fixture.query(),
            variant_index: 1,
        },
        location(),
        &mut state,
        &mut budget,
    ));
    moved(validate_partial_move_terminator_v1(
        &fixture.function,
        Some(&fixture.types),
        &SemanticTerminatorKindV1::Drop {
            place: fixture.query(),
            drop_glue: SemanticFunctionIdV1::from_index(0),
            target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::DropReturn,
                SemanticBlockIdV1::from_index(0),
            ),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        Some(0),
        location(),
        &mut state,
        &mut budget,
    ));
    assert_eq!(state, original);
    for _ in 0..2 {
        validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &state,
            &mut budget,
        )
        .unwrap();
    }
    assert_eq!(state, original);
    apply_partial_move_destination_write_v1(
        &fixture.function,
        Some(&fixture.types),
        &field_place(&fixture, 0, 0, WORD),
        location(),
        &mut state,
        &mut budget,
    )
    .unwrap();
    assert!(state.is_empty());
}

#[test]
fn original_integer_and_pointer_niche_bytes_distinguish_payload_moves() {
    for pointer in [false, true] {
        let fixture = Fixture::niche(pointer);
        fixture.check(&fixture.query(), &[payload(1, 1)]).unwrap();
        moved(fixture.check(&fixture.query(), &[payload(1, 0)]));
        moved(fixture.check(&fixture.query(), &[vec![]]));
        moved(fixture.check(&fixture.query(), &[payload(1, 1), payload(1, 0)]));
        let geometry = PartialMoveGeometryV1::root(&fixture.function, &fixture.types, 1).unwrap();
        assert_eq!(geometry.tag(&fixture.types), Some((0, 8)));
    }
}

#[test]
fn nested_reordered_fields_and_array_aliases_use_absolute_original_byte_ranges() {
    let mut types = base_types();
    let enumeration = add_direct(&mut types, vec![WORD, WORD], vec![16, 8], 24);
    let record = add_record(&mut types, vec![enumeration, WORD], vec![8, 0], 32);
    let root = add_array(&mut types, record, 3);
    let fixture = Fixture::new(types, root);
    let query = place(
        1,
        enumeration,
        vec![
            projection(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 1,
                    minimum_length: 3,
                    from_end: false,
                },
                record,
            ),
            projection(SemanticProjectionKindV1::Field(0), enumeration),
        ],
    );
    let prefix = |offset, from_end| {
        vec![
            SemanticMovePathElementV1::ConstantIndex { offset, from_end },
            SemanticMovePathElementV1::Field(0),
        ]
    };
    let mut disjoint = prefix(2, true);
    disjoint.extend(payload(0, 0));
    fixture.check(&query, &[disjoint]).unwrap();
    moved(fixture.check(&query, &[prefix(2, true)]));
    fixture.check(&query, &[prefix(0, false)]).unwrap();
    let mut budget = budget(100);
    assert_eq!(
        partial_move_query_geometry_v1(&fixture.function, &fixture.types, &query, &mut budget)
            .unwrap()
            .unwrap()
            .tag(&fixture.types),
        Some((40, 41))
    );
}

#[test]
fn physically_overlapping_variants_cannot_discharge_each_others_tag_reads() {
    let mut types = base_types();
    let inner = add_niche(&mut types, false);
    let outer = add_direct(&mut types, vec![inner], vec![8], 24);
    let fixture = Fixture::new(types, outer);
    let query = place(
        1,
        inner,
        vec![
            projection(SemanticProjectionKindV1::Downcast(0), outer),
            projection(SemanticProjectionKindV1::Field(0), inner),
        ],
    );
    let mut disjoint = payload(1, 0);
    disjoint.extend(payload(1, 1));
    fixture.check(&query, &[disjoint]).unwrap();
    let mut overlap = payload(1, 0);
    overlap.extend(payload(1, 0));
    moved(fixture.check(&query, &[overlap]));
    moved(fixture.check(&query, &[payload(1, 0)]));
}

#[test]
fn zero_sized_payload_moves_do_not_restore_the_logical_value() {
    let mut types = base_types();
    let root = add_direct(&mut types, vec![UNIT, WORD], vec![8, 8], 16);
    let fixture = Fixture::new(types, root);
    fixture.check(&fixture.query(), &[payload(0, 0)]).unwrap();
    let state = state(&[payload(0, 0)]);
    moved(validate_partial_move_place_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &fixture.query(),
        location(),
        &state,
        &mut budget(100),
    ));
    moved(validate_partial_move_place_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &field_place(&fixture, 0, 0, UNIT),
        location(),
        &state,
        &mut budget(100),
    ));
}

fn add_constant_enum(
    types: &mut Vec<SemanticTypeDeclV1>,
    empty: bool,
    zst: bool,
) -> SemanticTypeIdV1 {
    let ty = id(types.len());
    let fields = if zst { vec![UNIT] } else { vec![WORD] };
    let offsets = if empty { vec![] } else { vec![0] };
    let layout = SemanticTypeLayoutV1::with_exact_rustc_layout(
        if zst || empty { 0 } else { 8 },
        8,
        SemanticFieldsShapeV1::arbitrary(offsets.clone(), (0..offsets.len() as u32).collect())
            .unwrap(),
        if empty {
            SemanticRustcVariantsV1::Empty
        } else {
            SemanticRustcVariantsV1::Single { index: 0 }
        },
        SemanticBackendReprV1::memory(true),
        None,
        empty,
        None,
        8,
        0,
        if empty {
            SemanticTypeLayoutDetailsV1::None
        } else {
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
        },
    )
    .unwrap();
    types.push(declaration(
        types.len(),
        layout,
        SemanticTypeShapeV1::enum_type(
            WORD,
            if empty {
                vec![]
            } else {
                vec![SemanticEnumVariantV1::new(
                    0,
                    SemanticAggregateTypeV1::new(fields).unwrap(),
                )]
            },
        )
        .unwrap(),
    ));
    ty
}

#[test]
fn single_empty_and_uninhabited_enums_keep_conservative_availability() {
    for (empty, zst) in [(false, false), (false, true), (true, true)] {
        let mut types = base_types();
        let root = add_constant_enum(&mut types, empty, zst);
        let fixture = Fixture::new(types, root);
        validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &BTreeMap::new(),
            &mut budget(100),
        )
        .unwrap();
        moved(fixture.check(&fixture.query(), &[vec![]]));
        if !empty {
            moved(fixture.check(&fixture.query(), &[payload(0, 0)]));
        }
        let cursor = PartialMoveGeometryV1::root(&fixture.function, &fixture.types, 1);
        assert!(
            cursor
                .and_then(|cursor| cursor.tag(&fixture.types))
                .is_none()
        );
    }
    let mut fixture = Fixture::direct();
    let original = fixture.types[fixture.root.index() as usize].clone();
    let layout = original.layout();
    fixture.types[fixture.root.index() as usize] = declaration(
        fixture.root.index() as usize,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            layout.rustc_size_bytes(),
            layout.alignment_bytes(),
            layout.fields().clone(),
            layout.variants().clone(),
            layout.backend_repr().clone(),
            layout.largest_niche(),
            true,
            layout.max_repr_alignment_bytes(),
            layout.unadjusted_abi_alignment_bytes(),
            layout.randomization_seed(),
            layout.details().clone(),
        )
        .unwrap(),
        original.shape().clone(),
    );
    moved(fixture.check(&fixture.query(), &[payload(0, 0)]));
}

fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination.clone(),
            SemanticRvalueV1::new(destination.ty(), value),
        )),
    )
}
fn constant(ty: SemanticTypeIdV1, value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
    ))
}
fn construct(fixture: &Fixture, variant: u32) -> SemanticStatementV1 {
    let SemanticTypeShapeV1::Enum { variants, .. } =
        fixture.types[fixture.root.index() as usize].shape()
    else {
        panic!();
    };
    assign(
        fixture.query(),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::EnumVariant(variant),
                variants[variant as usize]
                    .fields()
                    .fields()
                    .iter()
                    .map(|&ty| constant(ty, 9))
                    .collect(),
            )
            .unwrap(),
        ),
    )
}
fn tag_statement(fixture: &Fixture) -> SemanticStatementV1 {
    assign(
        place(3, WORD, vec![]),
        SemanticRvalueKindV1::Discriminant(fixture.query()),
    )
}
fn move_statement(fixture: &Fixture, variant: u32, field: u32) -> SemanticStatementV1 {
    assign(
        place(2, WORD, vec![]),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field_place(
            fixture, variant, field, WORD,
        ))),
    )
}
fn owner(
    fixture: &Fixture,
    blocks: Vec<SemanticBasicBlockV1>,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
    let semantic = admit(fixture.types.clone(), function(fixture.root, blocks));
    let source = ProductionSemanticMirOwnerV1::try_new(
        semantic,
        crate::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(source, limits)
}

#[test]
fn admitted_source_owner_moves_before_tag_and_preserves_sibling_and_reinitialization() {
    let fixture = Fixture::direct();
    let moved_field = field_place(&fixture, 0, 0, WORD);
    for reinitialize in [false, true] {
        let mut statements = vec![
            construct(&fixture, 0),
            move_statement(&fixture, 0, 0),
            tag_statement(&fixture),
            assign(
                place(2, WORD, vec![]),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field_place(
                    &fixture, 0, 1, WORD,
                ))),
            ),
        ];
        if reinitialize {
            statements.extend([
                assign(
                    moved_field.clone(),
                    SemanticRvalueKindV1::Use(constant(WORD, 11)),
                ),
                assign(
                    place(4, fixture.root, vec![]),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(fixture.query())),
                ),
            ]);
        }
        let owner = owner(
            &fixture,
            vec![block(100, statements, SemanticTerminatorKindV1::Return)],
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        owner.verify_replay().unwrap();
        assert_eq!(
            owner.source_semantic().functions()[0].blocks()[0].statements()[1].kind(),
            move_statement(&fixture, 0, 0).kind()
        );
        assert!(
            owner
                .plan_for_function(SemanticFunctionIdV1::from_index(0))
                .unwrap()
                .partial_move_certificate()
                .work_units()
                > 0
        );
    }
    for operand in [
        SemanticOperandV1::Copy(fixture.query()),
        SemanticOperandV1::Move(fixture.query()),
    ] {
        let result = owner(
            &fixture,
            vec![block(
                100,
                vec![
                    construct(&fixture, 0),
                    move_statement(&fixture, 0, 0),
                    tag_statement(&fixture),
                    assign(
                        place(4, fixture.root, vec![]),
                        SemanticRvalueKindV1::Use(operand),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            ProductionSemanticSsaLimitsV1::default(),
        );
        assert!(matches!(
            result,
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn go(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}
fn switch(yes: u32, no: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(WORD, 0),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, yes),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, no),
        )
        .unwrap(),
    }
}

#[test]
fn admitted_joined_disjoint_moves_preserve_only_the_tag_not_the_whole_enum() {
    let fixture = Fixture::direct();
    for whole in [false, true] {
        let mut joined = vec![tag_statement(&fixture)];
        if whole {
            joined.push(assign(
                place(4, fixture.root, vec![]),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(fixture.query())),
            ));
        }
        let result = owner(
            &fixture,
            vec![
                block(100, vec![construct(&fixture, 0)], switch(1, 2)),
                block(101, vec![move_statement(&fixture, 0, 0)], go(3)),
                block(102, vec![move_statement(&fixture, 0, 1)], go(3)),
                block(103, joined, SemanticTerminatorKindV1::Return),
            ],
            ProductionSemanticSsaLimitsV1::default(),
        );
        if whole {
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove { .. })
            ));
        } else {
            result.unwrap().verify_replay().unwrap();
        }
    }
}

#[test]
fn joined_and_loop_carried_tag_overlap_or_storage_death_remain_unavailable() {
    let fixture = Fixture::niche(false);
    let mut joined = state(&[payload(1, 1)]);
    let mut budget = budget(1000);
    assert!(
        merge_partial_move_state_v1(&mut joined, &state(&[payload(1, 0)]), &mut budget).unwrap()
    );
    for _ in 0..2 {
        moved(validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &joined,
            &mut budget,
        ));
    }
    for statement in [
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        SemanticStatementKindV1::Deinitialize(fixture.query()),
    ] {
        let mut state = BTreeMap::new();
        validate_partial_move_statement_v1(
            &fixture.function,
            Some(&fixture.types),
            &statement,
            location(),
            &mut state,
            &mut budget,
        )
        .unwrap();
        moved(validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &state,
            &mut budget,
        ));
    }
    let fixture = Fixture::direct();
    for looped in [false, true] {
        let blocks = vec![
            block(
                100,
                vec![construct(&fixture, 0), move_statement(&fixture, 0, 0)],
                go(1),
            ),
            block(
                101,
                vec![SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
                )],
                if looped { switch(1, 2) } else { go(2) },
            ),
            block(
                102,
                vec![tag_statement(&fixture)],
                SemanticTerminatorKindV1::Return,
            ),
        ];
        assert!(owner(&fixture, blocks, ProductionSemanticSsaLimitsV1::default()).is_err());
    }
}

#[test]
fn admitted_niche_tag_reads_refuse_one_predecessor_or_loop_carried_overlap() {
    let fixture = Fixture::niche(false);
    owner(
        &fixture,
        vec![block(
            100,
            vec![
                construct(&fixture, 1),
                move_statement(&fixture, 1, 1),
                tag_statement(&fixture),
            ],
            SemanticTerminatorKindV1::Return,
        )],
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
    .verify_replay()
    .unwrap();
    let SemanticTypeShapeV1::Enum { variants, .. } =
        fixture.types[fixture.root.index() as usize].shape()
    else {
        panic!();
    };
    let niche_type = variants[1].fields().fields()[0];
    let overlap = || {
        assign(
            place(2, niche_type, vec![]),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field_place(
                &fixture, 1, 0, niche_type,
            ))),
        )
    };
    for looped in [false, true] {
        let blocks = if looped {
            vec![
                block(100, vec![construct(&fixture, 1)], go(1)),
                block(101, vec![tag_statement(&fixture)], switch(2, 3)),
                block(102, vec![overlap()], go(1)),
                block(103, vec![], SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![
                block(100, vec![construct(&fixture, 1)], switch(1, 2)),
                block(101, vec![overlap()], go(3)),
                block(102, vec![], go(3)),
                block(
                    103,
                    vec![tag_statement(&fixture)],
                    SemanticTerminatorKindV1::Return,
                ),
            ]
        };
        let semantic = admit(
            fixture.types.clone(),
            function_with_scratch(fixture.root, niche_type, blocks),
        );
        let source = ProductionSemanticMirOwnerV1::try_new(
            semantic,
            crate::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert!(matches!(
            ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default()),
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
}
