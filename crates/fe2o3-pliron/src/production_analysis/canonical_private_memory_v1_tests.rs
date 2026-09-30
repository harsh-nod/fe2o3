use super::super::super::tests::with_checked;
use super::super::tests::fixture;
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function,
    Kernel, MemoryAccess, Module, Operation, ScalarType, Signature, ValueDef, ValueId,
};

pub(super) fn split() -> Module {
    let mut module = fixture();
    let body = module.functions[1].body.as_mut().unwrap();
    let operations = body.blocks[0].operations.split_off(3);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(37),
        arguments: vec![],
    });
    let mut second = BasicBlock::new(BlockId(37));
    second.operations = operations;
    second.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(second);
    module
}

pub(super) fn limits() -> MemoryLimits {
    MemoryLimits { max_cells: 64 }
}

fn reports(
    policies: &CheckedCanonicalPrivateMemoryPoliciesV1<'_, '_>,
    budget: &mut Budget<'_>,
    expected: usize,
) -> Result<(), Failure> {
    let view = policies.trap_policies(budget)?;
    assert_eq!(view.definition_count(budget)?, expected);
    for ordinal in 0..expected {
        let report = view.report(ordinal, budget)?;
        assert!(report.reports().is_clean());
        assert_eq!(report.paired_stage_count(), 9);
        assert_eq!(report.reports().pass_order(),
            &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
        assert_eq!(report.reports().preservation().certificates().len(), 9);
        assert_eq!(report.reports().report_validation().stages().len(), 9);
        assert_eq!(
            view.history(ordinal, budget)?.function(),
            view.definition_coordinate(ordinal, budget)?.0 as usize
        );
        assert!(!report.grants_artifact_or_launch_authority());
    }
    assert_eq!(policies.pending_obligations().iter().count(), 19);
    assert!(!policies.ranked_verification_is_complete());
    assert!(!policies.grants_artifact_or_launch_authority());
    Ok(())
}

#[test]
fn physical_cross_block_keeps_exact_owner_roots_and_real_nine_reports() {
    let module = split();
    with_checked(&module, |checked, budget| {
        let inventory = checked.inventory(budget).unwrap();
        let inventory_address = std::ptr::from_ref(inventory);
        let owner = inventory.owner();
        let bytes = owner.canonical().canonical_bytes().to_vec();
        let symbols = owner
            .module()
            .functions
            .iter()
            .map(|f| f.id.as_str().to_owned())
            .collect::<Vec<_>>();
        let floor = budget.storage();
        with_canonical_private_memory_policy_checks_v1(
            checked,
            limits(),
            budget,
            |view, budget| {
                assert!(std::ptr::eq(view.owner(budget)?, owner));
                assert_eq!(view.owner(budget)?.canonical().canonical_bytes(), bytes);
                assert_eq!(view.owner(budget)?.module().kernels, module.kernels);
                assert_eq!(
                    view.owner(budget)?
                        .module()
                        .functions
                        .iter()
                        .map(|f| f.id.as_str().to_owned())
                        .collect::<Vec<_>>(),
                    symbols
                );
                let physical = view.physical_memory(budget)?;
                assert!(std::ptr::eq(physical.inventory(), inventory_address));
                assert_eq!(physical.latest_stores(), &[None, None, None, None, Some(3)]);
                assert!(physical.operation(2) && physical.operation(3) && physical.operation(4));
                assert!(!physical.operation(0) && !physical.operation(1));
                assert!(!physical.grants_authority());
                reports(view, budget, 2)
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn old_ordinary_and_census_entries_do_not_acquire_cross_block_permission() {
    with_checked(&split(), |checked, budget| {
        let floor = budget.storage();
        let ordinary =
            with_canonical_ranked_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            ordinary.failure(),
            Failure::UnsupportedGraph { .. }
        ));
        let private =
            with_canonical_private_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            private.failure(),
            Failure::PrivateRequirement {
                requirement: CanonicalPrivateRequirementV1::CompleteCells,
                ..
            }
        ));
        let trap = traps::with_canonical_trap_policy_checks_v1(checked, budget, |_, _| Ok(()))
            .unwrap_err();
        assert!(matches!(
            trap.failure(),
            Failure::PrivateRequirement {
                requirement: CanonicalPrivateRequirementV1::CompleteCells,
                ..
            }
        ));
        assert_eq!(budget.storage(), floor);
    });
}

fn call_in_interval() -> Module {
    let mut module = fixture();
    let ty = Type::Scalar(ScalarType::U64);
    let mut block = BasicBlock::new(BlockId(61));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(80)],
    });
    module.functions.push(Function::internal_helper(
        "scalar_leaf",
        Signature::new(vec![ty.clone()], vec![ty.clone()]),
        vec![ValueId(80)],
        vec![block],
    ));
    module.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            3,
            Operation::effect_free(
                ValueDef::new(ValueId(13), ty),
                Kind::Call {
                    callee: "scalar_leaf".into(),
                    arguments: vec![ValueId(10)],
                },
            ),
        );
    module
}

