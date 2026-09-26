use super::super::{
    AccessMode, KernelBindingIdV1, RustcAbiClassV1, ScalarTypeV1, TypedArgumentListV1,
    TypedDescriptorArgumentV1,
};
use super::*;
use fe2o3_artifacts::{
    PointerWidth, RustLayoutEvidenceV1, RustPhysicalComponentKindV1, RustPhysicalComponentV1,
    RustPointerMutabilityV1, RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_lower_mir_kernel::{
    ProductionExecutionSourceInputV29, ProductionKernelByValueAbiV29,
    ProductionScopeCallableCandidateV29,
    ProductionSemanticKirLimitsV1, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

pub(super) const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
pub(super) const BYTE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
pub(super) const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
pub(super) const OBJECT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
pub(super) const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
pub(super) const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
pub(super) const USIZE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
pub(super) const FLOOR: usize = 83;
pub(super) const LIMIT: usize = 100_000_000;

#[derive(Clone, Copy)]
pub(super) enum Shape {
    Record,
    Array,
    Pair,
    Overaligned,
    Ignored,
    OversizedArray,
}

pub(super) struct Fixture {
    pub(super) owner: ProductionSemanticSsaOwnerV1,
    pub(super) roots: Vec<TypedDescriptorRootV1>,
}

fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn scalar(tag: u8, bits: u16) -> SemanticTypeDeclV1 {
    let bytes = u64::from(bits / 8);
    declaration(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }),
    )
}

fn scalar_attributes(extension: SemanticAbiExtensionV1) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        extension,
        0,
        None,
    )
    .unwrap()
}

