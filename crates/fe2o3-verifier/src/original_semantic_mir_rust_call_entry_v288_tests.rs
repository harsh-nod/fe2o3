use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 100_000_000;
const FLOOR: usize = super::super::super::invocations::tests::FLOOR;

pub(super) fn argument_origin_header_oracle() -> usize {
    #[allow(dead_code)]
    enum OriginFields {
        Argument(u32),
        RustCallTupleField { argument: u32, field: u32 },
        ImplicitCapability,
    }
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(
        size_of::<OriginFields>(),
        size_of::<fe2o3_pliron::ProductionSemanticSsaEntryOriginV1>()
    );
    assert_eq!(
        std::mem::align_of::<OriginFields>(),
        std::mem::align_of::<fe2o3_pliron::ProductionSemanticSsaEntryOriginV1>()
    );
    h::<&AdmittedInertSemanticMirV1>()
        + h::<&Function>()
        + h::<&mut Writer<'_, '_>>()
        + h::<SemanticLocalIdV1>()
        + h::<&SemanticLocalDeclV1>()
        + h::<LocalRole>()
        + h::<SemanticExternAbiV1>()
        + h::<SemanticTypeIdV1>()
        + h::<&SemanticTypeDeclV1>()
        + h::<&Shape>()
        + h::<&SemanticAggregateTypeV1>()
        + h::<Option<&SemanticTypeIdV1>>()
        + h::<OriginFields>()
        + 3 * h::<u32>()
        + 3 * h::<usize>()
}

#[derive(Clone, Copy)]
enum Form {
    Unit,
    EmptyTuple,
    NonemptyTuple,
    ScalarTuple,
    NestedTuple,
    EmptyCompositeFields,
}

fn argument_operand(
    ty: SemanticTypeIdV1,
    types: &[SemanticTypeDeclV1],
    locals: &mut Vec<SemanticLocalDeclV1>,
    statements: &mut Vec<SemanticStatementV1>,
    source: SemanticSourceProvenanceV1,
) -> SemanticOperandV1 {
    let operands = match types[ty.index() as usize].shape() {
        Shape::Unit => {
            return SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::ZeroSized,
            ));
        }
        Shape::Tuple(tuple) | Shape::Aggregate(tuple) if tuple.fields().is_empty() => {
            return SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::ZeroSized,
            ));
        }
        Shape::Array { length: 0, .. } => {
            return SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::ZeroSized,
            ));
        }
        Shape::Tuple(tuple) => tuple
            .fields()
            .iter()
            .map(|&field| argument_operand(field, types, locals, statements, source))
            .collect(),
        _ => {
            assert_eq!(ty.index(), 0);
            return SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 4).unwrap()),
            ));
        }
    };
    let local = SemanticLocalIdV1::from_index(u32::try_from(locals.len()).unwrap());
    let mut identity = [250; 32];
    identity[28..].copy_from_slice(&local.index().to_be_bytes());
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(identity),
        ty,
        LocalRole::Temporary,
        source,
    ));
    let place = SemanticPlaceV1::new(local, vec![], ty).unwrap();
    statements.push(SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place.clone(),
            SemanticRvalueV1::new(
                ty,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Tuple, operands)
                        .unwrap(),
                ),
            ),
        )),
    ));
    SemanticOperandV1::Move(place)
}