#[test]
fn nonescaping_typed_call_inside_memory_interval_uses_all_three_real_definitions() {
    let module = call_in_interval();
    with_checked(&module, |checked, budget| {
        let old =
            with_canonical_private_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            old.failure(),
            Failure::PrivateRequirement {
                requirement: CanonicalPrivateRequirementV1::CompleteCells,
                ..
            }
        ));
        with_canonical_private_memory_policy_checks_v1(
            checked,
            limits(),
            budget,
            |view, budget| {
                let physical = view.physical_memory(budget)?;
                assert_eq!(
                    physical.latest_stores(),
                    &[None, None, None, None, None, Some(3)]
                );
                let actual = physical.inventory();
                assert_eq!(actual.calls().len(), 2);
                let second = &actual.calls()[1];
                assert_eq!(second.callee, "scalar_leaf");
                assert_eq!(second.target.unwrap().0, 2);
                reports(view, budget, 3)
            },
        )
        .unwrap();
    });
}

#[test]
fn constant_extent_and_nonzero_element_address_keep_exact_physical_anchor() {
    let mut module = fixture();
    let body = &mut module.functions[1].body.as_mut().unwrap().blocks[0];
    body.operations.insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(13), Type::INDEX),
            Kind::Constant(Constant::Index(2)),
        ),
    );
    if let Kind::Alloca { count, .. } = &mut body.operations[2].kind {
        *count = Some(ValueId(13));
    }
    let pointer = body.operations[2].results[0].ty.clone();
    body.operations.splice(
        3..3,
        [
            Operation::effect_free(
                ValueDef::new(ValueId(14), Type::INDEX),
                Kind::Constant(Constant::Index(1)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(15), pointer),
                Kind::GetElementPointer {
                    base: ValueId(11),
                    offset: ValueId(14),
                },
            ),
        ],
    );
    for op in &mut body.operations {
        match &mut op.kind {
            Kind::Store { pointer, .. } | Kind::Load { pointer, .. } => *pointer = ValueId(15),
            _ => {}
        }
    }
    with_checked(&module, |checked, budget| {
        with_canonical_private_memory_policy_checks_v1(
            checked,
            limits(),
            budget,
            |view, budget| {
                let proof = view.physical_memory(budget)?;
                assert_eq!(proof.latest_stores().last(), Some(&Some(6)));
                let read = proof.inventory().operations().last().unwrap();
                let definition = proof.inventory().uses()[read.operands.start].definition;
                let address = proof.address(definition).unwrap();
                assert_eq!(
                    (address.length(), address.offset(), address.stride()),
                    (2, 1, 8)
                );
                reports(view, budget, 2)
            },
        )
        .unwrap();
    });
}

#[test]
fn shared_sink_and_declaration_middle_compose_with_cross_block_physical_memory() {
    let mut module = traps::tests::fixture(1);
    let helper = module
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "original_helper")
        .unwrap();
    let body = helper.body.as_mut().unwrap();
    let operations = body.blocks[0].operations.split_off(3);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(37),
        arguments: vec![],
    });
    let mut second = BasicBlock::new(BlockId(37));
    second.operations = operations;
    second.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(second);
    with_checked(&module, |checked, budget| {
        with_canonical_private_memory_policy_checks_v1(
            checked,
            limits(),
            budget,
            |view, budget| {
                let traps = view.trap_policies(budget)?;
                assert_eq!(traps.module_function_count(budget)?, 3);
                assert_eq!(traps.pair_count(budget)?, 1);
                assert_eq!(traps.pair(0, budget)?.incoming_edges(), 0..2);
                assert_eq!(traps.pair(0, budget)?.declaration().0, 1);
                assert_eq!(traps.definition_coordinate(0, budget)?.0, 0);
                assert_eq!(traps.definition_coordinate(1, budget)?.0, 2);
                assert!(traps.incoming_edge(0, budget)?.success_when());
                assert!(!traps.incoming_edge(1, budget)?.success_when());
                reports(view, budget, 2)
            },
        )
        .unwrap();
    });
}

