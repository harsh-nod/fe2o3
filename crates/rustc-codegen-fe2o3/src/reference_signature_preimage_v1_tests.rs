use super::*;

fn scalar(scalar: ReferenceScalarTypeV1) -> ReferenceSignatureInputV1 {
    ReferenceSignatureInputV1::Scalar(scalar)
}

fn reference(
    region: ReferenceRegionV1,
    mutability: SemanticMutabilityV1,
    pointee: ReferencePointeeV1,
) -> ReferenceSignatureInputV1 {
    ReferenceSignatureInputV1::Reference {
        region,
        mutability,
        pointee,
    }
}

fn shared(region: ReferenceRegionV1, element: ReferenceScalarTypeV1) -> ReferenceSignatureInputV1 {
    reference(
        region,
        SemanticMutabilityV1::Immutable,
        ReferencePointeeV1::Slice(element),
    )
}

fn output(
    carrier: ReferenceCarrierV1,
    element: ReferenceScalarTypeV1,
) -> ReferenceSignatureInputV1 {
    ReferenceSignatureInputV1::NominalOutput { carrier, element }
}

fn signature(
    kernel: Vec<ReferenceSignatureInputV1>,
    reference: Vec<ReferenceSignatureInputV1>,
) -> ReferenceLogicalSignaturePreimageV1 {
    ReferenceLogicalSignaturePreimageV1::new(
        kernel.into_boxed_slice(),
        reference.into_boxed_slice(),
        ReferenceReturnShapeV1::Unit,
        SemanticExternAbiV1::Rust,
        SemanticFunctionSafetyV1::Safe,
        false,
    )
    .unwrap()
}

fn rows(signature: &ReferenceLogicalSignaturePreimageV1) -> Vec<ReferenceArgumentRelationV1> {
    let relation = signature.derive_relations_v1().unwrap();
    (0..relation.len())
        .map(|raw| {
            relation
                .relation_at_raw_argument_v1(u32::try_from(raw).unwrap())
                .unwrap()
        })
        .collect()
}

#[test]
fn zero_through_three_prefixes_preserve_original_and_raw_ordinals() {
    let input = shared(ReferenceRegionV1::Erased, ReferenceScalarTypeV1::U16);
    let scalar_input = scalar(ReferenceScalarTypeV1::I32);
    let output_input = output(
        ReferenceCarrierV1::DisjointSlice,
        ReferenceScalarTypeV1::U32,
    );
    let output_reference = reference(
        ReferenceRegionV1::Erased,
        SemanticMutabilityV1::Mutable,
        ReferencePointeeV1::Scalar(ReferenceScalarTypeV1::U32),
    );
    for axes in 0..=3 {
        let mut reference = vec![scalar(ReferenceScalarTypeV1::Usize); axes];
        reference.extend([scalar_input, input, output_reference]);
        let signature = signature(vec![scalar_input, input, output_input], reference);
        let relation = signature.derive_relations_v1().unwrap();
        let mut expected = (0..axes)
            .map(|axis| ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: axis as u32,
                axis: axis as u32,
            })
            .collect::<Vec<_>>();
        expected.extend([
            ReferenceArgumentRelationV1::ScalarInput {
                argument: 0,
                scalar: ReferenceScalarTypeV1::I32,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 1,
                element: ReferenceScalarTypeV1::U16,
            },
            ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                argument: 2,
                element: ReferenceScalarTypeV1::U32,
            },
        ]);
        assert_eq!(rows(&signature), expected);
        for source in 0..3 {
            assert_eq!(
                relation.reference_argument_for_kernel_argument_v1(source),
                Some(source + axes as u32)
            );
        }
        assert_eq!(relation.reference_argument_for_kernel_argument_v1(3), None);
        assert_eq!(
            relation.reference_argument_for_kernel_argument_v1(u32::MAX),
            None
        );
        assert_eq!(
            relation.relation_at_raw_argument_v1(relation.len() as u32),
            None
        );
        assert_eq!(relation.relation_at_raw_argument_v1(u32::MAX), None);
    }
}

