use super::*;
use fe2o3_kernel_ir::FormalIndexWidth;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(in super::super::super::super) enum Fixture {
    Direct,
    SignedDirect,
    SharedNull,
    MutableNull,
    SharedSome,
    MutableSome,
}

fn direct(types: &mut Vec<Declaration>, signed: bool) -> (TypeId, TypeId) {
    let word = TypeId::from_index(0);
    let logical = if signed {
        let id = TypeId::from_index(types.len() as u32);
        types.push(Declaration::new(
            SemanticTypeIdentityV1::from_sha256([220; 32]),
            SemanticLayoutIdentityV1::from_sha256([220; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                Backend::scalar(SemanticBackendScalarV1::initialized(
                    Primitive::integer(true, 32, 4),
                    Validity::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            Shape::Scalar(Scalar::Integer {
                signed: true,
                bits: 32,
            }),
        ));
        id
    } else {
        word
    };
    let id = TypeId::from_index(types.len() as u32);
    let tag = SemanticBackendScalarV1::initialized(
        Primitive::integer(signed, 8, 1),
        Validity::new(0, 255),
    );
    let layout = EnumLayout::new(
        (0..2)
            .map(|variant| {
                SemanticEnumVariantLayoutV1::from_rustc(
                    variant,
                    8,
                    4,
                    Fields::arbitrary(vec![4], vec![0]).unwrap(),
                    Backend::memory(true),
                    None,
                    false,
                    None,
                    4,
                    0,
                    SemanticAggregateLayoutV1::new(
                        vec![4],
                        vec![SemanticPaddingV1::new(1, 3).unwrap()],
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect(),
        Encoding::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
    )
    .unwrap();
    types.push(Declaration::new(
        SemanticTypeIdentityV1::from_sha256([221; 32]),
        SemanticLayoutIdentityV1::from_sha256([221; 32]),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            4,
            Backend::memory(true),
            false,
            layout,
        )
        .unwrap(),
        Shape::enum_type(
            logical,
            [if signed { u32::MAX.into() } else { 3 }, 7]
                .into_iter()
                .map(|value| Variant::new(value, SemanticAggregateTypeV1::new(vec![word]).unwrap()))
                .collect(),
        )
        .unwrap(),
    ));
    (id, logical)
}

pub(in super::super::super::super) fn transform(
    types: &mut Vec<Declaration>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    fixture: Fixture,
) -> (TypeId, TypeId) {
    let word = TypeId::from_index(0);
    let (enumeration, logical) = match fixture {
        Fixture::Direct | Fixture::SignedDirect => {
            direct(types, matches!(fixture, Fixture::SignedDirect))
        }
        Fixture::SharedNull | Fixture::MutableNull | Fixture::SharedSome | Fixture::MutableSome => {
            let (_, enumeration) = super::super::tests::append_reference_option(
                types,
                matches!(fixture, Fixture::MutableNull | Fixture::MutableSome),
            );
            (enumeration, word)
        }
    };
    let unrelated = TypeId::from_index(types.len() as u32);
    let declaration = &types[enumeration.index() as usize];
    types.push(Declaration::new(
        SemanticTypeIdentityV1::from_sha256([250; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        declaration.layout().clone(),
        declaration.shape().clone(),
    ));
    let raw = TypeId::from_index(types.len() as u32);
    types.push(Declaration::new(
        SemanticTypeIdentityV1::from_sha256([251; 32]),
        SemanticLayoutIdentityV1::from_sha256([251; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            Backend::scalar(SemanticBackendScalarV1::initialized(
                Primitive::pointer(0, 8, 8),
                Validity::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                enumeration,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let helper = functions.last_mut().unwrap();
    let source = helper.source();
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 4);
    locals.extend([
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([251; 32]),
            enumeration,
            SemanticLocalRoleV1::Temporary,
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([252; 32]),
            logical,
            SemanticLocalRoleV1::Temporary,
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([253; 32]),
            unrelated,
            SemanticLocalRoleV1::Temporary,
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([254; 32]),
            raw,
            SemanticLocalRoleV1::Temporary,
            source,
        ),
    ]);
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assignment = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let operands = if matches!(fixture, Fixture::Direct | Fixture::SignedDirect) {
        vec![SemanticOperandV1::Copy(place(1, word))]
    } else {
        vec![]
    };
    let mut statements = helper.blocks()[0].statements().to_vec();
    let selected = matches!(fixture, Fixture::SharedSome | Fixture::MutableSome);
    let operands = if selected {
        let Shape::Enum { variants, .. } = types[enumeration.index() as usize].shape() else {
            panic!("reference option expected")
        };
        let reference = variants[1].fields().fields()[0];
        let raw_word = TypeId::from_index(types.len() as u32);
        types.push(Declaration::new(
            SemanticTypeIdentityV1::from_sha256([252; 32]),
            SemanticLayoutIdentityV1::from_sha256([252; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                Backend::scalar(SemanticBackendScalarV1::initialized(
                    Primitive::pointer(0, 8, 8),
                    Validity::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    word,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        for (ordinal, ty) in [word, reference, raw_word].into_iter().enumerate() {
            let mut identity = [255; 32];
            identity[31] = ordinal as u8;
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity),
                ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(
                    8 + ordinal as u32,
                )),
            ));
        }
        statements.extend([
            assignment(
                8,
                word,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, word))),
            ),
            assignment(
                10,
                raw_word,
                SemanticRvalueKindV1::AddressOf {
                    place: place(8, word),
                    mutability: SemanticMutabilityV1::Mutable,
                },
            ),
            assignment(
                9,
                reference,
                SemanticRvalueKindV1::Borrow {
                    kind: if matches!(fixture, Fixture::MutableSome) {
                        SemanticBorrowKindV1::Mutable
                    } else {
                        SemanticBorrowKindV1::Shared
                    },
                    place: place(8, word),
                },
            ),
        ]);
        vec![if matches!(fixture, Fixture::MutableSome) {
            SemanticOperandV1::Move(place(9, reference))
        } else {
            SemanticOperandV1::Copy(place(9, reference))
        }]
    } else {
        operands
    };
    statements.push(SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(4)),
    ));
    if selected {
        statements.push(assignment(
            4,
            enumeration,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::EnumVariant(0), vec![])
                    .unwrap(),
            ),
        ));
    }
    statements.extend([
        assignment(
            4,
            enumeration,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(if selected { 1 } else { 0 }),
                    operands,
                )
                .unwrap(),
            ),
        ),
        // Taking the original address requires genuine typed object storage;
        // a known-variant SSA value alone can fold the discriminant away.
        assignment(
            7,
            raw,
            SemanticRvalueKindV1::AddressOf {
                place: place(4, enumeration),
                mutability: SemanticMutabilityV1::Mutable,
            },
        ),
        assignment(
            5,
            logical,
            SemanticRvalueKindV1::Discriminant(place(4, enumeration)),
        ),
        assignment(
            0,
            word,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: SemanticOperandV1::Copy(place(5, logical)),
            },
        ),
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(7)),
        ),
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)),
        ),
    ]);
    if selected {
        let dead_enum = statements.pop().unwrap();
        let dead_raw = statements.pop().unwrap();
        statements.push(assignment(
            4,
            enumeration,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::EnumVariant(0), vec![])
                    .unwrap(),
            ),
        ));
        statements.extend([dead_raw, dead_enum]);
        for local in [9, 10, 8] {
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)),
            ));
        }
    }
    let blocks = vec![
        SemanticBasicBlockV1::new(
            helper.blocks()[0].identity(),
            source,
            statements,
            helper.blocks()[0].terminator().clone(),
        )
        .unwrap(),
    ];
    *helper = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        helper.abi().clone(),
        locals,
        helper.entry(),
        blocks,
    )
    .unwrap();
    (enumeration, unrelated)
}

