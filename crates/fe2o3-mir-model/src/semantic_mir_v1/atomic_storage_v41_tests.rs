use super::*;
const V41: SemanticMirWireVersionV1 = SemanticMirWireVersionV1::V41;

fn request(signed: bool) -> InertSemanticMirRequestV1 {
    let mut r = minimal_request();
    let mut types = r.types.into_vec();
    if signed {
        types[0] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(1)),
            SemanticLayoutIdentityV1(identity(2)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(true, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            }),
        );
    }
    for index in 1..=3u32 {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(20 + index as u8)),
            SemanticLayoutIdentityV1(identity(30 + index as u8)),
            SemanticTypeLayoutV1::aggregate(
                Some(4),
                4,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(index - 1)])
                    .unwrap(),
            ),
        ));
    }
    types[3].rust_type_kind = if signed {
        SemanticRustTypeKindV1::AtomicI32
    } else {
        SemanticRustTypeKindV1::AtomicU32
    };
    r.types = types.into_boxed_slice();
    let mut locals = r.functions[0].locals.to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(250)),
        SemanticTypeIdV1::from_index(3),
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    r.functions[0].locals = locals.into_boxed_slice();
    r
}
#[test]
fn atomic_v41_exact_roundtrip_preserves_full_original_table_and_identity() {
    for signed in [false, true] {
        let r = request(signed);
        let a = r
            .clone()
            .admit_exact_v41(SemanticMirLimitsV1::default())
            .unwrap();
        let b = AdmittedInertSemanticMirV1::decode_exact_v41_canonical(
            a.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(a.types(), r.types.as_ref());
        assert_eq!(a.types(), b.types());
        assert_eq!(a.canonical_encoding(), b.canonical_encoding());
        assert_eq!(a.semantic_sha256(), b.semantic_sha256());
        assert_eq!(
            semantic_atomic_storage_chain_v41(a.types(), SemanticTypeIdV1::from_index(3)),
            Some([2, 1, 0].map(SemanticTypeIdV1::from_index))
        );
    }
}
#[test]
fn atomic_v41_old_schemas_and_automatic_paths_refuse() {
    let r = request(false);
    for version in (2..=15)
        .chain(28..=40)
        .map(|x| SemanticMirWireVersionV1::from_u16(x).unwrap())
    {
        assert!(
            r.clone()
                .admit_for_wire_version(version, SemanticMirLimitsV1::default())
                .is_err()
        );
        let mut w = CanonicalWriterV1::new(4096);
        assert!(encode_type(&mut w, &r.types[3], version).is_err());
    }
    assert!(r.clone().admit(SemanticMirLimitsV1::default()).is_err());
    assert!(
        r.clone()
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
    let a = r.admit_exact_v41(SemanticMirLimitsV1::default()).unwrap();
    assert!(
        AdmittedInertSemanticMirV1::decode_minimal_compatible_canonical(
            a.canonical_encoding(),
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            a.canonical_encoding(),
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
}
#[test]
fn atomic_v41_missing_reordered_cyclic_or_extra_fields_refuse() {
    for root in 1..=3 {
        for fields in [vec![], vec![root], vec![0, 0], vec![99]] {
            let mut r = request(false);
            r.types[root as usize].shape = SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(
                    fields
                        .into_iter()
                        .map(SemanticTypeIdV1::from_index)
                        .collect(),
                )
                .unwrap(),
            );
            assert!(r.admit_exact_v41(SemanticMirLimitsV1::default()).is_err());
        }
    }
}
#[test]
fn atomic_v41_mismatched_signedness_width_layout_or_nominal_child_refuse() {
    for fault in 0..7 {
        let mut r = request(false);
        match fault {
            0 => r.types[3].rust_type_kind = SemanticRustTypeKindV1::AtomicI32,
            1 => {
                r.types[0].shape = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                })
            }
            2 => r.types[1].layout.alignment_bytes = 8,
            3 => r.types[2].layout.size_bytes = Some(8),
            4 => r.types[2].rust_type_kind = SemanticRustTypeKindV1::AtomicU32,
            5 => {
                r.types[1].layout.details = SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(vec![1], vec![]).unwrap(),
                )
            }
            6 => r.types[0].rust_type_kind = SemanticRustTypeKindV1::Usize,
            _ => unreachable!(),
        }
        assert!(r.admit_exact_v41(SemanticMirLimitsV1::default()).is_err());
    }
}
#[test]
fn atomic_v41_absent_marker_is_not_structural_permission() {
    let mut r = request(false);
    r.types[3].rust_type_kind = SemanticRustTypeKindV1::Ordinary;
    assert_eq!(
        semantic_atomic_storage_chain_v41(&r.types, SemanticTypeIdV1::from_index(3)),
        None
    );
    let a = r
        .clone()
        .admit_exact_v40(SemanticMirLimitsV1::default())
        .unwrap();
    let b = r.admit_exact_v41(SemanticMirLimitsV1::default()).unwrap();
    assert_ne!(a.semantic_sha256(), b.semantic_sha256());
}
#[test]
fn atomic_v41_ordinary_layout_unchanged_and_marker_is_encoded() {
    let r = request(false);
    let mut ordinary = r.types[3].clone();
    ordinary.rust_type_kind = SemanticRustTypeKindV1::Ordinary;
    let encode = |ty: &SemanticTypeDeclV1, v| {
        let mut w = CanonicalWriterV1::new(4096);
        encode_type(&mut w, ty, v).unwrap();
        w.finish()
    };
    assert_eq!(
        encode(&ordinary, V41),
        encode(&ordinary, SemanticMirWireVersionV1::V40)
    );
    assert_ne!(encode(&ordinary, V41), encode(&r.types[3], V41));
    let mut changed = r.clone();
    let mut distinct = identity(22);
    distinct[31] = 23;
    changed.types[2].identity = SemanticTypeIdentityV1(distinct);
    let a = r.admit_exact_v41(SemanticMirLimitsV1::default()).unwrap();
    let b = changed
        .admit_exact_v41(SemanticMirLimitsV1::default())
        .unwrap();
    assert_ne!(a.semantic_sha256(), b.semantic_sha256());
}