#[test]
fn empty_kernel_and_coordinate_only_reference_are_representable() {
    for axes in 0..=3 {
        let signature = signature(vec![], vec![scalar(ReferenceScalarTypeV1::Usize); axes]);
        let relation = signature.derive_relations_v1().unwrap();
        assert_eq!(relation.len(), axes);
        assert_eq!(relation.reference_argument_for_kernel_argument_v1(0), None);
        assert_eq!(rows(&signature).len(), axes);
    }
}

#[test]
fn every_supported_scalar_is_distinct_including_pointer_sized_integers() {
    let scalars = [
        ReferenceScalarTypeV1::Bool,
        ReferenceScalarTypeV1::U8,
        ReferenceScalarTypeV1::U16,
        ReferenceScalarTypeV1::U32,
        ReferenceScalarTypeV1::U64,
        ReferenceScalarTypeV1::Usize,
        ReferenceScalarTypeV1::I8,
        ReferenceScalarTypeV1::I16,
        ReferenceScalarTypeV1::I32,
        ReferenceScalarTypeV1::I64,
        ReferenceScalarTypeV1::Isize,
        ReferenceScalarTypeV1::F32,
        ReferenceScalarTypeV1::F64,
    ];
    for kernel in scalars {
        for reference in scalars {
            let signature = signature(vec![scalar(kernel)], vec![scalar(reference)]);
            assert_eq!(signature.derive_relations_v1().is_ok(), kernel == reference);
        }
    }
}

#[test]
fn coordinate_prefix_requires_usize_not_a_layout_equivalent_scalar() {
    for invalid in [
        ReferenceScalarTypeV1::U64,
        ReferenceScalarTypeV1::Isize,
        ReferenceScalarTypeV1::U32,
    ] {
        let signature = signature(vec![], vec![scalar(invalid)]);
        assert_eq!(
            signature.derive_relations_v1().unwrap_err(),
            ReferenceSignatureErrorV1::InvalidPointCoordinate {
                reference_argument: 0
            }
        );
    }
}

#[test]
fn coordinates_cannot_be_moved_after_kernel_inputs() {
    let signature = signature(
        vec![scalar(ReferenceScalarTypeV1::U32)],
        vec![
            scalar(ReferenceScalarTypeV1::U32),
            scalar(ReferenceScalarTypeV1::Usize),
        ],
    );
    assert_eq!(
        signature.derive_relations_v1().unwrap_err(),
        ReferenceSignatureErrorV1::InvalidPointCoordinate {
            reference_argument: 0
        }
    );
}

#[test]
fn all_three_coordinate_slots_are_checked() {
    for invalid_slot in 0..3 {
        let mut inputs = vec![scalar(ReferenceScalarTypeV1::Usize); 3];
        inputs[invalid_slot] = scalar(ReferenceScalarTypeV1::U64);
        let signature = signature(vec![], inputs);
        assert_eq!(
            signature.derive_relations_v1().unwrap_err(),
            ReferenceSignatureErrorV1::InvalidPointCoordinate {
                reference_argument: invalid_slot
            }
        );
    }
}

#[test]
fn shared_slice_regions_are_not_erased_by_pure_derivation() {
    for kernel in [ReferenceRegionV1::Erased, ReferenceRegionV1::Static] {
        for reference in [ReferenceRegionV1::Erased, ReferenceRegionV1::Static] {
            let signature = signature(
                vec![shared(kernel, ReferenceScalarTypeV1::U32)],
                vec![shared(reference, ReferenceScalarTypeV1::U32)],
            );
            assert_eq!(signature.derive_relations_v1().is_ok(), kernel == reference);
        }
    }
}

