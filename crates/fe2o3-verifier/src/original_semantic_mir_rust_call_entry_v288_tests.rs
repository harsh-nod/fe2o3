use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 100_000_000;
const FLOOR: usize = super::super::super::invocations::tests::FLOOR;

#[derive(Clone, Copy)]
enum Form {
    Unit,
    EmptyTuple,
    NonemptyTuple,
}

fn transform(form: Form, types: &mut Vec<SemanticTypeDeclV1>, functions: &mut Vec<Function>) {
    let unit = SemanticTypeIdV1::from_index(
        types
            .iter()
            .position(|ty| matches!(ty.shape(), Shape::Unit))
            .unwrap() as u32,
    );
    let argument = match form {
        Form::Unit => unit,
        Form::EmptyTuple | Form::NonemptyTuple => {
            let fields = if matches!(form, Form::NonemptyTuple) {
                vec![unit]
            } else {
                vec![]
            };
            let ty = SemanticTypeIdV1::from_index(types.len() as u32);
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([247; 32]),
                SemanticLayoutIdentityV1::from_sha256([248; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![0; fields.len()], vec![]).unwrap(),
                )
                .unwrap(),
                Shape::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
            ));
            ty
        }
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
            if matches!(form, Form::NonemptyTuple) {
                adjusted.push(SemanticAbiArgumentV1::rust_call_tuple_field(
                    0,
                    SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
                ));
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([249; 32]),
                    unit,
                    LocalRole::RustCallTupleField {
                        argument: 2,
                        field: 0,
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
            for (block_index, block) in blocks.iter_mut().enumerate() {
                if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                    assert_eq!(call.callee().index() as usize, helper);
                    let mut arguments = call.arguments().to_vec();
                    let mut statements = block.statements().to_vec();
                    let operand = if matches!(form, Form::NonemptyTuple) {
                        let local = SemanticLocalIdV1::from_index(locals.len() as u32);
                        locals.push(SemanticLocalDeclV1::new(
                            SemanticLocalIdentityV1::from_sha256(
                                [u8::try_from(250 + block_index).unwrap(); 32],
                            ),
                            argument,
                            LocalRole::Temporary,
                            prior.source(),
                        ));
                        let place = SemanticPlaceV1::new(local, vec![], argument).unwrap();
                        statements.push(SemanticStatementV1::new(
                            prior.source(),
                            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                place.clone(),
                                SemanticRvalueV1::new(
                                    argument,
                                    SemanticRvalueKindV1::Aggregate(
                                        SemanticAggregateRvalueV1::new(
                                            SemanticAggregateKindV1::Tuple,
                                            vec![SemanticOperandV1::Constant(
                                                SemanticConstantV1::new(
                                                    unit,
                                                    SemanticConstantValueV1::ZeroSized,
                                                ),
                                            )],
                                        )
                                        .unwrap(),
                                    ),
                                ),
                            )),
                        ));
                        SemanticOperandV1::Move(place)
                    } else {
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            argument,
                            SemanticConstantValueV1::ZeroSized,
                        ))
                    };
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
                        Form::NonemptyTuple => unreachable!(),
                    };
                    assert!(
                        matches!(entry.arguments[2], Some(EntryArgument::ExpandedEmpty(actual)) if actual == expected)
                    );
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
fn original_rust_call_nonempty_expansion_remains_an_explicit_typed_refusal() {
    run(Form::NonemptyTuple, LIMIT, LIMIT, |plan, slots, out| {
        let row = plan.instance(0, 1, out)?;
        assert!(matches!(SourceFrameEnter::derive(plan, slots, 0, 1, out), Err(Error::SourceEntry {
            root: 0, instance: 1, function: Some(function), argument: Some((2, _)), local: None,
            phase: "source-entry-expanded-argument", reason: "original MIR byte argument lifetime or payload is not modeled",
        }) if function == row.function.index()));
        Ok(())
    }).0.unwrap();
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
    let measured = run(Form::EmptyTuple, LIMIT, LIMIT, emit);
    measured.0.unwrap();
    let exact = run(Form::EmptyTuple, measured.1, measured.3, emit);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, FLOOR, measured.3));
    let short_work = run(Form::EmptyTuple, measured.1 - 1, measured.3, emit);
    assert!(
        matches!(&short_work.0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1),
        "{short_work:?}"
    );
    let short_storage = run(Form::EmptyTuple, measured.1, measured.3 - 1, emit);
    assert!(
        matches!(&short_storage.0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1),
        "{short_storage:?}"
    );
}