#[test]
fn repeated_calls_and_multiple_roots_preserve_complete_callee_roster() {
    let mut module = split();
    let extra_call = module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].clone();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(extra_call);
    let mut root = module.functions[0].clone();
    root.id = "alpha".into();
    module.functions.push(root);
    let mut kernel: Kernel = module.kernels[0].clone();
    kernel.id = "alpha".into();
    kernel.entry = "alpha".into();
    module.kernels.push(kernel);
    with_checked(&module, |checked, budget| {
        with_canonical_private_memory_policy_checks_v1(
            checked,
            limits(),
            budget,
            |view, budget| {
                assert_eq!(view.owner(budget)?.module().kernels, module.kernels);
                assert_eq!(view.physical_memory(budget)?.inventory().calls().len(), 4);
                reports(view, budget, 3)
            },
        )
        .unwrap();
    });
}

fn refuse_memory(module: &Module) {
    with_checked(module, |checked, budget| {
        let floor = budget.storage();
        let error = with_canonical_private_memory_policy_checks_v1::<()>(
            checked,
            limits(),
            budget,
            |_, _| panic!("incomplete physical proof reached callback"),
        )
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::PrivateRequirement {
                requirement: CanonicalPrivateRequirementV1::CompleteCells,
                ..
            }
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(error.observation().work_upper_bound(), 0);
    });
}

#[test]
fn missing_store_volatile_and_misaligned_access_never_become_empty_success() {
    let mut missing = split();
    missing.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    refuse_memory(&missing);
    for volatile in [false, true] {
        let mut module = split();
        let Kind::Load { access, .. } =
            &mut module.functions[1].body.as_mut().unwrap().blocks[1].operations[0].kind
        else {
            unreachable!()
        };
        if volatile {
            access.volatile = true;
        } else {
            access.alignment = 16;
        }
        refuse_memory(&module);
    }
}

#[test]
fn complete_first_cell_cannot_cover_a_later_uninitialized_candidate() {
    let mut module = fixture();
    let ty = Type::Scalar(ScalarType::U64);
    let ptr = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    module.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .extend([
            Operation::effect_free(
                ValueDef::new(ValueId(71), ptr),
                Kind::Alloca {
                    element: ty.clone(),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 8,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(72), ty),
                Kind::Load {
                    pointer: ValueId(71),
                    access: MemoryAccess::new(AddressSpace::Private, 8),
                },
            ),
        ]);
    refuse_memory(&module);
}

#[test]
fn recursive_or_external_call_is_not_hidden_by_a_complete_memory_proof() {
    for external in [false, true] {
        let mut module = fixture();
        if external {
            module.functions.push(Function::declaration(
                "unknown",
                Signature::new(vec![], vec![]),
            ));
        }
        module.functions[1].body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::new(
                vec![],
                Kind::Call {
                    callee: if external {
                        "unknown"
                    } else {
                        "original_helper"
                    }
                    .into(),
                    arguments: vec![],
                },
            ));
        with_checked(&module, |checked, budget| {
            let floor = budget.storage();
            let error = with_canonical_private_memory_policy_checks_v1::<()>(
                checked,
                limits(),
                budget,
                |_, _| panic!("call closure cannot be omitted"),
            )
            .unwrap_err();
            let expected = if external {
                CanonicalPrivateRequirementV1::TerminalPairs
            } else {
                CanonicalPrivateRequirementV1::DefinedAcyclicCalls
            };
            assert!(matches!(error.failure(),
                Failure::PrivateRequirement { requirement, .. } if *requirement == expected));
            assert_eq!(budget.storage(), floor);
            assert_eq!(error.observation().work_upper_bound(), 0);
        });
    }
}

