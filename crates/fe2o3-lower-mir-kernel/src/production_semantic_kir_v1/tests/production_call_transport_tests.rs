use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticSwitchTargetV1, SemanticSwitchTargetsV1};
use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};

fn diamond_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = call_owner(false, ArgumentCallResult::Scalar);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    for (index, function) in functions.iter_mut().enumerate().skip(1) {
        let old = function.blocks();
        let source = function.source();
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
                vec![],
                SemanticTerminatorV1::new(source, returning.clone()),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([49 + 20 * index as u8; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, returning),
            )
            .unwrap(),
            old[2].clone(),
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
            function.locals().to_vec(),
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
                        .result_transport()
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
                        transport: Some(_),
                        ..
                    }
                )
            })
        else {
            panic!("missing phi anchor");
        };
        match mutation {
            0 => *transport = None,
            1 => transport.as_mut().unwrap().slot = u32::MAX,
            _ => transport.as_mut().unwrap().conversion = Some(*destination_end),
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
