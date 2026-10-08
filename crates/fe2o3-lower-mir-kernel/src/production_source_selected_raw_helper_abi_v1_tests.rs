// Constructor-backed ABI transport controls. These do not grant interior
// mutability, authenticate a rustc run, or complete ordinary atomic lowering.
fn selected_raw_helper_request_v1(
    immutable: bool,
    graph: u8,
    ownership: SemanticSourceArgumentOwnershipV1,
    fault: u8,
) -> InertSemanticMirRequestV1 {
    let base = selected_pointer_test_owner_v29(entrance_control_owner(false), immutable, graph);
    let semantic = base.source_semantic();
    let old = &semantic.functions()[1];
    let original = old.abi().arguments()[0].value();
    let raw = original.ty();
    let attributes = match original.mode() {
        SemanticAbiPassModeV1::Direct(attributes) => attributes.clone(),
        _ => panic!("original fixture has one direct raw pointer word"),
    };
    let value = match fault {
        0 => original.clone(),
        1 => SemanticAbiValueV1::new(raw, SemanticAbiPassModeV1::Ignore),
        2 => SemanticAbiValueV1::new(
            raw,
            SemanticAbiPassModeV1::Pair {
                first: attributes.clone(),
                second: attributes.clone(),
            },
        ),
        3 => SemanticAbiValueV1::new(
            raw,
            SemanticAbiPassModeV1::Indirect {
                attributes,
                metadata_attributes: None,
                on_stack: false,
            },
        ),
        4 => SemanticAbiValueV1::new_with_adjusted_type(
            raw,
            SemanticAbiAdjustedTypeV1::new(
                raw,
                semantic.types()[raw.index() as usize].layout_identity(),
                semantic.types()[raw.index() as usize].layout().clone(),
            ),
            original.mode().clone(),
        ),
        5 => original.clone().with_pointee_override(
            SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
        ),
        _ => panic!("closed raw helper ABI fault"),
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        old.abi().identity(),
        old.abi().layout_identity(),
        old.abi().canon_abi(),
        old.abi().extern_abi(),
        old.abi().can_unwind(),
        old.abi().c_variadic(),
        old.abi().fixed_count(),
        vec![SemanticAbiArgumentV1::source(value)],
        old.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    let mut functions = semantic.functions().to_vec();
    functions[1] = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        abi,
        old.locals().to_vec(),
        old.entry(),
        old.blocks().to_vec(),
    )
    .unwrap();
    InertSemanticMirRequestV1::new_with_callables(
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
}

fn selected_raw_helper_owner_v1(
    immutable: bool,
    graph: u8,
    ownership: SemanticSourceArgumentOwnershipV1,
    fault: u8,
) -> Result<ProductionSemanticSsaOwnerV1, String> {
    let admitted = selected_raw_helper_request_v1(immutable, graph, ownership, fault)
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .map_err(|error| format!("original admission: {error:?}"))?;
    let mir = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .map_err(|error| format!("original MIR owner: {error:?}"))?;
    ProductionSemanticSsaOwnerV1::try_new(mir, ProductionSemanticSsaLimitsV1::default())
        .map_err(|error| format!("original SSA owner: {error:?}"))
}

fn check_selected_raw_helper_signatures_v1(
    owner: ProductionSemanticSsaOwnerV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    immutable: bool,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut checked = 0;
    let result = with_selected_pointer_test_plan_v29(owner, |plan, budget| {
        for index in 0..plan.instances.instances().len() {
            let instance = ProductionCallInstanceIdV1(index);
            if instance == plan.root {
                continue;
            }
            let row = plan.instances.instance(instance).unwrap();
            assert_eq!(row.function().index(), 1);
            let original = plan
                .instances
                .owner()
                .source_semantic()
                .logical_arguments_v1(row.function())
                .unwrap();
            let mapped = original.adjusted_arguments().next().unwrap();
            assert_eq!(mapped.source_ownership(), ownership);
            assert!(mapped.local_field().is_none() && mapped.tuple_field().is_none());
            let node =
                source_reference_entry_node_v29(plan, instance, mapped.local(), None, budget)?
                    .unwrap();
            assert!(matches!(
                plan.nodes[node].kind,
                SourceReferenceNodeKindV29::Address(_)
            ));
            let selected =
                source_reference_selected_pointer_type_v29(plan, node, mapped.abi().ty(), budget)?
                    .unwrap();
            let Type::Pointer(pointer) = &selected else {
                panic!("selected pointer");
            };
            assert_eq!(pointer.address_space, AddressSpace::Private);
            assert_eq!(
                pointer.access,
                if immutable {
                    AccessMode::ReadOnly
                } else {
                    AccessMode::ReadWrite
                }
            );
            let signature = execution_function_signature_with_references_v29(
                plan.instances,
                instance,
                Some(plan),
                budget,
            )?;
            assert_eq!(signature.parameter_types, vec![selected.clone()]);
            assert_eq!(signature.result_types, vec![selected]);
            checked += 1;
        }
        Ok(())
    });
    if result.is_ok() {
        assert_eq!(checked, 2, "both original helper instances checked");
    }
    result
}

#[test]
fn selected_raw_helper_abi_transports_actual_raw_ownership_from_original_origins() {
    for immutable in [false, true] {
        for graph in 0..3 {
            let ownership = SemanticSourceArgumentOwnershipV1::RawPointer;
            let owner = selected_raw_helper_owner_v1(immutable, graph, ownership, 0).unwrap();
            let result = check_selected_raw_helper_signatures_v1(owner, ownership, immutable);
            assert!(
                result.is_ok(),
                "immutable {immutable}, graph {graph}: {result:?}"
            );
        }
    }
}

#[test]
fn selected_raw_helper_abi_preserves_historical_by_value_compatibility() {
    for immutable in [false, true] {
        for graph in 0..3 {
            let ownership = SemanticSourceArgumentOwnershipV1::ByValue;
            let owner = selected_raw_helper_owner_v1(immutable, graph, ownership, 0).unwrap();
            let result = check_selected_raw_helper_signatures_v1(owner, ownership, immutable);
            assert!(
                result.is_ok(),
                "immutable {immutable}, graph {graph}: {result:?}"
            );
        }
    }
}

fn selected_raw_helper_emission_owner_v1() -> ProductionSemanticSsaOwnerV1 {
    let (immutable, graph) = SELECTED_POINTER_FIXTURE.get();
    selected_raw_helper_owner_v1(
        immutable,
        graph,
        SemanticSourceArgumentOwnershipV1::RawPointer,
        0,
    )
    .unwrap()
}

#[test]
fn selected_raw_helper_abi_reaches_original_scoped_emission_without_generic_pointer_casts() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_selected_pointer_emission_v29)));
    for immutable in [false, true] {
        for graph in 0..3 {
            SELECTED_POINTER_FIXTURE.set((immutable, graph));
            SELECTED_POINTER_OBSERVED.set(0);
            SELECTED_POINTER_EXIT.set(0);
            pointer_structural_layouts_v29(selected_raw_helper_emission_owner_v1);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = scalar_payload_prepared_from_v18(
                selected_raw_helper_emission_owner_v1,
                &mut budget,
            );
            let result = prepared.with_source_consumer_v18(
                &mut budget,
                |_, _| -> SourceOwnedResultV18<()> {
                    panic!("final production completion remains gated");
                },
            );
            assert_eq!(
                SELECTED_POINTER_OBSERVED.get(),
                1,
                "immutable {immutable}, graph {graph}: {result:?}"
            );
            assert!(format!("{:?}", result.unwrap_err()).contains(SELECTED_POINTER_STOP));
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn selected_raw_helper_abi_cannot_substitute_borrow_or_exclusive_ownership() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        SemanticSourceArgumentOwnershipV1::Unspecified,
    ] {
        // Earlier original admission is also an intended refusal, not an
        // alternative success or an invented source-reference capability.
        let result = selected_raw_helper_owner_v1(false, 0, ownership, 0).and_then(|owner| {
            check_selected_raw_helper_signatures_v1(owner, ownership, false)
                .map_err(|error| format!("source ABI: {error:?}"))
        });
        assert!(
            result.is_err(),
            "foreign ownership {ownership:?} acquired transport"
        );
    }
}