#[test]
fn physical_anchor_does_not_hide_a_reached_callee_nontermination() {
    let mut module = fixture();
    module.functions[1].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    with_checked(&module, |checked, budget| {
        let floor = budget.storage();
        let error = with_canonical_private_memory_policy_checks_v1::<()>(
            checked,
            limits(),
            budget,
            |_, _| panic!("actual callee progress must run"),
        )
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Analysis { function: 1, .. }
        ));
        assert_eq!(error.last_invocation().unwrap().function(), 1);
        assert!(error.last_invocation().unwrap().floor().work_upper_bound() > 0);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn physical_and_trap_queries_share_sticky_custody_and_index_failure() {
    for foreign in [false, true] {
        with_checked(&split(), |checked, budget| {
            let floor = budget.storage();
            let mut work = Work::new(1 << 48);
            let mut other = Budget::new(&mut work, 1 << 32);
            let error = with_canonical_private_memory_policy_checks_v1(
                checked,
                limits(),
                budget,
                |view, budget| {
                    if foreign {
                        assert!(view.physical_memory(&mut other).is_err());
                    } else {
                        assert!(view.trap_policies(budget)?.report(2, budget).is_err());
                    }
                    assert!(view.owner(budget).is_err());
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::Resource(Resource::Accounting) | Failure::InvalidQuery { function: 2 }
            ));
            assert_eq!(other.work(), 0);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn cell_limit_refusal_drops_backing_then_same_public_entry_recovers() {
    with_checked(&split(), |checked, budget| {
        let floor = budget.storage();
        let error = with_canonical_private_memory_policy_checks_v1::<()>(
            checked,
            MemoryLimits { max_cells: 0 },
            budget,
            |_, _| panic!("zero ceiling"),
        )
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::PrivateRequirement {
                requirement: CanonicalPrivateRequirementV1::CompleteCells,
                ..
            }
        ));
        assert_eq!(budget.storage(), floor);
        with_canonical_private_memory_policy_checks_v1(
            checked,
            limits(),
            budget,
            |view, budget| reports(view, budget, 2),
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn same_bytes_foreign_inventory_is_not_the_physical_proof_subject() {
    let module = split();
    with_checked(&module, |checked, budget| {
        with_canonical_private_memory_policy_checks_v1(
            checked,
            limits(),
            budget,
            |view, budget| {
                let proof = view.physical_memory(budget)?;
                with_checked(&module, |other, other_budget| {
                    let foreign = other.inventory(other_budget).unwrap();
                    assert_eq!(
                        foreign.owner().canonical().canonical_bytes(),
                        proof.inventory().owner().canonical().canonical_bytes()
                    );
                    assert!(!proof.is_for(foreign));
                });
                Ok(())
            },
        )
        .unwrap();
    });
}

#[test]
fn physical_projection_keeps_memory_and_call_occurrences_distinct_from_scalar_data() {
    use pliron::{builtin::op_interfaces::OneRegionInterface, linked_list::ContainsLinkedList};
    with_checked(&split(), |checked, budget| {
        protected(budget, |budget| {
            let inventory = checked.inventory(budget)?;
            let terminals = traps::CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
            let facts = CanonicalPrivateGraphFactsV1::derive_physical(
                inventory,
                &terminals,
                limits(),
                budget,
            )?;
            let mut projection = NativeCanonicalPrivateProjectionV1::import(&facts, budget)?;
            for ordinal in 0..2 {
                projection.with_function(ordinal, budget, |input| {
                    let mut kinds = Vec::new();
                    for block in input
                        .function()
                        .get_region(input.context())
                        .deref(input.context())
                        .iter(input.context())
                    {
                        for operation in block.deref(input.context()).iter(input.context()) {
                            kinds.push(input.operation(input.context(), operation).unwrap());
                        }
                    }
                    for (kind, expected) in [
                        (PrivateOperationKindV1::Allocate, usize::from(ordinal == 1)),
                        (PrivateOperationKindV1::Write, usize::from(ordinal == 1)),
                        (PrivateOperationKindV1::Read, usize::from(ordinal == 1)),
                        (PrivateOperationKindV1::Call, usize::from(ordinal == 0)),
                    ] {
                        assert_eq!(
                            kinds.iter().filter(|actual| **actual == kind).count(),
                            expected
                        );
                    }
                })?;
            }
            projection.check(budget)?;
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn physical_projection_rejects_epoch_restore_and_forged_schema_after_test_rebase() {
    use pliron::{builtin::attributes::StringAttr, linked_list::ContainsLinkedList};
    for hostile_schema in [false, true] {
        with_checked(&split(), |checked, budget| {
            protected(budget, |budget| {
                let inventory = checked.inventory(budget)?;
                let terminals = traps::CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
                let facts = CanonicalPrivateGraphFactsV1::derive_physical(
                    inventory,
                    &terminals,
                    limits(),
                    budget,
                )?;
                let mut projection = NativeCanonicalPrivateProjectionV1::import(&facts, budget)?;
                projection.test_live(|context, root| {
                    let function = root
                        .deref(context)
                        .get_region(0)
                        .deref(context)
                        .iter(context)
                        .next()
                        .unwrap()
                        .deref(context)
                        .iter(context)
                        .nth(1)
                        .unwrap();
                    let load = function
                        .deref(context)
                        .get_region(0)
                        .deref(context)
                        .iter(context)
                        .nth(1)
                        .unwrap()
                        .deref(context)
                        .iter(context)
                        .next()
                        .unwrap();
                    if hostile_schema {
                        load.deref_mut(context).attributes.set(
                            "unproved_physical_semantics".try_into().unwrap(),
                            StringAttr::new("claim".into()),
                        );
                    } else {
                        let original = load.deref(context).attributes.clone();
                        load.deref_mut(context).attributes = original;
                    }
                });
                assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
                if hostile_schema {
                    projection.test_rebase_epoch();
                    assert!(matches!(
                        projection.check(budget),
                        Err(Failure::NativeSchema)
                    ));
                }
                Ok(())
            })
            .unwrap();
        });
    }
}
