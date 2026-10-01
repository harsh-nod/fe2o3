fn owner_without_referent_addresses_v45() -> ProductionSemanticSsaOwnerV1 {
    let base = original_reference_enum_owner_v44();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut removed = 0;
    let blocks = helper
        .blocks()
        .iter()
        .map(|block| {
            let statements = block
                .statements()
                .iter()
                .filter(|statement| {
                    let remove = match statement.kind() {
                        SemanticStatementKindV1::Assign(assignment) => matches!(
                            assignment.value().kind(),
                            SemanticRvalueKindV1::AddressOf { place, .. }
                                if [4, 7].contains(&place.local().index())
                        ),
                        SemanticStatementKindV1::StorageLive(local)
                        | SemanticStatementKindV1::StorageDead(local) => {
                            [6, 9].contains(&local.index())
                        }
                        _ => false,
                    };
                    removed += usize::from(remove);
                    !remove
                })
                .cloned()
                .collect();
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        removed, 6,
        "two raw addresses and their four lifetime markers"
    );
    functions[2] = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        helper.entry(),
        blocks,
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn observe_demand_backed_reference_enum_v45(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    identity: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    observe_original_reference_enum_v44(
        instances, emitted, slots, references, identity, root, budget,
    )?;
    let before = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        let plan = references.unwrap().plan;
        let mut backed = [0, 0];
        for (loan, row) in plan.loans.iter().enumerate() {
            budget.charge_work(1)?;
            let origin = &plan.origins[row.origin];
            let function = instances.instance(origin.instance).unwrap().declaration();
            if function.locals().len() != 10 {
                continue;
            }
            let Some(index) = [4, 7]
                .iter()
                .position(|local| *local == origin.local.index())
            else {
                continue;
            };
            assert!(!function.blocks().iter().flat_map(|block| block.statements()).any(|statement|
            matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { place, .. }
                    if place.local() == origin.local))));
            let (_, cell) = plan
                .backing_cell(loan, budget)?
                .expect("stored original loan needs automatic backing");
            assert_eq!(
                (cell.instance, cell.local, cell.generation, cell.ty),
                (origin.instance, origin.local, origin.generation, U32)
            );
            let payload = source_reference_payload_types_v29(plan, loan, budget)?;
            assert!(matches!(payload.as_slice(), [Type::Pointer(pointer)]
            if pointer.address_space == AddressSpace::Private));
            backed[index] += 1;
        }
        assert!(
            backed.into_iter().all(|count| count != 0),
            "both distinct referents require backing"
        );
        Ok(())
    })?;
    assert_eq!(budget.storage(), before);
    Ok(())
}

#[test]
fn stored_shared_and_mutable_enum_references_materialize_referents_without_raw_addresses() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, bool, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            REFERENCE_ENUM_MUTABLE_V44.set(self.1);
            ENUM_CONSTRUCTION_FIELDS_V43.set(self.2);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_demand_backed_reference_enum_v45)),
        REFERENCE_ENUM_MUTABLE_V44.get(),
        ENUM_CONSTRUCTION_FIELDS_V43.get(),
    );
    for mutable in [false, true] {
        REFERENCE_ENUM_MUTABLE_V44.set(mutable);
        REFERENCE_ENUM_QUERIES_V44.set(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(COMPLETE_OBJECT_SOURCE_WORK_LIMIT_V43);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            scalar_payload_prepared_from_v18(owner_without_referent_addresses_v45, &mut budget);
        let completed = std::cell::Cell::new(false);
        let result =
            prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
                completed.set(true);
                Ok(())
            });
        assert!(result.is_ok(), "mutable={mutable}: {result:?}");
        assert!(completed.get());
        assert!(REFERENCE_ENUM_QUERIES_V44.get() >= 2);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
