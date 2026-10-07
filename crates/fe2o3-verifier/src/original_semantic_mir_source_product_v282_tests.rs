//! Schema classification is not construction, copyability, lease, or proof authority.
use super::super::super::invocations::tests::run_source_transform;
use super::super::source_function::tests::with_slots;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
use source_products::SourceProductTypesV282;
use std::cell::Cell;

const LIMIT: usize = 100_000_000;
type Type = SemanticTypeDeclV1;
type TypeId = SemanticTypeIdV1;
type Shape = SemanticTypeShapeV1;
type Projection = SemanticProjectionKindV1;

#[derive(Clone, Copy)]
struct Rows {
    word: TypeId,
    pointer: TypeId,
    mutable: TypeId,
    reference: TypeId,
    mutable_reference: TypeId,
    pair: TypeId,
    array: TypeId,
    outer: TypeId,
    empty: TypeId,
    zero_array: TypeId,
}

fn append(types: &mut Vec<Type>, layout: SemanticTypeLayoutV1, shape: Shape) -> TypeId {
    let index = u32::try_from(types.len()).unwrap();
    let mut identity = [230; 32];
    identity[28..].copy_from_slice(&index.to_be_bytes());
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256(identity),
        SemanticLayoutIdentityV1::from_sha256(identity),
        layout,
        shape,
    ));
    TypeId::from_index(index)
}

fn tuple(types: &mut Vec<Type>, fields: Vec<TypeId>, bytes: u64, offsets: Vec<u64>) -> TypeId {
    append(
        types,
        SemanticTypeLayoutV1::aggregate(
            Some(bytes),
            8,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        Shape::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn array_layout(stride: u64, length: u64) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        stride * length,
        8,
        SemanticFieldsShapeV1::array(stride, length),
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::memory(true),
        None,
        false,
        None,
        8,
        0,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap()
}

fn extend(types: &mut Vec<Type>, functions: &mut [SemanticFunctionDeclV1]) -> Rows {
    let word = TypeId::from_index(0);
    assert!(matches!(
        types[0].shape(),
        Shape::Scalar(SemanticScalarTypeV1::Integer { .. })
    ));
    let mut pointer = |kind, mutable| {
        append(
            types,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(
                        u128::from(kind == SemanticPointerKindV1::Reference),
                        u64::MAX.into(),
                    ),
                )),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    word,
                    kind,
                    mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
    };
    let immutable = pointer(SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable);
    let mutable = pointer(SemanticPointerKindV1::Raw, SemanticMutabilityV1::Mutable);
    let reference = pointer(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let mutable_reference = pointer(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
    );
    let pair = tuple(types, vec![immutable, word], 16, vec![0, 8]);
    let array = append(
        types,
        array_layout(16, 3),
        Shape::Array {
            element: pair,
            length: 3,
        },
    );
    let outer = tuple(types, vec![word, array], 56, vec![0, 8]);
    let empty = tuple(types, vec![], 0, vec![]);
    let zero_array = append(
        types,
        array_layout(8, 0),
        Shape::Array {
            element: immutable,
            length: 0,
        },
    );
    // Unused original locals request schemas without asserting a constructor or value exists.
    let function = functions.last_mut().unwrap();
    let mut locals = function.locals().to_vec();
    for (offset, ty) in [
        outer,
        mutable,
        reference,
        mutable_reference,
        empty,
        zero_array,
    ]
    .into_iter()
    .enumerate()
    {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([240 + offset as u8; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            function.source(),
        ));
    }
    *function = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals,
        function.entry(),
        function.blocks().to_vec(),
    )
    .unwrap();
    Rows {
        word,
        pointer: immutable,
        mutable,
        reference,
        mutable_reference,
        pair,
        array,
        outer,
        empty,
        zero_array,
    }
}

fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(Rows, &SourceSlots<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    let rows = Cell::new(None);
    run_source_transform(
        work,
        storage,
        |types, functions| rows.set(Some(extend(types, functions))),
        |plan, out| {
            with_slots(plan, out, |slots, out| {
                examine(rows.get().unwrap(), slots, out)
            })
        },
    )
}

fn project(kind: Projection, ty: TypeId) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(kind, ty).unwrap()
}

