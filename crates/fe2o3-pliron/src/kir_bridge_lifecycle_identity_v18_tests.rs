use super::*;
use crate::ProductionAnalysisResourceLimitsV1 as Limits;
use crate::{
    LivePlironStructuralIdentityProviderV1,
    PlironStructuralIdentityProviderV1,
};
use fe2o3_kernel_ir::{
    BasicBlock as KirBlock, CanonicalKernelIrWorkBudgetV1 as Work, ExecutionOperationV15 as E,
    ExecutionRoleV15 as R, Function, Kernel, LaunchDomain, LaunchExtent, Signature,
    StorageLayoutLimitsV1, ValueDef,
};
use pliron::builtin::op_interfaces::OneRegionInterface;

const AMPLE: usize = 1 << 40;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

fn fixture() -> Module {
    let mut module = Module::new("lifecycle-identity-controls");
    for name in ["first", "second"] {
        let mut block = KirBlock::new(BlockId(0));
        block.operations = vec![
            KirOperation::new(
                vec![ValueDef::new(ValueId(0), Type::Execution(R::Context))],
                OperationKind::Execution(E::ContextIssue),
            ),
            KirOperation::new(
                vec![ValueDef::new(ValueId(1), Type::Execution(R::Workgroup))],
                OperationKind::Execution(E::WorkgroupDerive {
                    context: ValueId(0),
                }),
            ),
            KirOperation::new(
                vec![],
                OperationKind::Execution(E::ScopeEnd {
                    workgroup: ValueId(1),
                    discarded: vec![],
                }),
            ),
        ];
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            name,
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            name,
            name,
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
    }
    module
}

fn with_graph(run: impl FnOnce(&mut KirPlironGraphV18<'_>, &mut Budget<'_>)) {
    with_module_graph(&fixture(), run)
}