fn scalar_layout() -> RustLayoutEvidenceV1 {
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(RustScalarElementTypeV1::U8)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        1,
        1,
        vec![
            RustPhysicalComponentV1::new(
                0,
                1,
                1,
                RustPhysicalComponentKindV1::Scalar {
                    scalar: RustScalarElementTypeV1::U8,
                },
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn slice_layout() -> RustLayoutEvidenceV1 {
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::shared_slice(
            RustScalarElementTypeV1::U64,
        )),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        16,
        8,
        vec![
            RustPhysicalComponentV1::new(
                0,
                8,
                8,
                RustPhysicalComponentKindV1::Pointer {
                    mutability: RustPointerMutabilityV1::Const,
                    pointee: RustScalarElementTypeV1::U64,
                },
            )
            .unwrap(),
            RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
        ],
    )
    .unwrap()
}

pub(super) fn fixture(shape: Shape, order: [usize; 4], roots: usize) -> Fixture {
    let target = SemanticLayoutIdentityV1::from_sha256([250; 32]);
    let source = SemanticSourceProvenanceV1::unavailable();
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
    let (layout, object_shape) = match shape {
        Shape::Record | Shape::Overaligned => {
            let size = if matches!(shape, Shape::Overaligned) {
                64
            } else {
                24
            };
            let align = if size == 64 { 64 } else { 8 };
            (
                SemanticTypeLayoutV1::aggregate(
                    Some(size),
                    align,
                    SemanticAggregateLayoutV1::new(
                        vec![0, 16],
                        vec![
                            SemanticPaddingV1::new(8, 8).unwrap(),
                            SemanticPaddingV1::new(17, size - 17).unwrap(),
                        ],
                    )
                    .unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![WORD, BYTE]).unwrap(),
                ),
            )
        }
        Shape::Array | Shape::OversizedArray => {
            let count = if matches!(shape, Shape::Array) {
                3
            } else {
                MAX_ABI_BYTES / 8 + 1
            };
            (
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    8 * count,
                    8,
                    SemanticFieldsShapeV1::array(8, count),
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
                SemanticTypeShapeV1::Array {
                    element: WORD,
                    length: count,
                },
            )
        }
        Shape::Pair => {
            let word = SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
            );
            (
                SemanticTypeLayoutV1::aggregate_with_backend_repr(
                    Some(16),
                    8,
                    SemanticBackendReprV1::scalar_pair(word, word),
                    false,
                    SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, WORD]).unwrap()),
            )
        }
        Shape::Ignored => (
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                64,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ),
    };
    let pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
    );
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
    );
    let types = vec![
        unit,
        scalar(2, 8),
        scalar(3, 64),
        declaration(4, layout, object_shape),
        declaration(
            5,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                8,
                SemanticFieldsShapeV1::array(8, 0),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(false),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Slice { element: WORD },
        ),
        declaration(
            6,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(pointer, length),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SLICE,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        0,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
        scalar(7, 64)
            .with_rust_type_kind(SemanticRustTypeKindV1::Usize)
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false)
                    .with_rustc_layout_is_noundef(true),
            ),
    ];
    let object_layout = types[OBJECT.index() as usize].layout();
    let object_mode = match shape {
        Shape::Ignored => SemanticAbiPassModeV1::Ignore,
        Shape::Pair => SemanticAbiPassModeV1::Pair {
            first: scalar_attributes(SemanticAbiExtensionV1::None),
            second: scalar_attributes(SemanticAbiExtensionV1::None),
        },
        _ => SemanticAbiPassModeV1::Indirect {
            attributes: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesNone),
                    true,
                    false,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                object_layout.size_bytes().unwrap(),
                Some(object_layout.alignment_bytes()),
            )
            .unwrap(),
            metadata_attributes: None,
            on_stack: false,
        },
    };
    let pointer_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(8),
    )
    .unwrap();
    let modes = [
        SemanticAbiPassModeV1::Direct(scalar_attributes(SemanticAbiExtensionV1::ZeroExtend)),
        object_mode,
        SemanticAbiPassModeV1::Pair {
            first: pointer_attributes,
            second: scalar_attributes(SemanticAbiExtensionV1::None),
        },
        SemanticAbiPassModeV1::Direct(scalar_attributes(SemanticAbiExtensionV1::None)),
    ];
    let source_types = [BYTE, OBJECT, REFERENCE, USIZE];
    let ownership = [
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ];
    let mut functions = Vec::new();
    let mut typed_roots = Vec::new();
    for root in 0..roots {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([30 + root as u8; 32]),
            target,
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            4,
            order
                .into_iter()
                .map(|index| {
                    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        source_types[index],
                        modes[index].clone(),
                    ))
                })
                .collect(),
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(order.into_iter().map(|index| ownership[index]).collect())
        .unwrap();
        let mut locals = vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([40; 32]),
            UNIT,
            SemanticLocalRoleV1::Return,
            source,
        )];
        for (ordinal, index) in order.into_iter().enumerate() {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([41 + ordinal as u8; 32]),
                source_types[index],
                SemanticLocalRoleV1::Argument(ordinal as u32),
                source,
            ));
        }
        let block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([50; 32]),
            source,
            vec![SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], UNIT).unwrap(),
                    SemanticRvalueV1::new(
                        UNIT,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(UNIT, SemanticConstantValueV1::ZeroSized),
                        )),
                    ),
                )),
            )],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap();
        let export = format!("prepared_inline_{root}");
        let binding = [110 + root as u8; 32];
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([60 + root as u8; 32]),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256([70 + root as u8; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([80 + root as u8; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([90 + root as u8; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([100 + root as u8; 32]),
                source,
                abi,
                locals,
                SemanticBlockIdV1::from_index(0),
                vec![block],
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(export.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(binding),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(
                            Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                            Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                            None,
                        )
                        .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
            )),
        );
        let arguments = order
            .into_iter()
            .enumerate()
            .map(|(ordinal, index)| {
                let (kind, access, class, layout) = match index {
                    0 => (
                        Kind::Scalar(ScalarTypeV1::U8),
                        AccessMode::ByValue,
                        RustcAbiClassV1::Scalar,
                        Some(scalar_layout()),
                    ),
                    1 => (
                        Kind::CompilerLaidOutByValue,
                        AccessMode::ByValue,
                        if matches!(shape, Shape::Pair) {
                            RustcAbiClassV1::ScalarPair
                        } else {
                            RustcAbiClassV1::Aggregate
                        },
                        None,
                    ),
                    2 => (
                        Kind::SharedSlice(ScalarTypeV1::U64),
                        AccessMode::ReadOnly,
                        RustcAbiClassV1::ScalarPair,
                        Some(slice_layout()),
                    ),
                    3 => (
                        Kind::CompilerLaidOutUsize,
                        AccessMode::ByValue,
                        RustcAbiClassV1::Scalar,
                        None,
                    ),
                    _ => unreachable!(),
                };
                let declaration = &types[source_types[index].index() as usize];
                TypedDescriptorArgumentV1 {
                    name: format!("argument{ordinal}"),
                    kind,
                    access,
                    offset: 0,
                    layout,
                    source_size: declaration.layout().size_bytes().unwrap(),
                    source_alignment: declaration.layout().alignment_bytes() as u32,
                    rustc_abi_class: class,
                    semantic_type_identity: declaration.identity(),
                }
            })
            .collect();
        typed_roots.push(TypedDescriptorRootV1 {
            logical_name: format!("inline{root}"),
            export_name: export,
            kernel_binding: KernelBindingIdV1::from_bytes(binding),
            arguments: TypedArgumentListV1::new(arguments).unwrap(),
            explicit_argument_bytes: 0,
            kernarg_alignment_bytes: 1,
            source_launch: None,
        });
    }
    let roots: Vec<_> = (0..roots)
        .map(|index| SemanticFunctionIdV1::from_index(index as u32))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(target),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        roots
            .iter()
            .copied()
            .map(SemanticCallableDeclV1::defined)
            .collect(),
        roots,
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    Fixture {
        owner,
        roots: typed_roots,
    }
}

pub(super) fn launch(fixture: &Fixture) -> ProductionSourceLaunchRosterV1 {
    let inputs: Vec<_> = fixture
        .roots
        .iter()
        .map(|root| {
            ProductionSourceLaunchRootInputV1::new(
                root.logical_name(),
                root.kernel_binding_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    ProductionSourceLaunchRosterV1::try_new(fixture.owner.source_semantic(), &inputs).unwrap()
}

pub(super) fn prepare(
    fixture: Fixture,
    budget: &mut Budget<'_>,
) -> Result<fe2o3_lower_mir_kernel::ProductionPreparedSourceV18, Error> {
    let launch = launch(&fixture);
    let source = *fixture.owner.source_semantic_sha256();
    let classes = vec![
        ProductionScopeCallableCandidateV29::Ordinary;
        fixture.owner.source_semantic().callables().len()
    ];
    super::super::prepare_source_with_kernel_arguments_v18(
        fixture.owner,
        launch,
        ProductionExecutionSourceInputV29 {
            semantic_sha256: &source,
            roots: &[],
            classes: &classes,
            events: &[],
        },
        &fixture.roots,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
}

#[test]
fn real_prepared_source_resolves_mixed_deferred_packing_without_mutating_registration() {
    let fixture = fixture(Shape::Record, [0, 1, 2, 3], 2);
    let original = fixture.roots.clone();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    for root in 0..2 {
        let mut cursor = Packing::new(
            &fixture.roots[root],
            fixture.owner.source_semantic(),
            &fixture.owner.source_semantic().functions()[root],
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            (0..4)
                .map(|index| cursor.offset(index).unwrap())
                .collect::<Vec<_>>(),
            [0, 8, 32, 48]
        );
        assert_eq!(
            cursor.finish().unwrap(),
            Extent {
                bytes: 56,
                alignment: 8
            }
        );
    }
    assert_eq!(fixture.roots, original);
    let prepared = prepare(fixture, &mut budget).unwrap();
    assert!(prepared.adopted_storage() > 0);
    prepared
        .with_checked_source_v18(&mut budget, |view, budget| {
            for root in 0..2 {
                let region = view
                    .kernel_argument_by_value_abi_v29(root, 1, budget)?
                    .unwrap();
                assert_eq!(
                    (
                        region.explicit_argument_bytes(),
                        region.kernarg_alignment_bytes()
                    ),
                    (56, 8)
                );
                assert_eq!(region.source_type(), OBJECT);
                let ProductionKernelByValueAbiV29::Inline(region) = region else {
                    panic!("nonzero inline region");
                };
                assert_eq!(
                    (
                        region.offset(),
                        region.size_bytes(),
                        region.alignment_bytes()
                    ),
                    (8, 24, 8)
                );
                assert_eq!(
                    view.kernel_argument_abi_kind(root, 2, budget)?,
                    Some(
                        fe2o3_kernel_descriptor::SourceTypeDescriptorV3::SharedSlice(
                            ScalarTypeV1::U64
                        )
                    )
                );
                assert_eq!(
                    view.kernel_argument_abi_kind(root, 3, budget)?,
                    Some(fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Usize)
                );
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn real_prepared_source_accepts_arrays_pairs_and_overaligned_original_regions() {
    for (shape, offset, size, align, extent) in [
        (Shape::Array, 8, 24, 8, 56),
        (Shape::Pair, 8, 16, 8, 48),
        (Shape::Overaligned, 64, 64, 64, 192),
    ] {
        let fixture = fixture(shape, [0, 1, 2, 3], 1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let prepared = prepare(fixture, &mut budget).unwrap();
        prepared
            .with_checked_source_v18(&mut budget, |view, budget| {
                let region = view
                    .kernel_argument_by_value_abi_v29(0, 1, budget)?
                    .unwrap();
                assert_eq!(
                    (
                        region.explicit_argument_bytes(),
                        region.kernarg_alignment_bytes()
                    ),
                    (extent, align)
                );
                let ProductionKernelByValueAbiV29::Inline(region) = region else {
                    panic!("inline object");
                };
                assert_eq!(
                    (
                        region.offset(),
                        region.size_bytes(),
                        region.alignment_bytes()
                    ),
                    (offset, size, u64::from(align))
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn original_ignored_arguments_before_between_and_after_values_add_no_physical_padding() {
    for (order, ordinal, expected_offsets) in [
        ([1, 0, 2, 3], 0, [0, 0, 8, 24]),
        ([0, 1, 2, 3], 1, [0, 1, 8, 24]),
        ([0, 2, 3, 1], 3, [0, 8, 24, 32]),
    ] {
        let fixture = fixture(Shape::Ignored, order, 1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut cursor = Packing::new(
            &fixture.roots[0],
            fixture.owner.source_semantic(),
            &fixture.owner.source_semantic().functions()[0],
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            (0..4)
                .map(|index| cursor.offset(index).unwrap())
                .collect::<Vec<_>>(),
            expected_offsets
        );
        assert_eq!(
            cursor.finish().unwrap(),
            Extent {
                bytes: 32,
                alignment: 8
            }
        );
        let prepared = prepare(fixture, &mut budget).unwrap();
        prepared
            .with_checked_source_v18(&mut budget, |view, budget| {
                let region = view
                    .kernel_argument_by_value_abi_v29(0, ordinal, budget)?
                    .unwrap();
                assert_eq!(region.source_declaration().layout().alignment_bytes(), 64);
                assert_eq!(
                    (
                        region.explicit_argument_bytes(),
                        region.kernarg_alignment_bytes()
                    ),
                    (32, 8)
                );
                assert!(matches!(region, ProductionKernelByValueAbiV29::Ignored(_)));
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn original_deferred_packing_refuses_forged_extent_offset_identity_and_abi_geometry() {
    for case in 0..6 {
        let mut fixture = fixture(Shape::Record, [0, 1, 2, 3], 1);
        let mut arguments = fixture.roots[0].arguments.as_slice().to_vec();
        match case {
            0 => fixture.roots[0].explicit_argument_bytes = 56,
            1 => fixture.roots[0].kernarg_alignment_bytes = 8,
            2 => arguments[1].offset = 8,
            3 => {
                arguments[1].semantic_type_identity = SemanticTypeIdentityV1::from_sha256([200; 32])
            }
            4 => arguments[1].source_size = u64::MAX,
            5 => arguments[1].rustc_abi_class = RustcAbiClassV1::Scalar,
            _ => unreachable!(),
        }
        fixture.roots[0].arguments = TypedArgumentListV1::new(arguments).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(matches!(
            prepare(fixture, &mut budget),
            Err(Error::DescriptorEvidence(_))
        ));
        assert_eq!(budget.storage(), FLOOR);
    }
}

pub(super) fn fixed_fixture() -> Fixture {
    let template = fixture(Shape::Record, [0, 1, 2, 3], 1);
    let source = template.owner.source_semantic();
    let original = &source.functions()[0];
    let ty = SemanticTypeIdV1::from_index(2);
    let abi = SemanticFunctionAbiV1::from_rustc(
        original.abi().identity(),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        [BYTE, ty]
            .into_iter()
            .map(|ty| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty,
                    SemanticAbiPassModeV1::Direct(scalar_attributes(if ty == BYTE {
                        SemanticAbiExtensionV1::ZeroExtend
                    } else {
                        SemanticAbiExtensionV1::None
                    })),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let locals = vec![
        original.locals()[0].clone(),
        original.locals()[1].clone(),
        SemanticLocalDeclV1::new(
            original.locals()[4].identity(),
            ty,
            SemanticLocalRoleV1::Argument(1),
            original.source(),
        ),
    ];
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi,
        locals,
        original.entry(),
        original.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        vec![
            source.types()[0].clone(),
            source.types()[1].clone(),
            source.types()[6].clone(),
        ],
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
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut root = template.roots[0].clone();
    let mut arguments = vec![
        root.arguments.as_slice()[0].clone(),
        root.arguments.as_slice()[3].clone(),
    ];
    let extent = super::super::laid_out_plan_v1::capture(&mut arguments)
        .unwrap()
        .unwrap();
    root.arguments = TypedArgumentListV1::new(arguments).unwrap();
    root.explicit_argument_bytes = extent.bytes;
    root.kernarg_alignment_bytes = extent.alignment;
    Fixture {
        owner,
        roots: vec![root],
    }
}

#[test]
fn real_prepared_source_keeps_fixed_scalar_nominal_offsets_and_source_kinds() {
    let fixture = fixed_fixture();
    assert_eq!(
        (
            fixture.roots[0].explicit_argument_bytes,
            fixture.roots[0].kernarg_alignment_bytes
        ),
        (16, 8)
    );
    assert_eq!(
        fixture.roots[0]
            .arguments
            .as_slice()
            .iter()
            .map(|row| row.offset)
            .collect::<Vec<_>>(),
        [0, 8]
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let prepared = prepare(fixture, &mut budget).unwrap();
    prepared
        .with_checked_source_v18(&mut budget, |view, budget| {
            assert_eq!(
                view.kernel_argument_abi_kind(0, 0, budget)?,
                Some(fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Scalar(
                    ScalarTypeV1::U8
                ))
            );
            assert_eq!(
                view.kernel_argument_abi_kind(0, 1, budget)?,
                Some(fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Usize)
            );
            assert!(
                view.kernel_argument_by_value_abi_v29(0, 0, budget)?
                    .is_none()
            );
            assert!(
                view.kernel_argument_by_value_abi_v29(0, 1, budget)?
                    .is_none()
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn original_gpu_kernel_cast_mode_is_rejected_before_any_region_capture() {
    let fixture = fixture(Shape::Record, [0, 1, 2, 3], 1);
    let source = fixture.owner.source_semantic();
    let original = &source.functions()[0];
    let mut arguments = original.abi().adjusted_arguments().to_vec();
    arguments[1] = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        OBJECT,
        SemanticAbiPassModeV1::cast(
            false,
            SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                    24,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        ),
    ));
    let abi = SemanticFunctionAbiV1::from_rustc(
        original.abi().identity(),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        4,
        arguments,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(original.abi().source_argument_ownership().to_vec())
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi,
        original.locals().to_vec(),
        original.entry(),
        original.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let request = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        request.admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn authenticated_large_array_is_refused_before_payload_expansion_or_source_adoption() {
    let fixture = fixture(Shape::OversizedArray, [0, 1, 2, 3], 1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let before = budget.work();
    let mut cursor = Packing::new(
        &fixture.roots[0],
        fixture.owner.source_semantic(),
        &fixture.owner.source_semantic().functions()[0],
        &mut budget,
    )
    .unwrap();
    assert_eq!(cursor.offset(0).unwrap(), 0);
    assert!(matches!(
        cursor.offset(1),
        Err(Error::DescriptorEvidence(
            CompilerDescriptorError::ProductionDescriptorMismatch("inline ABI explicit byte limit")
        ))
    ));
    assert_eq!(budget.work() - before, 12 + 12 * 4);
    assert_eq!(budget.storage(), FLOOR);
    assert!(matches!(
        prepare(fixture, &mut budget),
        Err(Error::DescriptorEvidence(
            CompilerDescriptorError::ProductionDescriptorMismatch("inline ABI explicit byte limit")
        ))
    ));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn original_ignore_requires_zero_size_and_zero_size_requires_original_ignore() {
    for zero in [false, true] {
        let fixture = fixture(
            if zero { Shape::Ignored } else { Shape::Record },
            [0, 1, 2, 3],
            1,
        );
        let source = fixture.owner.source_semantic();
        let original = &source.functions()[0];
        let mut arguments = original.abi().adjusted_arguments().to_vec();
        arguments[1] = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            OBJECT,
            if zero {
                SemanticAbiPassModeV1::Direct(scalar_attributes(SemanticAbiExtensionV1::None))
            } else {
                SemanticAbiPassModeV1::Ignore
            },
        ));
        let abi = SemanticFunctionAbiV1::from_rustc(
            original.abi().identity(),
            original.abi().layout_identity(),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            4,
            arguments,
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(original.abi().source_argument_ownership().to_vec())
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            abi,
            original.locals().to_vec(),
            original.entry(),
            original.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
        let request = InertSemanticMirRequestV1::new_with_callables(
            source.target(),
            source.types().to_vec(),
            vec![],
            vec![],
            vec![],
            vec![function],
            source.callables().to_vec(),
            source.roots().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            request.admit_current_production(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}
