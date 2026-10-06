fn check_source_execution_owners_v199(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let archive = relation
        .source
        .root_row(0)?
        .rvalue_results
        .as_ref()
        .unwrap();
    let semantic = relation.source.source_semantic(budget)?;
    let mut roles = [0usize; 4];
    let mut borrows = 0;
    for row in &archive.values {
        let (retained, borrowed) = match row.typed.physical {
            SourceSsaPhysicalV36::Execution(owner) => (owner, false),
            SourceSsaPhysicalV36::ExecutionBorrow(borrow) => (borrow.owner, true),
            _ => continue,
        };
        let endpoint = relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
        let before = budget.storage();
        assert_eq!(endpoint.execution_owner_v199(budget)?, Some(retained));
        assert_eq!(budget.storage(), before);
        assert!(endpoint.reference(budget)?.is_none());
        assert_eq!(endpoint.execution_borrow_v163(budget)?.is_some(), borrowed);
        let definition = endpoint.original_definition(budget)?.unwrap();
        let actual = &relation.inventory.definitions()[definition];
        assert_eq!(actual.value, Some(retained.identity.value));
        assert_eq!(
            actual.ty,
            &Type::Execution(semantic_execution_kir_role_v29(retained.role).unwrap())
        );
        assert_eq!(endpoint.physical_type(budget)?, Some(actual.ty));
        for identity in [
            Some(retained.identity),
            Some(retained.context),
            retained.workgroup,
        ]
        .into_iter()
        .flatten()
        {
            let (function, _) = relation.source.instance(0, identity.instance, budget)?;
            let block = &semantic.functions()[function.index() as usize].blocks()
                [identity.block.index() as usize];
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                panic!("execution identity must retain its actual producer call");
            };
            let destination = call.destination().unwrap().place();
            assert_eq!(destination.local(), identity.destination);
            assert_eq!(destination.ty(), identity.source_type);
            assert!(destination.projections().is_empty());
        }
        if borrowed {
            borrows += 1;
        } else {
            roles[match retained.role {
                SemanticExecutionRoleV29::KernelContext => {
                    assert_eq!(retained.identity, retained.context);
                    assert!(retained.workgroup.is_none());
                    0
                }
                SemanticExecutionRoleV29::Workgroup => {
                    assert_eq!(retained.workgroup, Some(retained.identity));
                    1
                }
                SemanticExecutionRoleV29::MaskedTileU32 { .. } => 2,
                SemanticExecutionRoleV29::LaneFragmentU32 { .. } => 3,
            }] += 1;
        }
    }
    assert!(roles.iter().all(|count| *count > 0));
    assert!(borrows > 0);
    Ok(())
}

#[test]
fn source_execution_owner_endpoint_keeps_owned_roles_and_borrowed_producers() {
    source_execution_borrow_probe_v163(
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        check_source_execution_owners_v199,
    )
    .0
    .unwrap();
}

#[test]
fn source_execution_owner_endpoint_exact_and_one_short_resources() {
    let run = |work, storage| {
        source_execution_borrow_probe_v163(work, storage, check_source_execution_owners_v199)
    };
    let (result, work, peak) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    let (result, used, retained_peak) = run(work, peak);
    result.unwrap();
    assert_eq!((used, retained_peak), (work, peak));
    assert!(run(work - 1, peak).0.is_err());
    assert!(run(work, peak - 1).0.is_err());
}

#[test]
fn source_execution_owner_endpoint_keeps_original_account_and_sticky_refusal() {
    for foreign in [false, true] {
        let (result, _, _) = source_execution_borrow_probe_v163(
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |relation, budget| {
                let archive = relation
                    .source
                    .root_row(0)?
                    .rvalue_results
                    .as_ref()
                    .unwrap();
                let row = archive
                    .values
                    .iter()
                    .find(|row| matches!(row.typed.physical, SourceSsaPhysicalV36::Execution(_)))
                    .unwrap();
                let endpoint =
                    relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
                let error = if foreign {
                    let mut work =
                        CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                    let mut other = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                    other.reserve_storage(budget.storage())?;
                    endpoint.execution_owner_v199(&mut other).unwrap_err()
                } else {
                    budget.release_storage(1)?;
                    endpoint.execution_owner_v199(budget).unwrap_err()
                };
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
                assert!(endpoint.execution_owner_v199(budget).is_err());
                Err(error)
            },
        );
        assert!(result.is_err());
    }
}
