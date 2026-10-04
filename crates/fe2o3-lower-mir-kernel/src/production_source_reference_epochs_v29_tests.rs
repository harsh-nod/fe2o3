const ADDRESS_TEST_LIMIT: usize = 20_000_000;

fn with_address_builder(
    consume: impl FnOnce(
        &mut SourceReferenceBuilderV29<'_, '_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owner = address_owner(AddressFlow::Read);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(ADDRESS_TEST_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, ADDRESS_TEST_LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        let index = builder.cfg_index(instances.root(), budget).unwrap();
        let result = consume(&mut builder, budget);
        drop(index);
        drop(builder);
        assert!(ledger == budget.work_ledger_identity_v1());
        assert!(budget.storage() >= floor);
        budget.release_storage(budget.storage() - floor).unwrap();
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
    })
    .unwrap()
}

#[test]
fn original_activation_union_is_canonical_object_scoped_and_not_an_incarnation() {
    let mut completed = false;
    with_address_builder(|builder, budget| {
        let instance = builder.plan.root;
        let local = SemanticLocalIdV1::from_index(2);
        let a = builder.join_storage_epochs(instance, local, 0, 1, budget)?;
        let limits = (
            builder.plan.epoch_sets.len(),
            builder.plan.epoch_members.len(),
            builder.plan.epoch_objects.len(),
        );
        for _ in 0..12 {
            assert_eq!(
                builder.join_storage_epochs(instance, local, 1, 0, budget)?,
                a
            );
            assert_eq!(
                builder.join_storage_epochs(instance, local, a, 1, budget)?,
                a
            );
            assert_eq!(
                (
                    builder.plan.epoch_sets.len(),
                    builder.plan.epoch_members.len(),
                    builder.plan.epoch_objects.len()
                ),
                limits
            );
        }
        assert!(builder.epoch_includes(instance, local, a, 0, budget)?);
        assert!(builder.epoch_includes(instance, local, a, 1, budget)?);
        assert!(!builder.epoch_includes(instance, local, 0, a, budget)?);
        assert!(builder.epochs_overlap(instance, local, 0, a, budget)?);
        assert!(!builder.epochs_overlap(instance, local, 0, 1, budget)?);
        assert!(!builder.epoch_includes(instance, local, u32::MAX, a, budget)?);
        let limit = builder.epoch_limit(instance, budget)?;
        assert!(
            builder
                .epoch_atoms(instance, SemanticLocalIdV1::from_index(3), a, limit, budget)
                .is_err()
        );
        assert!(
            builder
                .epoch_atoms(instance, SemanticLocalIdV1::from_index(3), 1, limit, budget)
                .is_err()
        );
        assert!(
            builder
                .epoch_atoms(instance, local, 2, limit, budget)
                .is_err()
        );
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
}

#[test]
fn raw_formation_rejects_cloned_places_wrong_source_sites_and_result_types() {
    let mut completed = false;
    with_address_builder(|builder, budget| {
        let instance = builder.plan.root;
        let function = builder
            .plan
            .instances
            .instance(instance)
            .unwrap()
            .declaration();
        let assignment = match function.blocks()[0].statements()[2].kind() {
            SemanticStatementKindV1::Assign(assignment) => assignment,
            _ => panic!("original address statement changed"),
        };
        let SemanticRvalueKindV1::AddressOf { place, mutability } = assignment.value().kind()
        else {
            panic!("original address rvalue changed");
        };
        let site = SourceReferenceSiteV29 {
            instance,
            block: SemanticBlockIdV1::from_index(0),
            statement: Some(2),
        };
        let clone = place.clone();
        assert!(
            builder
                .address_value(
                    site,
                    &clone,
                    assignment.value().result_type(),
                    *mutability,
                    budget
                )
                .is_err()
        );
        assert!(
            builder
                .address_value(site, place, U32, *mutability, budget)
                .is_err()
        );
        assert!(
            builder
                .address_value(
                    SourceReferenceSiteV29 {
                        statement: Some(1),
                        ..site
                    },
                    place,
                    assignment.value().result_type(),
                    *mutability,
                    budget
                )
                .is_err()
        );
        assert!(builder.plan.raw_origins.is_empty());
        assert!(builder.plan.raw_origin_sites.is_empty());
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
}

#[test]
fn raw_choice_union_keeps_expiry_on_the_same_original_origin() {
    let mut completed = false;
    with_address_plan(AddressFlow::Read, |plan, budget| {
        let live = SourceReferenceRawChoicesV29::Singleton(SourceReferenceRawChoiceV29 {
            origin: 0,
            expired: false,
        });
        let expired = SourceReferenceRawChoicesV29::Singleton(SourceReferenceRawChoiceV29 {
            origin: 0,
            expired: true,
        });
        let mut union = SourceReferenceRawUnionV29 {
            left: live,
            right: expired,
            left_index: 0,
            right_index: 0,
            expire: false,
        };
        assert_eq!(
            plan.next_raw_union(&mut union, budget)?,
            Some(SourceReferenceRawChoiceV29 {
                origin: 0,
                expired: true
            })
        );
        assert_eq!(plan.next_raw_union(&mut union, budget)?, None);
        assert_eq!(plan.raw_origins.len(), 1);
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
}