fn run(
    fixture: Fixture,
    work: usize,
    storage: usize,
    examine: impl FnOnce(&SourceSlots<'_, '_>, TypeId, TypeId, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    let ids = std::cell::Cell::new(None);
    super::super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| ids.set(Some(transform(types, functions, fixture))),
        |plan, out| {
            super::super::super::super::source_function::tests::with_slots(
                plan,
                out,
                |slots, out| {
                    let (enumeration, unrelated) = ids.get().unwrap();
                    let original = plan.source(out)?.source_semantic(out.budget)?;
                    let mut objects = [0; 2];
                    for root in 0..2 {
                        for instance in 0..plan.root(root, out)?.instances.len() {
                            let row = plan.instance(root, instance, out)?;
                            if row.active
                                && original.functions()[row.function.index() as usize]
                                    .locals()
                                    .get(4)
                                    .is_some_and(|local| local.ty() == enumeration)
                            {
                                assert!(
                                    slots.has_original_object(root, instance, 4, out)?,
                                    "original address-taken enum needs a retained typed object"
                                );
                                assert!(
                                    !slots.has_original_object(root, instance, 6, out)?,
                                    "same-layout unrelated nominal enum is not the source object"
                                );
                                objects[root] += 1;
                            }
                        }
                    }
                    assert_eq!(objects, [2, 2], "two real helper invocations in each root");
                    examine(slots, enumeration, unrelated, out)
                },
            )
        },
    )
}

