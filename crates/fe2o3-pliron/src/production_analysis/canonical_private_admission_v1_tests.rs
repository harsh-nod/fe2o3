use super::super::tests::{noop, with_checked};
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, Constant, Function, MemoryAccess, Module, Operation,
    ScalarType, Signature, ValueDef, ValueId,
};
use pliron::{builtin::op_interfaces::OneRegionInterface, linked_list::ContainsLinkedList, op::Op};

pub(crate) fn fixture() -> Module {
    let mut module = noop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            Kind::Call {
                callee: "original_helper".into(),
                arguments: vec![],
            },
        ));
    let ty = Type::Scalar(ScalarType::U64);
    let pointer = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(10), ty.clone()),
            Kind::Constant(Constant::U64(11)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), pointer),
            Kind::Alloca {
                element: ty.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(11),
                value: ValueId(10),
                access,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), ty),
            Kind::Load {
                pointer: ValueId(11),
                access,
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "original_helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

fn helper(module: &mut Module) -> &mut BasicBlock {
    &mut module.functions[1].body.as_mut().unwrap().blocks[0]
}

pub(crate) fn with_projection<T>(
    module: &Module,
    run: impl FnOnce(&mut NativeCanonicalPrivateProjectionV1<'_>, &mut Budget<'_>) -> Result<T, Failure>,
) -> Result<T, Failure> {
    with_checked(module, |checked, budget| {
        protected(budget, |budget| {
            let inventory = checked.inventory(budget)?;
            let facts = CanonicalPrivateGraphFactsV1::derive(inventory, budget)?;
            let mut projection = NativeCanonicalPrivateProjectionV1::import(&facts, budget)?;
            run(&mut projection, budget)
        })
    })
}

fn requirement(module: &Module, expected: CanonicalPrivateRequirementV1) {
    with_checked(module, |checked, budget| {
        let floor = budget.storage();
        let failure = with_canonical_private_policy_checks_v1::<()>(checked, budget, |_, _| {
            panic!("rejected original graph cannot reach reports")
        })
        .err()
        .expect("must reject");
        assert!(
            matches!(failure.failure(), Failure::PrivateRequirement { requirement, .. } if *requirement == expected)
        );
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn graph_census_exactly_covers_real_alloca_store_load_and_actual_call() {
    with_checked(&fixture(), |checked, budget| {
        protected(budget, |budget| {
            let inventory = checked.inventory(budget)?;
            let facts = CanonicalPrivateGraphFactsV1::derive(inventory, budget)?;
            assert_eq!(facts.cells.census().unwrap().allocations().len(), 1);
            assert_eq!(facts.cells.census().unwrap().addresses().len(), 1);
            assert_eq!(facts.cells.census().unwrap().accesses().len(), 2);
            assert_eq!(facts.inventory.calls().len(), 1);
            assert_eq!(facts.completion_order, vec![1, 0]);
            facts.require_completed(2, budget)?;
            assert!(facts.require_completed(1, budget).is_err());
            Ok(())
        })
        .unwrap()
    });
}

#[test]
fn eligible_first_cell_cannot_hide_an_excluded_second_candidate() {
    let mut module = fixture();
    let ty = Type::BOOL;
    helper(&mut module).operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(20),
            Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
        ),
        Kind::Alloca {
            element: ty,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    ));
    requirement(&module, CanonicalPrivateRequirementV1::CompleteCells);
}

#[test]
fn uninitialized_load_and_volatile_access_do_not_become_empty_private_proofs() {
    let mut uninitialized = fixture();
    helper(&mut uninitialized).operations.swap(2, 3);
    requirement(&uninitialized, CanonicalPrivateRequirementV1::CompleteCells);
    let mut volatile = fixture();
    if let Kind::Load { access, .. } = &mut helper(&mut volatile).operations[3].kind {
        access.volatile = true;
    }
    requirement(&volatile, CanonicalPrivateRequirementV1::CompleteCells);
}

#[test]
fn every_call_is_closed_even_outside_a_cell_access_interval() {
    let mut recursive = fixture();
    helper(&mut recursive).operations.push(Operation::new(
        vec![],
        Kind::Call {
            callee: "original_helper".into(),
            arguments: vec![],
        },
    ));
    requirement(
        &recursive,
        CanonicalPrivateRequirementV1::DefinedAcyclicCalls,
    );
    let mut declaration = fixture();
    declaration.functions.push(Function::declaration(
        "external",
        Signature::new(vec![], vec![]),
    ));
    requirement(
        &declaration,
        CanonicalPrivateRequirementV1::DefinedAcyclicCalls,
    );
}

#[test]
fn a_defined_call_inside_store_load_interval_still_excludes_the_cell() {
    let mut module = fixture();
    let mut leaf = BasicBlock::new(BlockId(81));
    leaf.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "leaf",
        Signature::new(vec![], vec![]),
        vec![],
        vec![leaf],
    ));
    helper(&mut module).operations.insert(
        3,
        Operation::new(
            vec![],
            Kind::Call {
                callee: "leaf".into(),
                arguments: vec![],
            },
        ),
    );
    requirement(&module, CanonicalPrivateRequirementV1::CompleteCells);
}