fn transform(form: Form, types: &mut Vec<SemanticTypeDeclV1>, functions: &mut Vec<Function>) {
    let unit = SemanticTypeIdV1::from_index(
        types
            .iter()
            .position(|ty| matches!(ty.shape(), Shape::Unit))
            .unwrap() as u32,
    );
    let nested = if matches!(form, Form::NestedTuple) {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        let scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        );
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([245; 32]),
            SemanticLayoutIdentityV1::from_sha256([246; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                4,
                SemanticBackendReprV1::scalar_pair(scalar, scalar),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
            )
            .unwrap(),
            Shape::Tuple(
                SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(0); 2]).unwrap(),
            ),
        ));
        Some(ty)
    } else {
        None
    };
    let empty_fields = if matches!(form, Form::EmptyCompositeFields) {
        let mut fields = vec![unit];
        for (ordinal, shape) in [
            Shape::Tuple(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            Shape::Array {
                element: SemanticTypeIdV1::from_index(0),
                length: 0,
            },
            Shape::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ]
        .into_iter()
        .enumerate()
        {
            let ty = SemanticTypeIdV1::from_index(types.len() as u32);
            let layout = if matches!(shape, Shape::Array { .. }) {
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    4,
                    SemanticFieldsShapeV1::array(4, 0),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    4,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap()
            } else {
                SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                )
                .unwrap()
            };
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([241 + 2 * ordinal as u8; 32]),
                SemanticLayoutIdentityV1::from_sha256([242 + 2 * ordinal as u8; 32]),
                layout,
                shape,
            ));
            fields.push(ty);
        }
        fields
    } else {
        vec![]
    };
    let argument = match form {
        Form::Unit => unit,
        Form::EmptyTuple
        | Form::NonemptyTuple
        | Form::ScalarTuple
        | Form::NestedTuple
        | Form::EmptyCompositeFields => {
            let fields = match form {
                Form::NonemptyTuple => vec![unit],
                Form::ScalarTuple => vec![SemanticTypeIdV1::from_index(0); 2],
                Form::NestedTuple => vec![unit, nested.unwrap(), SemanticTypeIdV1::from_index(0)],
                Form::EmptyCompositeFields => empty_fields,
                _ => vec![],
            };
            let (bytes, alignment, offsets) = match form {
                Form::ScalarTuple => (8, 4, vec![0, 4]),
                Form::NestedTuple => (12, 4, vec![0, 0, 8]),
                Form::EmptyCompositeFields => (0, 4, vec![0; fields.len()]),
                _ => (0, 1, vec![0; fields.len()]),
            };
            let ty = SemanticTypeIdV1::from_index(types.len() as u32);
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([247; 32]),
                SemanticLayoutIdentityV1::from_sha256([248; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(bytes),
                    alignment,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
                )
                .unwrap(),
                Shape::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
            ));
            ty
        }
    };
    let field_types = match types[argument.index() as usize].shape() {
        Shape::Tuple(tuple) => tuple.fields().to_vec(),
        _ => vec![],
    };
    let helper = functions.len() - 1;
    for (index, function) in functions.iter_mut().enumerate() {
        let prior = &*function;
        let mut abi = prior.abi().clone();
        let mut locals = prior.locals().to_vec();
        let mut blocks = prior.blocks().to_vec();
        if index == helper {
            assert_eq!(prior.abi().source_input_types().len(), 2);
            let mut inputs = prior.abi().source_input_types().to_vec();
            inputs.push(argument);
            let mut adjusted = prior.abi().adjusted_arguments().to_vec();
            for (field, &ty) in field_types.iter().enumerate() {
                let mode = if types[ty.index() as usize].layout().size_bytes() == Some(0) {
                    SemanticAbiPassModeV1::Ignore
                } else if Some(ty) == nested {
                    let SemanticAbiPassModeV1::Direct(attributes) =
                        prior.abi().adjusted_arguments()[0].mode()
                    else {
                        panic!("scalar fixture ABI");
                    };
                    SemanticAbiPassModeV1::Pair {
                        first: *attributes,
                        second: *attributes,
                    }
                } else {
                    prior.abi().adjusted_arguments()[0].mode().clone()
                };
                adjusted.push(SemanticAbiArgumentV1::rust_call_tuple_field(
                    field as u32,
                    SemanticAbiValueV1::new(ty, mode),
                ));
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([249 + field as u8; 32]),
                    ty,
                    LocalRole::RustCallTupleField {
                        argument: 2,
                        field: field as u32,
                    },
                    prior.source(),
                ));
            }
            let mut ownership = prior.abi().source_argument_ownership().to_vec();
            ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
            abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
                prior.abi().identity(),
                prior.abi().layout_identity(),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::RustCall,
                false,
                false,
                2,
                inputs,
                prior.abi().source_output_type(),
                adjusted,
                prior.abi().return_value().clone(),
            )
            .unwrap()
            .with_source_argument_ownership(ownership)
            .unwrap();
        } else {
            for block in &mut blocks {
                if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                    assert_eq!(call.callee().index() as usize, helper);
                    let mut arguments = call.arguments().to_vec();
                    let mut statements = block.statements().to_vec();
                    let operand = argument_operand(
                        argument,
                        types,
                        &mut locals,
                        &mut statements,
                        prior.source(),
                    );
                    arguments.push(operand);
                    *block = SemanticBasicBlockV1::new(
                        block.identity(),
                        block.source(),
                        statements,
                        SemanticTerminatorV1::new(
                            block.terminator().source(),
                            SemanticTerminatorKindV1::Call(
                                SemanticDirectCallV1::new_callable(
                                    call.callee(),
                                    arguments,
                                    call.destination().cloned(),
                                    call.unwind(),
                                )
                                .unwrap(),
                            ),
                        ),
                    )
                    .unwrap();
                }
            }
        }
        let mut rebuilt = Function::new(
            prior.identity(),
            prior.role(),
            prior.item_definition_identity(),
            prior.monomorphization_identity(),
            prior.generic_type_arguments_identity(),
            prior.const_generic_arguments_identity(),
            prior.source(),
            abi,
            locals,
            prior.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = prior.kernel_entry() {
            rebuilt = rebuilt.with_kernel_entry(entry.clone());
        }
        *function = rebuilt;
    }
}

