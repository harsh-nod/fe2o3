fn check_inserted_lifecycle(
    root: OwnedPendingScopedRootV29,
    limits: ProductionSemanticKirLimitsV1,
    fixture: ScopedFixture,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let before = root.pending.function.clone();
    let spans_before: Vec<_> = root.pending.coordinates.spans.rows.to_vec();
    let floor = budget.storage();
    let expected_events: usize = root
        .pending
        .sidecars
        .rows
        .iter()
        .map(|row| row.lifecycle_events.as_ref().unwrap().rows.len())
        .sum();
    let mut donor = Some(root);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
    assert!(matches!(
        insert_pending_lifecycle_v29(&mut donor, limits, &mut foreign),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!(foreign.storage(), 0);
    assert_eq!(foreign_work.work(), 0);
    assert_eq!(donor.as_ref().unwrap().pending.function, before);

    let inserted = match insert_pending_lifecycle_v29(&mut donor, limits, budget) {
        Ok(inserted) => inserted,
        Err(error) => {
            assert!(donor.is_some());
            assert_eq!(budget.storage(), floor);
            let retained = donor.as_ref().unwrap().retained_emission_storage;
            drop(donor);
            budget.release_storage(retained)?;
            return Err(error);
        }
    };
    assert!(donor.is_none());
    assert_eq!(inserted.insertions.len(), expected_events);
    assert_eq!(inserted.root.pending.coordinates.spans.rows, spans_before);
    let mut restored = inserted.root.pending.function.clone();
    for witness in inserted.insertions.iter().rev() {
        let body = restored.body.as_mut().unwrap();
        let block = body
            .blocks
            .iter_mut()
            .find(|block| block.id == witness.after.block)
            .unwrap();
        let operation = block.operations.remove(witness.after.first as usize);
        assert!(matches!(operation.kind, OperationKind::Execution(_)));
        assert_eq!(witness.before.count, 0);
        assert_eq!(witness.after.count, 1);
        assert_eq!(witness.before.block, witness.after.block);
        let sidecar = &inserted.root.pending.sidecars.rows[witness.instance.index()];
        let event = sidecar.lifecycle_events.as_ref().unwrap().rows[witness.event];
        let kind = match event.kind {
            DeferredLifecycleKindV29::Issue { result } => {
                assert_eq!(
                    operation.results,
                    vec![ValueDef::new(
                        result.value,
                        Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context)
                    )]
                );
                fe2o3_kernel_ir::ExecutionOperationV15::ContextIssue
            }
            DeferredLifecycleKindV29::Derive { context, result } => {
                assert_eq!(
                    operation.results,
                    vec![ValueDef::new(
                        result.value,
                        Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Workgroup)
                    )]
                );
                fe2o3_kernel_ir::ExecutionOperationV15::WorkgroupDerive {
                    context: context.value,
                }
            }
            DeferredLifecycleKindV29::End { workgroup } => {
                assert!(operation.results.is_empty());
                fe2o3_kernel_ir::ExecutionOperationV15::ScopeEnd {
                    workgroup: workgroup.value,
                    discarded: vec![],
                }
            }
        };
        assert_eq!(operation.kind, OperationKind::Execution(kind));
        assert_eq!(
            inserted.root.pending.coordinates.spans.rows[witness.source_span].instance,
            witness.instance
        );
    }
    assert_eq!(
        restored, before,
        "insertion must preserve every existing operation and control edge"
    );

    let mut module = Module::new("inserted_lifecycle");
    module
        .functions
        .push(inserted.root.pending.function.clone());
    module.kernels.push(inserted.root.kernel.clone());
    let mut declarations = BTreeMap::new();
    for row in &inserted.root.pending.sidecars.rows {
        for (id, function) in row
            .diagnostic_declarations
            .iter()
            .chain(&row.float_declarations)
        {
            if let Some(prior) = declarations.insert(id.clone(), function.clone()) {
                assert_eq!(prior, *function);
            }
        }
    }
    module.functions.extend(declarations.into_values());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut validation = ArgumentBudgetV1::new(&mut work, 10_000_000);
    let admission = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(&module, &mut validation);
    if matches!(fixture, ScopedFixture::Assertion) {
        assert!(
            admission.is_err(),
            "current lifecycle admission rejects a trap in a live scope"
        );
    } else {
        admission.unwrap();
    }
    let retained = inserted.root.retained_emission_storage;
    drop(inserted);
    budget.release_storage(retained)?;
    Ok(())
}
