use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::collections::BTreeMap;

const LIMIT: usize = 256 * 1024 * 1024;

#[path = "original_semantic_mir_source_scalar_ranges_v45_tests.rs"]
mod scalar_range_tests;

#[path = "original_semantic_mir_source_typed_borrows_v51_tests.rs"]
mod typed_borrow_tests;

fn retained_transform_v44(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    operation: SemanticCheckedBinaryOpV1,
) {
    super::super::super::paired::aggregate_tests::checked_transform(
        types, functions, operation, false, false,
    );
    let old = functions.last_mut().unwrap();
    let pair = old.locals()[4].ty();
    let raw = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([250; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pair,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = old.locals().to_vec();
    assert_eq!(locals.len(), 5);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([250; 32]),
        raw,
        SemanticLocalRoleV1::Temporary,
        old.source(),
    ));
    let mut blocks = old.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    statements.push(SemanticStatementV1::new(
        old.source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], raw).unwrap(),
            SemanticRvalueV1::new(
                raw,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], pair)
                        .unwrap(),
                },
            ),
        )),
    ));
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    let SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Move(condition),
        expected,
        message,
        target,
        unwind,
    } = blocks[1].terminator().kind()
    else {
        panic!("original checked overflow condition");
    };
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        blocks[1].source(),
        blocks[1].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[1].terminator().source(),
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(condition.clone()),
                expected: *expected,
                message: message.clone(),
                target: *target,
                unwind: *unwind,
            },
        ),
    )
    .unwrap();
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
}

fn retained_program_v44(
    operation: SemanticCheckedBinaryOpV1,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| retained_transform_v44(types, functions, operation),
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let original = plan.source(out)?.source_semantic(out.budget)?;
                let pair = original.functions().last().unwrap().locals()[4].ty();
                assert_eq!(
                    slots.aggregate_leaf_count(pair, out)?,
                    None,
                    "retained objects must not require promoted component state"
                );
                let contract = slots.checked_object_type_v47(pair, out)?.unwrap();
                assert_eq!(
                    (
                        contract.scalar,
                        contract.bytes,
                        contract.alignment,
                        contract.offsets
                    ),
                    (
                        ScalarV30::Integer {
                            width: 32,
                            signed: false
                        },
                        8,
                        4,
                        [0, 4]
                    )
                );
                for root in 0..2 {
                    for instance in 1..=2 {
                        assert!(slots.has_original_object(root, instance, 4, out)?);
                    }
                }
                let mut program = super::super::super::source_function::SourceByteProgram::derive(
                    plan, slots, out,
                )?;
                slots.emit(out)?;
                program.emit(out)?;
                assert_eq!(out.text.matches(
                    "InvocationSourceByteEventV36::CheckedObject(InvocationSourceCheckedObjectV44"
                ).count(), 4);
                assert_eq!(
                    out.text
                        .matches("value_offset: 0int, overflow_offset: 4int")
                        .count(),
                    4
                );
                assert!(!out.text.contains("InvocationSourceByteEventV36::Checked {"));
                assert!(out.text.contains(&format!(
                    "ty == {}int && bits == 32int && signed == false && bytes == 8int && alignment == 4int && value_offset == 0int && overflow_offset == 4int",
                    pair.index())));
                assert!(!out.text.contains("assume("));
                Ok(())
            })
        },
    )
}