#[test]
fn shared_slice_mutability_element_and_pointee_shape_must_match() {
    let kernel = shared(ReferenceRegionV1::Erased, ReferenceScalarTypeV1::Usize);
    for invalid in [
        shared(ReferenceRegionV1::Erased, ReferenceScalarTypeV1::U64),
        reference(
            ReferenceRegionV1::Erased,
            SemanticMutabilityV1::Mutable,
            ReferencePointeeV1::Slice(ReferenceScalarTypeV1::Usize),
        ),
        reference(
            ReferenceRegionV1::Erased,
            SemanticMutabilityV1::Immutable,
            ReferencePointeeV1::Scalar(ReferenceScalarTypeV1::Usize),
        ),
    ] {
        assert!(matches!(
            signature(vec![kernel], vec![invalid]).derive_relations_v1(),
            Err(ReferenceSignatureErrorV1::ArgumentMismatch { .. })
        ));
    }
}

#[test]
fn both_nominal_carriers_support_only_exact_mutable_output_elements() {
    for carrier in [
        ReferenceCarrierV1::DisjointSlice,
        ReferenceCarrierV1::WriteOnlyDisjointSlice,
    ] {
        for region in [ReferenceRegionV1::Erased, ReferenceRegionV1::Static] {
            for pointee in [
                ReferencePointeeV1::Scalar(ReferenceScalarTypeV1::U32),
                ReferencePointeeV1::Slice(ReferenceScalarTypeV1::U32),
            ] {
                let signature = signature(
                    vec![output(carrier, ReferenceScalarTypeV1::U32)],
                    vec![reference(region, SemanticMutabilityV1::Mutable, pointee)],
                );
                let expected = match pointee {
                    ReferencePointeeV1::Scalar(_) => {
                        ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                            argument: 0,
                            element: ReferenceScalarTypeV1::U32,
                        }
                    }
                    ReferencePointeeV1::Slice(_) => {
                        ReferenceArgumentRelationV1::DisjointOutputSlice {
                            argument: 0,
                            element: ReferenceScalarTypeV1::U32,
                        }
                    }
                };
                assert_eq!(rows(&signature), vec![expected]);
            }
        }
    }
}

#[test]
fn output_reference_mutability_and_element_are_not_inferred_from_layout() {
    let kernel = output(
        ReferenceCarrierV1::DisjointSlice,
        ReferenceScalarTypeV1::Usize,
    );
    for invalid in [
        shared(ReferenceRegionV1::Erased, ReferenceScalarTypeV1::Usize),
        reference(
            ReferenceRegionV1::Erased,
            SemanticMutabilityV1::Mutable,
            ReferencePointeeV1::Scalar(ReferenceScalarTypeV1::U64),
        ),
        reference(
            ReferenceRegionV1::Erased,
            SemanticMutabilityV1::Mutable,
            ReferencePointeeV1::Slice(ReferenceScalarTypeV1::U64),
        ),
        scalar(ReferenceScalarTypeV1::Usize),
        kernel,
    ] {
        assert!(matches!(
            signature(vec![kernel], vec![invalid]).derive_relations_v1(),
            Err(ReferenceSignatureErrorV1::ArgumentMismatch { .. })
        ));
    }
}

#[test]
fn ordinary_references_do_not_gain_nominal_output_relations() {
    for unsupported in [
        reference(
            ReferenceRegionV1::Erased,
            SemanticMutabilityV1::Mutable,
            ReferencePointeeV1::Slice(ReferenceScalarTypeV1::U32),
        ),
        reference(
            ReferenceRegionV1::Erased,
            SemanticMutabilityV1::Mutable,
            ReferencePointeeV1::Scalar(ReferenceScalarTypeV1::U32),
        ),
        reference(
            ReferenceRegionV1::Erased,
            SemanticMutabilityV1::Immutable,
            ReferencePointeeV1::Scalar(ReferenceScalarTypeV1::U32),
        ),
    ] {
        assert_eq!(
            signature(vec![unsupported], vec![unsupported])
                .derive_relations_v1()
                .unwrap_err(),
            ReferenceSignatureErrorV1::UnsupportedKernelArgument { argument: 0 }
        );
    }
}