#[test]
fn selected_raw_helper_abi_refuses_non_direct_adjusted_and_pointee_override_words() {
    for fault in 1..=5 {
        let ownership = SemanticSourceArgumentOwnershipV1::RawPointer;
        let result = selected_raw_helper_owner_v1(false, 0, ownership, fault).and_then(|owner| {
            check_selected_raw_helper_signatures_v1(owner, ownership, false)
                .map_err(|error| format!("source ABI: {error:?}"))
        });
        assert!(
            result.is_err(),
            "ABI fault {fault} acquired selected-pointer transport"
        );
    }
}

#[test]
fn selected_raw_helper_abi_checks_the_original_ledger_before_any_debit() {
    let reached = std::cell::Cell::new(false);
    let owner =
        selected_raw_helper_owner_v1(false, 0, SemanticSourceArgumentOwnershipV1::RawPointer, 0)
            .unwrap();
    let result = with_selected_pointer_test_plan_v29(owner, |plan, budget| {
        let index = (0..plan.instances.instances().len())
            .find(|i| *i != plan.root.index())
            .unwrap();
        let instance = ProductionCallInstanceIdV1(index);
        let row = plan.instances.instance(instance).unwrap();
        let original = plan
            .instances
            .owner()
            .source_semantic()
            .logical_arguments_v1(row.function())
            .unwrap();
        let mapped = original.adjusted_arguments().next().unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let before = (budget.work(), budget.storage());
        assert!(matches!(
            check_source_reference_parameter_v29(plan, instance, mapped, &mut foreign),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((foreign.work(), foreign.storage()), (0, 0));
        assert_eq!((budget.work(), budget.storage()), before);
        reached.set(true);
        Ok(())
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}

#[test]
fn selected_raw_helper_word_validation_retains_raw_kind_width_and_address_space() {
    let owner =
        selected_raw_helper_owner_v1(false, 0, SemanticSourceArgumentOwnershipV1::RawPointer, 0)
            .unwrap();
    let original = &owner.source_semantic().types()[2];
    assert!(source_reference_raw_abi_scalar_v29(original).is_ok());
    let SemanticTypeShapeV1::Pointer(raw) = original.shape() else {
        panic!("raw source type");
    };
    for (kind, space, width) in [
        (SemanticPointerKindV1::Reference, 0, 64),
        (SemanticPointerKindV1::Raw, 1, 64),
        (SemanticPointerKindV1::Raw, 0, 32),
    ] {
        let changed = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            original.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    raw.pointee(),
                    kind,
                    raw.mutability(),
                    space,
                    width,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
        assert!(source_reference_raw_abi_scalar_v29(&changed).is_err());
    }
}