fn accounting(error: &Error) -> bool {
    matches!(
        error,
        Error::Resource(Resource::Accounting)
            | Error::Source(SourceError::Resource(Resource::Accounting))
    )
}

#[test]
fn original_product_schema_paths_and_atoms_remain_bound_to_the_original_owner() {
    run(LIMIT, LIMIT, |r, slots, out| {
        assert!(slots.product_type_supported_v282(r.outer, out)?);
        assert_eq!(slots.product_component_count_v282(r.outer, out)?, Some(7));
        assert!(slots.is_product_v282(r.outer, out)?);
        let paths: &[&[u32]] = &[
            &[0],
            &[1, 0, 0],
            &[1, 0, 1],
            &[1, 1, 0],
            &[1, 1, 1],
            &[1, 2, 0],
            &[1, 2, 1],
        ];
        let semantic = slots
            .relation
            .source(out.budget)?
            .source_semantic(out.budget)?;
        for (ordinal, expected) in paths.iter().enumerate() {
            let row = slots.product_component_v282(r.outer, ordinal, out)?;
            assert_eq!(row.path(out)?, *expected);
            let pointer = ordinal % 2 == 1;
            assert_eq!(
                row.source_type(out)?,
                if pointer { r.pointer } else { r.word }
            );
            if pointer {
                assert_eq!(
                    row.atom(out)?,
                    ProductAtomV282::Pointer {
                        mutable: false,
                        reference: false
                    }
                );
            } else {
                assert_eq!(
                    row.atom(out)?,
                    ProductAtomV282::Scalar(super::super::super::ScalarV30::from_source(
                        semantic.types(),
                        r.word
                    )?)
                );
            }
        }
        assert_eq!(
            slots.product_component_v282(r.mutable, 0, out)?.atom(out)?,
            ProductAtomV282::Pointer {
                mutable: true,
                reference: false
            }
        );
        assert!(matches!(
            slots.product_component_v282(r.outer, 7, out),
            Err(Error::Statement(
                "original product component ordinal differs"
            ))
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_pointer_kind_is_not_inferred_from_mutability_or_layout() {
    run(LIMIT, LIMIT, |r, slots, out| {
        let source = slots
            .relation
            .source(out.budget)?
            .source_semantic(out.budget)?;
        for (ty, reference, mutable) in [
            (r.pointer, false, false),
            (r.mutable, false, true),
            (r.reference, true, false),
            (r.mutable_reference, true, true),
        ] {
            let Shape::Pointer(pointer) = source.types()[ty.index() as usize].shape() else {
                panic!("original pointer declaration");
            };
            assert_eq!(
                pointer.kind() == SemanticPointerKindV1::Reference,
                reference
            );
            assert_eq!(
                pointer.mutability() == SemanticMutabilityV1::Mutable,
                mutable
            );
            assert!(slots.product_type_supported_v282(ty, out)?);
            assert_eq!(
                slots.product_component_v282(ty, 0, out)?.atom(out)?,
                ProductAtomV282::Pointer { reference, mutable }
            );
        }
        // No Copy operand is executed by this schema-only check.
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_product_copy_query_uses_original_pointer_kind_and_keeps_its_account() {
    run(LIMIT, LIMIT, |r, slots, out| {
        let floor = out.budget.storage();
        let ledger = out.budget.work_ledger_identity_v1();
        let before = out.budget.work();
        for (ty, expected) in [
            (r.word, true),
            (r.pointer, true),
            (r.mutable, true),
            (r.reference, true),
            (r.mutable_reference, false),
            (r.outer, true),
            (r.empty, true),
            (r.zero_array, true),
        ] {
            assert!(slots.product_type_supported_v282(ty, out)?);
            assert_eq!(slots.product_type_copyable_v282(ty, out)?, expected);
        }
        assert!(out.budget.work() > before);
        assert_eq!(out.budget.storage(), floor);
        assert!(ledger == out.budget.work_ledger_identity_v1());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_nested_ranges_check_indices_and_exact_result_types() {
    run(LIMIT, LIMIT, |r, slots, out| {
        for (offset, from_end, ordinal) in
            [(0, false, 0), (2, false, 2), (1, true, 2), (3, true, 0)]
        {
            let path = [
                project(Projection::Field(1), r.array),
                project(
                    Projection::ConstantIndex {
                        offset,
                        minimum_length: 3,
                        from_end,
                    },
                    r.pair,
                ),
                project(Projection::Field(1), r.word),
            ];
            let first = 2 + ordinal * 2;
            assert_eq!(
                slots.product_component_range_v282(r.outer, &path, out)?,
                Some((first..first + 1, r.word))
            );
        }
        assert_eq!(
            slots.product_component_range_v282(r.outer, &[], out)?,
            Some((0..7, r.outer))
        );
        assert_eq!(
            slots.product_component_range_v282(
                r.outer,
                &[project(Projection::Field(1), r.array)],
                out
            )?,
            Some((1..7, r.array))
        );
        for (offset, minimum_length, from_end) in [(3, 4, false), (4, 4, true), (0, 4, false)] {
            let path = [
                project(Projection::Field(1), r.array),
                project(
                    Projection::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    },
                    r.pair,
                ),
            ];
            assert!(matches!(
                slots.product_component_range_v282(r.outer, &path, out),
                Err(Error::Statement(
                    "original MIR allocation descriptor census differs"
                ))
            ));
        }
        assert!(matches!(
            slots.product_component_range_v282(
                r.outer,
                &[project(Projection::Field(1), r.word)],
                out
            ),
            Err(Error::Statement(
                "original MIR allocation descriptor census differs"
            ))
        ));
        assert_eq!(
            slots.product_component_range_v282(
                r.outer,
                &[project(Projection::Field(2), r.word)],
                out
            )?,
            None
        );
        assert_eq!(
            slots.product_component_range_v282(
                r.outer,
                &[
                    project(Projection::Field(0), r.word),
                    project(Projection::Field(0), r.word)
                ],
                out
            )?,
            None
        );
        for (offset, minimum_length, from_end) in [(3, 3, false), (0, 3, true), (4, 3, true)] {
            assert!(matches!(
                SemanticProjectionV1::new(
                    Projection::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end
                    },
                    r.pair
                ),
                Err(SemanticMirErrorV1::InvalidProjectionShape)
            ));
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_zero_length_composites_are_not_nonplain_products() {
    run(LIMIT, LIMIT, |r, slots, out| {
        for ty in [r.empty, r.zero_array] {
            assert!(slots.product_type_supported_v282(ty, out)?);
            assert!(!slots.is_product_v282(ty, out)?);
            assert_eq!(slots.product_component_count_v282(ty, out)?, None);
            let row = slots.product_component_v282(ty, 0, out)?;
            assert!(row.path(out)?.is_empty());
            assert_eq!(row.source_type(out)?, ty);
            assert_eq!(
                row.atom(out)?,
                ProductAtomV282::Scalar(super::super::super::ScalarV30::Unit)
            );
        }
        let source = slots
            .relation
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let unit = TypeId::from_index(
            source
                .types()
                .iter()
                .position(|ty| matches!(ty.shape(), Shape::Unit))
                .expect("original root unit return") as u32,
        );
        slots.products.emit(out)?;
        // This fragment check distinguishes empty composites from scalar Unit;
        // it is not a generated-program/model acceptance check.
        let composite = out
            .text
            .split_once("spec fn invocation_source_product_composite_v282(ty: int) -> bool {\n")
            .unwrap()
            .1
            .split_once("\n}\n")
            .unwrap()
            .0;
        for ty in [r.empty, r.zero_array] {
            assert!(composite.contains(&format!(" ty == {}int ||", ty.index())));
        }
        assert!(!composite.contains(&format!(" ty == {}int ||", unit.index())));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_nominal_execution_snapshots_are_opaque_not_layout_fields() {
    super::super::source_function::tile_fixture_tests::run_fixture_with_plan(
        fe2o3_kernel_ir::ExecutionTileLayoutV1::Blocked,
        LIMIT,
        LIMIT,
        |plan, slots, _, out| {
            let source = plan.source(out)?.source_semantic(out.budget)?;
            let mut seen = 0;
            let mut unsupported = 0;
            for (index, ty) in source.types().iter().enumerate() {
                if let SemanticRustTypeKindV1::Execution(
                    role @ (SemanticExecutionRoleV29::KernelContext
                    | SemanticExecutionRoleV29::Workgroup),
                ) = ty.rust_type_kind()
                {
                    let id = TypeId::from_index(index as u32);
                    assert!(slots.product_type_supported_v282(id, out)?);
                    assert!(!slots.product_type_copyable_v282(id, out)?);
                    assert!(slots.aggregate_leaf_count(id, out)?.unwrap() > 1);
                    let atom = slots.product_component_v282(id, 0, out)?;
                    assert!(atom.path(out)?.is_empty());
                    assert_eq!(atom.source_type(out)?, id);
                    assert_eq!(atom.atom(out)?, ProductAtomV282::ExecutionAggregate(role));
                    assert!(!slots.is_product_v282(id, out)?);
                    assert_eq!(
                        slots.product_component_range_v282(
                            id,
                            &[project(Projection::Field(0), TypeId::from_index(0))],
                            out
                        )?,
                        None
                    );
                    assert!(matches!(
                        slots.product_component_v282(id, 1, out),
                        Err(Error::Statement(
                            "original product component ordinal differs"
                        ))
                    ));
                    seen += 1;
                } else if matches!(
                    ty.rust_type_kind(),
                    SemanticRustTypeKindV1::Execution(
                        SemanticExecutionRoleV29::MaskedTileU32 { .. }
                            | SemanticExecutionRoleV29::LaneFragmentU32 { .. }
                    )
                ) {
                    let id = TypeId::from_index(index as u32);
                    assert!(!slots.product_type_supported_v282(id, out)?);
                    assert!(!slots.product_type_copyable_v282(id, out)?);
                    assert_eq!(slots.product_component_count_v282(id, out)?, None);
                    assert!(matches!(
                        slots.product_component_v282(id, 0, out),
                        Err(Error::Statement(
                            "original product component ordinal differs"
                        ))
                    ));
                    unsupported += 1;
                }
            }
            assert_eq!(seen, 2);
            assert!(unsupported >= 2);
            assert!(matches!(
                slots.product_type_supported_v282(
                    TypeId::from_index(source.types().len() as u32),
                    out
                ),
                Err(Error::Statement(
                    "original MIR allocation descriptor census differs"
                ))
            ));
            Ok(())
        },
    )
    .0
    .unwrap();
}

// Direct schema mutations below are not source admission. The ABI table has the
// same size and no nominal descriptor rows for the changed ordinary fixture types.
fn derive_mutation(
    r: Rows,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
    mutate: impl FnOnce(Rows, &mut [Type]),
    examine: impl FnOnce(Result<SourceProductTypesV282>, &mut Writer<'_, '_>) -> Result<()>,
) -> Result<()> {
    let mut types = slots
        .relation
        .source(out.budget)?
        .source_semantic(out.budget)?
        .types()
        .to_vec();
    mutate(r, &mut types);
    let requested = vec![true; types.len()];
    let index =
        SourceProductTypesV282::derive(&types, &slots.abi, &slots.aggregates, &requested, out);
    examine(index, out)
}

fn replace(types: &mut [Type], ty: TypeId, shape: Shape) {
    let old = &types[ty.index() as usize];
    types[ty.index() as usize] = Type::new(
        old.identity(),
        old.layout_identity(),
        old.layout().clone(),
        shape,
    );
}

#[test]
fn original_product_schema_shared_dag_is_not_a_cycle() {
    run(LIMIT, LIMIT, |r, slots, out| {
        derive_mutation(
            r,
            slots,
            out,
            |r, types| {
                replace(
                    types,
                    r.outer,
                    Shape::Tuple(SemanticAggregateTypeV1::new(vec![r.pair, r.pair]).unwrap()),
                )
            },
            |index, out| {
                let index = index?;
                assert!(index.is_product(r.outer, out)?);
                assert_eq!(
                    index.component_range(
                        r.outer,
                        &[project(Projection::Field(1), r.pair)],
                        out
                    )?,
                    Some((2..4, r.pair))
                );
                Ok(())
            },
        )
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_unsupported_child_does_not_become_an_empty_supported_product() {
    run(LIMIT, LIMIT, |r, slots, out| {
        derive_mutation(
            r,
            slots,
            out,
            |r, types| replace(types, r.pointer, Shape::Opaque),
            |index, out| {
                let index = index?;
                for ty in [r.pair, r.array, r.outer] {
                    assert!(!index.is_product(ty, out)?);
                    assert_eq!(index.component_range(ty, &[], out)?, None);
                }
                assert!(
                    slots.product_type_supported_v282(r.outer, out)?,
                    "the separate admitted owner's original types were not mutated"
                );
                Ok(())
            },
        )
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_rejects_self_and_mutual_cycles() {
    for mutual in [false, true] {
        run(LIMIT, LIMIT, |r, slots, out| {
            derive_mutation(
                r,
                slots,
                out,
                |r, types| {
                    replace(
                        types,
                        r.outer,
                        Shape::Tuple(
                            SemanticAggregateTypeV1::new(vec![if mutual {
                                r.pair
                            } else {
                                r.outer
                            }])
                            .unwrap(),
                        ),
                    );
                    if mutual {
                        replace(
                            types,
                            r.pair,
                            Shape::Tuple(SemanticAggregateTypeV1::new(vec![r.outer]).unwrap()),
                        );
                    }
                },
                |result, _| {
                    assert!(matches!(
                        result,
                        Err(Error::Statement("original product type graph is cyclic"))
                    ));
                    Ok(())
                },
            )
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_product_schema_refuses_component_explosion_before_payload_allocation() {
    run(LIMIT, LIMIT, |r, slots, out| {
        let before = out.budget.peak_storage();
        derive_mutation(
            r,
            slots,
            out,
            |r, types| {
                replace(
                    types,
                    r.array,
                    Shape::Array {
                        element: r.pointer,
                        length: 1 << 20,
                    },
                )
            },
            |result, out| {
                assert!(matches!(
                    result,
                    Err(Error::Statement(
                        "original product component census exceeds structural limit"
                    ))
                ));
                assert!(out.budget.peak_storage() - before < 1024 * 1024);
                Ok(())
            },
        )
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_rejects_foreign_accounts_and_refunded_owner_floor() {
    for foreign in [false, true] {
        let reached = Cell::new(false);
        let result = run(LIMIT, LIMIT, |r, slots, out| {
            let row = slots.product_component_v282(r.outer, 1, out)?;
            reached.set(true);
            let error = if foreign {
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut other = Writer::new(&mut budget)?;
                row.source_type(&mut other).unwrap_err()
            } else {
                out.budget.release_storage(1)?;
                row.path(out).unwrap_err()
            };
            assert!(accounting(&error));
            assert!(accounting(
                &slots.is_product_v282(r.outer, out).unwrap_err()
            ));
            assert!(accounting(
                &slots.product_type_supported_v282(r.outer, out).unwrap_err()
            ));
            assert!(accounting(
                &slots.product_type_copyable_v282(r.outer, out).unwrap_err()
            ));
            Err(error)
        });
        assert!(reached.get());
        assert!(accounting(&result.0.unwrap_err()));
    }
}

#[test]
fn original_product_schema_prior_resource_denial_stays_sticky() {
    for work_denial in [false, true] {
        let reached = Cell::new(false);
        let result = run(LIMIT, LIMIT, |r, slots, out| {
            reached.set(true);
            let floor = out.budget.storage();
            let ledger = out.budget.work_ledger_identity_v1();
            let row = slots.product_component_v282(r.outer, 1, out)?;
            let mut denied = None;
            let error = slots
                .with_source_query_v42(out, |out| -> Result<()> {
                    let result = if work_denial {
                        out.budget.charge_work(LIMIT)
                    } else {
                        out.budget.reserve_storage(LIMIT)
                    };
                    let resource = result.unwrap_err();
                    denied = Some(resource);
                    Err(Error::Resource(resource))
                })
                .unwrap_err();
            assert!(matches!(
                (&denied, work_denial),
                (Some(Resource::Work(_)), true) | (Some(Resource::Storage(_)), false)
            ));
            let denied = denied.unwrap();
            for refusal in [
                &error,
                &slots.product_type_supported_v282(r.outer, out).unwrap_err(),
                &slots.product_type_copyable_v282(r.outer, out).unwrap_err(),
                &row.path(out).unwrap_err(),
            ] {
                let Error::Source(SourceError::Resource(resource)) = refusal else {
                    panic!("owner must retain the original resource refusal: {refusal:?}");
                };
                assert_eq!(*resource, denied);
            }
            assert!(matches!(
                (&error, work_denial),
                (
                    Error::Source(SourceError::Resource(Resource::Work(_))),
                    true
                ) | (
                    Error::Source(SourceError::Resource(Resource::Storage(_))),
                    false
                )
            ));
            assert_eq!(out.budget.storage(), floor);
            assert!(ledger == out.budget.work_ledger_identity_v1());
            Err(error)
        });
        assert!(reached.get());
        assert!(matches!(
            (result.0, work_denial),
            (
                Err(Error::Source(SourceError::Resource(Resource::Work(_)))),
                true
            ) | (
                Err(Error::Source(SourceError::Resource(Resource::Storage(_)))),
                false
            )
        ));
    }
}

#[test]
fn original_product_schema_equal_byte_owners_do_not_share_correspondence() {
    run(LIMIT, LIMIT, |r, slots, out| {
        let first = slots.relation.source(out.budget)?;
        let first_digest = *first.source_ssa(out.budget)?.source_semantic_sha256();
        run(LIMIT, LIMIT, |other_r, other_slots, other_out| {
            let other = other_slots.relation.source(other_out.budget)?;
            assert_eq!(
                first_digest,
                *other.source_ssa(other_out.budget)?.source_semantic_sha256()
            );
            assert!(!std::ptr::eq(first, other));
            assert_eq!(r.outer, other_r.outer);
            assert!(matches!(
                slots.check_source(other_slots.relation, out),
                Err(Error::Statement(
                    "original MIR allocation descriptor census differs"
                ))
            ));
            assert_eq!(slots.product_component_count_v282(r.outer, out)?, Some(7));
            assert!(slots.product_type_copyable_v282(r.outer, out)?);
            Ok(())
        })
        .0?;
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_product_schema_queries_preserve_account_and_complete_exact_budget() {
    let execute = |work, storage| {
        run(work, storage, |r, slots, out| {
            let floor = out.budget.storage();
            let ledger = out.budget.work_ledger_identity_v1();
            assert_eq!(slots.product_component_count_v282(r.outer, out)?, Some(7));
            assert!(slots.product_type_copyable_v282(r.outer, out)?);
            for ordinal in 0..7 {
                let row = slots.product_component_v282(r.outer, ordinal, out)?;
                row.path(out)?;
                row.source_type(out)?;
                row.atom(out)?;
            }
            assert_eq!(out.budget.storage(), floor);
            assert!(ledger == out.budget.work_ledger_identity_v1());
            Ok(())
        })
    };
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = execute(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert_eq!(measured.2, super::super::super::invocations::tests::FLOOR);
    assert!(matches!(
        execute(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(_))))
            | Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        execute(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(_))))
            | Err(Error::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn original_product_schema_scoped_drop_restores_storage_after_success_error_and_unwind() {
    run(LIMIT, LIMIT, |r, slots, out| {
        for mode in 0..3 {
            let floor = out.budget.storage();
            let ledger = out.budget.work_ledger_identity_v1();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                out.budget
                    .with_prepaid_scope(floor, 1, 1, 0, |budget| -> Result<()> {
                        let mut writer = Writer::new(budget)?;
                        derive_mutation(
                            r,
                            slots,
                            &mut writer,
                            |_, _| {},
                            |index, out| {
                                let index = index?;
                                assert!(index.is_product(r.outer, out)?);
                                match mode {
                                    0 => Ok(()),
                                    1 => Err(Error::Statement("product schema consumer refusal")),
                                    _ => std::panic::panic_any(282u32),
                                }
                            },
                        )
                    })
            }));
            match mode {
                0 => caught.unwrap().unwrap(),
                1 => assert!(matches!(
                    caught.unwrap(),
                    Err(Error::Statement("product schema consumer refusal"))
                )),
                _ => assert_eq!(caught.unwrap_err().downcast_ref::<u32>(), Some(&282)),
            }
            assert_eq!(out.budget.storage(), floor);
            assert!(ledger == out.budget.work_ledger_identity_v1());
            assert_eq!(slots.product_component_count_v282(r.outer, out)?, Some(7));
        }
        Ok(())
    })
    .0
    .unwrap();
}
