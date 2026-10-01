use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn append_reference_option(types: &mut Vec<Declaration>, mutable: bool) -> (TypeId, TypeId) {
    let reference = TypeId::from_index(types.len() as u32);
    let option = TypeId::from_index(types.len() as u32 + 1);
    let primitive = Primitive::pointer(0, 8, 8);
    let nonnull = Validity::new(1, u128::from(u64::MAX));
    let scalar = SemanticBackendScalarV1::initialized(primitive, nonnull);
    let pointer = Declaration::new(
        SemanticTypeIdentityV1::from_sha256([240; 32]),
        SemanticLayoutIdentityV1::from_sha256([240; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, Backend::scalar(scalar), false)
            .unwrap(),
        Shape::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                TypeId::from_index(0),
                PointerKind::Reference,
                if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                Metadata::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if mutable {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    types[0].layout().size_bytes().unwrap(),
                    types[0].layout().alignment_bytes(),
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let source_niche = SemanticLayoutNicheV1::new(0, primitive, nonnull).unwrap();
    let tag = SemanticBackendScalarV1::initialized(primitive, Validity::new(1, 0));
    let physical = [false, true]
        .into_iter()
        .enumerate()
        .map(|(index, payload)| {
            let offsets = if payload { vec![0] } else { vec![] };
            SemanticEnumVariantLayoutV1::from_rustc(
                index as u32,
                8,
                8,
                Fields::arbitrary(offsets.clone(), if payload { vec![0] } else { vec![] }).unwrap(),
                if payload {
                    Backend::scalar(scalar)
                } else {
                    Backend::memory(true)
                },
                payload.then_some(source_niche),
                false,
                None,
                8,
                index as u64,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let optional = Declaration::new(
        SemanticTypeIdentityV1::from_sha256([241; 32]),
        SemanticLayoutIdentityV1::from_sha256([241; 32]),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            8,
            Backend::scalar(tag),
            false,
            EnumLayout::new(
                physical,
                Encoding::Niche(
                    Niche::new(
                        0,
                        SemanticNicheSourceV1::new(vec![Path::Field(0)], 0).unwrap(),
                        source_niche,
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
        Shape::enum_type(
            TypeId::from_index(0),
            vec![
                Variant::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                Variant::new(1, SemanticAggregateTypeV1::new(vec![reference]).unwrap()),
            ],
        )
        .unwrap(),
    );
    types.extend([pointer, optional]);
    (reference, option)
}

fn run(
    work: usize,
    storage: usize,
    mutable: bool,
    examine: impl FnOnce(&SourceSlots<'_, '_>, TypeId, TypeId, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    let ids = std::cell::Cell::new(None);
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            let (reference, option) = append_reference_option(types, mutable);
            ids.set(Some((reference, option)));
            // Retain the enum and its reference payload in the exact root type
            // closure without changing any argument, ABI, or executable block.
            let root = &mut functions[0];
            let mut locals = root.locals().to_vec();
            let identity = SemanticLocalIdentityV1::from_sha256([254; 32]);
            assert!(locals.last().unwrap().identity().as_bytes() < identity.as_bytes());
            locals.push(SemanticLocalDeclV1::new(
                identity,
                option,
                SemanticLocalRoleV1::Temporary,
                root.source(),
            ));
            *root = SemanticFunctionDeclV1::new(
                root.identity(),
                root.role(),
                root.item_definition_identity(),
                root.monomorphization_identity(),
                root.generic_type_arguments_identity(),
                root.const_generic_arguments_identity(),
                root.source(),
                root.abi().clone(),
                locals,
                root.entry(),
                root.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(root.kernel_entry().unwrap().clone());
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let (reference, option) = ids.get().unwrap();
                examine(slots, reference, option, out)
            })
        },
    )
}

#[test]
fn source_tag_contracts_derive_shared_and_mutable_null_niches_from_original_owners() {
    for mutable in [false, true] {
        run(LIMIT, LIMIT, mutable, |slots, reference, option, out| {
            let recipe = slots.tag_contract(option, out)?;
            assert_eq!(recipe.source_type(out)?, option);
            let original = slots
                .relation
                .source(out.budget)?
                .source_semantic(out.budget)?;
            let root = &original.functions()[original.roots()[0].index() as usize];
            assert_eq!(root.locals().last().unwrap().ty(), option);
            assert_eq!(
                root.locals().last().unwrap().role(),
                SemanticLocalRoleV1::Temporary
            );
            assert!(std::ptr::eq(
                recipe.declaration(out)?,
                &original.types()[option.index() as usize]
            ));
            assert_eq!(
                slots.tag_contract(reference, out)?.class(out)?,
                SourceTagClassV39::NotTagged
            );
            assert_eq!(
                slots.tag_contract(option, out)?.class(out)?,
                SourceTagClassV39::PointerNullReference {
                    terminal: reference
                }
            );
            slots.emit_source_tag_contracts(0, out)?;
            assert!(out.text.contains("owner: 0, rows: Map::empty()"));
            assert!(
                out.text
                    .contains("MemoryTagEncodingV38::PointerNullNiche { nonnull: 1, null: 0 }")
            );
            assert!(out.text.contains("untagged_valid_bits: seq![], encoding:"));
            assert!(!out.text.contains("memory_value_modulus"));
            assert!(!out.text.contains("byte_target_view_contracts"));
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn source_tag_contracts_reject_out_of_roster_coordinates_without_rebinding_valid_rows() {
    run(LIMIT, LIMIT, false, |slots, reference, option, out| {
        assert!(
            slots
                .tag_contract(TypeId::from_index(u32::MAX), out)
                .is_err()
        );
        assert_eq!(
            slots.tag_contract(option, out)?.class(out)?,
            SourceTagClassV39::PointerNullReference {
                terminal: reference
            }
        );
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn source_tag_contracts_keep_exact_and_one_short_work_and_storage() {
    let execute = |work, storage| {
        run(work, storage, false, |slots, _, option, out| {
            slots.tag_contract(option, out)?.class(out)?;
            slots.emit_source_tag_contracts(0, out)
        })
    };
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    execute(measured.1, measured.3).0.unwrap();
    assert!(matches!(execute(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(execute(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

#[test]
fn source_tag_contracts_do_not_turn_pointer_shape_or_width_into_reference_authority() {
    run(LIMIT, LIMIT, false, |slots, reference, option, out| {
        let original = slots
            .relation
            .source(out.budget)?
            .source_semantic(out.budget)?;
        // These inert mutations exercise the classifier, not semantic admission.
        for (kind, width, address_space, metadata) in [
            (PointerKind::Raw, 64, 0, Metadata::None),
            (PointerKind::Reference, 32, 0, Metadata::None),
            (PointerKind::Reference, 64, 1, Metadata::None),
            (PointerKind::Reference, 64, 0, Metadata::SliceLength),
        ] {
            let mut types = original.types().to_vec();
            let row = &types[reference.index() as usize];
            types[reference.index() as usize] = Declaration::new(
                row.identity(),
                row.layout_identity(),
                row.layout().clone(),
                Shape::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        TypeId::from_index(0),
                        kind,
                        SemanticMutabilityV1::Immutable,
                        address_space,
                        width,
                        metadata,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(row.abi_properties());
            assert_eq!(
                classify(&types, &types[option.index() as usize], out)?,
                SourceTagClassV39::Unsupported
            );
        }
        assert_eq!(
            slots.tag_contract(option, out)?.class(out)?,
            SourceTagClassV39::PointerNullReference {
                terminal: reference
            }
        );
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn source_tag_contracts_retain_undercut_failure_after_credit_restoration() {
    let result = run(LIMIT, LIMIT, false, |slots, _, option, out| {
        let recipe = slots.tag_contract(option, out)?;
        let before = out.budget.storage();
        let credit = before - slots.required + 1;
        out.budget.release_storage(credit)?;
        assert!(matches!(
            recipe.class(out),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        out.budget.reserve_storage(credit)?;
        assert_eq!(out.budget.storage(), before);
        assert!(matches!(
            slots.tag_contract(option, out),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        slots.emit_source_tag_contracts(0, out)
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn source_tag_contracts_keep_borrowed_recipe_and_minimal_index_fields() {
    assert_eq!(
        size_of::<SourceTagIndexV39>(),
        size_of::<Vec<SourceTagClassV39>>()
    );
    assert_eq!(
        size_of::<SourceTagRecipeV39<'_, '_, '_>>(),
        size_of::<(&SourceSlots<'_, '_>, TypeId)>()
    );
}

#[test]
fn source_tag_contracts_sign_extend_direct_discriminants_without_numeric_pointer_bits() {
    let signed8 = Scalar::Integer {
        signed: true,
        bits: 8,
    };
    assert_eq!(
        direct_tag(signed8, Primitive::integer(true, 16, 2), 255).unwrap(),
        65535
    );
    assert_eq!(
        direct_tag(signed8, Primitive::integer(true, 8, 1), 128).unwrap(),
        128
    );
    assert!(direct_tag(signed8, Primitive::integer(false, 8, 1), 255).is_err());
    assert!(direct_tag(signed8, Primitive::pointer(0, 8, 8), 1).is_err());
    assert!(
        direct_tag(
            Scalar::Integer {
                signed: false,
                bits: 32
            },
            Primitive::integer(false, 8, 1),
            256
        )
        .is_err()
    );
}

#[test]
fn source_tag_contracts_keep_char_holes_and_original_wrapping_validity() {
    run(LIMIT, LIMIT, false, |_, _, _, out| {
        let before = out.text.len();
        emit_validity(
            &Shape::Scalar(Scalar::Char),
            Validity::new(0, 0x10ffff),
            u128::from(u32::MAX),
            out,
        )?;
        assert_eq!(
            &out.text[before..],
            "(0int,55295int),(57344int,1114111int),"
        );
        let before = out.text.len();
        emit_validity(
            &Shape::Scalar(Scalar::Integer {
                signed: false,
                bits: 8,
            }),
            Validity::new(250, 3),
            255,
            out,
        )?;
        assert_eq!(&out.text[before..], "(0int,3int),(250int,255int),");
        let before = out.text.len();
        let limited = Shape::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                Scalar::Integer {
                    signed: false,
                    bits: 8,
                },
                vec![Validity::new(2, 5), Validity::new(200, 251)],
            )
            .unwrap(),
        );
        emit_validity(&limited, Validity::new(250, 3), 255, out)?;
        assert_eq!(&out.text[before..], "(2int,3int),(250int,251int),");
        Ok(())
    })
    .0
    .unwrap();
}
