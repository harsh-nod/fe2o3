use super::same_structural_layout_v1;
use crate::semantic_mir_v1::*;

fn unit(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
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
    )
}

fn empty_aggregate(tag: u8) -> SemanticTypeDeclV1 {
    let mut ty = unit(tag);
    ty.layout_identity = unit(1).layout_identity;
    ty.shape = SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap());
    ty.layout.details = SemanticTypeLayoutDetailsV1::Aggregate(
        SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
    );
    ty
}

fn request(types: Vec<SemanticTypeDeclV1>) -> InertSemanticMirRequestV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([240; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(SemanticTypeIdV1(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    // The reachable root retains each tested nominal type in its MIR local
    // inventory; complete admission intentionally rejects detached type records.
    let locals = (0..types.len())
        .map(|index| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([u8::try_from(index + 1).unwrap(); 32]),
                SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()),
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([1; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([1; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([1; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1(0),
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
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1(0)],
    )
    .unwrap()
}

fn reject_layout(request: InertSemanticMirRequestV1) {
    assert!(matches!(
        layout_diagnostics_v1::diagnose_type_layouts_v1(&request, SemanticMirLimitsV1::default()),
        Err((_, SemanticMirErrorV1::InvalidTypeLayout))
    ));
    assert!(matches!(
        request.admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

#[test]
fn distinct_empty_nominal_types_share_structural_identity_and_round_trip() {
    let types = vec![unit(1), empty_aggregate(2)];
    assert_ne!(types[0].identity, types[1].identity);
    assert_eq!(types[0].layout_identity, types[1].layout_identity);
    assert_ne!(types[0].layout, types[1].layout);
    assert!(same_structural_layout_v1(
        &types[0].layout,
        &types[1].layout
    ));
    let request = request(types);
    assert_eq!(
        layout_diagnostics_v1::diagnose_type_layouts_v1(&request, SemanticMirLimitsV1::default()),
        Ok(())
    );
    let mut detached = request.clone();
    detached.functions[0].locals = detached.functions[0].locals[..1]
        .to_vec()
        .into_boxed_slice();
    assert_eq!(
        detached
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::TypeOutsideRootClosure {
            ty: SemanticTypeIdV1(1),
        }
    );
    let admitted = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        admitted.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.types(), admitted.types());
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
}

#[test]
fn structural_identity_does_not_allow_nominal_return_type_substitution() {
    let baseline = request(vec![unit(1), empty_aggregate(2)]);
    baseline
        .clone()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let mut changed = baseline;
    // Both return layouts are Ignore, but the exact source type is still unit.
    changed.functions[0].locals[0].ty = SemanticTypeIdV1(1);
    assert!(
        changed
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
}

#[test]
fn every_structural_field_remains_in_the_identity_consistency_check() {
    let baseline = unit(1).layout;
    let mutations: &[fn(&mut SemanticTypeLayoutV1)] = &[
        |layout| layout.rustc_size_bytes = 1,
        |layout| layout.size_bytes = None,
        |layout| layout.alignment_bytes = 2,
        |layout| layout.fields = SemanticFieldsShapeV1::Primitive,
        |layout| layout.variants = SemanticRustcVariantsV1::Single { index: 1 },
        |layout| layout.backend_repr = SemanticBackendReprV1::memory(false),
        |layout| {
            layout.largest_niche = Some(
                SemanticLayoutNicheV1::new(
                    0,
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(1, u8::MAX.into()),
                )
                .unwrap(),
            );
        },
        |layout| layout.uninhabited = true,
        |layout| layout.max_repr_alignment_bytes = Some(2),
        |layout| layout.unadjusted_abi_alignment_bytes = 2,
        |layout| layout.randomization_seed = 1,
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut changed = baseline.clone();
        mutate(&mut changed);
        assert_ne!(baseline, changed);
        assert!(
            !same_structural_layout_v1(&baseline, &changed),
            "field {index}"
        );
        assert!(
            !same_structural_layout_v1(&changed, &baseline),
            "field {index}"
        );
    }
}

#[test]
fn individually_valid_but_different_structural_layouts_cannot_share_identity() {
    let mut left = unit(2);
    left.shape = SemanticTypeShapeV1::Opaque;
    let mut right = unit(3);
    right.shape = SemanticTypeShapeV1::Opaque;
    right.layout.rustc_size_bytes = 1;
    right.layout.size_bytes = Some(1);
    request(vec![unit(1), left.clone(), right.clone()])
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    right.layout_identity = left.layout_identity;
    reject_layout(request(vec![unit(1), left, right]));
}

#[test]
fn shared_identity_never_waives_shape_dependent_aggregate_details() {
    let baseline = empty_aggregate(2);
    for details in [
        SemanticTypeLayoutDetailsV1::None,
        SemanticTypeLayoutDetailsV1::Aggregate(
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        ),
        SemanticTypeLayoutDetailsV1::Aggregate(
            SemanticAggregateLayoutV1::new(vec![], vec![SemanticPaddingV1::new(0, 1).unwrap()])
                .unwrap(),
        ),
    ] {
        let mut changed = baseline.clone();
        changed.layout.details = details;
        assert!(same_structural_layout_v1(&unit(1).layout, &changed.layout));
        reject_layout(request(vec![unit(1), changed]));
    }
    let mut changed_unit = unit(1);
    changed_unit.layout.details = baseline.layout.details;
    reject_layout(request(vec![changed_unit, empty_aggregate(2)]));
}

#[test]
fn padding_annotations_are_validated_against_each_nominal_field_roster() {
    let scalar = |tag, bits, bytes| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, bits, bytes),
                    SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        )
    };
    let aggregate = |tag, field, padding| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([40; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0], padding).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![field]).unwrap()),
        )
    };
    let left = aggregate(
        4,
        SemanticTypeIdV1(1),
        vec![SemanticPaddingV1::new(4, 4).unwrap()],
    );
    let right = aggregate(5, SemanticTypeIdV1(2), vec![]);
    assert!(same_structural_layout_v1(&left.layout, &right.layout));
    assert_ne!(left.layout, right.layout);
    let types = vec![unit(1), scalar(2, 32, 4), scalar(3, 64, 8), left, right];
    request(types.clone())
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let mut changed = types;
    changed[4].layout.details = changed[3].layout.details.clone();
    reject_layout(request(changed));
}
