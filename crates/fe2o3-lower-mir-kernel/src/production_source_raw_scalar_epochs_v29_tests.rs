#[test]
fn raw_scalar_epoch_locators_keep_exact_original_identity_and_resolved_target() {
    let mut completed = false;
    with_selected_pointer_test_plan_v29(joined_backing_owner_v29(true, true, false), |plan, budget| {
        let mut kinds = [0; 2];
        for row in &plan.accesses {
            let original = plan.instances.instance(row.key.site.instance).unwrap();
            if original.function().index() != 3 || row.key.site.block.index() != 3 { continue; }
            let statement = &original.declaration().blocks()[3].statements()[row.key.site.statement.unwrap()];
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else { continue; };
            let (source, kind) = match (row.key.access, assignment.value().kind()) {
                (SourceReferenceAccessV29::Address, SemanticRvalueKindV1::AddressOf { place, .. }) => (place, 0),
                (SourceReferenceAccessV29::Read, SemanticRvalueKindV1::Load(load)) => (load.source(), 1),
                _ => continue,
            };
            assert_eq!(row.key.source, source as *const SemanticPlaceV1 as usize);
                assert_eq!(row.local.index(), 2);
                assert!(row.loan.is_none() && row.projections.is_empty() && row.traversed.is_empty());
                let snapshot = plan.blocks.iter().find(|entry|
                    entry.instance == row.instance && entry.block == row.key.site.block).unwrap();
                assert_eq!(row.generation, plan.states[snapshot.entry][2].generation,
                    "the locator must match the converged original target state");
                let declaration = original.declaration();
                let limit = 1 + declaration.blocks().iter().map(|block| block.statements().len()).sum::<usize>();
                let restart = declaration.blocks()[2].statements().iter().position(|statement|
                    matches!(statement.kind(), SemanticStatementKindV1::StorageLive(local) if local.index() == 2)).unwrap();
                let atom = 1 + declaration.blocks()[..2].iter().map(|block| block.statements().len()).sum::<usize>() + restart;
                let epochs = &plan.epoch_sets[row.generation as usize - limit];
                assert_eq!((epochs.instance, epochs.local), (row.instance, row.local));
                assert_eq!(&plan.epoch_members[epochs.first..epochs.first + epochs.count], &[0, atom as u32]);
                let resolved = || SourceReferencePlaceV29 {
                instance: row.instance, local: row.local, generation: row.generation,
                // This helper retains epoch metadata only. These deliberately
                // absent value identities must never become pointer authority.
                value: usize::MAX, representation_root: usize::MAX, node: usize::MAX,
                projections: vec![], selector_source: None, anchor: None, loan: None,
                shared_path: row.shared_path, traversed: vec![],
            };
            assert!(source_reference_raw_scalar_epoch_access_v29(plan, row.key.site, source, row.key.access, &resolved(), budget)?);
            kinds[kind] += 1;
            if kind == 0 {
                // An original AddressOf has no operands: 18 preflight, 2
                // scratch, 2 site, and two 3-work place comparisons. The three
                // unit query envelopes are temporary, not pointer authority.
                let storage = 6 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
                for short in 0..3 {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(28 - usize::from(short == 1));
                    let mut query = ArgumentBudgetV1::new(&mut work, storage - usize::from(short == 2));
                    let result = source_reference_raw_scalar_epoch_access_v29(plan, row.key.site, source, row.key.access, &resolved(), &mut query);
                    match short {
                        0 => { assert!(result.unwrap()); assert_eq!(query.work(), 28); },
                        1 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit))) if (limit.actual(), limit.limit()) == (28, 27))),
                        2 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(limit))) if (limit.actual(), limit.limit()) == (storage, storage - 1))),
                        _ => unreachable!(),
                    }
                    assert_eq!(query.storage(), 0);
                }
            }
            for fault in 0..7 {
                let mut changed = resolved();
                match fault {
                    0 => changed.instance = plan.instances.root(),
                    1 => changed.local = SemanticLocalIdV1::from_index(0),
                    2 => changed.projections.push(SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), source.ty()).unwrap()),
                    3 => changed.loan = Some(0),
                    4 => changed.traversed.push(0),
                    5 => changed.selector_source = Some((row.key.site, row.key.source)),
                    6 => changed.shared_path = !changed.shared_path,
                    _ => unreachable!(),
                }
                assert!(!source_reference_raw_scalar_epoch_access_v29(plan, row.key.site, source, row.key.access, &changed, budget)?, "kind {kind}, fault {fault}");
            }
            let cloned = source.clone();
            assert!(!source_reference_raw_scalar_epoch_access_v29(plan, row.key.site, &cloned, row.key.access, &resolved(), budget)?);
            assert!(!source_reference_raw_scalar_epoch_access_v29(plan, row.key.site, source, SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared), &resolved(), budget)?);
            assert!(source_reference_raw_scalar_epoch_access_v29(plan, row.key.site, source, row.key.access, &resolved(), budget)?);
        }
        assert_eq!(kinds, [2, 2], "two repeated original helper instances, both source roles");
        completed = true;
        Ok(())
    }).unwrap();
    assert!(completed);
}

#[test]
fn raw_scalar_epoch_preflight_has_a_fixed_prepaid_rejection_boundary() {
    let mut completed = false;
    with_selected_pointer_test_plan_v29(joined_backing_owner_v29(false, false, false), |plan, _| {
        let source = place(2, U32);
        let site = SourceReferenceSiteV29 { instance: plan.instances.root(), block: SemanticBlockIdV1::from_index(0), statement: Some(0) };
        let resolved = SourceReferencePlaceV29 {
            instance: site.instance, local: source.local(), generation: 0,
            value: usize::MAX, representation_root: usize::MAX, node: usize::MAX,
            projections: vec![], selector_source: None, anchor: None, loan: Some(0),
            shared_path: false, traversed: vec![],
        };
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(18 - usize::from(short));
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_reference_raw_scalar_epoch_access_v29(plan, site, &source, SourceReferenceAccessV29::Address, &resolved, &mut budget);
            if short {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit)))
                    if (limit.actual(), limit.limit()) == (18, 17)));
                assert_eq!(budget.work(), 0);
            } else { assert_eq!(result.unwrap(), false); assert_eq!(budget.work(), 18); }
            assert_eq!(budget.storage(), 0);
        }
        completed = true;
        Ok(())
    }).unwrap();
    assert!(completed);
}
