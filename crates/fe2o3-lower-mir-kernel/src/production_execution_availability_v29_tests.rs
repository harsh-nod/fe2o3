use super::super::super::{
    ExecutionAvailabilityV29, ExecutionOperandV29, ProductionSemanticKirErrorV1,
    execution_site_v29, with_execution_availability_v29,
};
use super::*;
use fe2o3_mir_model::{SsaResolvedEventV1, SsaValueV1};

const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

mod cfg_tests {
    include!("production_execution_cfg_transport_v29_tests.rs");
}

#[derive(Clone, Copy)]
enum Flow {
    Linear,
    StorageKill { live: bool },
    Branches,
    Loop { redefine: bool },
}

// Exact V29 semantic/SSA owners, but still inert fixtures: ContextIssue here is
// not rustc producer authentication, protected proof execution or a GPU run.
fn execution_owner(
    flow: Flow,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    let base = fixture(Case::Ordinary, false);
    let mut types = base.source_semantic().types()[..2].to_vec();
    let marker = SemanticTypeIdV1::from_index(2);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([120; 32]),
        SemanticLayoutIdentityV1::from_sha256([120; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([121; 32]),
            SemanticLayoutIdentityV1::from_sha256([121; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![0; 5], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![marker; 5]).unwrap()),
        )
        .with_rust_type_kind(SemanticRustTypeKindV1::Execution(
            SemanticExecutionRoleV29::KernelContext,
        )),
    );
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([122; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(ignored(CONTEXT)),
            SemanticAbiArgumentV1::source(direct(U32)),
        ],
        ignored(UNIT),
    )
    .unwrap();
    let moved = || {
        assign(
            place(3, CONTEXT),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, CONTEXT))),
        )
    };
    let blocks = match flow {
        Flow::Linear | Flow::StorageKill { .. } => {
            let mut statements = vec![moved()];
            if let Flow::StorageKill { live } = flow {
                let local = SemanticLocalIdV1::from_index(3);
                statements.push(SemanticStatementV1::new(
                    source(),
                    if live {
                        SemanticStatementKindV1::StorageLive(local)
                    } else {
                        SemanticStatementKindV1::StorageDead(local)
                    },
                ));
            }
            vec![block(130, statements, SemanticTerminatorKindV1::Return)]
        }
        Flow::Branches => vec![
            block(
                130,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(131, vec![moved()], SemanticTerminatorKindV1::Return),
            block(132, vec![moved()], SemanticTerminatorKindV1::Return),
        ],
        Flow::Loop { redefine } => {
            let mut statements = vec![moved()];
            if redefine {
                statements.push(assign(
                    place(1, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3, CONTEXT))),
                ));
            }
            vec![block(
                130,
                statements,
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0)),
            )]
        }
    };
    let helper = function(
        123,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        vec![
            local(124, UNIT, SemanticLocalRoleV1::Return),
            local(125, CONTEXT, SemanticLocalRoleV1::Argument(0)),
            local(126, U32, SemanticLocalRoleV1::Argument(1)),
            local(127, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        blocks,
    );
    let issue = |target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(2),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(2, CONTEXT),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let invoke = |target| {
        call(
            1,
            vec![
                SemanticOperandV1::Move(place(2, CONTEXT)),
                SemanticOperandV1::Copy(place(1, U32)),
            ],
            target,
        )
    };
    let root = function(
        120,
        SemanticFunctionRoleV1::KernelRoot,
        abi(140, true, &[U32]),
        vec![
            local(141, UNIT, SemanticLocalRoleV1::Return),
            local(142, U32, SemanticLocalRoleV1::Argument(0)),
            local(143, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(150, vec![], issue(1)),
            block(151, vec![], invoke(2)),
            block(152, vec![], issue(3)),
            block(153, vec![], invoke(4)),
            block(154, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"execution_availability".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([155; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let issue_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([160; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        ignored(CONTEXT),
    )
    .unwrap();
    let issuer = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([160; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([160; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([160; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([160; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([160; 32]),
            source(),
            issue_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Execution(
            SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([160; 32]),
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            issuer,
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
}

fn captured_execution(
    flow: Flow,
    body: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut Budget<'_>),
) {
    let mut owner = execution_owner(flow).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |plan, budget| {
        let floor = budget.storage();
        body(plan, budget);
        assert_eq!(budget.storage(), floor);
        Ok::<_, Error>(())
    })
    .unwrap();
    assert_eq!(budget.storage(), capture.retained_storage());
}

fn source_definition(
    cursor: &ExecutionAvailabilityV29<'_>,
    block: u32,
    statement: u32,
) -> SsaValueV1 {
    cursor
        .occurrences
        .events()
        .iter()
        .find_map(|event| {
            if event.site()
                != execution_site_v29(SemanticBlockIdV1::from_index(block), Some(statement))
            {
                return None;
            }
            match event.resolved() {
                Some(SsaResolvedEventV1::Define { value, .. }) => Some(value),
                _ => None,
            }
        })
        .unwrap()
}

#[test]
fn execution_call_result_and_two_helper_instances_keep_source_ssa_identity() {
    captured_execution(Flow::Linear, |plan, budget| {
        let calls = plan.calls(plan.root()).unwrap();
        let first = calls[1].child().unwrap();
        let second = calls[3].child().unwrap();
        assert_ne!(first, second);
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(1);
            cursor.begin_block(block, budget)?;
            let expected = cursor
                .occurrences
                .edge_definitions()
                .iter()
                .find(|row| row.edge().source().get() == 0 && row.variable().get() == 2)
                .unwrap()
                .value()
                .unwrap();
            assert_eq!(
                cursor.use_place(
                    execution_site_v29(block, None),
                    ExecutionOperandV29::CallArgument(0),
                    &place(2, CONTEXT),
                    true,
                    budget
                )?,
                expected
            );
            Ok(())
        })
        .unwrap();
        for instance in [first, second] {
            with_execution_availability_v29(plan, instance, budget, |mut cursor, budget| {
                let block = SemanticBlockIdV1::from_index(0);
                cursor.begin_block(block, budget)?;
                let definition = source_definition(&cursor, 0, 0);
                cursor.use_place(
                    execution_site_v29(block, Some(0)),
                    ExecutionOperandV29::RvalueOperand(0),
                    &place(1, CONTEXT),
                    true,
                    budget,
                )?;
                cursor.define(
                    execution_site_v29(block, Some(0)),
                    SemanticLocalIdV1::from_index(3),
                    definition,
                    budget,
                )?;
                assert_eq!(cursor.instance, instance);
                assert_eq!(cursor.current[1], None);
                assert_eq!(cursor.current[3], Some(definition));
                Ok(())
            })
            .unwrap();
        }
    });
}

#[test]
fn captured_storage_kills_require_the_exact_local_and_single_claim() {
    for live in [false, true] {
        captured_execution(Flow::StorageKill { live }, |plan, budget| {
            let instance = plan.calls(plan.root()).unwrap()[1].child().unwrap();
            with_execution_availability_v29(plan, instance, budget, |mut cursor, budget| {
                let block = SemanticBlockIdV1::from_index(0);
                cursor.begin_block(block, budget)?;
                let definition = source_definition(&cursor, 0, 0);
                cursor.use_place(
                    execution_site_v29(block, Some(0)),
                    ExecutionOperandV29::RvalueOperand(0),
                    &place(1, CONTEXT),
                    true,
                    budget,
                )?;
                let local = SemanticLocalIdV1::from_index(3);
                cursor.define(
                    execution_site_v29(block, Some(0)),
                    local,
                    definition,
                    budget,
                )?;
                let site = execution_site_v29(block, Some(1));
                let operand = if live {
                    ExecutionOperandV29::StorageLive
                } else {
                    ExecutionOperandV29::StorageDead
                };
                assert!(
                    cursor
                        .storage_kill(site, operand, SemanticLocalIdV1::from_index(1), budget)
                        .is_err()
                );
                assert_eq!(cursor.current[3], Some(definition));
                cursor.storage_kill(site, operand, local, budget)?;
                assert_eq!(cursor.current[3], None);
                assert!(cursor.storage_kill(site, operand, local, budget).is_err());
                Ok(())
            })
            .unwrap();
        });
    }
}

#[test]
fn sibling_branches_consume_the_same_incoming_role_independently() {
    captured_execution(Flow::Branches, |plan, budget| {
        let instance = plan.calls(plan.root()).unwrap()[1].child().unwrap();
        with_execution_availability_v29(plan, instance, budget, |mut cursor, budget| {
            let mut incoming = None;
            for ordinal in [1, 2] {
                let block = SemanticBlockIdV1::from_index(ordinal);
                cursor.begin_block(block, budget)?;
                let selected = cursor.current[1].unwrap();
                assert_eq!(*incoming.get_or_insert(selected), selected);
                let definition = source_definition(&cursor, ordinal, 0);
                cursor.current[1] = Some(definition);
                assert!(
                    cursor
                        .use_place(
                            execution_site_v29(block, Some(0)),
                            ExecutionOperandV29::RvalueOperand(0),
                            &place(1, CONTEXT),
                            true,
                            budget
                        )
                        .is_err()
                );
                cursor.current[1] = Some(selected);
                cursor.use_place(
                    execution_site_v29(block, Some(0)),
                    ExecutionOperandV29::RvalueOperand(0),
                    &place(1, CONTEXT),
                    true,
                    budget,
                )?;
                cursor.define(
                    execution_site_v29(block, Some(0)),
                    SemanticLocalIdV1::from_index(3),
                    definition,
                    budget,
                )?;
                cursor.finish_block(budget)?;
            }
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn loop_consumption_requires_redefinition_before_the_backedge() {
    use fe2o3_mir_model::SsaPlannerErrorV1;
    use fe2o3_pliron::{ProductionSemanticSsaErrorV1, SemanticPartialMoveViolationV1};
    let rejected = execution_owner(Flow::Loop { redefine: false })
        .expect_err("consumed loop input must be rejected");
    match rejected {
        ProductionSemanticSsaErrorV1::Planner { function, error } => {
            assert_eq!(function.index(), 1);
            assert!(matches!(error,
                SsaPlannerErrorV1::UndefinedAtUse { variable, .. }
                | SsaPlannerErrorV1::UndefinedAtEdge { variable, .. }
                | SsaPlannerErrorV1::UndefinedAtEntry { variable } if variable.get() == 1
            ));
        }
        ProductionSemanticSsaErrorV1::PartialMove {
            function,
            local: 1,
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        } => assert_eq!(function.index(), 1),
        error => panic!("unexpected loop rejection: {error:?}"),
    }
    captured_execution(Flow::Loop { redefine: true }, |plan, budget| {
        let instance = plan.calls(plan.root()).unwrap()[1].child().unwrap();
        with_execution_availability_v29(plan, instance, budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            let incoming = cursor.current[1].unwrap();
            assert!(matches!(incoming, SsaValueV1::BlockArgument { .. }));
            for (statement, from, to) in [(0, 1, 3), (1, 3, 1)] {
                let site = execution_site_v29(block, Some(statement));
                cursor.use_place(
                    site,
                    ExecutionOperandV29::RvalueOperand(0),
                    &place(from, CONTEXT),
                    true,
                    budget,
                )?;
                cursor.define(
                    site,
                    SemanticLocalIdV1::from_index(to),
                    source_definition(&cursor, 0, statement),
                    budget,
                )?;
            }
            assert_ne!(cursor.current[1], Some(incoming));
            assert_eq!(cursor.current[3], None);
            Ok(())
        })
        .unwrap();
    });
}

fn captured(case: Case, body: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut Budget<'_>)) {
    let mut owner = fixture(case, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |plan, budget| {
        let floor = budget.storage();
        body(plan, budget);
        assert_eq!(budget.storage(), floor);
        Ok::<_, Error>(())
    })
    .unwrap();
    assert_eq!(budget.storage(), capture.retained_storage());
}

#[test]
fn source_move_claims_the_exact_use_and_kill_once() {
    captured(Case::RustCall, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            let define = cursor
                .occurrences
                .events()
                .iter()
                .find_map(|event| match event.resolved() {
                    Some(SsaResolvedEventV1::Define { variable, value }) if variable.get() == 2 => {
                        Some(value)
                    }
                    _ => None,
                })
                .unwrap();
            cursor.define(
                execution_site_v29(block, Some(0)),
                SemanticLocalIdV1::from_index(2),
                define,
                budget,
            )?;
            let site = execution_site_v29(block, None);
            let role = ExecutionOperandV29::CallArgument(1);
            let operand = plan.calls(plan.root()).unwrap()[0]
                .source()
                .arguments()
                .get(1)
                .unwrap();
            assert!(std::ptr::eq(
                cursor.retained_operand(site, role).unwrap(),
                operand
            ));
            assert!(!std::ptr::eq(
                cursor.retained_operand(site, role).unwrap(),
                &operand.clone()
            ));
            let SemanticOperandV1::Move(place) = operand else {
                panic!("move fixture");
            };
            assert_eq!(cursor.use_place(site, role, place, true, budget)?, define);
            assert_eq!(cursor.current[2], None);
            assert!(cursor.use_place(site, role, place, true, budget).is_err());
            assert_eq!(cursor.claimed.iter().filter(|claimed| **claimed).count(), 3);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn source_assignment_must_define_before_its_move() {
    captured(Case::RustCall, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            assert_eq!(cursor.current[2], None);
            assert!(
                cursor
                    .use_place(
                        execution_site_v29(block, None),
                        ExecutionOperandV29::CallArgument(1),
                        &place(2, TUPLE),
                        true,
                        budget
                    )
                    .is_err()
            );
            assert!(cursor.claimed.iter().all(|claimed| !claimed));
            assert!(cursor.begin_block(block, budget).is_err());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn cursor_rejects_another_owners_equal_declaration_and_ssa() {
    let other = fixture(Case::Ordinary, true);
    captured(Case::Ordinary, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |cursor, _| {
            let row = plan.instance(plan.root()).unwrap();
            cursor.check_source(row.declaration(), row.ssa())?;
            assert!(
                cursor
                    .check_source(&other.source_semantic().functions()[0], row.ssa())
                    .is_err()
            );
            assert!(
                cursor
                    .check_source(row.declaration(), other.plan_for_function(ROOT).unwrap())
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn unresolved_borrow_is_not_an_execution_availability_proof() {
    captured(Case::Borrow, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            let borrowed = plan.borrow_at(plan.root(), block, 0, budget).unwrap();
            assert!(
                cursor
                    .use_place(
                        execution_site_v29(block, Some(0)),
                        ExecutionOperandV29::RvaluePlace,
                        borrowed.source,
                        false,
                        budget
                    )
                    .is_err()
            );
            assert!(cursor.claimed.iter().all(|claimed| !claimed));
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn repeated_helper_instances_own_independent_cursors() {
    captured(Case::Ordinary, |plan, budget| {
        let calls = plan.calls(plan.root()).unwrap();
        let left = calls[0].child().unwrap();
        let right = calls[1].child().unwrap();
        assert_ne!(left, right);
        for instance in [left, right] {
            with_execution_availability_v29(plan, instance, budget, |mut cursor, budget| {
                assert_eq!(cursor.instance, instance);
                cursor.begin_block(SemanticBlockIdV1::from_index(0), budget)?;
                assert!(std::ptr::eq(cursor.ssa, plan.instance(left).unwrap().ssa()));
                Ok(())
            })
            .unwrap();
        }
    });
}

#[test]
fn cursor_scratch_releases_on_error_and_unwind_without_releasing_escaped_rows() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    captured(Case::Ordinary, |plan, budget| {
        let floor = budget.storage();
        let error = with_execution_availability_v29(plan, plan.root(), budget, |_, budget| {
            budget.reserve_storage(19)?;
            Err::<(), _>(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        });
        assert!(error.is_err());
        assert_eq!(budget.storage(), floor + 19);
        budget.release_storage(19).unwrap();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_execution_availability_v29(
                plan,
                plan.root(),
                budget,
                |_, _| -> Result<(), ProductionSemanticKirErrorV1> { panic!("consumer unwind") },
            );
        }));
        assert!(panic.is_err());
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn cursor_unwind_preserves_escaped_storage_and_spent_work() {
    captured(Case::Ordinary, |plan, budget| {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let mut output = None;
        let mut accepted_work = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_execution_availability_v29(
                plan,
                plan.root(),
                budget,
                |_, budget| -> Result<(), ProductionSemanticKirErrorV1> {
                    budget.reserve_storage(19)?;
                    output = Some(Box::new([7_u8; 19]));
                    budget.charge_work(5)?;
                    accepted_work = budget.work();
                    panic!("retained output");
                },
            );
        }));
        assert!(panic.is_err());
        assert_eq!(output.as_deref(), Some(&[7_u8; 19]));
        assert_eq!(budget.storage(), floor + 19);
        assert_eq!(budget.work(), accepted_work);
        assert!(budget.work_ledger_identity_v1() == ledger);
        drop(output.take());
        budget.release_storage(19).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn cursor_construction_and_query_have_paid_exact_limits() {
    captured(Case::RustCall, |plan, _| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let bytes = with_execution_availability_v29(plan, plan.root(), &mut budget, |_, budget| {
            Ok(budget.storage())
        })
        .unwrap();
        let construction_work = budget.work();
        for limit in [0, 1, construction_work - 1, construction_work] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, 1_000_000);
            let result =
                with_execution_availability_v29(plan, plan.root(), &mut budget, |_, _| Ok(()));
            assert_eq!(result.is_ok(), limit == construction_work);
            assert_eq!(budget.storage(), 0);
        }
        for limit in [bytes - 1, bytes] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut budget = Budget::new(&mut work, limit);
            let result =
                with_execution_availability_v29(plan, plan.root(), &mut budget, |_, _| Ok(()));
            assert_eq!(result.is_ok(), limit == bytes);
            assert_eq!(budget.storage(), 0);
        }
        let run_query = |work_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, 1_000_000);
            let result = with_execution_availability_v29(
                plan,
                plan.root(),
                &mut budget,
                |mut cursor, budget| {
                    cursor.begin_block(SemanticBlockIdV1::from_index(0), budget)?;
                    cursor.event(
                        execution_site_v29(SemanticBlockIdV1::from_index(0), None),
                        ExecutionOperandV29::CallArgument(1),
                        super::super::super::ExecutionEventV29::BaseUse,
                        budget,
                    )?;
                    Ok(budget.work())
                },
            );
            assert_eq!(budget.storage(), 0);
            result
        };
        let exact_query = run_query(1_000_000).unwrap();
        assert!(run_query(exact_query - 1).is_err());
        assert_eq!(run_query(exact_query).unwrap(), exact_query);
        with_execution_availability_v29(plan, plan.root(), &mut budget, |mut cursor, budget| {
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
            assert!(
                cursor
                    .begin_block(SemanticBlockIdV1::from_index(0), &mut foreign)
                    .is_err()
            );
            assert_eq!(foreign.work(), 0);
            cursor.begin_block(SemanticBlockIdV1::from_index(0), budget)?;
            let site = execution_site_v29(SemanticBlockIdV1::from_index(0), None);
            let role = ExecutionOperandV29::CallArgument(1);
            let subrole = super::super::super::ExecutionEventV29::BaseUse;
            assert!(cursor.event(site, role, subrole, &mut foreign).is_err());
            assert_eq!(foreign.work(), 0);
            cursor.event(site, role, subrole, budget)?;
            assert!(cursor.claimed.iter().all(|claimed| !claimed));
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn aggregate_classification_pays_before_descending_into_ordinary_prefixes() {
    use super::super::super::{
        SemanticValueBindingV1 as Binding, execution_binding_contains_paid_v29,
    };
    let tree = Binding::Aggregate(vec![
        Binding::Aggregate(vec![Binding::Unit; 64]),
        Binding::MovedExecution,
    ]);
    let exact = 4 * (1 + 1 + 64 + 1);
    for limit in [0, 4, exact - 1, exact] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let result = execution_binding_contains_paid_v29(&tree, &mut budget);
        assert_eq!(result.is_ok(), limit == exact);
        if limit == exact {
            assert!(result.unwrap());
        }
        assert_eq!(budget.storage(), 0);
    }
}