#[test]
fn original_checked_object_contract_rejects_wrong_fields_extents_and_overlaps() {
    run(SemanticCheckedBinaryOpV1::Add, false, LIMIT, LIMIT, |_, plan, out| {
        let source = plan.source(out)?.source_semantic(out.budget)?;
        let pair = source.functions().last().unwrap().locals()[4].ty();
        let original = &source.types()[pair.index() as usize];
        let Shape::Tuple(fields) = original.shape() else { panic!("genuine checked pair") };
        let check = |types: &[Type], limit| {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, LIMIT);
            budget.reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT
                + super::super::checked_types::headers()).unwrap();
            let mut writer = Writer::new(&mut budget).unwrap();
            let before = (writer.budget.work(), writer.budget.storage(), writer.budget.peak_storage());
            let result = super::super::checked_types::classify(types, pair, &mut writer);
            let observed = (writer.budget.work() - before.0, writer.budget.storage(), writer.budget.peak_storage());
            assert_eq!((observed.1, observed.2), (before.1, before.2));
            (result, observed.0)
        };
        let exact = check(source.types(), 12);
        assert!(exact.0.unwrap().is_some());
        assert_eq!(exact.1, 12);
        assert!(matches!(check(source.types(), 11).0,
            Err(Error::Resource(Resource::Work(error))) if error.actual() == 12 && error.limit() == 11));
        for (bytes, offsets, shape) in [
            (8, vec![0, 0], original.shape().clone()),
            (4, vec![0, 4], original.shape().clone()),
            (8, vec![0, 4], Shape::Tuple(SemanticAggregateTypeV1::new(vec![fields.fields()[0]; 2]).unwrap())),
            (8, vec![0, 4], Shape::Aggregate(fields.clone())),
        ] {
            let mut inert = source.types().to_vec();
            inert[pair.index() as usize] = Type::new(original.identity(), original.layout_identity(),
                SemanticTypeLayoutV1::aggregate(Some(bytes), 4,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap()).unwrap(), shape);
            assert_eq!(check(&inert, 12).0?, None);
        }
        let runtime = include_str!("original_semantic_mir_source_checked_objects_v44.vrs");
        assert!(runtime.contains("invocation_source_checked_object_type_v47(event.source_type,"));
        assert!(runtime.contains("source.objects[local].slot.semantic_type == event.source_type"));
        assert!(!runtime.contains("invocation_source_aggregate_leaf_"));
        Ok(())
    }).0.unwrap();
}

#[test]
fn retained_checked_objects_emit_independent_source_events_in_every_call_instance() {
    for operation in [
        SemanticCheckedBinaryOpV1::Add,
        SemanticCheckedBinaryOpV1::Subtract,
        SemanticCheckedBinaryOpV1::Multiply,
    ] {
        retained_program_v44(operation, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn retained_checked_object_program_has_exact_and_one_short_complete_resources() {
    let operation = SemanticCheckedBinaryOpV1::Multiply;
    let measured = retained_program_v44(operation, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = retained_program_v44(operation, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for (work, storage, is_work) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let refused = retained_program_v44(operation, work, storage).0;
        let mut error: &(dyn std::error::Error + 'static) = refused.as_ref().unwrap_err();
        loop {
            if let Some(resource) = error.downcast_ref::<Resource>() {
                match resource {
                    Resource::Work(limit) if is_work => {
                        assert_eq!(limit.limit(), work);
                        assert_eq!(limit.actual(), measured.1);
                    }
                    Resource::Storage(limit) if !is_work => {
                        assert_eq!(limit.limit(), storage);
                        assert_eq!(limit.actual(), measured.3);
                    }
                    other => panic!("wrong resource: {other:?}"),
                }
                break;
            }
            error = error
                .source()
                .unwrap_or_else(|| panic!("missing resource: {refused:?}"));
        }
    }
}

fn transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    operation: SemanticCheckedBinaryOpV1,
    nested: bool,
) {
    super::super::super::paired::aggregate_tests::checked_transform(
        types, functions, operation, true, false,
    );
    if !nested {
        return;
    }
    let pair = TypeId::from_index(types.len() as u32 - 1);
    let unit = TypeId::from_index(
        types
            .iter()
            .position(|ty| matches!(ty.shape(), Shape::Unit))
            .unwrap() as u32,
    );
    let array = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([247; 32]),
        SemanticLayoutIdentityV1::from_sha256([247; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::array(0, 2),
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
        Shape::Array {
            element: unit,
            length: 2,
        },
    ));
    let nested = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([248; 32]),
        SemanticLayoutIdentityV1::from_sha256([248; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        Shape::Tuple(SemanticAggregateTypeV1::new(vec![pair, array]).unwrap()),
    ));
    let old = functions.last_mut().unwrap();
    let mut locals = old.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([249; 32]),
        nested,
        SemanticLocalRoleV1::Temporary,
        old.source(),
    ));
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        locals,
        old.entry(),
        old.blocks().to_vec(),
    )
    .unwrap();
}

fn run(
    operation: SemanticCheckedBinaryOpV1,
    nested: bool,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &SourceSlots<'_, '_>,
        &InvocationPlan<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| transform(types, functions, operation, nested),
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                examine(slots, plan, out)
            })
        },
    )
}