fn run(
    form: Form,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| transform(form, types, functions),
        |plan, out| {
            let source = plan.source(out)?;
            let (inventory, receipt) = super::super::super::super::Inventory::derive_v18(
                source.canonical(out.budget)?,
                out.budget,
            )?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let result = source.with_ranked_correspondence_v18(
                &inventory,
                out.budget,
                |relation, budget| {
                    let mut writer = Writer::new(budget)?;
                    let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                    examine(plan, &slots, &mut writer)
                },
            );
            drop(inventory);
            if result.is_ok() {
                out.budget.release_storage(receipt.retained_storage())?;
            }
            result
        },
    )
}

#[test]
fn original_rust_call_zero_field_entry_keeps_unit_and_tuple_arguments_without_fake_locals() {
    for form in [Form::Unit, Form::EmptyTuple] {
        let result = run(form, LIMIT, LIMIT, |plan, slots, out| {
            for root in 0..2 {
                for instance in 1..3 {
                    let row = plan.instance(root, instance, out)?;
                    let source = plan.source(out)?.source_semantic(out.budget)?;
                    let function = &source.functions()[row.function.index() as usize];
                    let entry = SourceFrameEnter::derive(plan, slots, root, instance, out)?;
                    assert_eq!(entry.arguments.len(), 3);
                    assert_eq!(entry.locals.len(), function.locals().len());
                    assert!(!entry.heap_conservation_shape(out)?);
                    let expected = match form {
                        Form::Unit => EmptyExpanded::Unit,
                        Form::EmptyTuple => {
                            EmptyExpanded::Tuple(function.abi().source_input_types()[2].index())
                        }
                        Form::NonemptyTuple
                        | Form::ScalarTuple
                        | Form::NestedTuple
                        | Form::EmptyCompositeFields => {
                            unreachable!()
                        }
                    };
                    assert!(
                        matches!(entry.arguments[2], Some(EntryArgument::ExpandedEmpty(actual)) if actual == expected)
                    );
                    assert_eq!(entry.fields.len(), 2);
                    for ordinal in 0..2 {
                        let local = function
                            .locals()
                            .iter()
                            .position(|local| local.role() == LocalRole::Argument(ordinal as u32))
                            .unwrap();
                        assert!(
                            matches!(entry.arguments[ordinal], Some(EntryArgument::Whole(index)) if index == ordinal)
                        );
                        assert_eq!(entry.fields[ordinal].local, row.locals.start + local);
                    }
                    let before = out.text.len();
                    entry.emit(out)?;
                    let text = &out.text[before..];
                    assert!(text.contains("arguments.len() != 3"));
                    assert!(text.contains("match arguments[2]"));
                    let body = text
                        .split("let entered = invocation_source_entry_initialize_v166")
                        .nth(1)
                        .unwrap();
                    assert!(!body.contains("arguments[2]") && !body.contains("argument_2"));
                    assert_eq!(body.matches("match arguments[0]").count(), 1);
                    assert_eq!(body.matches("match arguments[1]").count(), 1);
                    match expected {
                        EmptyExpanded::Unit => assert!(text.contains("InvocationSourceValueV42::Carrier(MemoryValueV30::Unit) => true")),
                        EmptyExpanded::Tuple(ty) => assert!(text.contains(&format!("value.source_type == {ty} && value.execution_lease.is_none() && invocation_source_aggregate_complete_v42(value)"))),
                    }
                }
            }
            Ok(())
        });
        result.0.unwrap();
        assert_eq!(result.2, FLOOR);
        assert!(result.3 > FLOOR);
    }
}

