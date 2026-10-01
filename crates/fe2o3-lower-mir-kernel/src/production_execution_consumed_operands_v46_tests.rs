use super::super::super::super::{ExecutionEventV29, unit_local_source_key_v1};
use super::*;

fn assert_availability_refusal(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "execution availability differs from its source SSA instance",
            })
        ),
        "{result:?}"
    );
}

#[test]
fn claimed_original_move_requires_exact_base_kill_and_consumed_state() {
    let completed = std::cell::Cell::new(false);
    captured_execution(Flow::Linear, |plan, budget| {
        let instance = plan.calls(plan.root()).unwrap()[1].child().unwrap();
        with_execution_availability_v29(plan, instance, budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            let function = cursor.function;
            let SemanticStatementKindV1::Assign(assignment) =
                function.blocks()[0].statements()[0].kind()
            else {
                panic!("original assignment")
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(original)) =
                assignment.value().kind()
            else {
                panic!("original Move")
            };
            let site = execution_site_v29(block, Some(0));
            let role = ExecutionOperandV29::RvalueOperand(0);
            let base = cursor.event(site, role, ExecutionEventV29::BaseUse, budget)?;
            let kill = cursor.event(site, role, ExecutionEventV29::MoveKill, budget)?;
            let Some(SsaResolvedEventV1::Use {
                value: definition, ..
            }) = cursor.occurrences.events()[base].resolved()
            else {
                panic!("resolved original")
            };
            assert_availability_refusal(
                cursor.check_claimed_original_operand_v46(site, role, original, definition, budget),
            );
            assert_eq!(
                cursor.use_place(site, role, original, true, budget)?,
                definition
            );
            cursor.check_claimed_original_operand_v46(site, role, original, definition, budget)?;
            assert_eq!(cursor.current[original.local().index() as usize], None);
            for event in [base, kill] {
                cursor.claimed[event] = false;
                assert_availability_refusal(
                    cursor.check_claimed_original_operand_v46(
                        site, role, original, definition, budget,
                    ),
                );
                cursor.claimed[event] = true;
            }
            let local = original.local().index() as usize;
            cursor.current[local] = Some(definition);
            assert_availability_refusal(
                cursor.check_claimed_original_operand_v46(site, role, original, definition, budget),
            );
            cursor.current[local] = None;
            let other = source_definition(&cursor, 0, 0);
            assert_ne!(other, definition);
            assert_availability_refusal(
                cursor.check_claimed_original_operand_v46(site, role, original, other, budget),
            );
            assert_availability_refusal(cursor.check_claimed_original_operand_v46(
                site,
                role,
                &original.clone(),
                definition,
                budget,
            ));
            assert_availability_refusal(cursor.check_claimed_original_operand_v46(
                site,
                ExecutionOperandV29::Destination,
                original,
                definition,
                budget,
            ));
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
            assert_availability_refusal(cursor.check_claimed_original_operand_v46(
                site,
                role,
                original,
                definition,
                &mut foreign,
            ));
            assert_eq!(foreign.work(), 0);
            cursor.check_claimed_original_operand_v46(site, role, original, definition, budget)?;
            completed.set(true);
            Ok(())
        })
        .unwrap();
    });
    assert!(completed.get());
}

#[test]
fn claimed_original_copy_retains_its_current_definition_without_move_authority() {
    let completed = std::cell::Cell::new(false);
    captured_execution(Flow::Linear, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(1);
            cursor.begin_block(block, budget)?;
            let function = cursor.function;
            let SemanticTerminatorKindV1::Call(call) = function.blocks()[1].terminator().kind()
            else {
                panic!("original call")
            };
            let SemanticOperandV1::Move(context) = &call.arguments()[0] else {
                panic!("original first argument")
            };
            let SemanticOperandV1::Copy(original) = &call.arguments()[1] else {
                panic!("original copied argument")
            };
            let site = execution_site_v29(block, None);
            cursor.use_place(
                site,
                ExecutionOperandV29::CallArgument(0),
                context,
                true,
                budget,
            )?;
            let role = ExecutionOperandV29::CallArgument(1);
            let definition = cursor.use_place(site, role, original, false, budget)?;
            cursor.check_claimed_original_operand_v46(site, role, original, definition, budget)?;
            let local = original.local().index() as usize;
            assert_eq!(cursor.current[local], Some(definition));
            cursor.current[local] = None;
            assert_availability_refusal(
                cursor.check_claimed_original_operand_v46(site, role, original, definition, budget),
            );
            cursor.current[local] = Some(definition);
            cursor.check_claimed_original_operand_v46(site, role, original, definition, budget)?;
            completed.set(true);
            Ok(())
        })
        .unwrap();
    });
    assert!(completed.get());
}

#[test]
fn consumed_operand_query_has_independent_work_and_exact_transaction_limits() {
    captured_execution(Flow::Linear, |plan, _| {
        let instance = plan.calls(plan.root()).unwrap()[1].child().unwrap();
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            let result = with_execution_availability_v29(
                plan,
                instance,
                &mut budget,
                |mut cursor, budget| {
                    let block = SemanticBlockIdV1::from_index(0);
                    cursor.begin_block(block, budget)?;
                    let function = cursor.function;
                    let SemanticStatementKindV1::Assign(assignment) =
                        function.blocks()[0].statements()[0].kind()
                    else {
                        panic!("original assignment")
                    };
                    let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(original)) =
                        assignment.value().kind()
                    else {
                        panic!("original Move")
                    };
                    let site = execution_site_v29(block, Some(0));
                    let role = ExecutionOperandV29::RvalueOperand(0);
                    let definition = cursor.use_place(site, role, original, true, budget)?;
                    let mut comparisons = 0;
                    for kind in [ExecutionEventV29::BaseUse, ExecutionEventV29::MoveKill] {
                        let key = unit_local_source_key_v1(site, role, Some(kind));
                        let found = cursor.index.iter().position(|row| row.key == key).unwrap();
                        let (mut lo, mut hi) = (0, cursor.index.len());
                        loop {
                            comparisons += 1;
                            let mid = lo + (hi - lo) / 2;
                            if mid == found {
                                break;
                            }
                            if mid < found {
                                lo = mid + 1;
                            } else {
                                hi = mid;
                            }
                        }
                    }
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    cursor.check_claimed_original_operand_v46(
                        site, role, original, definition, budget,
                    )?;
                    assert_eq!(budget.work() - before.0, 5 + 7 + 8 * comparisons);
                    assert_eq!(
                        (budget.storage(), budget.peak_storage()),
                        (before.1, before.2)
                    );
                    Ok(())
                },
            );
            assert_eq!(budget.storage(), 0);
            (result, budget.work(), budget.peak_storage())
        };
        let measured = run(1_000_000, 1_000_000);
        measured.0.unwrap();
        let exact = run(measured.1, measured.2);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(matches!(
            run(measured.1 - 1, measured.2).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    super::super::super::super::ArgumentResourceV1::Work { .. }
                )
            )
        ));
        assert!(matches!(
            run(measured.1, measured.2 - 1).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    super::super::super::super::ArgumentResourceV1::Storage { .. }
                )
            )
        ));
    });
}
