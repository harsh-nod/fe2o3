use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 100_000_000;

fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            let word = SemanticTypeIdV1::from_index(0);
            let pair = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([220; 32]),
                SemanticLayoutIdentityV1::from_sha256([221; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    4,
                    SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![word, word]).unwrap()),
            ));
            let function = functions.last_mut().unwrap();
            let mut locals = function.locals().to_vec();
            let local = SemanticLocalIdV1::from_index(u32::try_from(locals.len()).unwrap());
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([240; 32]),
                pair,
                SemanticLocalRoleV1::Temporary,
                function.source(),
            ));
            let mut blocks = function.blocks().to_vec();
            let block = &blocks[0];
            let mut statements = block.statements().to_vec();
            statements.push(SemanticStatementV1::new(
                function.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(local, vec![], pair).unwrap(),
                    SemanticRvalueV1::new(
                        pair,
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Tuple,
                                [1, 2]
                                    .into_iter()
                                    .map(|input| {
                                        SemanticOperandV1::Copy(
                                            SemanticPlaceV1::new(
                                                SemanticLocalIdV1::from_index(input),
                                                vec![],
                                                word,
                                            )
                                            .unwrap(),
                                        )
                                    })
                                    .collect(),
                            )
                            .unwrap(),
                        ),
                    ),
                )),
            ));
            blocks[0] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap();
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
                blocks,
            )
            .unwrap();
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                examine(plan, slots, out)
            })
        },
    )
}

#[test]
fn original_product_constructor_uses_retained_statement_and_ordered_typed_fields() {
    let result = run(LIMIT, LIMIT, |plan, slots, out| {
        out.budget.reserve_storage(super::super::headers())?;
        let semantic = slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let mut observed = 0;
        for root in 0..2 {
            for instance in 0..plan.root(root, out)?.instances.len() {
                let row = plan.instance(root, instance, out)?;
                if !row.active {
                    continue;
                }
                let function = &semantic.functions()[row.function.index() as usize];
                let context = Context {
                    slots,
                    types: semantic.types(),
                    function,
                    root,
                    instance,
                    locals: row.locals.clone(),
                };
                for (block, body) in function.blocks().iter().enumerate() {
                    for (statement, event) in body.statements().iter().enumerate() {
                        let Statement::Assign(assignment) = event.kind() else {
                            continue;
                        };
                        let Rvalue::Aggregate(aggregate) = assignment.value().kind() else {
                            continue;
                        };
                        let mut fields = vector(aggregate.operands().len(), out)?;
                        let constructor = Construct::derive(
                            &context,
                            plan,
                            row.function,
                            block,
                            statement,
                            assignment,
                            &mut fields,
                            out,
                        )?
                        .unwrap();
                        assert_eq!(fields.len(), 2);
                        assert_eq!(constructor.count, 2);
                        assert_eq!(constructor.ty, assignment.value().result_type());
                        for (ordinal, field) in fields.iter().enumerate() {
                            assert_eq!(field.ty, aggregate.operands()[ordinal].ty());
                            assert!(matches!(field.kind, OperandKind::Scalar {
                                value: Value::Local { local, moved: false }, ..
                            } if local == row.locals.start + ordinal + 1));
                        }
                        let at = out.text.len();
                        constructor.emit(&fields, out)?;
                        let text = &out.text[at..];
                        assert!(text.starts_with("InvocationSourceByteEventV36::ProductConstruct"));
                        assert_eq!(
                            text.matches("InvocationSourceProductOperandV282 {").count(),
                            2
                        );
                        let copy = assignment.clone();
                        let mut rejected = vector(2, out)?;
                        assert!(matches!(
                            Construct::derive(
                                &context,
                                plan,
                                row.function,
                                block,
                                statement,
                                &copy,
                                &mut rejected,
                                out
                            ),
                            Err(Error::Statement(_))
                        ));
                        assert!(rejected.is_empty());
                        observed += 1;
                    }
                }
            }
        }
        assert_eq!(observed, 4);
        Ok(())
    });
    result.0.unwrap();
    let floor = super::super::super::super::invocations::tests::FLOOR;
    assert_eq!(result.2, floor);
    assert!(result.3 > floor);
}

#[test]
fn original_product_constructor_model_keeps_order_provenance_and_move_currentness() {
    let text = include_str!("original_semantic_mir_source_product_values_v282.vrs");
    assert!(text.contains("invocation_source_product_child_v282(source_type, ordinal) != Some(fields[ordinal].source_type)"));
    assert!(text.contains("let evaluated = invocation_source_product_operand_v282(source, fields[ordinal], root, instance, little_endian);"));
    assert!(text.contains(
        "invocation_source_product_fields_v282(ready, source_type, fields, ordinal + 1,"
    ));
    assert!(text.contains("InvocationSourceOperandV36::Product { .. } => snapshot,"));
    assert!(text.contains("Some(recipe) => Some(Map::empty().insert(seq![], InvocationSourceProductAtomV282::Descriptor { recipe, snapshot }))"));
    assert!(text.contains("observations: evaluated.observations + rest.observations"));
    assert!(text.contains("invocation_source_product_components_current_v282(evaluated.source,"));
    assert!(text.contains("invocation_source_product_components_current_v282(installed, source_type, components, little_endian)"));
    assert!(!text.contains("origin_version: source.logical.versions"));
}