#[test]
fn source_tag_pairs_bind_generic_encoded_some_references_to_the_selected_private_space() {
    for fixture in [Fixture::SharedSome, Fixture::MutableSome] {
        run(fixture, LIMIT, LIMIT, |slots, enumeration, _, out| {
            let inventory = slots.relation.inventory(out.budget)?;
            let contracts = TargetContracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
            let pairs = SourceTagPairsV40::derive(slots, &contracts, out)?;
            let mut found = 0;
            for pair in &pairs.pairs {
                if pair.source != enumeration {
                    continue;
                }
                let row = &inventory.owner().module().storage_layouts[pair.physical.0 as usize];
                let Kind::Variants { encoding, .. } = &row.kind else {
                    panic!("enum row expected");
                };
                let Kind::Pointer(pointer) = inventory.owner().module().storage_layouts
                    [encoding.tag().layout.0 as usize]
                    .kind
                else {
                    panic!("pointer tag expected");
                };
                assert_eq!(pointer.encoded_space, AddressSpace::Generic);
                assert_eq!(pointer.value_space, AddressSpace::Private);
                assert_eq!(pointer.stored_bits, 64);
                assert_eq!(
                    pointer.access,
                    if matches!(fixture, Fixture::MutableSome) {
                        fe2o3_kernel_ir::AccessMode::ReadWrite
                    } else {
                        fe2o3_kernel_ir::AccessMode::ReadOnly
                    }
                );
                assert_eq!(pair.selected_space, Some(AddressSpace::Private));
                pairs.require_pair(enumeration, pair.physical, out)?;
                found += 1;
            }
            assert!(found > 0);
            Ok(())
        })
        .0
        .unwrap();
    }
}

