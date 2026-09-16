use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticSwitchTargetV1, SemanticSwitchTargetsV1};
use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};

fn diamond_owner() -> ProductionSemanticSsaOwnerV1 {
    diamond_owner_from(call_owner(false, ArgumentCallResult::Scalar))
}

pub(super) fn diamond_owner_from(
    original: ProductionSemanticSsaOwnerV1,
) -> ProductionSemanticSsaOwnerV1 {
    diamond_owner_with_prefix(original, None)
}

pub(super) fn diamond_owner_with_prefix(
    original: ProductionSemanticSsaOwnerV1,
    prefix: Option<u32>,
) -> ProductionSemanticSsaOwnerV1 {
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    for (index, function) in functions.iter_mut().enumerate().skip(1) {
        let old = function.blocks();
        let source = function.source();
        let mut locals = function.locals().to_vec();
        let mut continuation = old[2].statements().to_vec();
        let copy = |destination, input, ty| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(destination), vec![], ty)
                        .unwrap(),
                    SemanticRvalueV1::new(
                        ty,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(input), vec![], ty)
                                .unwrap(),
                        )),
                    ),
                )),
            )
        };
        let prefix_statements = match prefix {
            Some(2) => {
                continuation.push(copy(3, 2, PAIR));
                vec![copy(2, 3, PAIR)]
            }
            Some(4) => {
                let observed = locals.len() as u32;
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([201 + index as u8; 32]),
                    ZERO,
                    SemanticLocalRoleV1::Temporary,
                    source,
                ));
                continuation.push(copy(observed, 4, ZERO));
                vec![copy(4, 4, ZERO)]
            }
            None => vec![],
            _ => panic!("known prefix local"),
        };
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let source_call = match old[0].terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => call,
            _ => unreachable!(),
        };
        let returning = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                source_call.callee(),
                source_call.arguments().to_vec(),
                Some(SemanticCallDestinationV1::new(
                    source_call.destination().unwrap().place().clone(),
                    edge(SemanticEdgeRoleV1::CallReturn, 3),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
        let first = SemanticBasicBlockV1::new(
            old[0].identity(),
            source,
            old[0].statements().to_vec(),
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], U32)
                            .unwrap(),
                    ),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
        )
        .unwrap();
        let blocks = vec![
            first,
            SemanticBasicBlockV1::new(
                old[1].identity(),
                source,
                prefix_statements.clone(),
                SemanticTerminatorV1::new(source, returning.clone()),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([49 + 20 * index as u8; 32]),
                source,
                prefix_statements,
                SemanticTerminatorV1::new(source, returning),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                old[2].identity(),
                source,
                continuation,
                old[2].terminator().clone(),
            )
            .unwrap(),
        ];
        let replacement = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            source,
            function.abi().clone(),
            locals,
            function.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone());
        *function = replacement;
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn call_result_phi_transport_borrows_the_original_plan_and_exact_physical_slot() {
    let owner = materialize(diamond_owner());
    let root = SemanticFunctionIdV1::from_index(1);
    let plan = owner.semantic_ssa.plan_for_function(root).unwrap().plan();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    for block in [1, 2] {
        let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(block), 0);
        owner
            .with_checked_call_v1(
                root,
                root,
                SemanticBlockIdV1::from_index(block),
                &mut budget,
                |view| {
                    let transport = view
                        .result_transport(0)
                        .expect("diamond requires the result phi");
                    assert!(transport.conversion().is_none());
                    assert_eq!(transport.value(), view.operation().results[0].id);
                    let Some(Terminator::Branch { target, arguments }) = &view.block().terminator
                    else {
                        panic!("returning call");
                    };
                    assert_eq!(target.0, 3);
                    assert_eq!(arguments[transport.slot() as usize], transport.value());
                    let function = owner
                        .executable
                        .module()
                        .function(&view.caller().kernel_ir_function)
                        .unwrap();
                    let successor = function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .find(|block| block.id == *target)
                        .unwrap();
                    assert_eq!(
                        successor.parameters[transport.slot() as usize].ty,
                        Type::Scalar(ScalarType::U32)
                    );
                    assert!(std::ptr::eq(
                        view.edge_definitions()?,
                        plan.edge_definitions(edge).unwrap()
                    ));
                    assert!(std::ptr::eq(
                        view.edge_arguments()?,
                        plan.edge_arguments(edge).unwrap()
                    ));
                    inspect(view, ArgumentCallResult::Scalar)
                },
            )
            .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn call_result_transport_rejects_missing_wrong_slot_and_invalid_conversion() {
    let source = diamond_owner();
    let limits = ProductionSemanticKirLimitsV1::default();
    let (graph, rows) = lower_argument_owner(&source, limits).unwrap();
    for mutation in 0..3 {
        let mut changed = rows.clone();
        let Some(SemanticKirCallReturnKindV1::Call {
            transport,
            destination_end,
            ..
        }) = changed
            .call_returns
            .iter_mut()
            .map(|row| &mut row.kind)
            .find(|kind| {
                matches!(
                    kind,
                    SemanticKirCallReturnKindV1::Call {
                        transport: CallComponentSpanV1 { count: 1, .. },
                        ..
                    }
                )
            })
        else {
            panic!("missing phi anchor");
        };
        if mutation == 0 {
            *transport = CallComponentSpanV1::EMPTY;
        } else {
            let CallResultComponentV1::Transport { slot, conversion } =
                &mut changed.call_result_components[transport.first as usize]
            else {
                panic!("wrong component kind");
            };
            if mutation == 1 {
                *slot = u32::MAX;
            } else {
                *conversion = Some(*destination_end);
            }
        }
        check_both(
            &source,
            &graph,
            source.source_semantic().roots(),
            limits.max_blocks,
            &changed,
            Expected::CorrespondenceMismatch,
        );
    }
}
