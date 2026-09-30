use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

pub(super) const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
pub(super) const BYTE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
pub(super) const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

#[derive(Clone, Copy, Debug)]
pub(super) enum Shape {
    Record,
    Array,
    Pair,
    Overaligned,
    Ignored,
    DirectEnum,
    NicheEnum,
}

fn scalar(tag: u8, bits: u16) -> SemanticTypeDeclV1 {
    let size = u64::from(bits / 8);
    declaration(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            size,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, bits, size),
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

pub(super) fn attributes(extension: SemanticAbiExtensionV1) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        extension,
        0,
        None,
    )
    .unwrap()
}

pub(super) fn fixture_owner(shape: Shape, roots: usize) -> ProductionSemanticSsaOwnerV1 {
    assert!((1..=2).contains(&roots));
    let source = SemanticSourceProvenanceV1::unavailable();
    let target = SemanticLayoutIdentityV1::from_sha256([250; 32]);
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
    let mut types = vec![unit, scalar(2, 8), scalar(3, 64)];
    let (layout, shape_decl) = match shape {
        Shape::Record | Shape::Overaligned => {
            let align = if matches!(shape, Shape::Overaligned) {
                64
            } else {
                8
            };
            let size = if align == 64 { 64 } else { 24 };
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
        Shape::Array => (
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                24,
                8,
                SemanticFieldsShapeV1::array(8, 3),
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
                length: 3,
            },
        ),
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
        Shape::DirectEnum => {
            let tag = SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 255),
            );
            let variants = (0..2)
                .map(|index| {
                    SemanticEnumVariantLayoutV1::from_rustc(
                        index,
                        16,
                        8,
                        SemanticFieldsShapeV1::arbitrary(vec![8], vec![0]).unwrap(),
                        SemanticBackendReprV1::memory(true),
                        None,
                        false,
                        None,
                        8,
                        u64::from(index),
                        SemanticAggregateLayoutV1::new(
                            vec![8],
                            vec![SemanticPaddingV1::new(1, 7).unwrap()],
                        )
                        .unwrap(),
                    )
                    .unwrap()
                })
                .collect();
            (
                SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                    16,
                    8,
                    SemanticBackendReprV1::memory(true),
                    false,
                    SemanticEnumLayoutV1::new(
                        variants,
                        SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                            0, 0, tag,
                        )),
                    )
                    .unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::enum_type(
                    BYTE,
                    (0..2)
                        .map(|index| {
                            SemanticEnumVariantV1::new(
                                index,
                                SemanticAggregateTypeV1::new(vec![WORD]).unwrap(),
                            )
                        })
                        .collect(),
                )
                .unwrap(),
            )
        }
        Shape::NicheEnum => {
            let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
            let valid = SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX));
            let reference = declaration(
                4,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        primitive, valid,
                    )),
                    false,
                )
                .unwrap(),
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
                ),
            )
            .with_rustc_abi_properties(
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
            );
            types.push(reference);
            let reference = SemanticTypeIdV1::from_index(3);
            let niche = SemanticLayoutNicheV1::new(0, primitive, valid).unwrap();
            let tag = SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(1, 0),
            );
            let variants = (0..2)
                .map(|index| {
                    let offsets = if index == 0 { vec![] } else { vec![0] };
                    let order = if index == 0 { vec![] } else { vec![0] };
                    SemanticEnumVariantLayoutV1::from_rustc(
                        index,
                        8,
                        8,
                        SemanticFieldsShapeV1::arbitrary(offsets.clone(), order).unwrap(),
                        SemanticBackendReprV1::memory(true),
                        (index == 1).then_some(niche),
                        false,
                        None,
                        8,
                        u64::from(index),
                        SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
                    )
                    .unwrap()
                })
                .collect();
            (
                SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                    8,
                    8,
                    SemanticBackendReprV1::memory(true),
                    false,
                    SemanticEnumLayoutV1::new(
                        variants,
                        SemanticEnumEncodingV1::Niche(
                            SemanticNicheEnumEncodingV1::new(
                                0,
                                SemanticNicheSourceV1::new(
                                    vec![SemanticNichePathComponentV1::Field(0)],
                                    0,
                                )
                                .unwrap(),
                                niche,
                                tag,
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
                    BYTE,
                    vec![
                        SemanticEnumVariantV1::new(
                            0,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            1,
                            SemanticAggregateTypeV1::new(vec![reference]).unwrap(),
                        ),
                    ],
                )
                .unwrap(),
            )
        }
    };
    let composite = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(declaration(5, layout, shape_decl));
    let inputs = [BYTE, WORD, composite];
    let mut functions = Vec::new();
    for root in 0..roots {
        let mode = match shape {
            Shape::Ignored => SemanticAbiPassModeV1::Ignore,
            Shape::Pair => SemanticAbiPassModeV1::Pair {
                first: attributes(SemanticAbiExtensionV1::None),
                second: attributes(SemanticAbiExtensionV1::None),
            },
            _ => {
                let layout = types[composite.index() as usize].layout();
                SemanticAbiPassModeV1::Indirect {
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
                        layout.size_bytes().unwrap(),
                        Some(layout.alignment_bytes()),
                    )
                    .unwrap(),
                    metadata_attributes: None,
                    on_stack: false,
                }
            }
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([30 + root as u8; 32]),
            target,
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            3,
            inputs
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        ty,
                        if index == 2 {
                            mode.clone()
                        } else {
                            SemanticAbiPassModeV1::Direct(attributes(if ty == BYTE {
                                SemanticAbiExtensionV1::ZeroExtend
                            } else {
                                SemanticAbiExtensionV1::None
                            }))
                        },
                    ))
                })
                .collect(),
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 3])
        .unwrap();
        let local = |index: u8, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([40 + index; 32]),
                ty,
                role,
                source,
            )
        };
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
                vec![
                    local(0, UNIT, SemanticLocalRoleV1::Return),
                    local(1, BYTE, SemanticLocalRoleV1::Argument(0)),
                    local(2, WORD, SemanticLocalRoleV1::Argument(1)),
                    local(3, composite, SemanticLocalRoleV1::Argument(2)),
                ],
                SemanticBlockIdV1::from_index(0),
                vec![block],
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(format!("inline_source_{root}").into_bytes()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([110 + root as u8; 32]),
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
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

pub(super) fn fixture_input(owner: &ProductionSemanticSsaOwnerV1) -> FixtureKernelAbiV18 {
    let mut input = FixtureKernelAbiV18::new(owner);
    for (index, &id) in owner.source_semantic().roots().iter().enumerate() {
        let function = &owner.source_semantic().functions()[id.index() as usize];
        if matches!(
            function.abi().adjusted_arguments()[2].mode(),
            SemanticAbiPassModeV1::Ignore
        ) {
            input.arguments[index][2].kind =
                ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset: 16 };
            input.extents[index] = (16, 8);
        }
    }
    input
}