#[test]
fn original_rust_call_empty_binding_rejects_wrong_type_ordinal_shape_and_nonempty_fields() {
    run(Form::Unit, LIMIT, LIMIT, |plan, _, out| {
        let row = plan.instance(0, 1, out)?;
        let source = plan.source(out)?.source_semantic(out.budget)?;
        let function = &source.functions()[row.function.index() as usize];
        let ty = function.abi().source_input_types()[2];
        let shape = source.types()[ty.index() as usize].shape();
        assert_eq!(
            empty_expanded_argument(
                function,
                2,
                ty,
                shape,
                &[],
                SemanticSourceArgumentOwnershipV1::ByValue,
                out
            )?,
            EmptyExpanded::Unit
        );
        for (ordinal, input) in [(1, ty), (2, function.abi().source_input_types()[0])] {
            assert!(matches!(
                empty_expanded_argument(
                    function,
                    ordinal,
                    input,
                    shape,
                    &[],
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    out
                ),
                Err(Error::Statement(
                    "original MIR byte frame entry differs from its exact invocation"
                ))
            ));
        }
        assert!(matches!(
            empty_expanded_argument(
                function,
                2,
                ty,
                shape,
                &[],
                SemanticSourceArgumentOwnershipV1::Unspecified,
                out,
            ),
            Err(Error::Statement(
                "original MIR byte frame entry differs from its exact invocation"
            ))
        ));
        let wrong_shape =
            source.types()[function.abi().source_input_types()[0].index() as usize].shape();
        assert!(matches!(
            empty_expanded_argument(
                function,
                2,
                ty,
                wrong_shape,
                &[],
                SemanticSourceArgumentOwnershipV1::ByValue,
                out
            ),
            Err(Error::Statement(
                "original MIR byte argument lifetime or payload is not modeled"
            ))
        ));
        assert!(matches!(
            empty_expanded_argument(
                function,
                2,
                ty,
                shape,
                &[SemanticLocalIdV1::from_index(1)],
                SemanticSourceArgumentOwnershipV1::ByValue,
                out
            ),
            Err(Error::Statement(
                "original MIR byte argument lifetime or payload is not modeled"
            ))
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_rust_call_nonempty_expansion_keeps_typed_field_order_and_original_locals() {
    for form in [Form::NonemptyTuple, Form::ScalarTuple, Form::NestedTuple] {
        let result = run(form, LIMIT, LIMIT, |plan, slots, out| {
            for root in 0..2 {
                for instance in 1..3 {
                    let row = plan.instance(root, instance, out)?;
                    let source = plan.source(out)?.source_semantic(out.budget)?;
                    let function = &source.functions()[row.function.index() as usize];
                    let ty = function.abi().source_input_types()[2];
                    let logical = source.logical_arguments_v1(row.function).unwrap();
                    let argument = logical.source_arguments().nth(2).unwrap();
                    let ArgumentBinding::ExpandedTuple(locals) = argument.binding() else {
                        panic!("expanded source argument");
                    };
                    let entry = SourceFrameEnter::derive(plan, slots, root, instance, out)?;
                    assert_eq!(entry.arguments.len(), 3);
                    assert_eq!(entry.locals.len(), function.locals().len());
                    assert!(
                        matches!(entry.arguments[2], Some(EntryArgument::Expanded {ty: found, first: 2, count}) if found == ty.index() && count == locals.len())
                    );
                    assert_eq!(entry.fields.len(), 2 + locals.len());
                    for (field, local) in locals.iter().enumerate() {
                        let installed = entry.fields[field + 2];
                        assert_eq!(installed.local, row.locals.start + local.index() as usize);
                        assert_eq!(
                            installed.ty,
                            function.locals()[local.index() as usize].ty().index()
                        );
                        assert_eq!(
                            argument_origin(source, function, *local, out)?,
                            fe2o3_pliron::ProductionSemanticSsaEntryOriginV1::RustCallTupleField {
                                argument: 2,
                                field: field as u32
                            }
                        );
                        if matches!(form, Form::NestedTuple) && field == 1 {
                            assert!(
                                matches!(installed.class, Class::Aggregate(ty) if ty == installed.ty)
                            );
                            assert_eq!(
                                slots.aggregate_leaf_count(
                                    SemanticTypeIdV1::from_index(installed.ty),
                                    out
                                )?,
                                Some(2)
                            );
                        }
                    }
                    let before = out.text.len();
                    entry.emit(out)?;
                    let text = &out.text[before..];
                    assert!(text.contains("if arguments.len() != 3 { None }"));
                    assert_eq!(
                        text.matches(" = invocation_source_entry_field_v289(source, arguments[2],")
                            .count(),
                        locals.len()
                    );
                    let wrapper = text
                        .split(&format!(
                            "spec fn invocation_source_enter_{root}_{instance}_v36("
                        ))
                        .nth(1)
                        .unwrap();
                    assert_eq!(
                        wrapper
                            .matches("invocation_source_entry_arguments_")
                            .count(),
                        1
                    );
                    assert!(
                        wrapper.contains("Some(fields) => invocation_source_entry_select_v167")
                    );
                    assert!(!text.contains("invocation_source_value_evaluate_v42("));
                }
            }
            Ok(())
        });
        result.0.unwrap();
        assert_eq!(result.2, FLOOR);
        assert!(result.3 > FLOOR);
    }
}

#[test]
fn original_rust_call_empty_composite_fields_retain_types_distinct_from_unit() {
    let result = run(
        Form::EmptyCompositeFields,
        LIMIT,
        LIMIT,
        |plan, slots, out| {
            let row = plan.instance(0, 1, out)?;
            let source = plan.source(out)?.source_semantic(out.budget)?;
            let function = &source.functions()[row.function.index() as usize];
            let outer = function.abi().source_input_types()[2];
            let logical = source.logical_arguments_v1(row.function).unwrap();
            let argument = logical.source_arguments().nth(2).unwrap();
            let ArgumentBinding::ExpandedTuple(locals) = argument.binding() else {
                panic!("four original RustCall fields");
            };
            assert_eq!(locals.len(), 4);
            let entry = SourceFrameEnter::derive(plan, slots, 0, 1, out)?;
            assert!(matches!(entry.arguments[2], Some(EntryArgument::Expanded {
            ty, first: 2, count: 4,
        }) if ty == outer.index()));
            assert_eq!(entry.fields.len(), 6);
            let mut exact_types = Vec::new();
            for (field, &local) in locals.iter().enumerate() {
                let ty = function.locals()[local.index() as usize].ty();
                exact_types.push(ty);
                let bound = entry.fields[field + 2];
                assert_eq!(bound.local, row.locals.start + local.index() as usize);
                assert_eq!(bound.ty, ty.index());
                assert_eq!(slots.aggregate_leaf_count(ty, out)?, Some(1));
                assert_eq!(
                    slots.aggregate_leaf(ty, 0, out)?.scalar(out)?,
                    ScalarV30::Unit
                );
                let atom = slots.product_component_v282(ty, 0, out)?;
                assert_eq!(atom.source_type(out)?, ty);
                assert!(atom.path(out)?.is_empty());
                if field == 0 {
                    assert!(matches!(
                        source.types()[ty.index() as usize].shape(),
                        Shape::Unit
                    ));
                    assert!(matches!(bound.class, Class::Scalar(0)));
                } else {
                    assert!(matches!(bound.class, Class::Aggregate(found) if found == ty.index()));
                    assert!(matches!(
                        source.types()[ty.index() as usize].shape(),
                        Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { length: 0, .. }
                    ));
                }
            }
            for (ordinal, &ty) in exact_types.iter().enumerate() {
                assert!(!exact_types[..ordinal].contains(&ty));
            }
            for (ty, fields) in [(outer, &locals[..3]), (exact_types[0], locals)] {
                assert!(matches!(
                    expanded_field_binding(
                        function,
                        2,
                        ty,
                        source.types()[outer.index() as usize].shape(),
                        fields,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                        out,
                    ),
                    Err(Error::Statement(
                        "original MIR byte frame entry differs from its exact invocation"
                    ))
                ));
            }
            let before = out.text.len();
            slots.emit(out)?;
            let composite = out.text[before..]
                .split_once("spec fn invocation_source_product_composite_v282(ty: int) -> bool {\n")
                .unwrap()
                .1
                .split_once(" false\n}")
                .unwrap()
                .0;
            assert!(!composite.contains(&format!(" ty == {}int ||", exact_types[0].index())));
            for ty in &exact_types[1..] {
                assert!(composite.contains(&format!(" ty == {}int ||", ty.index())));
            }
            let before = out.text.len();
            entry.emit(out)?;
            let text = &out.text[before..];
            for (field, ty) in exact_types.iter().enumerate() {
                assert!(text.contains(&format!(
                "invocation_source_entry_field_v289(source, arguments[2], {}, {field}, {}, little_endian)",
                outer.index(), ty.index(),
            )));
            }
            assert_eq!(
                text.matches("invocation_source_aggregate_install_v42(entered,")
                    .count(),
                3
            );
            // These checks bind the emitted schema and entry representation. They
            // do not execute the generated spec projector or establish a proof.
            let values = include_str!("original_semantic_mir_source_product_values_v282.vrs");
            assert!(values.contains("else if !invocation_source_product_composite_v282(ty) && components.contains_key(seq![])"));
            assert!(values.contains("let aggregate = InvocationSourceAggregateV42 { source_type: ty, execution_lease: None,"));
            Ok(())
        },
    );
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
    assert!(result.3 > FLOOR);
}

#[test]
fn original_rust_call_field_bindings_reject_wrong_arity_order_types_and_roles() {
    run(Form::ScalarTuple, LIMIT, LIMIT, |plan, _, out| {
        let row = plan.instance(0, 1, out)?;
        let source = plan.source(out)?.source_semantic(out.budget)?;
        let function = &source.functions()[row.function.index() as usize];
        let ty = function.abi().source_input_types()[2];
        let logical = source.logical_arguments_v1(row.function).unwrap();
        let argument = logical.source_arguments().nth(2).unwrap();
        let ArgumentBinding::ExpandedTuple(fields) = argument.binding() else {
            panic!("expanded argument");
        };
        let shape = source.types()[ty.index() as usize].shape();
        expanded_field_binding(
            function,
            2,
            ty,
            shape,
            fields,
            argument.source_ownership(),
            out,
        )?;
        let unit = SemanticTypeIdV1::from_index(
            source
                .types()
                .iter()
                .position(|ty| matches!(ty.shape(), Shape::Unit))
                .unwrap() as u32,
        );
        let wrong_type_shape = Shape::Tuple(SemanticAggregateTypeV1::new(vec![unit; 2]).unwrap());
        let swapped = [fields[1], fields[0]];
        let duplicate = [fields[0], fields[0]];
        let extra = [fields[0], fields[1], fields[1]];
        let wrong_role = [SemanticLocalIdV1::from_index(0), fields[1]];
        for (ordinal, input, shape, fields, ownership) in [
            (1, ty, shape, fields, argument.source_ownership()),
            (2, unit, shape, fields, argument.source_ownership()),
            (
                2,
                ty,
                &wrong_type_shape,
                fields,
                argument.source_ownership(),
            ),
            (2, ty, shape, &fields[..1], argument.source_ownership()),
            (2, ty, shape, &swapped, argument.source_ownership()),
            (2, ty, shape, &duplicate, argument.source_ownership()),
            (2, ty, shape, &extra, argument.source_ownership()),
            (2, ty, shape, &wrong_role, argument.source_ownership()),
            (
                2,
                ty,
                shape,
                fields,
                SemanticSourceArgumentOwnershipV1::Unspecified,
            ),
        ] {
            assert!(matches!(
                expanded_field_binding(function, ordinal, input, shape, fields, ownership, out),
                Err(Error::Statement(
                    "original MIR byte frame entry differs from its exact invocation"
                ))
            ));
        }
        assert!(matches!(
            expanded_field_binding(
                function,
                2,
                ty,
                &Shape::Unit,
                fields,
                argument.source_ownership(),
                out
            ),
            Err(Error::Statement(
                "original MIR byte argument lifetime or payload is not modeled"
            ))
        ));
        assert!(matches!(
            argument_origin(source, function, SemanticLocalIdV1::from_index(0), out),
            Err(Error::Statement(
                "original MIR byte frame entry differs from its exact invocation"
            ))
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_rust_call_projection_preserves_complete_current_atoms_and_recipe_identity() {
    let source = include_str!("original_semantic_mir_source_entry_fields_v289.vrs");
    for required in [
        "invocation_source_product_child_v282(source_type, field) != Some(result_type)",
        "InvocationSourceValueV42::Product(value) => value.source_type == source_type",
        "&& invocation_source_product_complete_v282(value)",
        "&& invocation_source_product_current_v282(source, value, little_endian)",
        "value.execution_lease.is_none() && invocation_source_aggregate_complete_v42(value)",
        "let prefix = seq![field];",
        "invocation_source_path_prefix_v42(prefix, path)",
        ".map(|path: Seq<int>| path.drop_first())",
        "|path: Seq<int>| original[prefix + path]",
        "!invocation_source_product_components_complete_v282(result_type, components)",
        "!invocation_source_product_components_current_v282(source, result_type, components, little_endian)",
        "InvocationSourceProductAtomV282::Descriptor { recipe, .. } => Some(recipe)",
    ] {
        assert!(source.contains(required), "{required}");
    }
    for forbidden in [
        "value_evaluate",
        "put_local",
        "logical_write",
        "loan_identity:",
        "origin_version:",
        "execution_transfer_install",
        "assume(",
        "admit(",
        "external_body",
        "uninterpreted",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    let laws = include_str!("original_semantic_mir_source_entry_fields_v289_tests.vrs");
    assert_eq!(laws.matches("proof fn entry_field_").count(), 11);
    for required in [
        "entry_field_wrong_product_parent_refused_v289",
        "entry_field_stale_nested_lease_refused_v289",
        "entry_field_exact_prefix_domain_v289",
        "entry_field_descriptor_recipe_preserved_v289",
    ] {
        assert!(laws.contains(required));
    }
    macro_rules! h {
        ($ty:ty) => {
            size_of::<$ty>() + 2 * size_of::<Result<$ty>>()
        };
    }
    #[allow(dead_code)]
    struct ArgumentFields {
        local: usize,
        ty: u32,
        class: Class,
        descriptor: Option<usize>,
        bytes: u64,
        alignment: u32,
        object: bool,
    }
    #[allow(dead_code)]
    struct BindingFields {
        argument: usize,
        field: Option<usize>,
        local: usize,
    }
    #[allow(dead_code)]
    enum OriginFields {
        Argument(u32),
        RustCallTupleField { argument: u32, field: u32 },
        ImplicitCapability,
    }
    #[allow(dead_code)]
    enum EntryFields {
        Whole(usize),
        ExpandedEmpty(EmptyExpanded),
        Expanded { ty: u32, first: usize, count: usize },
    }
    macro_rules! same_layout {
        ($actual:ty, $mirror:ty) => {
            assert_eq!(size_of::<$actual>(), size_of::<$mirror>());
            assert_eq!(
                std::mem::align_of::<$actual>(),
                std::mem::align_of::<$mirror>()
            );
        };
    }
    same_layout!(Argument, ArgumentFields);
    same_layout!(FieldBinding, BindingFields);
    same_layout!(EntryArgument, EntryFields);
    same_layout!(
        fe2o3_pliron::ProductionSemanticSsaEntryOriginV1,
        OriginFields
    );
    assert_eq!(
        field_plan_headers(),
        h!(Vec<ArgumentFields>)
            + h!(Vec<BindingFields>)
            + h!(BindingFields)
            + h!(OriginFields)
            + h!(std::slice::Iter<'_, BindingFields>)
            + h!(std::iter::Enumerate<std::slice::Iter<'_, SemanticLocalIdV1>>)
            + h!((usize, &SemanticLocalIdV1))
            + h!(
                std::iter::Enumerate<
                    std::iter::Zip<
                        std::slice::Iter<'_, SemanticLocalIdV1>,
                        std::slice::Iter<'_, SemanticTypeIdV1>,
                    >,
                >
            )
            + h!((usize, (&SemanticLocalIdV1, &SemanticTypeIdV1)))
    );
    assert_eq!(
        projection_headers(),
        h!(&ArgumentFields)
            + h!(&EntryFields)
            + h!(EntryFields)
            + h!(&str)
            + h!(std::slice::Iter<'_, Option<EntryFields>>)
            + h!(std::iter::Enumerate<std::slice::Iter<'_, Option<EntryFields>>>)
            + h!((usize, &Option<EntryFields>))
            + h!(Range<usize>)
            + h!(Class)
            + 8 * h!(usize)
            + 8 * size_of::<&()>()
    );
    assert_eq!(argument_origin_headers(), argument_origin_header_oracle());
}

#[test]
fn original_rust_call_schema_demand_covers_no_local_type_once_per_function() {
    run(Form::EmptyTuple, LIMIT, LIMIT, |plan, slots, out| {
        let row = plan.instance(0, 1, out)?;
        let source = plan.source(out)?.source_semantic(out.budget)?;
        let ty = source.functions()[row.function.index() as usize]
            .abi()
            .source_input_types()[2];
        assert!(
            source
                .functions()
                .iter()
                .all(|function| function.locals().iter().all(|local| local.ty() != ty))
        );
        assert_eq!(slots.aggregate_leaf_count(ty, out)?, Some(1));
        let leaf = slots.aggregate_leaf(ty, 0, out)?;
        assert_eq!(leaf.scalar(out)?, ScalarV30::Unit);
        assert_eq!(leaf.source_type(out)?, ty);

        let mut requested = vector(source.types().len(), out)?;
        let mut seen = vector(source.functions().len(), out)?;
        out.budget
            .charge_work(source.types().len() + source.functions().len())?;
        requested.resize(source.types().len(), false);
        seen.resize(source.functions().len(), false);
        let floor = out.budget.storage();
        let before = out.budget.work();
        super::super::slots::demand_expanded_arguments(
            source,
            row.function,
            &mut requested,
            &mut seen,
            out,
        )?;
        assert!(out.budget.work() > before + 4);
        assert_eq!(out.budget.storage(), floor);
        assert_eq!(requested.iter().filter(|&&yes| yes).count(), 1);
        assert!(requested[ty.index() as usize]);
        for root in 0..2 {
            for instance in 1..3 {
                let repeated = plan.instance(root, instance, out)?;
                assert_eq!(repeated.function, row.function);
                let before = out.budget.work();
                super::super::slots::demand_expanded_arguments(
                    source,
                    repeated.function,
                    &mut requested,
                    &mut seen,
                    out,
                )?;
                assert_eq!(out.budget.work() - before, 4);
                assert_eq!(out.budget.storage(), floor);
            }
        }
        assert_eq!(seen.iter().filter(|&&yes| yes).count(), 1);
        let credit = requested.capacity() + seen.capacity();
        drop(requested);
        drop(seen);
        out.budget.release_storage(credit)?;
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_rust_call_argument_plan_has_checked_capacity_and_exact_resource_boundaries() {
    macro_rules! h {
        ($ty:ty) => {
            size_of::<$ty>() + 2 * size_of::<Result<$ty>>()
        };
    }
    assert_eq!(
        logical_argument_headers(),
        h!(EmptyExpanded)
            + h!(SemanticLogicalArgumentMapV1<'_>)
            + h!((SemanticLogicalArgumentMapV1<'_>, usize))
            + h!(
                std::result::Result<
                    SemanticLogicalArgumentMapV1<'_>,
                    SemanticLogicalArgumentErrorV1,
                >
            )
            + h!(fe2o3_mir_model::SemanticSourceArgumentV1<'_>)
            + h!(ArgumentBinding<'_>)
            + h!((usize, usize))
            + h!(&AdmittedInertSemanticMirV1)
            + h!(SemanticFunctionIdV1)
            + 8 * size_of::<usize>()
            + 8 * size_of::<&()>()
    );
    assert!(matches!(
        logical_argument_storage((usize::MAX, 1)),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        logical_argument_storage((1, usize::MAX)),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    let emit =
        |plan: &InvocationPlan<'_, '_>, slots: &SourceSlots<'_, '_>, out: &mut Writer<'_, '_>| {
            SourceFrameEnter::derive(plan, slots, 0, 1, out)?.emit(out)
        };
    for form in [
        Form::EmptyTuple,
        Form::NonemptyTuple,
        Form::ScalarTuple,
        Form::NestedTuple,
        Form::EmptyCompositeFields,
    ] {
        let measured = run(form, LIMIT, LIMIT, emit);
        measured.0.unwrap();
        let exact = run(form, measured.1, measured.3, emit);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2, exact.3), (measured.1, FLOOR, measured.3));
        let short_work = run(form, measured.1 - 1, measured.3, emit);
        assert!(
            matches!(&short_work.0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1),
            "{short_work:?}"
        );
        let short_storage = run(form, measured.1, measured.3 - 1, emit);
        assert!(
            matches!(&short_storage.0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1),
            "{short_storage:?}"
        );
    }
}