fn with_selected_niche_v42(
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
    examine: impl FnOnce(
        &ObjectRecipe<'_, '_>,
        usize,
        SchemaRole,
        &fe2o3_lower_mir_kernel::ProductionSourceSelectedPointerNicheV42<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> Result<()> {
    let source = slots.relation.source(out.budget)?;
    for root in 0..source.root_count(out.budget)? {
        for instance in 0..source.instance_count(root, out.budget)? {
            for anchor in 0..source.memory_anchor_count(root, instance, out.budget)? {
                let Some(recipe) = slots
                    .relation
                    .memory_object_recipe_v39(root, instance, anchor, out.budget)?
                else {
                    continue;
                };
                for ordinal in 0..recipe.endpoint_count(out.budget)? {
                    for component in [SchemaRole::Root, SchemaRole::Projected] {
                        if let Some(selected) =
                            recipe.selected_pointer_niche_v42(ordinal, component, out.budget)?
                        {
                            selected.check_binding(&recipe, ordinal, component, out.budget)?;
                            return examine(&recipe, ordinal, component, &selected, out);
                        }
                    }
                }
            }
        }
    }
    panic!("genuine selected pointer niche endpoint not reached");
}

#[test]
fn selected_pointer_niche_binding_rejects_endpoint_and_role_substitution_stickily() {
    for wrong_role in [false, true] {
        let (result, _, _, _) = run(Fixture::SharedSome, LIMIT, LIMIT, |slots, _, _, out| {
            with_selected_niche_v42(slots, out, |recipe, ordinal, component, selected, out| {
                let foreign_ordinal = if wrong_role { ordinal } else { ordinal + 1 };
                let foreign_component = if wrong_role {
                    match component {
                        SchemaRole::Root => SchemaRole::Projected,
                        SchemaRole::Projected => SchemaRole::Root,
                    }
                } else {
                    component
                };
                assert!(
                    selected
                        .check_binding(recipe, foreign_ordinal, foreign_component, out.budget)
                        .is_err()
                );
                assert!(
                    selected
                        .check_binding(recipe, ordinal, component, out.budget)
                        .is_err()
                );
                assert!(selected.pointer(out.budget).is_err());
                Ok(())
            })
        });
        assert!(result.is_err(), "binding failure must prevent publication");
    }
}

#[test]
fn selected_pointer_niche_guard_observes_transient_credit_undercut_before_restore() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let (result, _, _, _) = run(Fixture::SharedSome, LIMIT, LIMIT, |slots, _, _, out| {
        with_selected_niche_v42(slots, out, |_, _, _, selected, out| {
            out.budget.release_storage(1)?;
            let first = selected.pointer(out.budget);
            out.budget.reserve_storage(1)?;
            assert!(matches!(
                first,
                Err(SourceError::Resource(Resource::Accounting))
            ));
            assert!(matches!(
                selected.pointer(out.budget),
                Err(SourceError::Resource(Resource::Accounting))
            ));
            Ok(())
        })
    });
    assert!(
        result.is_err(),
        "restoring credits must not restore owner authority"
    );
}

#[test]
fn selected_pointer_niche_binding_rejects_the_same_nominal_contract_from_another_root() {
    let result = run(Fixture::SharedSome, LIMIT, LIMIT, |slots, _, _, out| {
        with_selected_niche_v42(slots, out, |recipe, ordinal, component, selected, out| {
            let source = slots.relation.source(out.budget)?;
            assert!(source.root_count(out.budget)? > 1);
            let original = recipe.original_operation(out.budget)?;
            let expected = selected.types_and_schema(out.budget)?;
            let expected_pointer = selected.pointer(out.budget)?;
            for instance in 0..source.instance_count(1, out.budget)? {
                for anchor in 0..source.memory_anchor_count(1, instance, out.budget)? {
                    let Some(other) = slots
                        .relation
                        .memory_object_recipe_v39(1, instance, anchor, out.budget)?
                    else {
                        continue;
                    };
                    if other.original_operation(out.budget)? == original {
                        continue;
                    }
                    if ordinal >= other.endpoint_count(out.budget)? {
                        continue;
                    }
                    let Some(other_selection) =
                        other.selected_pointer_niche_v42(ordinal, component, out.budget)?
                    else {
                        continue;
                    };
                    let other_types = other_selection.types_and_schema(out.budget)?;
                    if (other_types.0, other_types.1) != (expected.0, expected.1) {
                        continue;
                    }
                    let other_pointer = other_selection.pointer(out.budget)?;
                    assert_eq!(
                        (
                            other_pointer.value_space,
                            other_pointer.encoded_space,
                            other_pointer.stored_bits,
                            other_pointer.access
                        ),
                        (
                            expected_pointer.value_space,
                            expected_pointer.encoded_space,
                            expected_pointer.stored_bits,
                            expected_pointer.access
                        )
                    );
                    assert!(
                        selected
                            .check_binding(&other, ordinal, component, out.budget)
                            .is_err()
                    );
                    assert!(
                        selected
                            .check_binding(recipe, ordinal, component, out.budget)
                            .is_err()
                    );
                    return Ok(());
                }
            }
            panic!("second genuine root object not reached");
        })
    });
    assert!(result.0.is_err());
}

#[test]
fn selected_pointer_niche_rejects_funded_foreign_ledger_before_any_work() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let result = run(Fixture::SharedSome, LIMIT, LIMIT, |slots, _, _, out| {
        with_selected_niche_v42(slots, out, |_, _, _, selected, out| {
            let mut work = Work::new(LIMIT);
            let mut foreign = Budget::new(&mut work, LIMIT);
            foreign.reserve_storage(out.budget.storage())?;
            let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
            assert!(matches!(
                selected.pointer(&mut foreign),
                Err(SourceError::Resource(Resource::Accounting))
            ));
            assert_eq!(
                (foreign.work(), foreign.storage(), foreign.peak_storage()),
                before
            );
            assert!(matches!(
                selected.pointer(out.budget),
                Err(SourceError::Resource(Resource::Accounting))
            ));
            Ok(())
        })
    });
    assert!(result.0.is_err());
}

