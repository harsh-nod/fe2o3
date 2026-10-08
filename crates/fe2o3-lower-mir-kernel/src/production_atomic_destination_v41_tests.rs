use super::*;

// These controls use the original captured semantic/SSA owner. They exercise
// definition-role consumption only, not atomic memory permission or emission.
// The separate full original emitter and whole-root tests cover those gates.
fn with_original_atomic_definition_v41(
    atomic: bool,
    inspect: impl FnOnce(
        &mut ExecutionAvailabilityV29<'_>,
        ExecutionSiteV29,
        SemanticLocalIdV1,
        SsaValueV1,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) {
    let owner = owner41_atomic(
        Fault41::None,
        Some((
            SemanticAtomicRmwOpV1::Add,
            SemanticAtomicOrderingV1::Relaxed,
        )),
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner, ROOT, &mut budget, |instances, budget| {
            let instance = if atomic { instances.root() } else { instances.id_at(1).unwrap() };
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                with_execution_availability_v29(instances, instance, budget, |mut cursor, budget| {
                    let role = if atomic { ExecutionOperandV29::AtomicDestination }
                        else { ExecutionOperandV29::Destination };
                    let rows: Vec<_> = cursor.occurrences.events().iter().filter(|event|
                        event.operand() == role && event.role() == ExecutionEventV29::DestinationDefine
                    ).collect();
                    assert_eq!(rows.len(), 1, "one exact original definition in this fixture instance");
                    let row = rows[0];
                    assert!(row.is_reachable() && row.is_promoted());
                    let site = row.site();
                    let Some(SsaResolvedEventV1::Define { variable, value }) = row.resolved()
                        else { panic!("original resolved definition"); };
                    let local = SemanticLocalIdV1::from_index(variable.get());
                    let original = scoped_source_statement_v29(cursor.function, site).unwrap();
                    if atomic {
                        assert!(matches!(original, SemanticStatementKindV1::AtomicRmw(op)
                            if op.destination().local() == local && op.destination().projections().is_empty()));
                    } else {
                        assert!(matches!(original, SemanticStatementKindV1::Assign(op)
                            if op.destination().local() == local && op.destination().projections().is_empty()));
                    }
                    cursor.begin_block(SemanticBlockIdV1::from_index(execution_event_block_v29(site).get()), budget)?;
                    inspect(&mut cursor, site, local, value, budget)
                })
            )
        },
    ).unwrap().unwrap();
}

#[test]
fn atomic_definition_v41_exact_original_role_and_archive_are_consumed() {
    with_original_atomic_definition_v41(true, |cursor, site, local, value, budget| {
        assert_eq!(
            cursor.definition_roles_v29(site, local, budget)?,
            &[ExecutionOperandV29::AtomicDestination]
        );
        let index = cursor.event(
            site,
            ExecutionOperandV29::AtomicDestination,
            ExecutionEventV29::DestinationDefine,
            budget,
        )?;
        let previous = cursor.current[local.index() as usize];
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 1_000_000);
        assert!(cursor.define(site, local, value, &mut foreign).is_err());
        assert_eq!(
            foreign.work(),
            0,
            "foreign ledger is refused before the new selector charge"
        );
        assert_eq!(foreign.storage(), 0);
        assert!(!cursor.claimed[index]);
        assert_eq!(cursor.current[local.index() as usize], previous);
        cursor.define(site, local, value, budget)?;
        assert_eq!(cursor.current[local.index() as usize], Some(value));
        cursor.check_archive_definition_v29(
            value,
            ExecutionArchiveDefinitionSiteV29::Definition {
                site,
                local: local.index(),
            },
            budget,
        )?;
        Ok(())
    });
}

#[test]
fn atomic_definition_v41_wrong_local_site_and_ssa_value_refuse_without_claim() {
    for fault in 0..3 {
        with_original_atomic_definition_v41(true, |cursor, site, local, value, budget| {
            let index = cursor.event(
                site,
                ExecutionOperandV29::AtomicDestination,
                ExecutionEventV29::DestinationDefine,
                budget,
            )?;
            let previous = cursor.current[local.index() as usize];
            let wrong_site = match site {
                ExecutionSiteV29::Statement { block, statement } => ExecutionSiteV29::Statement {
                    block,
                    statement: statement.checked_sub(1).unwrap(),
                },
                _ => unreachable!(),
            };
            let wrong_value = SsaValueV1::BlockArgument {
                block: execution_event_block_v29(site),
                variable: fe2o3_mir_model::SsaVariableIdV1::new(local.index()),
            };
            assert_ne!(wrong_value, value);
            let result = match fault {
                0 => cursor.define(site, SemanticLocalIdV1::from_index(9), value, budget),
                1 => cursor.define(wrong_site, local, value, budget),
                2 => cursor.define(site, local, wrong_value, budget),
                _ => unreachable!(),
            };
            assert!(
                result.is_err(),
                "independent original-definition fault {fault}"
            );
            assert!(!cursor.claimed[index]);
            assert_eq!(cursor.current[local.index() as usize], previous);
            cursor.define(site, local, value, budget)?;
            cursor.check_archive_definition_v29(
                value,
                ExecutionArchiveDefinitionSiteV29::Definition {
                    site,
                    local: local.index(),
                },
                budget,
            )?;
            Ok(())
        });
    }
}

#[test]
fn atomic_definition_v41_unclaimed_archive_and_duplicate_definition_refuse() {
    with_original_atomic_definition_v41(true, |cursor, site, local, value, budget| {
        assert!(
            cursor
                .check_archive_definition_v29(
                    value,
                    ExecutionArchiveDefinitionSiteV29::Definition {
                        site,
                        local: local.index()
                    },
                    budget
                )
                .is_err()
        );
        cursor.define(site, local, value, budget)?;
        assert!(cursor.define(site, local, value, budget).is_err());
        cursor.check_archive_definition_v29(
            value,
            ExecutionArchiveDefinitionSiteV29::Definition {
                site,
                local: local.index(),
            },
            budget,
        )?;
        Ok(())
    });
}

#[test]
fn atomic_definition_v41_ordinary_assignment_roles_and_archive_are_unchanged() {
    with_original_atomic_definition_v41(false, |cursor, site, local, value, budget| {
        assert_eq!(
            cursor.definition_roles_v29(site, local, budget)?,
            &[
                ExecutionOperandV29::Destination,
                ExecutionOperandV29::ElidedBorrowDestination
            ]
        );
        cursor.define(site, local, value, budget)?;
        cursor.check_archive_definition_v29(
            value,
            ExecutionArchiveDefinitionSiteV29::Definition {
                site,
                local: local.index(),
            },
            budget,
        )?;
        Ok(())
    });
}