#[test]
fn atomic_v41_nominal_validation_has_bounded_prepaid_work() {
    let r = request(false);
    for remaining in [39, 40] {
        let mut context = ValidationContextV1 {
            request: &r,
            limits: SemanticMirLimitsV1::default()
                .with_limit(SemanticMirResourceV1::ValidationWork, 17 + remaining)
                .unwrap(),
            totals: ValidationTotalsV1::default(),
            work: 17,
            owned_execution_roles: vec![],
        };
        let result = atomic_storage_v41::validate_type(
            &mut context,
            SemanticTypeIdV1::from_index(3),
            &r.types[3],
        );
        assert_eq!(result.is_ok(), remaining == 40);
        assert!(context.owned_execution_roles.is_empty());
    }
}
#[test]
fn atomic_v41_closed_discriminants_do_not_grow_the_type_record_payload() {
    #[allow(dead_code)]
    #[derive(Clone, Copy)]
    enum OldKind {
        Ordinary,
        Str,
        Execution(SemanticExecutionRoleV29),
        Usize,
        Isize,
    }
    assert_eq!(
        std::mem::size_of::<OldKind>(),
        std::mem::size_of::<SemanticRustTypeKindV1>()
    );
    assert_eq!(
        std::mem::align_of::<OldKind>(),
        std::mem::align_of::<SemanticRustTypeKindV1>()
    );
}

#[test]
fn atomic_v41_declaration_commitment_covers_marker_child_identity_and_layout() {
    let original = request(false);
    let commit = |r: &InertSemanticMirRequestV1| {
        canonical_declaration_tables_commitment_v1(
            &r.types,
            &r.callables,
            V41,
            SemanticMirLimitsV1::default(),
            &mut |_| Ok(()),
        )
        .unwrap()
        .sha256()
    };
    let baseline = commit(&original);
    for fault in 0..5 {
        let mut changed = original.clone();
        match fault {
            0 => changed.types[3].rust_type_kind = SemanticRustTypeKindV1::Ordinary,
            1 => changed.types[2].identity = SemanticTypeIdentityV1(identity(71)),
            2 => changed.types[1].layout_identity = SemanticLayoutIdentityV1(identity(72)),
            3 => {
                changed.types[1].shape = SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(2)]).unwrap(),
                )
            }
            4 => {
                changed.types[2].layout.details = SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(vec![1], vec![]).unwrap(),
                )
            }
            _ => unreachable!(),
        }
        // This encoder observation is not admission of the mutated graph.
        assert_ne!(commit(&changed), baseline);
    }
    assert!(
        canonical_declaration_tables_commitment_v1(
            &original.types,
            &original.callables,
            SemanticMirWireVersionV1::V40,
            SemanticMirLimitsV1::default(),
            &mut |_| Ok(())
        )
        .is_err()
    );
}
#[test]
fn atomic_v41_relabelled_old_header_cannot_hide_new_type_tags() {
    let a = request(false)
        .admit_exact_v41(SemanticMirLimitsV1::default())
        .unwrap();
    let mut bytes = a.canonical_encoding().to_vec();
    assert_eq!(&bytes[..MAGIC.len()], MAGIC);
    assert_eq!(&bytes[MAGIC.len()..MAGIC.len() + 2], &41u16.to_le_bytes());
    bytes[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&40u16.to_le_bytes());
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v40_canonical(
            &bytes,
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
}

#[test]
fn atomic_v41_wrapper_must_belong_to_the_original_root_type_closure() {
    let mut original = request(false);
    assert_eq!(
        original.functions[0].locals.last().unwrap().ty,
        SemanticTypeIdV1::from_index(3)
    );
    let mut locals = original.functions[0].locals.to_vec();
    locals.pop().unwrap();
    original.functions[0].locals = locals.into_boxed_slice();
    assert_eq!(
        original
            .admit_exact_v41(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::TypeOutsideRootClosure {
            ty: SemanticTypeIdV1::from_index(1)
        }
    );
}