#[test]
fn exact_zero_offset_address_is_a_private_address_not_native_data() {
    let mut module = fixture();
    let pointer = helper(&mut module).operations[1].results[0].ty.clone();
    helper(&mut module).operations.splice(
        2..2,
        [
            Operation::effect_free(
                ValueDef::new(ValueId(13), Type::INDEX),
                Kind::Constant(Constant::Index(0)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(14), pointer),
                Kind::GetElementPointer {
                    base: ValueId(11),
                    offset: ValueId(13),
                },
            ),
        ],
    );
    for operation in &mut helper(&mut module).operations {
        match &mut operation.kind {
            Kind::Store { pointer, .. } | Kind::Load { pointer, .. } => *pointer = ValueId(14),
            _ => {}
        }
    }
    with_projection(&module, |projection, budget| {
        projection.with_function(1, budget, |input| {
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
            assert_eq!(
                kinds
                    .iter()
                    .filter(|kind| **kind == PrivateOperationKindV1::Address)
                    .count(),
                1
            );
        })?;
        Ok(())
    })
    .unwrap();
}

#[path = "canonical_private_policy_v1_tests.rs"]
mod policy;
#[path = "canonical_private_profile_v1_tests.rs"]
mod profile;

#[path = "canonical_private_nine_oracle_v1_tests.rs"]
pub(crate) mod nine_oracle;

pub(crate) fn add_private_switch(module: &mut Module, typed: bool) {
    use fe2o3_kernel_ir::{IntegerSwitchCase, SwitchCase};
    let body = module
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "original_helper")
        .unwrap()
        .body
        .as_mut()
        .unwrap();
    let payloads = [
        vec![ValueId(10), ValueId(12)],
        vec![ValueId(12), ValueId(10)],
    ];
    body.blocks[0].terminator = Some(if typed {
        Terminator::IntegerSwitch {
            selector: ValueId(12),
            cases: [0, u64::MAX]
                .into_iter()
                .enumerate()
                .map(|(ordinal, value)| IntegerSwitchCase {
                    value: Constant::U64(value),
                    target: BlockId(29),
                    arguments: payloads[ordinal].clone(),
                })
                .collect(),
            default_target: BlockId(29),
            default_arguments: vec![ValueId(12), ValueId(12)],
        }
    } else {
        Terminator::Switch {
            selector: ValueId(12),
            cases: [u64::MAX, 0]
                .into_iter()
                .enumerate()
                .map(|(ordinal, value)| SwitchCase {
                    value,
                    target: BlockId(29),
                    arguments: payloads[ordinal].clone(),
                })
                .collect(),
            default_target: BlockId(29),
            default_arguments: vec![ValueId(12), ValueId(12)],
        }
    });
    let mut join = BasicBlock::new(BlockId(29));
    join.parameters = vec![
        ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U64)),
        ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U64)),
    ];
    join.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(join);
}