fn schema(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let source = plan.source(out)?.source_semantic(out.budget)?;
    let function = source.functions().last().unwrap();
    let pair = function.locals()[4].ty();
    let boolean = source.types()[pair.index() as usize].shape();
    let Shape::Tuple(fields) = boolean else {
        panic!("original tuple");
    };
    assert_eq!(slots.aggregate_leaf_count(pair, out)?, Some(2));
    for (ordinal, scalar) in [
        ScalarV30::Integer {
            signed: false,
            width: 32,
        },
        ScalarV30::Bool,
    ]
    .into_iter()
    .enumerate()
    {
        let leaf = slots.aggregate_leaf(pair, ordinal, out)?;
        assert_eq!(leaf.path(out)?, &[ordinal as u32]);
        assert_eq!(leaf.source_type(out)?, fields.fields()[ordinal]);
        assert_eq!(leaf.scalar(out)?, scalar);
        // A borrowed wrapper's path remains owner-backed after the wrapper is dropped.
        let path = { slots.aggregate_leaf(pair, ordinal, out)?.path(out)? };
        assert_eq!(path, &[ordinal as u32]);
    }
    if function.locals().len() > 5 {
        let nested = function.locals()[5].ty();
        let Shape::Tuple(fields) = source.types()[nested.index() as usize].shape() else {
            panic!("nested tuple");
        };
        let array = fields.fields()[1];
        let Shape::Array {
            element: unit,
            length: 2,
        } = source.types()[array.index() as usize].shape()
        else {
            panic!("zero-sized array");
        };
        assert_eq!(slots.aggregate_leaf_count(nested, out)?, Some(4));
        for (ordinal, path) in [[0, 0], [0, 1], [1, 0], [1, 1]].into_iter().enumerate() {
            assert_eq!(
                slots.aggregate_leaf(nested, ordinal, out)?.path(out)?,
                &path
            );
        }
        let projection = [
            SemanticProjectionV1::new(Projection::Field(1), array).unwrap(),
            SemanticProjectionV1::new(
                Projection::ConstantIndex {
                    offset: 1,
                    minimum_length: 2,
                    from_end: true,
                },
                *unit,
            )
            .unwrap(),
        ];
        assert_eq!(
            slots.aggregate_component_range(nested, &projection, out)?,
            Some((3..4, *unit))
        );
        assert_eq!(slots.aggregate_leaf_count(*unit, out)?, Some(1));
        assert!(slots.aggregate_leaf(*unit, 0, out)?.path(out)?.is_empty());
        assert_eq!(
            slots.aggregate_leaf(*unit, 0, out)?.scalar(out)?,
            ScalarV30::Unit
        );
        assert_eq!(
            slots.aggregate_component_range(nested, &projection[..1], out)?,
            Some((2..4, array))
        );
        let invalid = [SemanticProjectionV1::new(
            Projection::ConstantIndex {
                offset: 2,
                minimum_length: 3,
                from_end: false,
            },
            *unit,
        )
        .unwrap()];
        assert!(
            slots
                .aggregate_component_range(array, &invalid, out)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn original_aggregate_schema_preserves_nested_paths_and_nonvacuous_unit_leaves() {
    run(SemanticCheckedBinaryOpV1::Add, true, LIMIT, LIMIT, schema)
        .0
        .unwrap();
}

#[test]
fn original_aggregate_leaf_queries_have_constant_paid_work_and_no_allocation() {
    run(
        SemanticCheckedBinaryOpV1::Add,
        false,
        LIMIT,
        LIMIT,
        |slots, plan, out| {
            let source = plan.source(out)?.source_semantic(out.budget)?;
            let pair = source.functions().last().unwrap().locals()[4].ty();
            let storage = out.budget.storage();
            let work = out.budget.work();
            for _ in 0..64 {
                assert_eq!(slots.aggregate_leaf_count(pair, out)?, Some(2));
                let leaf = slots.aggregate_leaf(pair, 1, out)?;
                assert_eq!(leaf.path(out)?, &[1]);
                assert_eq!(leaf.scalar(out)?, ScalarV30::Bool);
            }
            // Relation source query 1 + slot owner check 2, then each local query's 1/3 debit.
            assert_eq!(out.budget.work() - work, 64 * (4 + 6 + 4 + 4));
            assert_eq!(out.budget.storage(), storage);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_aggregate_schema_complete_transaction_has_exact_resource_boundaries() {
    let execute = |work, storage| {
        run(
            SemanticCheckedBinaryOpV1::Add,
            true,
            work,
            storage,
            |slots, plan, out| {
                schema(slots, plan, out)?;
                slots.emit(out)
            },
        )
    };
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = execute(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert!(matches!(execute(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.limit() == measured.1 - 1 && error.actual() == measured.1));
    assert!(matches!(execute(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.limit() == measured.3 - 1 && error.actual() == measured.3));
}

#[test]
fn original_aggregate_schema_queries_latch_transient_and_callback_floor_violations() {
    for callback in [false, true] {
        let result = run(
            SemanticCheckedBinaryOpV1::Add,
            false,
            LIMIT,
            LIMIT,
            |slots, _, out| {
                let debit = out.budget.storage() - slots.required + 1;
                let first = if callback {
                    slots.with_source_query_v42(out, |out| {
                        out.budget.release_storage(debit)?;
                        Ok(())
                    })
                } else {
                    out.budget.release_storage(debit)?;
                    slots
                        .aggregate_leaf_count(TypeId::from_index(0), out)
                        .map(|_| ())
                };
                assert!(matches!(
                    first,
                    Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                ));
                out.budget.reserve_storage(debit)?;
                let work = out.budget.work();
                assert!(matches!(
                    slots.aggregate_leaf_count(TypeId::from_index(0), out),
                    Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                ));
                assert_eq!(out.budget.work(), work);
                slots.emit(out)
            },
        );
        assert!(matches!(
            result.0,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}

#[test]
fn original_checked_program_emits_component_moves_and_exclusive_call_snapshots() {
    for (operation, code) in [
        (SemanticCheckedBinaryOpV1::Add, 0),
        (SemanticCheckedBinaryOpV1::Subtract, 1),
        (SemanticCheckedBinaryOpV1::Multiply, 2),
    ] {
        run(operation, false, LIMIT, LIMIT, |slots, plan, out| {
            let mut program =
                super::super::super::source_function::SourceByteProgram::derive(plan, slots, out)?;
            program.emit(out)?;
            assert!(
                out.text
                    .contains(&format!("operation: {code}int, bits: 32int, signed: false"))
            );
            assert!(out.text.contains("InvocationSourceByteEventV36::Checked"));
            assert!(out.text.contains("InvocationSourceByteValueV36::Component"));
            assert!(out.text.contains("ordinal: 1int, moved: true"));
            assert!(out.text.contains("ordinal: 0int, moved: false"));
            assert!(out.text.contains(
                "let assertion_failure_0 = invocation_source_value_evaluate_v42(source,"
            ));
            assert!(
                out.text
                    .contains("InvocationSourceValueV42::Carrier(assertion_condition.value)")
            );
            assert!(!out.text.contains("assume("));
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_partial_aggregate_program_preserves_the_planners_genuine_memory_representation() {
    for backedge in [false, true] {
        super::super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| {
                super::super::super::paired::aggregate_tests::partial_transform(
                    types, functions, backedge,
                )
            },
            |plan, out| {
                super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                    for root in 0..2 {
                        for instance in 1..=2 {
                            assert!(slots.has_original_object(root, instance, 4, out)?);
                        }
                    }
                    let mut program =
                        super::super::super::source_function::SourceByteProgram::derive(
                            plan, slots, out,
                        )?;
                    program.emit(out)?;
                    assert!(
                        out.text
                            .contains("InvocationSourceByteDestinationV36::Memory(")
                    );
                    assert!(
                        out.text
                            .contains("InvocationSourceByteBaseV36::ObjectLocal(")
                    );
                    assert!(
                        out.text
                            .contains("offset: 0int, width: 4int, alignment: 4int")
                    );
                    assert!(
                        !out.text
                            .contains("InvocationSourceByteDestinationV36::Component(")
                    );
                    assert!(!out.text.contains("InvocationSourceByteEventV36::Checked"));
                    assert!(
                        !out.text
                            .contains("InvocationSourceByteEventV36::AggregateTransfer")
                    );
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_aggregate_deinitialization_keeps_a_genuine_promoted_sibling() {
    for backedge in [false, true] {
        super::super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| {
                super::super::super::paired::aggregate_tests::deinitialized_transform(
                    types, functions, backedge,
                )
            },
            |plan, out| {
                super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                    for root in 0..2 {
                        for instance in 1..=2 {
                            assert!(!slots.has_original_object(root, instance, 4, out)?);
                        }
                    }
                    let mut program =
                        super::super::super::source_function::SourceByteProgram::derive(
                            plan, slots, out,
                        )?;
                    program.emit(out)?;
                    assert!(
                        out.text
                            .contains("InvocationSourceByteEventV36::AggregateDeinitialize(")
                    );
                    assert!(out.text.contains("first_leaf: 1int, depth: 1int"));
                    assert!(out.text.contains("ordinal: 1int, moved: false"));
                    assert!(out.text.contains("ordinal: 0int, moved: false"));
                    assert!(
                        !out.text
                            .contains("InvocationSourceByteDestinationV36::Memory(")
                    );
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_aggregate_schema_retained_fields_and_query_headers_are_independent() {
    type Fields = (
        Vec<Kind>,
        Vec<Child>,
        Vec<Option<Count>>,
        Vec<Range<usize>>,
        Vec<Leaf>,
        Vec<u32>,
    );
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(size_of::<SourceAggregateTypesV42>(), size_of::<Fields>());
    assert_eq!(
        headers(),
        h::<SourceAggregateTypesV42>()
            + h::<CountFrame>()
            + h::<WalkFrame>()
            + h::<Leaf>()
            + h::<Child>()
            + h::<Kind>()
            + h::<Count>()
            + 8 * size_of::<Vec<usize>>()
            + 24 * size_of::<usize>()
    );
}

#[test]
fn original_aggregate_schema_drops_temporary_walk_credit_before_retaining_the_index() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    run(
        SemanticCheckedBinaryOpV1::Add,
        true,
        LIMIT,
        LIMIT,
        |slots, plan, out| {
            let source = plan.source(out)?.source_semantic(out.budget)?;
            let requested = vec![true; source.types().len()];
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(SOURCE_LIMIT)?;
            let mut writer = Writer::new(&mut budget)?;
            let floor = writer.budget.storage();
            let index = SourceAggregateTypesV42::derive(
                source.types(),
                &slots.abi,
                &requested,
                &mut writer,
            )?;
            let retained = headers()
                + index.kinds.capacity() * size_of::<Kind>()
                + index.children.capacity() * size_of::<Child>()
                + index.counts.capacity() * size_of::<Option<Count>>()
                + index.roots.capacity() * size_of::<Range<usize>>()
                + index.leaves.capacity() * size_of::<Leaf>()
                + index.paths.capacity() * size_of::<u32>();
            assert_eq!(writer.budget.storage(), floor + retained);
            assert!(
                writer.budget.peak_storage()
                    >= floor
                        + retained
                        + source.types().len() * (size_of::<WalkFrame>() + size_of::<u32>())
            );
            drop(index);
            writer.budget.release_storage(retained)?;
            assert_eq!(writer.budget.storage(), floor);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_checked_program_has_exact_and_one_short_complete_resources() {
    let execute = |work, storage| {
        run(
            SemanticCheckedBinaryOpV1::Multiply,
            false,
            work,
            storage,
            |slots, plan, out| {
                let mut program = super::super::super::source_function::SourceByteProgram::derive(
                    plan, slots, out,
                )?;
                program.emit(out)
            },
        )
    };
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = execute(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert!(matches!(execute(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.limit() == measured.1 - 1 && error.actual() == measured.1));
    assert!(matches!(execute(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.limit() == measured.3 - 1 && error.actual() == measured.3));
}

#[test]
fn original_checked_eight_bit_reference_matrix_matches_rust_overflow_and_wrapping() {
    for a in 0..=u8::MAX {
        for b in 0..=u8::MAX {
            for signed in [false, true] {
                let (left, right) = if signed {
                    (i64::from(a as i8), i64::from(b as i8))
                } else {
                    (i64::from(a), i64::from(b))
                };
                for operation in 0..3 {
                    let mathematical = match operation {
                        0 => left + right,
                        1 => left - right,
                        _ => left * right,
                    };
                    let overflow = if signed {
                        !(-128..128).contains(&mathematical)
                    } else {
                        !(0..256).contains(&mathematical)
                    };
                    let value = mathematical.rem_euclid(256) as u8;
                    let expected = if signed {
                        let (value, overflow) = match operation {
                            0 => (a as i8).overflowing_add(b as i8),
                            1 => (a as i8).overflowing_sub(b as i8),
                            _ => (a as i8).overflowing_mul(b as i8),
                        };
                        (value as u8, overflow)
                    } else {
                        match operation {
                            0 => a.overflowing_add(b),
                            1 => a.overflowing_sub(b),
                            _ => a.overflowing_mul(b),
                        }
                    };
                    assert_eq!((value, overflow), expected);
                }
            }
        }
    }
}

// Executable finite reference cases check the intended partial-value algebra.
// They do not execute or prove the generated Verus specifications.
type Model = BTreeMap<Vec<u32>, u64>;

fn snapshot(schema: &[Vec<u32>], state: &Model, prefix: &[u32]) -> Option<Model> {
    schema
        .iter()
        .filter(|path| path.starts_with(prefix))
        .map(|path| {
            state
                .get(path)
                .map(|value| (path[prefix.len()..].to_vec(), *value))
        })
        .collect()
}

fn consume(state: &mut Model, prefix: &[u32]) {
    state.retain(|path, _| !path.starts_with(prefix));
}

fn replace(state: &mut Model, prefix: &[u32], value: &Model) {
    consume(state, prefix);
    for (path, value) in value {
        let mut key = prefix.to_vec();
        key.extend(path);
        state.insert(key, *value);
    }
}

#[test]
fn original_aggregate_partial_move_reference_cases_preserve_siblings_and_reject_missing_fields() {
    let schema = vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]];
    for mask in 0..16 {
        let initial: Model = schema
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(i, path)| (path.clone(), i as u64 + 1))
            .collect();
        assert_eq!(snapshot(&schema, &initial, &[]).is_some(), mask == 15);
        for prefix in [&[][..], &[0][..], &[1][..], &[0, 1][..]] {
            let saved = snapshot(&schema, &initial, prefix);
            let required = schema
                .iter()
                .enumerate()
                .filter(|(_, p)| p.starts_with(prefix))
                .fold(0, |bits, (i, _)| bits | (1 << i));
            assert_eq!(saved.is_some(), mask & required == required);
            if let Some(saved) = saved {
                let mut moved = initial.clone();
                consume(&mut moved, prefix);
                for path in &schema {
                    assert_eq!(
                        moved.get(path),
                        if path.starts_with(prefix) {
                            None
                        } else {
                            initial.get(path)
                        }
                    );
                }
                assert!(snapshot(&schema, &moved, prefix).is_none());
                replace(&mut moved, prefix, &saved);
                assert_eq!(moved, initial, "snapshot precedes self-move invalidation");
            }
        }
        let mut success = initial.clone();
        let failure = snapshot(&schema, &initial, &[0]);
        // Failure-only consumption is not executed on the successful path.
        if let Some(value) = failure {
            let mut failed = initial.clone();
            consume(&mut failed, &[0]);
            replace(&mut success, &[0], &value);
        }
        assert_eq!(success, initial);
    }
    let unit = vec![vec![]];
    assert!(snapshot(&unit, &Model::new(), &[]).is_none());
    assert_eq!(
        snapshot(&unit, &Model::from([(vec![], 0)]), &[])
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn original_aggregate_snapshot_reference_cases_cover_overlap_calls_and_storage_reuse() {
    let schema = vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]];
    let mut caller: Model = schema
        .iter()
        .enumerate()
        .map(|(i, p)| (p.clone(), i as u64 + 10))
        .collect();
    let argument = snapshot(&schema, &caller, &[0]).unwrap();
    consume(&mut caller, &[0]);
    let mut callee = argument.clone();
    // Callee partial moves do not mutate the immutable caller argument snapshot.
    consume(&mut callee, &[1]);
    assert_eq!(argument.len(), 2);
    assert!(snapshot(&[vec![0], vec![1]], &callee, &[]).is_none());
    let scalar_return = snapshot(&[vec![0], vec![1]], &callee, &[0]).unwrap();
    callee.clear();
    replace(&mut caller, &[0, 0], &scalar_return);
    assert!(snapshot(&schema, &caller, &[0]).is_none());
    assert_eq!(caller.get(&vec![1, 1]), Some(&13));
    // A complete source snapshot is captured before overlapping destination replacement.
    let saved = snapshot(&schema, &caller, &[1]).unwrap();
    consume(&mut caller, &[1]);
    replace(&mut caller, &[0], &saved);
    assert_eq!(caller.get(&vec![0, 0]), Some(&12));
    assert_eq!(caller.get(&vec![0, 1]), Some(&13));
    assert!(!caller.contains_key(&vec![1, 0]));
    // Reusing either a storage scope or a call frame starts with no defined leaves.
    caller.clear();
    assert!(snapshot(&schema, &caller, &[0]).is_none());
    assert!(snapshot(&[vec![0], vec![1]], &callee, &[]).is_none());
}

#[test]
fn original_aggregate_runtime_obligations_are_in_the_actual_generated_prelude() {
    let text = super::super::super::source_bytes::SOURCE_BYTES_V36;
    for law in [
        "missing_leaf_refuses",
        "snapshot_requires_complete",
        "partial_move_keeps_sibling",
        "second_move_refuses",
        "whole_move_consumes",
        "partial_replace_keeps_sibling",
        "zero_sized_leaf_is_not_vacuous",
        "frame_clear_removes_aggregate",
    ] {
        assert!(
            text.contains(&format!("proof fn invocation_source_aggregate_{law}_v42(")),
            "{law}"
        );
    }
    assert!(text.contains("value: Some(InvocationSourceValueV42::Aggregate(value))"));
    assert!(text.contains(
        "let evaluated = invocation_source_aggregate_place_evaluate_v42(source, input, moved)"
    ));
    assert!(text.contains("aggregates: logical.aggregates.remove(local)"));
    assert!(!text.contains("admit("));
    assert!(!text.contains("assume("));
}