#[test]
fn selected_pointer_tag_pairs_have_exact_and_one_short_cumulative_resources() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let execute = |work, storage| {
        run(Fixture::MutableSome, work, storage, |slots, _, _, out| {
            let contracts = TargetContracts::derive(
                slots.relation.inventory(out.budget)?,
                FormalIndexWidth::Bits64,
                out,
            )?;
            SourceTagPairsV40::derive(slots, &contracts, out)?.emit(0, 1, out)
        })
    };
    let (result, work, _, storage) = execute(LIMIT, LIMIT);
    result.unwrap();
    let exact = execute(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (work, storage));
    for (w, s, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let refused = execute(w, s);
        assert!(
            matches!((is_work, &refused.0), (true, Err(Error::Source(SourceError::Resource(Resource::Work(error))))) if error.limit() == w && error.actual() == work)
                || matches!((is_work, &refused.0), (false, Err(Error::Source(SourceError::Resource(Resource::Storage(error))))) if error.limit() == s && error.actual() == storage),
            "{:?}",
            refused.0
        );
        assert!(refused.1 <= w && refused.3 <= s);
    }
}

#[test]
fn source_tag_pairs_cover_genuine_direct_signed_and_null_niche_operations() {
    for fixture in [
        Fixture::Direct,
        Fixture::SignedDirect,
        Fixture::SharedNull,
        Fixture::MutableNull,
    ] {
        run(
            fixture,
            LIMIT,
            LIMIT,
            |slots, enumeration, unrelated, out| {
                let inventory = slots.relation.inventory(out.budget)?;
                let contracts = TargetContracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                let pairs = SourceTagPairsV40::derive(slots, &contracts, out)?;
                assert!(!pairs.pairs.is_empty());
            assert!(
                pairs.operations.len() >= 2,
                "genuine enum tag operations must survive original lowering"
            );
            assert_eq!(pairs.operations.len(), inventory.operations().iter().filter(|row|
                matches!(row.operation.kind, OperationKind::Storage(operation) if is_tag_operation(operation))).count());
                assert!(
                    pairs
                        .operations
                        .iter()
                        .all(|operation| operation.source == Some(enumeration))
                );
                for pair in &pairs.pairs {
                    pairs.require_pair(pair.source, pair.physical, out)?;
                    assert!(pairs.require_pair(unrelated, pair.physical, out).is_err());
                }
                slots.emit_source_tag_contracts(0, out)?;
                contracts.emit(1, out)?;
            pairs.emit(0, 1, out)?;
            assert!(pairs.emit(0, 0, out).is_err());
                assert!(
                    out.text
                        .contains("invocation_source_target_tag_pair_0_1_v40")
                );
                assert!(
                    !out.text
                        .contains(&format!("source_type == {}int", unrelated.index()))
                );
                assert!(!out.text.contains("assume("));
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn source_tag_pair_completion_refuses_one_missing_actual_operation_recipe() {
    run(Fixture::Direct, LIMIT, LIMIT, |slots, _, _, out| {
        let contracts = TargetContracts::derive(
            slots.relation.inventory(out.budget)?,
            FormalIndexWidth::Bits64,
            out,
        )?;
        let mut pairs = SourceTagPairsV40::derive(slots, &contracts, out)?;
        assert!(pairs.operations.len() >= 2);
        let original = pairs.operations[0].source.take();
        assert!(matches!(
            SourceTagPairsV40::complete(&pairs.pairs, &pairs.operations, out),
            Err(Error::Statement(
                "actual tag operation lacks an authenticated original object recipe"
            ))
        ));
        pairs.operations[0].source = original;
        SourceTagPairsV40::complete(&pairs.pairs, &pairs.operations, out)
    })
    .0
    .unwrap();
}

#[test]
fn source_tag_pairs_refuse_a_same_bytes_foreign_canonical_owner() {
    use fe2o3_kernel_ir::{StorageLayoutLimitsV1, VerifiedCanonicalKernelIrModuleV18};
    run(Fixture::Direct, LIMIT, LIMIT, |slots, _, _, out| {
        let original = slots.relation.inventory(out.budget)?;
        let (other, storage) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                original.owner().module(),
                StorageLayoutLimitsV1 {
                    rows: 128,
                    edges: 512,
                    containment_depth: 64,
                    object_bytes: 4096,
                },
                out.budget,
            )
            .unwrap();
        out.budget.reserve_storage(storage.retained_storage())?;
        let (inventory, storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(&other, out.budget)?;
        out.budget.reserve_storage(storage.retained_storage())?;
        assert_eq!(original.owner().module(), other.module());
        assert!(!std::ptr::eq(original.owner(), &other));
        let contracts = TargetContracts::derive(&inventory, FormalIndexWidth::Bits64, out)?;
        let before = out.text.len();
        assert!(SourceTagPairsV40::derive(slots, &contracts, out).is_err());
        assert_eq!(out.text.len(), before);
        let genuine = TargetContracts::derive(original, FormalIndexWidth::Bits64, out)?;
        SourceTagPairsV40::derive(slots, &genuine, out)?.check(out)
    })
    .0
    .unwrap();
}

#[test]
fn source_tag_pairs_retain_undercut_failure_after_the_budget_is_restored() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let result = run(Fixture::SharedNull, LIMIT, LIMIT, |slots, _, _, out| {
        let contracts = TargetContracts::derive(
            slots.relation.inventory(out.budget)?,
            FormalIndexWidth::Bits64,
            out,
        )?;
        let pairs = SourceTagPairsV40::derive(slots, &contracts, out)?;
        pairs.check(out)?;
        let before = (out.budget.work(), out.budget.storage(), out.text.len());
        out.budget.release_storage(1)?;
        assert!(matches!(
            pairs.check(out),
            Err(Error::Resource(Resource::Accounting))
        ));
        out.budget.reserve_storage(1)?;
        assert!(matches!(
            pairs.check(out),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(out.budget.storage(), before.1);
        assert_eq!(out.text.len(), before.2);
        pairs.emit(0, 1, out)
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn source_tag_pairs_reject_funded_foreign_ledger_without_rebinding_the_source() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let result = run(Fixture::Direct, LIMIT, LIMIT, |slots, _, _, out| {
        let contracts = TargetContracts::derive(
            slots.relation.inventory(out.budget)?,
            FormalIndexWidth::Bits64,
            out,
        )?;
        let pairs = SourceTagPairsV40::derive(slots, &contracts, out)?;
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(out.budget.storage())?;
        let before = (budget.work(), budget.storage(), budget.peak_storage());
        {
            let mut foreign = Writer::new(&mut budget)?;
            assert!(matches!(
                pairs.emit(0, 1, &mut foreign),
                Err(Error::Source(SourceError::Resource(Resource::Accounting)))
            ));
            assert!(foreign.text.is_empty());
        }
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            before
        );
        let primary = (out.budget.work(), out.budget.storage(), out.text.len());
        let refused = pairs.check(out);
        assert_eq!(
            (out.budget.work(), out.budget.storage(), out.text.len()),
            primary
        );
        refused
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn source_tag_pairs_have_exact_and_one_short_cumulative_resources() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let execute = |work, storage| {
        run(Fixture::SignedDirect, work, storage, |slots, _, _, out| {
            let contracts = TargetContracts::derive(
                slots.relation.inventory(out.budget)?,
                FormalIndexWidth::Bits64,
                out,
            )?;
            SourceTagPairsV40::derive(slots, &contracts, out)?.emit(0, 1, out)
        })
    };
    let (result, work, _, storage) = execute(LIMIT, LIMIT);
    result.unwrap();
    let exact = execute(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (work, storage));
    for (w, s, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let refused = execute(w, s);
        assert!(
            matches!((is_work, &refused.0),
            (true, Err(Error::Source(SourceError::Resource(Resource::Work(error))))) if error.limit() == w && error.actual() == work)
                || matches!((is_work, &refused.0),
                (false, Err(Error::Source(SourceError::Resource(Resource::Storage(error))))) if error.limit() == s && error.actual() == storage),
            "{:?}",
            refused.0
        );
        assert!(refused.1 <= w && refused.3 <= s);
    }
}

#[test]
fn source_tag_pair_lookup_uses_a_bounded_sorted_index_without_source_rescans() {
    run(Fixture::Direct, LIMIT, LIMIT, |_, _, _, out| {
        for size in [1usize, 16, 256] {
            let rows: Vec<_> = (0..size)
                .map(|value| Pair {
                    source: TypeId::from_index(value as u32),
                    physical: Id(value as u32),
                    selected_space: None,
                })
                .collect();
            let floor = out.budget.storage();
            let before = out.budget.work();
            let key = rows[size - 1];
            for _ in 0..64 {
                assert!(pair_present(&rows, (key.source, key.physical), out)?);
            }
            assert_eq!(out.budget.storage(), floor);
            let height = size.ilog2() as usize + 1;
            assert!(out.budget.work() - before <= 64 * (height + 1));
            assert!(!pair_present(
                &rows,
                (TypeId::from_index(size as u32), Id(0)),
                out
            )?);
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn source_tag_pair_headers_match_independent_retained_fields_and_frames() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    type PairFields = (TypeId, Id, Option<AddressSpace>);
    type OperationFields = (Operation, usize, Id, Option<TypeId>);
    type Fields<'a, 'view, 'source, 'inventory, 'owner> = (
        &'a SourceSlots<'view, 'source>,
        &'a TargetContracts<'inventory, 'owner>,
        Vec<Pair>,
        Vec<TagOperation>,
        usize,
    );
    assert_eq!(size_of::<Pair>(), size_of::<PairFields>());
    assert_eq!(size_of::<TagOperation>(), size_of::<OperationFields>());
    assert_eq!(
        size_of::<SourceTagPairsV40<'_, '_, '_, '_, '_>>(),
        size_of::<Fields<'_, '_, '_, '_, '_>>()
    );
    type LocalFrames<'a> = (
        &'a SourceSlots<'a, 'a>,
        &'a TargetContracts<'a, 'a>,
        &'a mut Writer<'a, 'a>,
        &'a crate::mixed_optimizer_refinement_v26::semantics::Inventory<'a>,
        &'a [Declaration],
        &'a [Layout],
        [&'a Layout; 4],
        &'a Declaration,
        &'a EnumLayout,
        std::slice::Iter<'a, Pair>,
        std::slice::Iter<'a, TagOperation>,
        std::iter::Zip<
            std::iter::Zip<
                std::slice::Iter<'a, Variant>,
                std::slice::Iter<'a, SemanticEnumVariantLayoutV1>,
            >,
            std::slice::Iter<'a, fe2o3_kernel_ir::StorageVariantV1>,
        >,
        std::array::IntoIter<(SchemaRole, TypeId, Id), 2>,
        [usize; 24],
        [u64; 4],
        Option<Scalar>,
        PhysicalEncoding,
        Primitive,
    );
    let expected = h::<Fields<'_, '_, '_, '_, '_>>()
        + h::<ObjectRecipe<'_, '_>>()
        + h::<Option<ObjectRecipe<'_, '_>>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectEndpointV39<'_, '_>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceSelectedPointerNicheV42<'_, '_>>()
        + h::<Option<fe2o3_lower_mir_kernel::ProductionSourceSelectedPointerNicheV42<'_, '_>>>()
        + h::<SchemaRole>()
        + h::<Option<AddressSpace>>()
        + h::<fe2o3_kernel_ir::StoragePointerV1>()
        + h::<(TypeId, TypeId, Id)>()
        + h::<Vec<Pair>>()
        + h::<Vec<TagOperation>>()
        + h::<PairFields>()
        + h::<OperationFields>()
        + h::<SourceTagRecipeV39<'_, '_, '_>>()
        + h::<SourceTagClassV39>()
        + h::<TargetClass>()
        + h::<EndpointRole>()
        + h::<(TypeId, TypeId)>()
        + h::<(Id, Id)>()
        + h::<Operation>()
        + h::<(Storage, Option<fe2o3_kernel_ir::ValueId>)>()
        + h::<bool>()
        + h::<usize>()
        + h::<()>()
        + size_of::<LocalFrames<'_>>();
    assert_eq!(headers(), expected);
}