#[test]
fn original_product_shared_operand_entry_refuses_noncopyable_product_copy() {
    let result = super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            let reference = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([220; 32]),
                SemanticLayoutIdentityV1::from_sha256([220; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SemanticTypeIdV1::from_index(0),
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ));
            let product = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([221; 32]),
                SemanticLayoutIdentityV1::from_sha256([221; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![reference]).unwrap()),
            ));
            let function = functions.last_mut().unwrap();
            let mut locals = function.locals().to_vec();
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([240; 32]),
                product,
                SemanticLocalRoleV1::Temporary,
                function.source(),
            ));
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
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                out.budget.reserve_storage(super::super::headers())?;
                let semantic = plan.source(out)?.source_semantic(out.budget)?;
                let mut checked = 0;
                for root in 0..2 {
                    for instance in 0..plan.root(root, out)?.instances.len() {
                        let row = plan.instance(root, instance, out)?;
                        let function = &semantic.functions()[row.function.index() as usize];
                        if !row.active
                            || row.function.index() as usize + 1 != semantic.functions().len()
                        {
                            continue;
                        }
                        let local = function.locals().len() - 1;
                        let ty = function.locals()[local].ty();
                        assert!(slots.is_product_v282(ty, out)?);
                        assert!(!slots.product_type_copyable_v282(ty, out)?);
                        let context = Context {
                            slots,
                            types: semantic.types(),
                            function,
                            root,
                            instance,
                            locals: row.locals.clone(),
                        };
                        let place = SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(u32::try_from(local).unwrap()),
                            vec![],
                            ty,
                        )
                        .unwrap();
                        assert!(matches!(
                            context.typed_operand(&Operand::Copy(place.clone()), out),
                            Err(Error::Statement(_))
                        ));
                        assert!(matches!(
                            context.typed_operand(&Operand::Move(place), out)?.kind,
                            OperandKind::Product { moved: true, .. }
                        ));
                        checked += 1;
                    }
                }
                assert_eq!(checked, 4);
                Ok(())
            })
        },
    );
    result.0.unwrap();
    let floor = super::super::super::super::invocations::tests::FLOOR;
    assert_eq!(result.2, floor);
    assert!(result.3 > floor);
}

#[test]
fn original_product_operand_gate_refuses_copy_of_an_authenticated_workgroup_type() {
    super::super::super::source_function::tile_fixture_tests::run_fixture_with_plan(
        fe2o3_kernel_ir::ExecutionTileLayoutV1::Blocked,
        LIMIT,
        LIMIT,
        |plan, slots, _, out| {
            out.budget.reserve_storage(super::super::headers())?;
            let semantic = plan.source(out)?.source_semantic(out.budget)?;
            let mut observed = 0;
            for root in 0..plan.source(out)?.root_count(out.budget)? {
                for instance in 0..plan.root(root, out)?.instances.len() {
                    let row = plan.instance(root, instance, out)?;
                    if !row.active {
                        continue;
                    }
                    let function = &semantic.functions()[row.function.index() as usize];
                    let context = Context {
                        slots,
                        types: semantic.types(),
                        function,
                        root,
                        instance,
                        locals: row.locals.clone(),
                    };
                    for (local, declaration) in function.locals().iter().enumerate() {
                        if semantic.types()[declaration.ty().index() as usize].rust_type_kind()
                            != SemanticRustTypeKindV1::Execution(
                                SemanticExecutionRoleV29::Workgroup,
                            )
                        {
                            continue;
                        }
                        let place = SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(local as u32),
                            vec![],
                            declaration.ty(),
                        )
                        .unwrap();
                        assert!(!slots.product_type_copyable_v282(declaration.ty(), out)?);
                        // This is an operand-admission negative, not an executed
                        // constructor or fabricated execution lease.
                        assert!(matches!(
                            original_operand(
                                &context,
                                plan,
                                row.function,
                                0,
                                0,
                                0,
                                &SemanticOperandV1::Copy(place),
                                out
                            ),
                            Err(Error::Statement(_))
                        ));
                        observed += 1;
                    }
                }
            }
            assert!(observed > 0);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_product_transport_preserves_direct_execution_return_refusal_and_all_atom_lifetimes() {
    let values = include_str!("original_semantic_mir_source_product_values_v282.vrs");
    let frames = include_str!("original_semantic_mir_invocation_source_frames_v36.rs");
    assert!(
        values
            .contains("InvocationSourceProductAtomV282::Execution(value) => value.frame == frame")
    );
    assert!(values.contains("spec fn invocation_source_product_return_escapes_frame_v282"));
    assert!(values.contains(
        "match value.components[path] {\n                InvocationSourceProductAtomV282::Execution(_) => true,\n                _ => false,\n            }"
    ));
    assert!(frames.contains("InvocationSourceValueV42::Product(value) => invocation_source_product_return_escapes_frame_v282(value, frame)"));
    assert!(
        frames.contains(
            "invocation_source_product_escapes_frame_v282(logical.products[local], frame)"
        )
    );
    assert!(
        values.contains("invocation_source_descriptor_snapshot_escapes_frame_v53(snapshot, frame)")
    );
    assert!(values.contains("!value.recipe.mutable\n                    && invocation_source_execution_snapshot_current_v170"));
    assert!(values.contains("InvocationSourceProductAtomKindV282::Slice { mutable, reference, .. } => !reference || !mutable"));
}