#[test]
fn nominal_carrier_tags_are_retained_not_reduced_to_a_write_flag() {
    let cpu = reference(
        ReferenceRegionV1::Erased,
        SemanticMutabilityV1::Mutable,
        ReferencePointeeV1::Slice(ReferenceScalarTypeV1::U32),
    );
    let disjoint = signature(
        vec![output(
            ReferenceCarrierV1::DisjointSlice,
            ReferenceScalarTypeV1::U32,
        )],
        vec![cpu],
    );
    let write_only = signature(
        vec![output(
            ReferenceCarrierV1::WriteOnlyDisjointSlice,
            ReferenceScalarTypeV1::U32,
        )],
        vec![cpu],
    );
    assert_ne!(disjoint, write_only);
    assert_eq!(rows(&disjoint), rows(&write_only));
}

#[test]
fn argument_swaps_are_not_repaired_by_type_search() {
    let signature = signature(
        vec![
            scalar(ReferenceScalarTypeV1::U32),
            scalar(ReferenceScalarTypeV1::U64),
        ],
        vec![
            scalar(ReferenceScalarTypeV1::Usize),
            scalar(ReferenceScalarTypeV1::U64),
            scalar(ReferenceScalarTypeV1::U32),
        ],
    );
    assert_eq!(
        signature.derive_relations_v1().unwrap_err(),
        ReferenceSignatureErrorV1::ArgumentMismatch {
            kernel_argument: 0,
            reference_argument: 1
        }
    );
}

#[test]
fn same_typed_arguments_keep_separate_source_ordinals_without_identity_claims() {
    let signature = signature(
        vec![scalar(ReferenceScalarTypeV1::U32); 2],
        vec![scalar(ReferenceScalarTypeV1::U32); 2],
    );
    assert_eq!(
        rows(&signature),
        vec![
            ReferenceArgumentRelationV1::ScalarInput {
                argument: 0,
                scalar: ReferenceScalarTypeV1::U32
            },
            ReferenceArgumentRelationV1::ScalarInput {
                argument: 1,
                scalar: ReferenceScalarTypeV1::U32
            },
        ]
    );
    // Identical type rows cannot authenticate a source/effect correspondence.
    let mut swapped_inputs = signature.reference_inputs().to_vec();
    swapped_inputs.swap(0, 1);
    let swapped = self::signature(signature.kernel_inputs().to_vec(), swapped_inputs);
    assert_eq!(swapped, signature);
}

#[test]
fn too_few_arguments_and_four_coordinate_prefixes_are_refused() {
    let check = |kernel, reference| {
        ReferenceLogicalSignaturePreimageV1::check_header_v1(
            kernel,
            reference,
            ReferenceReturnShapeV1::Unit,
            SemanticExternAbiV1::Rust,
            SemanticFunctionSafetyV1::Safe,
            false,
        )
    };
    assert_eq!(
        check(2, 1),
        Err(ReferenceSignatureErrorV1::TooFewReferenceArguments {
            kernel: 2,
            reference: 1
        })
    );
    assert_eq!(
        check(0, 4),
        Err(ReferenceSignatureErrorV1::TooManyPointAxes { actual: 4 })
    );
}

#[test]
fn reference_return_safety_abi_and_variadic_headers_fail_closed() {
    let check = |result, abi, safety, variadic| {
        ReferenceLogicalSignaturePreimageV1::new(
            Box::default(),
            Box::default(),
            result,
            abi,
            safety,
            variadic,
        )
    };
    assert_eq!(
        check(
            ReferenceReturnShapeV1::NonUnit,
            SemanticExternAbiV1::Rust,
            SemanticFunctionSafetyV1::Safe,
            false
        )
        .unwrap_err(),
        ReferenceSignatureErrorV1::NonUnitReturn
    );
    assert_eq!(
        check(
            ReferenceReturnShapeV1::Unit,
            SemanticExternAbiV1::Rust,
            SemanticFunctionSafetyV1::Unsafe,
            false
        )
        .unwrap_err(),
        ReferenceSignatureErrorV1::UnsafeReference
    );
    assert_eq!(
        check(
            ReferenceReturnShapeV1::Unit,
            SemanticExternAbiV1::C { unwind: false },
            SemanticFunctionSafetyV1::Safe,
            false
        )
        .unwrap_err(),
        ReferenceSignatureErrorV1::InvalidReferenceAbi
    );
    assert_eq!(
        check(
            ReferenceReturnShapeV1::Unit,
            SemanticExternAbiV1::Rust,
            SemanticFunctionSafetyV1::Safe,
            true
        )
        .unwrap_err(),
        ReferenceSignatureErrorV1::InvalidReferenceAbi
    );
}