fn with_module_graph(
    module: &Module,
    run: impl FnOnce(&mut KirPlironGraphV18<'_>, &mut Budget<'_>),
) {
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    budget.reserve_storage(19).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (mut graph, receipt) = KirPlironGraphV18::import(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    run(&mut graph, &mut budget);
}

#[test]
fn exact_lifecycle_identity_rejects_foreign_function_occurrence_and_ordinary_entry() {
    with_graph(|graph, budget| {
        let epoch = graph.ranked_policy_epoch_v18().unwrap();
        let mut visits = 0;
        graph.visit_ranked_policy_functions_v18(epoch, budget, |_, input| {
            let context = input.context();
            let function = input.function();
            let region = function.get_region(context);
            let block = region.deref(context).iter(context).next().unwrap();
            let block_ref = block.deref(context);
            let mut iter = block_ref.iter(context);
            let operations = [iter.next().unwrap(), iter.next().unwrap(), iter.next().unwrap(), iter.next().unwrap()];
            assert!(iter.next().is_none());
            for operation in &operations {
                assert!(input.operation(context, *operation).is_some());
            }
            let actual = LivePlironStructuralIdentityProviderV1::lifecycle_v18(input)
                .capture_with_resource_limits_v1(Limits::production_hard_ceiling()).ok().expect("exact lifecycle identity");
            assert_eq!(actual.input_census.operations, 4);
            assert!(matches!(LivePlironStructuralIdentityProviderV1::new(context, function)
                .capture_with_resource_limits_v1(Limits::production_hard_ceiling()),
                Err(crate::IdentityCaptureFailureV1::Unavailable { source_code: "FE2O3-PRESERVE-001", .. })));
            let foreign_context = Context::new();
            assert!(!input.authenticate(&foreign_context, function));
            let parent = function.get_operation().deref(context).get_parent_block().unwrap();
            let other = parent.deref(context).iter(context).find(|pointer| *pointer != function.get_operation()).unwrap();
            let other = Operation::get_op::<FuncOp>(other, context).unwrap();
            assert!(!input.authenticate(context, &other));
            let other_block = other.get_region(context).deref(context).iter(context).next().unwrap();
            let other_operation = other_block.deref(context).iter(context).next().unwrap();
            assert!(input.operation(context, other_operation).is_none());
            assert!(!input.attribute(context, operations[0], "wrong", "gpu", "preserved_operation_kind"));
            visits += 1;
            Ok(())
        }).unwrap();
        assert_eq!(visits, 2);
    });
}

#[test]
fn lifecycle_identity_keeps_epoch_and_exact_native_role_types_independent() {
    for wrong_type in [false, true] {
        with_graph(|graph, budget| {
            let epoch = graph.ranked_policy_epoch_v18().unwrap();
            let mut completed = false;
            let result = graph.visit_ranked_policy_functions_v18(epoch, budget, |_, input| {
                let context = input.context();
                let block = input
                    .function()
                    .get_region(context)
                    .deref(context)
                    .iter(context)
                    .next()
                    .unwrap();
                let pointer = block.deref(context).iter(context).next().unwrap();
                let value = pointer.deref(context).get_result(0);
                assert!(input.operation(context, pointer).is_some());
                let old = value.get_type(context);
                let replacement = if wrong_type {
                    use dialect_gpu::storage_types_v18::{
                        ExecutionRoleTypeV18, StorageOrdinalAttrV18,
                    };
                    ExecutionRoleTypeV18::get(
                        context,
                        StorageOrdinalAttrV18(2),
                        StorageOrdinalAttrV18(0),
                        StorageOrdinalAttrV18(0),
                    )
                    .into()
                } else {
                    old
                };
                value.set_type(context, replacement);
                assert!(!input.authenticate(context, input.function()));
                assert!(input.operation(context, pointer).is_none());
                // Inert internal rebasing isolates the schema check from the
                // independent mandatory mutation-attempt gate above.
                let rebased = NativeLifecycleIdentityAdmissionV18 {
                    epoch: context.ir_mutation_attempt_epoch().unwrap().value(),
                    ..*input
                };
                assert_eq!(rebased.operation(context, pointer).is_some(), !wrong_type);
                completed = true;
                Ok(())
            });
            assert!(matches!(result, Err(Failure::Mutation)));
            assert!(completed);
        });
    }
}

#[test]
fn lifecycle_identity_lookup_bound_and_capture_exact_cuts_are_checked() {
    assert_eq!(
        std::mem::size_of::<NativeLifecycleIdentityAdmissionV18<'_>>(),
        6 * std::mem::size_of::<usize>()
    );
    with_graph(|graph, budget| {
        let epoch = graph.ranked_policy_epoch_v18().unwrap();
        graph.visit_ranked_policy_functions_v18(epoch, budget, |_, input| {
            // Two source functions/blocks, six preserved rows, four SSA values,
            // four operations including Return per function, eight fixed probes.
            assert_eq!(input.identity_lookup_work(), Some(5 * (2 + 2 + 6 + 4 + 8) * 512));
            assert!(NativeLifecycleIdentityAdmissionV18 { operations: usize::MAX, ..*input }.identity_lookup_work().is_none());
            let full = LivePlironStructuralIdentityProviderV1::lifecycle_v18(input)
                .capture_with_resource_limits_v1(Limits::production_hard_ceiling()).ok().expect("calibrated complete capture");
            let bound = full.resource_upper_bound;
            let work = bound.work_upper_bound();
            let peak = bound.peak_storage_upper_bound();
            drop(full);
            let exact = LivePlironStructuralIdentityProviderV1::lifecycle_v18(input)
                .capture_with_resource_limits_v1(Limits::new(work, peak)).ok().expect("exact complete capture bound");
            assert_eq!(exact.resource_upper_bound, bound);
            drop(exact);
            for (limits, resource) in [(Limits::new(work - 1, peak), "work upper bound"), (Limits::new(work, peak - 1), "peak storage upper bound")] {
                let Err(crate::IdentityCaptureFailureV1::ResourceLimit(error)) =
                    LivePlironStructuralIdentityProviderV1::lifecycle_v18(input).capture_with_resource_limits_v1(limits)
                else { panic!("short identity capture did not retain its resource refusal") };
                assert_eq!(error.resource, resource);
                assert_eq!(error.phase, crate::ProductionAnalysisResourcePhaseV1::StructuralIdentity);
            }
            Ok(())
        }).unwrap();
    });
}

#[test]
fn lifecycle_identity_does_not_admit_real_tile_operations_or_nonempty_scope_end() {
    let mut module = fixture();
    for function in &mut module.functions {
        function.signature.parameters = vec![
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::INDEX,
        ];
        let body = function.body.as_mut().unwrap();
        body.parameters = vec![ValueId(2), ValueId(3)];
        let operations = &mut body.blocks[0].operations;
        operations.insert(
            2,
            KirOperation::new(
                vec![ValueDef::new(
                    ValueId(4),
                    Type::Execution(R::MaskedTileU32 {
                        lanes: 64,
                        elements: 1,
                    }),
                )],
                OperationKind::Execution(E::MaskedTileLoadU32 {
                    workgroup: ValueId(1),
                    input: ValueId(2),
                    base: ValueId(3),
                    lanes: 64,
                    elements: 1,
                }),
            ),
        );
        operations[3].kind = OperationKind::Execution(E::ScopeEnd {
            workgroup: ValueId(1),
            discarded: vec![ValueId(4)],
        });
    }
    with_module_graph(&module, |graph, budget| {
        let epoch = graph.ranked_policy_epoch_v18().unwrap();
        let mut checked = 0;
        graph
            .visit_ranked_policy_functions_v18(epoch, budget, |_, input| {
                let context = input.context();
                let block = input
                    .function()
                    .get_region(context)
                    .deref(context)
                    .iter(context)
                    .next()
                    .unwrap();
                let block = block.deref(context);
                let mut operations = block.iter(context);
                assert!(
                    input
                        .operation(context, operations.next().unwrap())
                        .is_some()
                );
                assert!(
                    input
                        .operation(context, operations.next().unwrap())
                        .is_some()
                );
                let tile = operations.next().unwrap();
                let end = operations.next().unwrap();
                assert!(input.operation(context, tile).is_none());
                assert!(input.operation(context, end).is_none());
                checked += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(checked, 2);
    });
}

#[test]
fn lifecycle_identity_requires_the_exact_preserved_occurrence_record() {
    for substitute in [false, true] {
        with_graph(|graph, budget| {
            let context = &graph.session.context;
            let target = *graph
                .origins
                .preserved_operations
                .iter()
                .find(|(_, kind)| matches!(kind, OperationKind::Execution(E::ContextIssue)))
                .unwrap()
                .0;
            let block = target.deref(context).get_parent_block().unwrap();
            let ordinal = graph.origins.blocks[&block].0;
            let epoch = graph.ranked_policy_epoch_v18().unwrap();
            graph
                .visit_ranked_policy_functions_v18(epoch, budget, |index, input| {
                    if index == ordinal {
                        assert!(input.operation(input.context(), target).is_some());
                    }
                    Ok(())
                })
                .unwrap();
            let original = graph.origins.preserved_operations.remove(&target).unwrap();
            if substitute {
                graph.origins.preserved_operations.insert(
                    target,
                    OperationKind::Execution(E::WorkgroupDerive {
                        context: ValueId(999),
                    }),
                );
            }
            let mut refused = false;
            graph
                .visit_ranked_policy_functions_v18(epoch, budget, |index, input| {
                    if index == ordinal {
                        assert!(input.operation(input.context(), target).is_none());
                        refused = true;
                    }
                    Ok(())
                })
                .unwrap();
            assert!(refused);
            graph.origins.preserved_operations.insert(target, original);
            graph
                .visit_ranked_policy_functions_v18(epoch, budget, |index, input| {
                    if index == ordinal {
                        assert!(input.operation(input.context(), target).is_some());
                    }
                    Ok(())
                })
                .unwrap();
        });
    }
}

fn unreachable_fixture() -> Module {
    let mut module = fixture();
    for function in &mut module.functions {
        function.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
    }
    module
}

#[test]
fn exact_unreachable_identity_is_foreign_closed_and_independently_metered() {
    with_module_graph(&unreachable_fixture(), |graph, budget| {
        let epoch = graph.ranked_policy_epoch_v18().unwrap();
        let mut visits = 0;
        graph
            .visit_ranked_policy_functions_v18(epoch, budget, |_, input| {
                let context = input.context();
                let function = input.function();
                let block = function
                    .get_region(context)
                    .deref(context)
                    .iter(context)
                    .next()
                    .unwrap();
                let terminal = block.deref(context).get_terminator(context).unwrap();
                assert!(input.operation(context, terminal).is_some());
                assert!(input.unreachable(context, terminal));
                assert_eq!(terminal.deref(context).get_num_successors(), 0);
                assert!(input.attribute(
                    context,
                    terminal,
                    "gpu_preserved_terminator_kind",
                    "gpu",
                    "preserved_terminator_kind"
                ));
                assert!(!input.attribute(
                    context,
                    terminal,
                    "gpu_preserved_operation_kind",
                    "gpu",
                    "preserved_operation_kind"
                ));
                let foreign = Context::new();
                assert!(!input.unreachable(&foreign, terminal));
                let parent = function
                    .get_operation()
                    .deref(context)
                    .get_parent_block()
                    .unwrap();
                let other = parent
                    .deref(context)
                    .iter(context)
                    .find(|pointer| *pointer != function.get_operation())
                    .unwrap();
                let other = Operation::get_op::<FuncOp>(other, context).unwrap();
                let block = other
                    .get_region(context)
                    .deref(context)
                    .iter(context)
                    .next()
                    .unwrap();
                assert!(!input.unreachable(
                    context,
                    block.deref(context).get_terminator(context).unwrap()
                ));
                assert!(matches!(
                    LivePlironStructuralIdentityProviderV1::new(context, function)
                        .capture_with_resource_limits_v1(Limits::production_hard_ceiling()),
                    Err(crate::IdentityCaptureFailureV1::Unavailable {
                        source_code: "FE2O3-PRESERVE-001",
                        ..
                    })
                ));
                // Functions2 + blocks2 + preserved operations6 + terminals2 +
                // values4 + fixed8, times the five-visit envelope.
                assert_eq!(
                    input.identity_lookup_work(),
                    Some(5 * (2 + 2 + 6 + 2 + 4 + 8) * 512)
                );
                let full = LivePlironStructuralIdentityProviderV1::lifecycle_v18(input)
                    .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
                    .ok()
                    .expect("exact imported zero-edge terminal");
                let bound = full.resource_upper_bound;
                assert_eq!(full.input_census.operations, 4);
                drop(full);
                let work = bound.work_upper_bound();
                let peak = bound.peak_storage_upper_bound();
                let exact = LivePlironStructuralIdentityProviderV1::lifecycle_v18(input)
                    .capture_with_resource_limits_v1(Limits::new(work, peak))
                    .ok()
                    .expect("exact terminal identity budget");
                assert_eq!(exact.resource_upper_bound, bound);
                drop(exact);
                for (limits, resource) in [
                    (Limits::new(work - 1, peak), "work upper bound"),
                    (Limits::new(work, peak - 1), "peak storage upper bound"),
                ] {
                    let Err(crate::IdentityCaptureFailureV1::ResourceLimit(error)) =
                        LivePlironStructuralIdentityProviderV1::lifecycle_v18(input)
                            .capture_with_resource_limits_v1(limits)
                    else {
                        panic!("short terminal identity capture lost its exact refusal");
                    };
                    assert_eq!(
                        error.phase,
                        crate::ProductionAnalysisResourcePhaseV1::StructuralIdentity
                    );
                    assert_eq!(error.resource, resource);
                }
                visits += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(visits, 2);
    });
}

#[test]
fn unreachable_identity_rechecks_attempt_kind_attributes_and_all_zero_arities() {
    use pliron::builtin::attributes::{OperandSegmentSizesAttr, StringAttr};
    for fault in 0..7 {
        with_module_graph(&unreachable_fixture(), |graph, budget| {
            let epoch = graph.ranked_policy_epoch_v18().unwrap();
            let mut completed = false;
            let result = graph.visit_ranked_policy_functions_v18(epoch, budget, |_, input| {
                let context = input.context();
                let block = input
                    .function()
                    .get_region(context)
                    .deref(context)
                    .iter(context)
                    .next()
                    .unwrap();
                let terminal = block.deref(context).get_terminator(context).unwrap();
                assert!(input.unreachable(context, terminal));
                let first = block.deref(context).iter(context).next().unwrap();
                match fault {
                    0 => {
                        let attributes = terminal.deref(context).attributes.clone();
                        terminal.deref_mut(context).attributes = attributes;
                    }
                    1 => terminal.deref_mut(context).attributes.set(
                        "gpu_preserved_terminator_kind".try_into().unwrap(),
                        PreservedTerminatorKindAttr::Switch,
                    ),
                    2 => terminal.deref_mut(context).attributes.set(
                        "unexpected".try_into().unwrap(),
                        StringAttr::new("changed".into()),
                    ),
                    3 => terminal.deref_mut(context).attributes.set(
                        "operand_segment_sizes".try_into().unwrap(),
                        OperandSegmentSizesAttr(vec![0]),
                    ),
                    4 => {
                        let value = first.deref(context).get_result(0);
                        Operation::push_operand(terminal, context, value);
                    }
                    5 => {
                        let ty = first.deref(context).get_type(0);
                        Operation::push_result(terminal, context, ty);
                    }
                    6 => {
                        Operation::push_successor(terminal, context, block);
                    }
                    _ => unreachable!(),
                }
                assert!(!input.authenticate(context, input.function()));
                assert!(input.operation(context, terminal).is_none());
                // Internal inert rebasing isolates each shape predicate;
                // production never reissues identity after a mutation attempt.
                let rebased = NativeLifecycleIdentityAdmissionV18 {
                    epoch: context.ir_mutation_attempt_epoch().unwrap().value(),
                    ..*input
                };
                assert_eq!(rebased.unreachable(context, terminal), fault == 0);
                assert_eq!(rebased.operation(context, terminal).is_some(), fault == 0);
                completed = true;
                Ok(())
            });
            assert!(completed);
            assert!(matches!(result, Err(Failure::Mutation)));
        });
    }
}

#[test]
fn unreachable_identity_requires_the_original_terminal_record_not_its_name() {
    for substitute in [false, true] {
        with_module_graph(&unreachable_fixture(), |graph, budget| {
            let target = *graph.origins.preserved_terminators.keys().next().unwrap();
            let block = target
                .deref(&graph.session.context)
                .get_parent_block()
                .unwrap();
            let ordinal = graph.origins.blocks[&block].0;
            let epoch = graph.ranked_policy_epoch_v18().unwrap();
            graph
                .visit_ranked_policy_functions_v18(epoch, budget, |index, input| {
                    if index == ordinal {
                        assert!(input.unreachable(input.context(), target));
                    }
                    Ok(())
                })
                .unwrap();
            let original = graph.origins.preserved_terminators.remove(&target).unwrap();
            if substitute {
                graph
                    .origins
                    .preserved_terminators
                    .insert(target, Terminator::Return { values: vec![] });
            }
            let mut refused = false;
            graph
                .visit_ranked_policy_functions_v18(epoch, budget, |index, input| {
                    if index == ordinal {
                        assert!(!input.unreachable(input.context(), target));
                        assert!(input.operation(input.context(), target).is_none());
                        refused = true;
                    }
                    Ok(())
                })
                .unwrap();
            assert!(refused);
            graph.origins.preserved_terminators.insert(target, original);
            graph
                .visit_ranked_policy_functions_v18(epoch, budget, |index, input| {
                    if index == ordinal {
                        assert!(input.unreachable(input.context(), target));
                    }
                    Ok(())
                })
                .unwrap();
        });
    }
}

#[test]
fn ordinary_identity_does_not_accept_an_unreachable_only_function() {
    let mut module = unreachable_fixture();
    for function in &mut module.functions {
        function.body.as_mut().unwrap().blocks[0].operations.clear();
    }
    with_module_graph(&module, |graph, budget| {
        let epoch = graph.ranked_policy_epoch_v18().unwrap();
        let mut visits = 0;
        graph
            .visit_ranked_policy_functions_v18(epoch, budget, |_, input| {
                let context = input.context();
                let function = input.function();
                let block = function
                    .get_region(context)
                    .deref(context)
                    .iter(context)
                    .next()
                    .unwrap();
                let terminal = block.deref(context).get_terminator(context).unwrap();
                assert!(input.unreachable(context, terminal));
                assert!(matches!(
                    LivePlironStructuralIdentityProviderV1::new(context, function)
                        .capture_with_resource_limits_v1(Limits::production_hard_ceiling()),
                    Err(crate::IdentityCaptureFailureV1::Unavailable {
                        source_code: "FE2O3-PRESERVE-001",
                        ..
                    })
                ));
                let exact = LivePlironStructuralIdentityProviderV1::lifecycle_v18(input)
                    .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
                    .ok()
                    .expect("private exact terminal identity");
                assert_eq!(exact.input_census.operations, 1);
                visits += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(visits, 2);
    });
}