pub(super) fn capture(
    owner: &ProductionSemanticSsaOwnerV1,
    input: &FixtureKernelAbiV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CapturedKernelArgumentAbiV18 {
    CapturedKernelArgumentAbiV18::capture(
        owner,
        ProductionKernelArgumentAbiInputV18 {
            roots: &input.roots(),
        },
        budget,
    )
    .unwrap()
}

#[test]
fn inline_regions_retain_original_modes_padding_and_argument_identity_without_pointer_facts() {
    for shape in [
        Shape::Record,
        Shape::Array,
        Shape::Pair,
        Shape::Overaligned,
        Shape::Ignored,
        Shape::DirectEnum,
        Shape::NicheEnum,
    ] {
        let owner = fixture_owner(shape, 2);
        let input = fixture_input(&owner);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let profile = capture(&owner, &input, &mut budget);
        let held = budget.storage();
        for root in 0..2 {
            for argument in 0..2 {
                assert!(
                    profile
                        .by_value_argument_v29(&owner, root, argument, &mut budget)
                        .unwrap()
                        .is_none()
                );
            }
            let region = profile
                .by_value_argument_v29(&owner, root, 2, &mut budget)
                .unwrap()
                .unwrap();
            let function = &owner.source_semantic().functions()[root];
            assert_eq!(
                (region.root(), region.function(), region.argument()),
                (root, owner.source_semantic().roots()[root], 2)
            );
            assert_eq!(region.source_type(), function.locals()[3].ty());
            assert_eq!(
                region.source_type_identity(),
                input.arguments[root][2].semantic_type_identity
            );
            assert!(std::ptr::eq(
                region.source_abi(),
                &function.abi().adjusted_arguments()[2]
            ));
            assert_eq!(region.source_sha256(), owner.source_semantic_sha256());
            assert_eq!(region.kernel_binding(), &input.bindings[root]);
            assert_eq!(
                (
                    region.explicit_argument_bytes(),
                    region.kernarg_alignment_bytes()
                ),
                input.extents[root]
            );
            match (&region, shape) {
                (ProductionKernelByValueAbiV29::Ignored(_), Shape::Ignored) => {
                    assert_eq!(region.source_declaration().layout().alignment_bytes(), 64);
                    assert_eq!(input.extents[root], (16, 8));
                }
                (ProductionKernelByValueAbiV29::Inline(inline), _) => {
                    let layout = region.source_declaration().layout();
                    assert_eq!(inline.size_bytes(), layout.size_bytes().unwrap());
                    assert_eq!(inline.alignment_bytes(), layout.alignment_bytes());
                    assert_eq!(
                        inline.offset(),
                        if matches!(shape, Shape::Overaligned) {
                            64
                        } else {
                            16
                        }
                    );
                }
                _ => panic!("wrong ignored/inline classification"),
            }
            assert_eq!(
                profile.arguments[profile.roots[root].arguments.start + 2].components,
                [None, None]
            );
        }
        assert_eq!(budget.storage(), held);
        let credit = profile.retained_storage();
        drop(profile);
        budget.release_storage(credit).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn inline_capture_rejects_overlap_misalignment_extent_and_physical_ignored_padding() {
    for case in 0..5 {
        let owner = fixture_owner(
            if case == 4 {
                Shape::Ignored
            } else {
                Shape::Record
            },
            1,
        );
        let mut input = fixture_input(&owner);
        match case {
            0 => {
                input.arguments[0][2].kind =
                    ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset: 8 }
            }
            1 => {
                input.arguments[0][2].kind =
                    ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset: 17 }
            }
            2 => input.extents[0].0 -= 1,
            3 => {
                input.arguments[0][2].semantic_type_identity =
                    SemanticTypeIdentityV1::from_sha256([199; 32])
            }
            4 => {
                input.arguments[0][2].kind =
                    ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset: 64 }
            }
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            CapturedKernelArgumentAbiV18::capture(
                &owner,
                ProductionKernelArgumentAbiInputV18 {
                    roots: &input.roots()
                },
                &mut budget
            )
            .is_err()
        );
        // The failed unit capture owns all of this test's dropped vector scratch.
        budget.release_storage(budget.storage() - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn borrowed_inline_queries_reject_changed_captured_owner_and_original_interval() {
    for case in 0..8 {
        let owner = fixture_owner(Shape::Record, 2);
        let foreign = fixture_owner(Shape::Array, 2);
        let input = fixture_input(&owner);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut profile = capture(&owner, &input, &mut budget);
        match case {
            0 => profile.source = [0; 32],
            1 => profile.roots[0].binding = [0; 32],
            2 => profile.arguments[2].identity = SemanticTypeIdentityV1::from_sha256([0; 32]),
            3 => profile.arguments[2].kind = Kind::CompilerLaidOutByValue { offset: 0 },
            4 => profile.arguments[2].access = DescriptorAccess::ReadOnly,
            7 => profile.arguments[2].ownership = SemanticSourceArgumentOwnershipV1::SharedBorrow,
            _ => {}
        }
        let queried_owner = if case == 5 { &foreign } else { &owner };
        let ordinal = if case == 6 { usize::MAX } else { 2 };
        assert!(
            profile
                .by_value_argument_v29(queried_owner, 0, ordinal, &mut budget)
                .is_err()
        );
        let held = profile.retained_storage();
        drop(profile);
        budget.release_storage(held).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn inline_root_lens_requires_original_instance_argument_local_and_profile_identity() {
    let mut owner = fixture_owner(Shape::Record, 2);
    let input = fixture_input(&owner);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let occurrence = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap()
        .retained_storage();
    budget.reserve_storage(occurrence).unwrap();
    let profile = capture(&owner, &input, &mut budget);
    let lens = profile.descriptor_root(&owner, 0, &mut budget).unwrap();
    let held = budget.storage();
    for root in 0..2 {
        crate::production_semantic_kir_v1::production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            owner.source_semantic().roots()[root],
            &mut budget,
            |instances, budget| {
                let before = budget.work();
                let result = lens.by_value_argument_v29(
                    instances,
                    2,
                    SemanticLocalIdV1::from_index(3),
                    budget,
                );
                if root == 0 {
                    assert!(result.unwrap().is_some());
                    assert_eq!(budget.work() - before, 8 + 32 + 12);
                } else {
                    assert!(result.is_err());
                    assert_eq!(budget.work() - before, 8);
                }
                for (argument, local) in [(2, 0), (1, 3), (u32::MAX, 3), (2, u32::MAX)] {
                    assert!(
                        lens.by_value_argument_v29(
                            instances,
                            argument,
                            SemanticLocalIdV1::from_index(local),
                            budget
                        )
                        .is_err()
                    );
                }
                Ok::<_, crate::production_semantic_kir_v1::production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), held);
    }
    let credit = profile.retained_storage();
    drop(profile);
    drop(owner);
    budget.release_storage(credit + occurrence).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