fn switch_fixture(typed: bool) -> Module {
    let mut module = fixture();
    add_private_switch(&mut module, typed);
    module
}

#[test]
fn private_switch_real_memory_calls_and_all_nine_reports_keep_complete_payloads() {
    use crate::production_analysis::pliron_control_edges_v1::ControlViewV1;
    use dialect_gpu::switch_v3::SwitchOpV3;
    for typed in [false, true] {
        let module = switch_fixture(typed);
        with_projection(&module, |projection, budget| {
            projection.with_function(1, budget, |input| {
                let context = input.context();
                let region = input.function().get_region(context).deref(context);
                let entry = region.iter(context).next().unwrap();
                let raw = entry.deref(context);
                let a = raw
                    .iter(context)
                    .nth(0)
                    .unwrap()
                    .deref(context)
                    .get_result(0);
                let b = raw
                    .iter(context)
                    .nth(3)
                    .unwrap()
                    .deref(context)
                    .get_result(0);
                let terminator = raw.get_terminator(context).unwrap();
                assert!(pliron::operation::Operation::is_op::<SwitchOpV3>(
                    terminator, context
                ));
                assert_eq!(
                    input.operation(context, terminator),
                    Some(PrivateOperationKindV1::Scalar)
                );
                let control = ControlViewV1::observe(context, terminator).unwrap();
                assert_eq!(control.successor_count(), 3);
                for (ordinal, expected) in [[a, b], [b, a], [b, b]].into_iter().enumerate() {
                    let edge = control.edge(ordinal).unwrap();
                    assert_eq!(edge.argument_count(), 2);
                    for (position, value) in expected.into_iter().enumerate() {
                        assert_eq!(edge.argument_at(position).unwrap().0, value);
                    }
                }
            })?;
            projection.check(budget)?;
            Ok(())
        })
        .unwrap();
        with_checked(&module, |checked, budget| {
            let original = checked.inventory(budget).unwrap().owner();
            let floor = budget.storage();
            with_canonical_private_policy_checks_v1(checked, budget, |view, budget| {
                assert!(std::ptr::eq(view.owner(budget)?, original));
                assert_eq!(view.function_count(budget)?, 2);
                for function in 0..2 {
                    let report = view.report(function, budget)?;
                    assert_eq!(report.paired_stage_count(), 9);
                    assert!(report.reports().is_clean());
                    assert_eq!(report.reports().pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                }
                assert_eq!(view.history(1, budget)?.floor(), view.history(0, budget)?.invocation());
                assert_eq!(view.pending_obligations().iter().count(), 19);
                assert!(!view.ranked_verification_is_complete());
                assert!(!view.grants_artifact_or_launch_authority());
                Ok(())
            }).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

pub(crate) fn make_narrow_switch_unrepresentable(
    module: &mut Module,
    constant: Constant,
    key: u64,
) -> usize {
    let ordinal = module
        .functions
        .iter()
        .position(|f| f.id.as_str() == "original_helper")
        .unwrap();
    let entry = &mut module.functions[ordinal].body.as_mut().unwrap().blocks[0];
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(30), constant.ty()),
        Kind::Constant(constant),
    ));
    let Some(Terminator::Switch {
        selector, cases, ..
    }) = &mut entry.terminator
    else {
        unreachable!()
    };
    *selector = ValueId(30);
    cases[0].value = key;
    ordinal
}

#[test]
fn private_switch_unrepresentable_legacy_keys_refuse_before_native_reports() {
    for (constant, key) in [
        (Constant::U8(0), 256),
        (Constant::I8(0), u64::MAX),
        (Constant::U16(0), 1 << 16),
        (Constant::I32(0), u64::MAX),
    ] {
        let mut module = switch_fixture(false);
        let ordinal = make_narrow_switch_unrepresentable(&mut module, constant, key);
        with_checked(&module, |checked, budget| {
            let floor = budget.storage();
            let error = with_canonical_private_policy_checks_v1::<()>(checked, budget, |_, _| {
                panic!("narrow key reached reports")
            })
            .unwrap_err();
            assert!(
                matches!(error.failure(), Failure::UnsupportedGraph { function, block: Some(0), operation: None } if *function == ordinal)
            );
            assert_eq!(error.observation().work_upper_bound(), 0);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn private_switch_operation_defined_selector_lookup_has_literal_ten_work_prefix() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let module = switch_fixture(false);
    let function = &module.functions[1];
    let Some(Terminator::Switch {
        selector, cases, ..
    }) = &function.body.as_ref().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    // Block1 + operation headers4 + result definitions3 + keys2 = 10.
    for allowance in 0..=10 {
        let mut work = Work::new(13 + allowance);
        let mut budget = Budget::new(&mut work, 17);
        budget.charge_work(13).unwrap();
        budget.reserve_storage(17).unwrap();
        let result = crate::kir_bridge_v1::source_legacy_representable(
            function,
            *selector,
            cases,
            &mut budget,
        );
        if allowance == 10 {
            assert_eq!(result, Ok(true));
        } else {
            assert!(
                matches!(result, Err(Resource::Work(error)) if error.actual() == 14 + allowance)
            );
        }
        assert_eq!(budget.work(), 13 + allowance);
        assert_eq!(
            (
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (17, 17, None)
        );
    }
}

#[test]
fn private_switch_actual_payload_epoch_and_schema_mutations_never_pass_custody() {
    use dialect_gpu::switch_v3::SwitchOpV3;
    use pliron::builtin::attributes::StringAttr;
    use pliron::operation::Operation as LiveOperation;
    for mutation in 0..3 {
        with_projection(&switch_fixture(false), |projection, budget| {
            projection.test_live(|context, root| {
                let region = root.deref(context).get_region(0);
                let module_block = region.deref(context).iter(context).next().unwrap();
                let function = module_block.deref(context).iter(context).nth(1).unwrap();
                let region = function.deref(context).get_region(0);
                let entry = region.deref(context).iter(context).next().unwrap();
                let pointer = entry.deref(context).get_terminator(context).unwrap();
                assert!(LiveOperation::is_op::<SwitchOpV3>(pointer, context));
                match mutation {
                    0 => {
                        let value = pointer.deref(context).get_operand(2);
                        LiveOperation::replace_operand(pointer, context, 1, value);
                    }
                    1 => pointer.deref_mut(context).attributes.set(
                        "unexpected".try_into().unwrap(),
                        StringAttr::new("not a switch attribute".into()),
                    ),
                    _ => {
                        let original = pointer.deref(context).attributes.clone();
                        pointer.deref_mut(context).attributes = original;
                    }
                }
            });
            assert!(matches!(projection.check_epoch(), Err(Failure::Mutation)));
            projection.test_rebase_epoch();
            if mutation == 2 {
                projection.check(budget)?;
            } else {
                assert!(projection.check(budget).is_err());
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn private_switch_callback_error_and_panic_restore_the_original_live_floor() {
    for panic in [false, true] {
        with_checked(&switch_fixture(false), |checked, budget| {
            let floor = budget.storage();
            let error =
                with_canonical_private_policy_checks_v1::<()>(checked, budget, |view, budget| {
                    assert_eq!(view.report(1, budget)?.paired_stage_count(), 9);
                    if panic {
                        panic!("private switch callback");
                    }
                    Err(Failure::Callback("private switch"))
                })
                .unwrap_err();
            if panic {
                assert!(matches!(error.failure(), Failure::Panicked));
            } else {
                assert!(matches!(
                    error.failure(),
                    Failure::Callback("private switch")
                ));
            }
            assert_eq!(budget.storage(), floor);
        });
    }
}