#[test]
fn input_limit_checks_do_not_allocate_or_overflow() {
    let check = |kernel, reference| {
        ReferenceLogicalSignaturePreimageV1::check_header_v1(
            kernel,
            reference,
            ReferenceReturnShapeV1::Unit,
            SemanticExternAbiV1::Rust,
            SemanticFunctionSafetyV1::Safe,
            false,
        )
    };
    let max = MAX_REFERENCE_SIGNATURE_INPUTS_V1;
    assert_eq!(check(max, max), Ok(0));
    assert_eq!(check(max - 3, max), Ok(3));
    assert_eq!(
        check(max + 1, max + 1),
        Err(ReferenceSignatureErrorV1::InputLimit {
            side: ReferenceSignatureSideV1::Kernel,
            actual: max + 1
        })
    );
    assert_eq!(
        check(max, max + 1),
        Err(ReferenceSignatureErrorV1::InputLimit {
            side: ReferenceSignatureSideV1::Reference,
            actual: max + 1
        })
    );
    assert_eq!(
        check(usize::MAX, usize::MAX),
        Err(ReferenceSignatureErrorV1::InputLimit {
            side: ReferenceSignatureSideV1::Kernel,
            actual: usize::MAX
        })
    );
}

#[test]
fn borrowed_rederivation_leaves_owned_input_storage_unchanged() {
    let signature = signature(
        vec![scalar(ReferenceScalarTypeV1::Usize)],
        vec![scalar(ReferenceScalarTypeV1::Usize)],
    );
    let kernel_pointer = signature.kernel_inputs().as_ptr();
    let reference_pointer = signature.reference_inputs().as_ptr();
    let expected = rows(&signature);
    for _ in 0..3 {
        assert_eq!(rows(&signature), expected);
        assert_eq!(signature.kernel_inputs().as_ptr(), kernel_pointer);
        assert_eq!(signature.reference_inputs().as_ptr(), reference_pointer);
    }
}

#[test]
fn signature_retention_does_not_change_existing_effect_ir_digest() {
    use super::super::ReferenceEffectIrV1;

    let cpu = reference(
        ReferenceRegionV1::Erased,
        SemanticMutabilityV1::Mutable,
        ReferencePointeeV1::Scalar(ReferenceScalarTypeV1::U32),
    );
    let disjoint = signature(
        vec![output(
            ReferenceCarrierV1::DisjointSlice,
            ReferenceScalarTypeV1::U32,
        )],
        vec![cpu],
    );
    let write_only = signature(
        vec![output(
            ReferenceCarrierV1::WriteOnlyDisjointSlice,
            ReferenceScalarTypeV1::U32,
        )],
        vec![cpu],
    );
    let effect = |relations: Vec<_>| ReferenceEffectIrV1 {
        argument_count: 1,
        local_count: 2,
        relations: relations.into_boxed_slice(),
        blocks: Box::default(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    let old_rows = vec![ReferenceArgumentRelationV1::DisjointOutputCoordinate {
        argument: 0,
        element: ReferenceScalarTypeV1::U32,
    }];
    let expected = effect(old_rows).canonical_sha256_v1();
    assert_eq!(effect(rows(&disjoint)).canonical_sha256_v1(), expected);
    assert_eq!(effect(rows(&write_only)).canonical_sha256_v1(), expected);
}
