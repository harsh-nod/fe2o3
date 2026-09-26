use super::layout_tests::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

include!("production_source_storage_fixture_types_v29_tests.rs");

fn projected_pointer_geometry_query_v29(plan: &SourceReferencePlanV29<'_, '_>, budget: &mut Budget<'_>) -> Result<(), Error> {
    let cell = plan.cells.rows.iter().find(|cell| cell.local.index() == 11).unwrap();
    let SourceBackingKindV29::Object(schema) = cell.kind else { panic!("original record object"); };
    let function = plan.instances.instance(plan.root).unwrap().declaration();
    let original = function.blocks()[0].statements().iter().find_map(|statement| match statement.kind() {
        SemanticStatementKindV1::Assign(assignment) if assignment.destination().local().index() == 11
            && assignment.destination().ty() == SemanticTypeIdV1::from_index(2) => Some(assignment.destination()),
        _ => None,
    }).unwrap();
    let components = budget.source_object_projection_v29(plan, cell.ty, schema, original.projections())?;
    assert_eq!(components.len(), 1);
    assert!(std::ptr::eq(components[0].projection, &original.projections()[0]));
    assert!(matches!(components[0].kind, SourceSelectedComponentKindV29::Field { original: 0, physical: 0, byte_offset: 0 }));
    assert_eq!(components[0].result_type, original.ty());
    assert!(components[0].result_schema.is_some());
    Ok(())
}

#[test]
fn projected_pointer_schema_query_exact_and_one_short_work_storage_preserve_custody() {
    let mut measured = None;
    with_selected_pointer_test_plan_v29(projected_pointer_test_owner_v29(owner(), false, 1), |plan, budget| {
        budget.reserve_storage(budget.peak_storage() - budget.storage() + 1)?;
        let before = (budget.work(), budget.storage());
        projected_pointer_geometry_query_v29(plan, budget)?;
        measured = Some((budget.work() - before.0, budget.storage() - before.1, budget.peak_storage() - before.1));
        Ok(())
    }).unwrap();
    let (work, storage, peak) = measured.unwrap();
    assert!(work > 0 && storage > 0 && peak >= storage);
    for storage_cut in [false, true] {
        for short in [0, 1] {
            let mut entered = false;
            let mut published = false;
            let result = with_selected_pointer_test_plan_v29(projected_pointer_test_owner_v29(owner(), false, 1), |plan, budget| {
                entered = true;
                budget.reserve_storage(budget.peak_storage() - budget.storage() + 1)?;
                if storage_cut { budget.reserve_storage(64 * 1024 * 1024 - budget.storage() - (peak - short))?; }
                else { budget.charge_work(20_000_000 - budget.work() - (work - short))?; }
                let before = (budget.work(), budget.storage());
                let result = projected_pointer_geometry_query_v29(plan, budget);
                if short == 0 {
                    result?;
                    assert_eq!((budget.work() - before.0, budget.storage() - before.1), (work, storage));
                    assert_eq!(budget.peak_storage() - before.1, peak);
                    published = true;
                } else {
                    match (storage_cut, result) {
                        (true, Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))))
                        | (false, Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))) => {}
                        (_, other) => panic!("wrong one-short refusal: {other:?}"),
                    }
                    assert!(projected_pointer_geometry_query_v29(plan, budget).is_err());
                }
                Ok(())
            });
            assert!(entered);
            assert_eq!(published, short == 0);
            assert_eq!(result.is_ok(), short == 0, "storage {storage_cut}, short {short}: {result:?}");
        }
    }
}

#[test]
fn projected_pointer_write_query_rejects_wrong_type_and_cloned_original_locator() {
    for clone in [false, true] {
        let mut entered = false;
        let result = with_selected_pointer_test_plan_v29(projected_pointer_test_owner_v29(owner(), false, 1), |plan, budget| {
            entered = true;
            let function = plan.instances.instance(plan.root).unwrap().declaration();
            let (statement, assignment) = function.blocks()[0].statements().iter().enumerate().find_map(|(ordinal, statement)| match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) if assignment.destination().local().index() == 11
                    && assignment.destination().ty() == SemanticTypeIdV1::from_index(2) => Some((ordinal, assignment)),
                _ => None,
            }).unwrap();
            let site = SourceReferenceSiteV29 { instance: plan.root, block: SemanticBlockIdV1::from_index(0), statement: Some(statement) };
            let copied = assignment.destination().clone();
            let original = if clone { &copied } else { assignment.destination() };
            let ty = if clone { original.ty() } else { UNIT };
            assert!(source_reference_write_pointer_v29(plan, site, original, ty, budget).is_err());
            let before = (budget.work(), budget.storage());
            assert!(source_reference_assignment_pointer_v29(plan, site, assignment, budget).is_err());
            assert_eq!((budget.work(), budget.storage()), before);
            Ok(())
        });
        assert!(entered && result.is_err());
    }
}

#[test]
fn projected_pointer_schema_query_refuses_foreign_ledger_and_alternate_meter() {
    struct Alternate<'a, 'work>(&'a mut Budget<'work>);
    impl SemanticEmissionBudgetV1 for Alternate<'_, '_> {
        fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 { self.0.work_ledger_identity_v1() }
        fn charge_work(&mut self, amount: usize) -> Result<(), Error> { self.0.charge_work(amount).map_err(Into::into) }
        fn reserve_storage(&mut self, amount: usize) -> Result<(), Error> { self.0.reserve_storage(amount).map_err(Into::into) }
        fn release_storage(&mut self, amount: usize) -> Result<(), Error> { self.0.release_storage(amount).map_err(Into::into) }
        fn storage(&self) -> usize { self.0.storage() }
    }
    for alternate in [false, true] {
        let mut entered = false;
        let result = with_selected_pointer_test_plan_v29(projected_pointer_test_owner_v29(owner(), false, 1), |plan, budget| {
            entered = true;
            let cell = plan.cells.rows.iter().find(|cell| cell.local.index() == 11).unwrap();
            let SourceBackingKindV29::Object(schema) = cell.kind else { unreachable!() };
            let before = (budget.work(), budget.storage());
            if alternate {
                assert!(Alternate(budget).source_object_projection_v29(plan, cell.ty, schema, &[]).is_err());
            } else {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
                let mut foreign = Budget::new(&mut work, 1000);
                assert!(source_object_projection_v29(plan, cell.ty, schema, &[], &mut foreign).is_err());
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            }
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(projected_pointer_geometry_query_v29(plan, budget).is_err());
            Ok(())
        });
        assert!(entered && result.is_err());
    }
}

fn selected_pointer_query_node_v29(plan: &SourceReferencePlanV29<'_, '_>) -> usize {
    plan.nodes.iter().position(|row| row.ty == SemanticTypeIdV1::from_index(2)
        && matches!(row.kind, SourceReferenceNodeKindV29::Address(_))).unwrap()
}

#[test]
fn selected_raw_pointer_storage_lineage_preserves_chained_helper_return_snapshots() {
    for graph in [0, 1, 2] {
        for immutable in [false, true] {
            with_selected_pointer_test_plan_v29(selected_pointer_test_owner_v29(owner(), immutable, graph), |plan, budget| {
                let expected = SemanticTypeIdV1::from_index(2);
                let mut copies = 0;
                for (node, row) in plan.nodes.iter().enumerate() {
                    if row.ty != expected || row.value_origin.is_none() { continue; }
                    let SourceReferenceNodeKindV29::Address(set) = row.kind else { continue; };
                    let before = *row;
                    let storage = row.storage.expect("attached pointer snapshot");
                    let snapshot = plan.storage_snapshots[storage.snapshot];
                    let path = &plan.projections[storage.first..storage.first + storage.count];
                    let canonical = plan.raw_nodes[&(expected.index(), set)];
                    assert_eq!(row.value_origin, Some(canonical));
                    assert!(canonical < node);
                    assert!(plan.nodes[canonical].value_origin.is_none());
                    assert!(plan.nodes[canonical].storage.is_none());
                    let selected = source_reference_selected_pointer_type_v29(plan, node, expected, budget)?.unwrap();
                    assert!(matches!(selected, Type::Pointer(_)));
                    assert_eq!(plan.nodes[node], before, "selection must not erase the attached storage state");
                    assert_eq!(plan.storage_snapshots[storage.snapshot], snapshot);
                    assert_eq!(&plan.projections[storage.first..storage.first + storage.count], path);
                    copies += 1;
                }
                assert!(copies >= 2, "both original helper invocations must retain storage-annotated values");
                Ok(())
            }).unwrap();
        }
    }
}

#[test]
fn projected_pointer_original_write_query_admits_attached_value_lineage() {
    with_selected_pointer_test_plan_v29(projected_pointer_test_owner_v29(owner(), false, 1), |plan, budget| {
        let function = plan.instances.instance(plan.root).unwrap().declaration();
        let (statement, assignment) = function.blocks()[0].statements().iter().enumerate().find_map(|(ordinal, statement)| match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) if assignment.destination().local().index() == 11
                && assignment.destination().ty() == SemanticTypeIdV1::from_index(2) => Some((ordinal, assignment)),
            _ => None,
        }).unwrap();
        let site = SourceReferenceSiteV29 { instance: plan.root, block: SemanticBlockIdV1::from_index(0), statement: Some(statement) };
        let (selected, _) = source_reference_assignment_pointer_v29(plan, site, assignment, budget)?.unwrap();
        assert!(matches!(selected, Type::Pointer(_)));
        assert!(plan.nodes.iter().any(|row| row.ty == assignment.value().result_type()
            && row.storage.is_some() && row.value_origin.is_some()
            && matches!(row.kind, SourceReferenceNodeKindV29::Address(_))));
        Ok(())
    }).unwrap();
}

#[test]
fn selected_raw_pointer_lineage_checker_refuses_forged_roots_cycles_and_changed_values() {
    with_selected_pointer_test_plan_v29(selected_pointer_test_owner_v29(owner(), false, 0), |plan, _| {
        let node = plan.nodes.iter().position(|row| row.ty == SemanticTypeIdV1::from_index(2)
            && row.value_origin.is_some() && matches!(row.kind, SourceReferenceNodeKindV29::Address(_))).unwrap();
        let canonical = plan.nodes[node].value_origin.unwrap();
        assert!(canonical > 0);
        for fault in 0..12 {
            // These copies exercise only the inert lineage checker, never publish source authority.
            let mut rows = plan.nodes.clone();
            match fault {
                0 => rows[node].value_origin = None,
                1 => rows[node].value_origin = Some(node),
                2 => rows[node].value_origin = Some(node + 1),
                3 => rows[node].value_origin = Some(usize::MAX),
                4 => rows[node].kind = SourceReferenceNodeKindV29::Address(usize::MAX),
                5 => rows[node].ty = UNIT,
                6 => rows[node].storage = None,
                7 => rows[node].descriptor = Some(0),
                8 => rows[canonical].value_origin = Some(node),
                9 => rows[canonical].storage = rows[node].storage,
                10 => rows[canonical].kind = SourceReferenceNodeKindV29::Plain(None),
                11 => rows[node].value_origin = Some(canonical - 1),
                _ => unreachable!(),
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(12);
            let mut budget = Budget::new(&mut work, 71);
            budget.reserve_storage(71)?;
            assert!(matches!(source_reference_address_value_origin_v29(&rows, node, canonical, &mut budget),
                Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))), "fault {fault}");
            assert_eq!((budget.work(), budget.storage(), budget.peak_storage()), (12, 71, 71));
        }
        Ok(())
    }).unwrap();
}

#[test]
fn selected_raw_pointer_flattened_lineage_has_independent_exact_work_and_no_storage() {
    with_selected_pointer_test_plan_v29(selected_pointer_test_owner_v29(owner(), false, 0), |plan, _| {
        let mut count = 0;
        for (node, row) in plan.nodes.iter().enumerate() {
            if row.ty != SemanticTypeIdV1::from_index(2) { continue; }
            let SourceReferenceNodeKindV29::Address(set) = row.kind else { continue; };
            let Some(&canonical) = plan.raw_nodes.get(&(row.ty.index(), set)) else { continue; };
            if node != canonical && row.value_origin.is_none() { continue; }
            for short in [0, 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(12 - short);
                let mut budget = Budget::new(&mut work, 73);
                budget.reserve_storage(73)?;
                let result = source_reference_address_value_origin_v29(&plan.nodes, node, canonical, &mut budget);
                if short == 0 {
                    result?;
                    assert_eq!(budget.work(), 12);
                } else {
                    assert!(matches!(result, Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (73, 73));
            }
            count += 1;
        }
        assert!(count >= 3, "one original and multiple flattened attachment/return copies");
        Ok(())
    }).unwrap();
}

#[test]
fn selected_raw_pointer_query_seeds_unstored_addresses_and_preserves_stored_copy_schema() {
    for graph in [0, 1, 2, 3] {
        for immutable in [false, true] {
            with_boundary_schema_source_v29(selected_pointer_test_owner_v29(owner(), immutable, graph), |plan, budget| {
                let node = selected_pointer_query_node_v29(plan);
                let expected = SemanticTypeIdV1::from_index(2);
                let selected = source_reference_selected_pointer_type_v29(plan, node, expected, budget)?.unwrap();
                let Type::Pointer(pointer) = selected else { panic!("selected ordinary pointer"); };
                assert_eq!(pointer.address_space, AddressSpace::Private);
                assert_eq!(pointer.access, if immutable { AccessMode::ReadOnly } else { AccessMode::ReadWrite });
                let Type::StorageObject(pointee) = *pointer.pointee else { panic!("selected pointee schema"); };
                let origin = plan.raw_origins.iter().position(|origin| origin.pointer_type == expected).unwrap();
                let cell = &plan.cells.rows[plan.cells.raw_origins[origin]];
                assert_eq!(cell.kind, SourceBackingKindV29::Object(pointee));
                assert_eq!(cell.local.index(), 2);
                assert!(plan.loans.is_empty(), "raw selection cannot manufacture reference permission");
                if graph == 3 {
                    assert!(!plan.cells.rows.iter().any(|cell| cell.local.index() == 3), "unstored pointer is not a holder allocation");
                } else {
                    let holder = plan.cells.rows.iter().find(|cell| cell.local.index() == 3).unwrap();
                    assert_eq!(holder.kind, SourceBackingKindV29::Object(plan.selected_storage[node].unwrap()));
                }
                for raw in plan.raw_accesses.values() {
                    let holder = &plan.nodes[raw.holder.node];
                    assert!(matches!(holder.kind, SourceReferenceNodeKindV29::Address(set) if set == raw.set));
                    source_reference_selected_pointer_type_v29(plan, raw.holder.node, holder.ty, budget)?.unwrap();
                }
                Ok(())
            }).unwrap();
        }
    }
}

#[test]
fn selected_raw_pointer_query_has_exact_and_one_short_work_and_storage() {
    let mut measured = None;
    with_boundary_schema_source_v29(selected_pointer_test_owner_v29(owner(), false, 0), |plan, budget| {
        let node = selected_pointer_query_node_v29(plan);
        budget.reserve_storage(budget.peak_storage() - budget.storage() + 1)?;
        let before = (budget.work(), budget.storage());
        let value = source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty, budget)?.unwrap();
        measured = Some((budget.work() - before.0, budget.storage() - before.1, budget.peak_storage() - before.1));
        assert!(matches!(value, Type::Pointer(_)));
        Ok(())
    }).unwrap();
    let (work, storage, peak) = measured.unwrap();
    assert!(work > 0 && storage >= size_of::<Option<Type>>() + size_of::<Type>() && peak >= storage);
    for storage_cut in [false, true] {
        for short in [0, 1] {
            let mut entered = false;
            let mut published = false;
            let result = with_boundary_schema_source_v29(selected_pointer_test_owner_v29(owner(), false, 0), |plan, budget| {
                entered = true;
                let node = selected_pointer_query_node_v29(plan);
                budget.reserve_storage(budget.peak_storage() - budget.storage() + 1)?;
                if storage_cut { budget.reserve_storage(64 * 1024 * 1024 - budget.storage() - (peak - short))?; }
                else { budget.charge_work(20_000_000 - budget.work() - (work - short))?; }
                let before = (budget.work(), budget.storage());
                let query = source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty, budget);
                if short == 0 {
                    assert!(query?.is_some());
                    assert_eq!((budget.work() - before.0, budget.storage() - before.1), (work, storage));
                    assert_eq!(budget.peak_storage() - before.1, peak);
                    published = true;
                } else {
                    assert!(matches!(query, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_) | ArgumentResourceV1::Storage(_)))));
                    assert!(source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty, budget).is_err());
                }
                Ok(())
            });
            assert!(entered);
            assert_eq!(published, short == 0);
            assert_eq!(result.is_ok(), short == 0, "storage {storage_cut}, short {short}: {result:?}");
        }
    }
}

#[test]
fn selected_raw_pointer_query_refuses_changed_schema_and_keeps_failure_sticky() {
    for mutation in 0..4 {
        let mut entered = false;
        let result = with_boundary_schema_source_v29(selected_pointer_test_owner_v29(owner(), false, 0), |plan, budget| {
            entered = true;
            let node = selected_pointer_query_node_v29(plan);
            let expected = plan.nodes[node].ty;
            let schema = plan.selected_storage[node].unwrap();
            let layouts = plan.storage_root.as_ref().unwrap().source_layouts(plan.instances, budget)?;
            if mutation != 0 {
                let mut physical = layouts.physical.borrow_mut();
                let StorageLayoutKindV1::Pointer(pointer) = &mut physical.rows[schema.0 as usize].kind else { panic!("pointer row"); };
                match mutation {
                    1 => pointer.access = AccessMode::ReadOnly,
                    2 => pointer.value_space = AddressSpace::Generic,
                    3 => pointer.pointee = schema,
                    _ => unreachable!(),
                }
            }
            let wrong = if mutation == 0 { UNIT } else { expected };
            assert!(source_reference_selected_pointer_type_v29(plan, node, wrong, budget).is_err());
            let before = (budget.work(), budget.storage());
            assert!(source_reference_selected_pointer_type_v29(plan, node, expected, budget).is_err());
            assert_eq!((budget.work(), budget.storage()), before, "sticky refusal must stop further work");
            Ok(())
        });
        assert!(entered);
        assert!(result.is_err(), "swallowed selected-pointer custody failure: {mutation}");
    }
}

#[test]
fn selected_raw_pointer_query_rejects_foreign_ledger_and_alternate_meter() {
    struct Alternate<'a, 'work>(&'a mut ArgumentBudgetV1<'work>);
    impl SemanticEmissionBudgetV1 for Alternate<'_, '_> {
        fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 { self.0.work_ledger_identity_v1() }
        fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> { self.0.charge_work(amount).map_err(Into::into) }
        fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> { self.0.reserve_storage(amount).map_err(Into::into) }
        fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> { self.0.release_storage(amount).map_err(Into::into) }
        fn storage(&self) -> usize { self.0.storage() }
    }
    for alternate in [false, true] {
        let mut entered = false;
        let result = with_boundary_schema_source_v29(selected_pointer_test_owner_v29(owner(), false, 3), |plan, budget| {
            entered = true;
            let node = selected_pointer_query_node_v29(plan);
            let before = (budget.work(), budget.storage());
            let attempted = if alternate {
                Alternate(budget).source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty)
            } else {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
                let mut foreign = ArgumentBudgetV1::new(&mut work, 1000);
                let result = source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty, &mut foreign);
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                result
            };
            assert!(attempted.is_err());
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty, budget).is_err());
            Ok(())
        });
        assert!(entered && result.is_err());
    }
}

fn original_raw_boundary_owner_v29(declarations: Vec<SemanticTypeDeclV1>, ty: SemanticTypeIdV1) -> ProductionSemanticSsaOwnerV1 {
    let template = owner_with(declarations);
    let source = template.source_semantic();
    let closure = FixtureTypeClosureV29::new(source.types(), &[UNIT, ty]);
    let unit = closure.id(UNIT);
    let ty = closure.id(ty);
    let original = &source.functions()[0];
    let span = original.blocks()[0].source();
    let layout = closure.types[ty.index() as usize].layout();
    let mode = SemanticAbiPassModeV1::Indirect {
        attributes: SemanticAbiValueAttributesV1::new(SemanticAbiRegularAttributesV1::new(
            true, Some(SemanticAbiPointerCaptureV1::CapturesNone), true, false, false, true),
            SemanticAbiExtensionV1::None, layout.size_bytes().unwrap(), Some(layout.alignment_bytes())).unwrap(),
        metadata_attributes: None, on_stack: false,
    };
    let helper_mode = if layout.size_bytes() == Some(8) {
        SemanticAbiPassModeV1::cast(false, SemanticAbiCastV1::new([None; 8], None,
            SemanticAbiUniformV1::new(SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(), 8).unwrap(),
            SemanticAbiValueAttributesV1::plain()))
    } else { mode.clone() };
    let abi = |root| SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([if root { 230 } else { 231 }; 32]),
        original.abi().layout_identity(),
        if root { SemanticCanonAbiV1::GpuKernel } else { SemanticCanonAbiV1::Rust },
        if root { SemanticExternAbiV1::GpuKernel } else { SemanticExternAbiV1::Rust },
        false, false, 1, vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(ty, if root { mode.clone() } else { helper_mode.clone() }))],
        if root { SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore) }
            else { SemanticAbiValueV1::new(ty, helper_mode.clone()) }).unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue]).unwrap();
    let local = |tag, ty, role| SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([tag; 32]), ty, role, span);
    let place = |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, ty, value| SemanticStatementV1::new(span, SemanticStatementKindV1::Assign(
        SemanticAssignmentV1::new(place(local, ty), SemanticRvalueV1::new(ty, SemanticRvalueKindV1::Use(value)))));
    let block = |tag, statements, terminator| SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]), span, statements, SemanticTerminatorV1::new(span, terminator)).unwrap();
    let function = |tag, role, abi, locals, blocks| SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]), role,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]), SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]), SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        span, abi, locals, SemanticBlockIdV1::from_index(0), blocks).unwrap();
    let root = function(230, SemanticFunctionRoleV1::KernelRoot, abi(true), vec![
        local(230, unit, SemanticLocalRoleV1::Return), local(231, ty, SemanticLocalRoleV1::Argument(0)),
        local(232, ty, SemanticLocalRoleV1::Temporary),
    ], vec![
        block(230, vec![], SemanticTerminatorKindV1::Call(SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1), vec![SemanticOperandV1::Copy(place(1, ty))],
            Some(SemanticCallDestinationV1::new(place(2, ty), SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1::from_index(1)))), SemanticUnwindActionV1::Unreachable).unwrap())),
        block(231, vec![assign(0, unit, SemanticOperandV1::Constant(SemanticConstantV1::new(unit,
            SemanticConstantValueV1::ZeroSized)))], SemanticTerminatorKindV1::Return),
    ]).with_kernel_entry(original.kernel_entry().unwrap().clone());
    let helper = function(231, SemanticFunctionRoleV1::InternalHelper, abi(false), vec![
        local(233, ty, SemanticLocalRoleV1::Return), local(234, ty, SemanticLocalRoleV1::Argument(0)),
    ], vec![block(232, vec![assign(0, ty, SemanticOperandV1::Copy(place(1, ty)))], SemanticTerminatorKindV1::Return)]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(source.target(), closure.types,
        vec![], vec![], vec![], vec![root, helper], vec![SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1))], vec![ROOT]).unwrap()
        .admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(admitted,
        ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

#[test]
fn actual_empty_demand_raw_recursive_boundaries_select_original_closed_schemas() {
    for descriptor in [false, true] {
        let (declarations, ty) = original_cycle_types_v29(2, descriptor);
        let identity = declarations[ty.index() as usize].identity();
        with_boundary_schema_source_v29(original_raw_boundary_owner_v29(declarations, ty), |plan, budget| {
            let ty = fixture_type_identity_v29(plan.instances.owner(), identity);
            assert!(plan.storage_demands.unwrap().requests(plan.instances, budget)?.0.is_empty());
            assert!(plan.cells.rows.is_empty());
            assert!(plan.storage_activations.is_empty());
            assert!(plan.representation_demands.is_empty());
            let root = plan.instances.root();
            let site = SourceReferenceSiteV29 { instance: root, block: SemanticBlockIdV1::from_index(0), statement: None };
            let schema = source_reference_boundary_schema_v29(plan, site, SourceReferenceBoundaryRoleV29::Argument(0), budget)?.unwrap();
            let entry = plan.states[plan.entries[root.index()].unwrap()][1].node.unwrap();
            assert_eq!(source_reference_selected_value_schema_v29(plan, entry, ty, budget)?, Some(schema));
            let layouts = plan.storage_root.as_ref().unwrap().source_layouts(plan.instances, budget)?;
            assert!(layouts.keys.is_empty());
            assert_eq!(layouts.original_schema(plan.instances.owner(), ty, budget)?, None);
            assert_eq!(layouts.select_original_closure(plan.instances.owner(), ty, budget)?, schema);
            let rows = layouts.rows(plan.instances.owner(), budget)?;
            fe2o3_kernel_ir::check_storage_layouts_v1(&rows, layouts.limits, budget)
                .map_err(source_storage_limits_error_v29)?;
            assert_eq!(rows.len(), if descriptor { 8 } else { 4 });
            Ok(())
        }).unwrap();
    }
}

#[test]
fn original_extension_cache_cannot_adopt_a_different_selected_pointer_representation() {
    for changed in [false, true] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(263).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let original = layouts.select_original_closure(&owner, RAW, &mut budget).unwrap();
        let word = layouts.original_completed_id(RowKey::ty(WORD), &mut budget).unwrap().unwrap();
        let selected = layouts.select_schema(&owner, RAW, SourceStorageSelectionV29::Pointer {
            pointee: word, value_space: AddressSpace::Global, access: AccessMode::ReadOnly,
        }, &mut budget).unwrap();
        assert_ne!(selected, original);
        assert_eq!(layouts.select_original_closure(&owner, RAW, &mut budget).unwrap(), original);
        let before = layouts.lease.persistent.get();
        if changed {
            // Both rows were built by the actual authenticated owner first.
            layouts.physical.borrow_mut().original_extensions.insert(RowKey::ty(RAW), selected);
            assert!(layouts.select_original_closure(&owner, RAW, &mut budget).is_err());
        }
        assert_eq!(layouts.lease.persistent.get(), before);
        assert_eq!(layouts.original_schema(&owner, RAW, &mut budget).unwrap(), None);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 263);
    }
}

fn boundary_schema_owner_v29(ty: SemanticTypeIdV1) -> ProductionSemanticSsaOwnerV1 {
    let template = owner();
    let source = template.source_semantic();
    let original_ty = ty;
    let closure = FixtureTypeClosureV29::new(source.types(), &[UNIT, WORD, PAIR, ty]);
    let ty = closure.id(ty);
    let unit = closure.id(UNIT);
    let word = closure.id(WORD);
    let byte = closure.id(BYTE);
    let pair = closure.id(PAIR);
    assert_eq!((unit, word), (UNIT, WORD), "cloned scalar root ABI IDs");
    let root = &source.functions()[0];
    let span = root.blocks()[0].source();
    let place = |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let constant = |ty, value, bytes| SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty, SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, bytes).unwrap()),
    ));
    let assign = |local, ty, value| SemanticStatementV1::new(span,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(place(local, ty), SemanticRvalueV1::new(ty, value))));
    let local = |tag, ty, role| SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([tag; 32]), ty, role, span);
    let block = |tag, statements, terminator| SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]), span, statements,
        SemanticTerminatorV1::new(span, terminator)).unwrap();
    let make = |second: bool| {
        let value = if second { constant(word, 29, 8) } else { SemanticOperandV1::Copy(place(1, word)) };
        let mut statements = vec![];
        match original_ty {
            PAIR | ARRAY => {
                statements.push(assign(2, pair, SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Tuple,
                        vec![value, constant(byte, 3, 1)]).unwrap())));
                if original_ty == ARRAY {
                    statements.push(assign(3, ty, SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Array,
                            vec![SemanticOperandV1::Copy(place(2, pair)); 4]).unwrap())));
                }
            }
            DIRECT => statements.push(assign(3, ty, SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::EnumVariant(u32::from(second)),
                    vec![value]).unwrap()))),
            WORD => statements.push(assign(3, word, SemanticRvalueKindV1::Use(value))),
            _ => panic!("boundary fixture type"),
        }
        statements
    };
    let argument_local = if original_ty == PAIR { 2 } else { 3 };
    let call = |target| SemanticTerminatorKindV1::Call(SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1), vec![SemanticOperandV1::Copy(place(argument_local, ty))],
        Some(SemanticCallDestinationV1::new(place(4, ty),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1::from_index(target)))),
        SemanticUnwindActionV1::Unreachable).unwrap());
    let function = |tag, role, abi, locals, blocks| SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]), role,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        span, abi, locals, SemanticBlockIdV1::from_index(0), blocks).unwrap();
    let root = function(201, SemanticFunctionRoleV1::KernelRoot, root.abi().clone(), vec![
        local(201, unit, SemanticLocalRoleV1::Return), local(202, word, SemanticLocalRoleV1::Argument(0)),
        local(203, pair, SemanticLocalRoleV1::Temporary), local(204, ty, SemanticLocalRoleV1::Temporary),
        local(205, ty, SemanticLocalRoleV1::Temporary),
    ], vec![
        block(201, make(false), call(1)), block(202, make(true), call(2)),
        block(203, vec![assign(0, unit, SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
            SemanticConstantV1::new(unit, SemanticConstantValueV1::ZeroSized))))], SemanticTerminatorKindV1::Return),
    ]).with_kernel_entry(root.kernel_entry().unwrap().clone());
    let mode = if original_ty == WORD {
        SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None, 0, None).unwrap())
    } else {
        let layout = closure.types[ty.index() as usize].layout();
        SemanticAbiPassModeV1::Indirect {
            attributes: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(true, Some(SemanticAbiPointerCaptureV1::CapturesNone),
                    true, false, false, true), SemanticAbiExtensionV1::None,
                layout.size_bytes().unwrap(), Some(layout.alignment_bytes())).unwrap(),
            metadata_attributes: None, on_stack: false,
        }
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([211; 32]), SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust, SemanticExternAbiV1::Rust, false, false, 1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(ty, mode.clone()))],
        SemanticAbiValueV1::new(ty, mode)).unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue]).unwrap();
    let helper = function(211, SemanticFunctionRoleV1::InternalHelper, abi, vec![
        local(211, ty, SemanticLocalRoleV1::Return), local(212, ty, SemanticLocalRoleV1::Argument(0)),
    ], vec![block(211, vec![assign(0, ty,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, ty))))], SemanticTerminatorKindV1::Return)]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(source.target(), closure.types,
        vec![], vec![], vec![], vec![root, helper],
        vec![SemanticCallableDeclV1::defined(ROOT), SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1))],
        vec![ROOT]).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(admitted,
        ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

fn with_boundary_schema_source_v29(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(&SourceReferencePlanV29<'_, '_>, &mut Budget<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    with_boundary_schema_source_options_v29(owner, true, false, consume)
}

fn with_boundary_schema_source_options_v29(
    mut owner: ProductionSemanticSsaOwnerV1,
    include_demands: bool,
    warm: bool,
    consume: impl FnOnce(&SourceReferencePlanV29<'_, '_>, &mut Budget<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(181).unwrap();
    let capture = owner.try_capture_occurrences_with_budget_v1(&mut budget).unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
    let roots = fixture.roots();
    let result = production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            budget.reserve_storage(size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>()).unwrap();
            let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(&owner,
                ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget).unwrap();
            let descriptor = profile.descriptor_root(&owner, 0, budget).unwrap();
            let demands = source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, budget).unwrap();
            let mut layouts = SourceStorageLayoutsV29::new_with_limits(&owner, demands.types(&owner, budget).unwrap(),
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(), budget).unwrap();
            let keys = layouts.keys.clone();
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            let prior_credit = if warm {
                with_source_storage_descriptor_demands_root_v29(&mut layouts, instances,
                    Some(descriptor), lens, budget, |plan, _, budget| {
                        assert!(source_reference_has_schema_inputs_v29(plan, budget)?);
                        assert!(plan.cells.rows.is_empty());
                        Ok(())
                    }).unwrap();
                Some((layouts.lease.persistent.get(), layouts.physical.borrow().rows.len()))
            } else { None };
            let result = if include_demands {
                with_source_storage_descriptor_demands_root_v29(&mut layouts, instances,
                    Some(descriptor), lens, budget, |plan, _, budget| consume(plan, budget).map_err(Into::into))
            } else {
                with_source_storage_descriptor_root_v29(&mut layouts, instances,
                    Some(descriptor), budget, |plan, _, budget| consume(plan, budget).map_err(Into::into))
            };
            if let Some(prior) = prior_credit {
                assert_eq!((layouts.lease.persistent.get(), layouts.physical.borrow().rows.len()), prior,
                    "repeated boundary roots must reuse schemas without refunding their persistent table");
            }
            assert_eq!(layouts.keys, keys, "schema selection must not change original geometry IDs");
            let released = layouts.release(budget);
            let discarded = demands.discard(budget);
            drop(profile);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result.and(released).and(discarded))
        }).unwrap();
    assert_eq!(budget.storage(), 181 + capture.retained_storage());
    drop(roots);
    drop(fixture);
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 181);
    result
}

#[test]
fn actual_boundary_only_objects_select_schemas_without_manufactured_local_backing() {
    for ty in [PAIR, ARRAY, DIRECT] {
        with_boundary_schema_source_v29(boundary_schema_owner_v29(ty), |plan, budget| {
            let ty = fixture_type_v29(plan.instances.owner(), ty);
            let (requests, _) = plan.storage_demands.unwrap().requests(plan.instances, budget)?;
            assert!(requests.is_empty(), "whole promoted values need no original local backing");
            assert!(!plan.has_storage_demands);
            assert!(plan.cells.rows.is_empty());
            assert!(plan.storage_activations.is_empty());
            assert!(plan.representation_demands.is_empty());
            let layouts = plan.storage_root.as_ref().unwrap().source_layouts(plan.instances, budget)?;
            assert!(layouts.keys.is_empty());
            let mut callers = Vec::new();
            let mut returns = Vec::new();
            for ordinal in 0..plan.boundary_values.len() {
                let boundary = plan.boundary_value(ordinal, budget)?;
                if plan.nodes[boundary.node].ty != ty { continue; }
                let schema = source_reference_boundary_schema_v29(plan, boundary.site, boundary.role, budget)?.unwrap();
                layouts.check_selected_schema(plan.instances.owner(), ty, schema, budget)?;
                match boundary.role {
                    SourceReferenceBoundaryRoleV29::Argument(0) => callers.push((boundary, schema)),
                    SourceReferenceBoundaryRoleV29::Return => returns.push((boundary, schema)),
                    _ => panic!("unexpected fixture boundary"),
                }
            }
            assert_eq!(callers.len(), 2);
            assert_eq!(returns.len(), 2);
            assert_ne!(callers[0].0.site.block, callers[1].0.site.block);
            assert_ne!(callers[0].0.node, callers[1].0.node, "later source value must not replace caller evaluation");
            assert_eq!(callers[0].1, callers[1].1, "schema equality must not erase evaluation identity");
            assert_ne!(returns[0].0.site.instance, returns[1].0.site.instance);
            for (boundary, schema) in &returns {
                let entry = plan.entries[boundary.site.instance.index()].unwrap();
                let entry = plan.states[entry][1].node.unwrap();
                assert_eq!(source_reference_selected_value_schema_v29(plan, entry, ty, budget)?, Some(*schema));
                assert_eq!(source_reference_selected_value_schema_v29(plan,
                    plan.returns[boundary.site.instance.index()].unwrap(), ty, budget)?, Some(*schema));
            }
            assert!(source_reference_check_selected_backing_v29(plan, budget)?);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn scalar_only_boundaries_keep_ordinary_transport_and_the_empty_schema_fast_path() {
    with_boundary_schema_source_v29(boundary_schema_owner_v29(WORD), |plan, budget| {
        assert!(plan.storage_demands.unwrap().requests(plan.instances, budget)?.0.is_empty());
        assert!(plan.cells.rows.is_empty());
        assert!(plan.selected_storage.is_empty());
        assert!(!source_reference_has_schema_inputs_v29(plan, budget)?);
        assert!(!plan.boundary_values.is_empty());
        for ordinal in 0..plan.boundary_values.len() {
            let boundary = plan.boundary_value(ordinal, budget)?;
            assert_eq!(source_reference_boundary_schema_v29(plan, boundary.site, boundary.role, budget)?, None);
        }
        Ok(())
    }).unwrap();
}

#[test]
fn boundary_schema_queries_refuse_changed_source_coordinates_and_type_after_real_admission() {
    for mutation in 0..5 {
        let reached = std::cell::Cell::new(false);
        let result = with_boundary_schema_source_v29(boundary_schema_owner_v29(PAIR), |plan, budget| {
            let pair = fixture_type_v29(plan.instances.owner(), PAIR);
            let word = fixture_type_v29(plan.instances.owner(), WORD);
            let row = (0..plan.boundary_values.len()).map(|ordinal| plan.boundary_value(ordinal, budget).unwrap())
                .find(|row| row.role == SourceReferenceBoundaryRoleV29::Argument(0)).unwrap();
            assert!(source_reference_boundary_schema_v29(plan, row.site, row.role, budget)?.is_some());
            reached.set(true);
            let denied = match mutation {
                0 => source_reference_selected_value_schema_v29(plan, row.node, word, budget),
                1 => source_reference_selected_value_schema_v29(plan, usize::MAX, pair, budget),
                2 => source_reference_boundary_schema_v29(plan, row.site, SourceReferenceBoundaryRoleV29::Argument(u32::MAX), budget),
                3 => source_reference_boundary_schema_v29(plan,
                    SourceReferenceSiteV29 { block: SemanticBlockIdV1::from_index(u32::MAX), ..row.site }, row.role, budget),
                4 => {
                    let mut work = work();
                    let mut foreign = Budget::new(&mut work, 64 * 1024 * 1024);
                    foreign.reserve_storage(budget.storage()).unwrap();
                    source_reference_boundary_schema_v29(plan, row.site, row.role, &mut foreign)
                }
                _ => unreachable!(),
            };
            assert!(denied.is_err());
            denied.map(|_| ())
        });
        assert!(reached.get());
        assert!(result.is_err());
    }
}

#[test]
fn boundary_schema_cannot_adopt_the_legacy_fixture_route_without_original_demands() {
    let reached = std::cell::Cell::new(false);
    let result = with_boundary_schema_source_options_v29(boundary_schema_owner_v29(PAIR), false, false, |plan, budget| {
        assert!(plan.storage_demands.is_none());
        assert!(plan.selected_storage.is_empty());
        let row = (0..plan.boundary_values.len()).map(|index| plan.boundary_value(index, budget).unwrap())
            .find(|row| row.role == SourceReferenceBoundaryRoleV29::Argument(0)).unwrap();
        reached.set(true);
        source_reference_boundary_schema_v29(plan, row.site, row.role, budget).map(|_| ())
    });
    assert!(reached.get());
    assert!(result.is_err());
}

#[test]
fn repeated_boundary_roots_reuse_the_same_selected_schema_credit() {
    with_boundary_schema_source_options_v29(boundary_schema_owner_v29(ARRAY), true, true, |plan, budget| {
        assert!(source_reference_has_schema_inputs_v29(plan, budget)?);
        assert!(plan.cells.rows.is_empty());
        assert!(source_reference_check_selected_backing_v29(plan, budget)?);
        Ok(())
    }).unwrap();
}

#[test]
fn boundary_leaf_selection_uses_original_validation_and_keeps_geometry_ids_immutable() {
    for ty in [UNIT, WORD, BYTE, SIGNED] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(191).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let selected = layouts.select_original_leaf_schema(&owner, ty, &mut budget).unwrap();
        let expected = layouts.lower_row(RowKey::ty(ty), &mut budget).unwrap();
        assert_eq!(layouts.rows(&owner, &mut budget).unwrap()[selected.0 as usize], expected);
        assert_eq!(layouts.original_schema(&owner, ty, &mut budget).unwrap(), None);
        assert!(layouts.keys.is_empty());
        let retained = layouts.lease.persistent.get();
        assert!(retained > 0);
        assert_eq!(layouts.select_original_leaf_schema(&owner, ty, &mut budget).unwrap(), selected);
        assert_eq!(layouts.lease.persistent.get(), retained);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 191);
    }
    for ty in [REFERENCE, RAW, PAIR, ARRAY, SLICE, DESCRIPTOR, DIRECT, LINK] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(193).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        assert!(layouts.select_original_leaf_schema(&owner, ty, &mut budget).is_err());
        assert!(layouts.physical.borrow().rows.is_empty());
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 193);
    }
}

#[test]
fn selected_only_slices_publish_exact_original_length_leaves_without_a_fake_demand() {
    for original_word in [false, true] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(223).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, if original_word { &[WORD] } else { &[] }, &mut budget).unwrap();
        let keys = layouts.keys.clone();
        let element = layouts.select_original_leaf_schema(&owner, WORD, &mut budget).unwrap();
        let mut original_length = None;
        for space in [AddressSpace::Generic, AddressSpace::Global, AddressSpace::Private] {
            let schema = layouts.select_schema(&owner, DESCRIPTOR,
                SourceStorageSelectionV29::Slice { element, value_space: space, access: AccessMode::ReadOnly }, &mut budget).unwrap();
            let rows = layouts.rows(&owner, &mut budget).unwrap();
            let StorageLayoutKindV1::Slice { data, length, value_space, element: actual, access } = rows[schema.0 as usize].kind
            else { panic!("selected source slice"); };
            assert_eq!((actual, value_space, access), (element, space, AccessMode::ReadOnly));
            assert_eq!((data.offset, length.offset), (0, 8));
            assert!(matches!(rows[data.layout.0 as usize].kind, StorageLayoutKindV1::Pointer(pointer)
                if pointer.pointee == element && pointer.value_space == space
                    && pointer.encoded_space == AddressSpace::Generic && pointer.stored_bits == 64));
            assert_eq!(rows[length.layout.0 as usize], StorageLayoutV1 {
                size: 8, alignment: 8, kind: StorageLayoutKindV1::Scalar(ScalarType::Index),
            });
            drop(rows);
            assert_eq!(layouts.schema_key(length.layout, &mut budget).unwrap(),
                RowKey { ty: DESCRIPTOR, role: RowRole::DescriptorLength });
            if let Some(previous) = original_length { assert_eq!(length.layout, previous); }
            original_length = Some(length.layout);
            let credit = layouts.lease.persistent.get();
            assert_eq!(layouts.select_schema(&owner, DESCRIPTOR,
                SourceStorageSelectionV29::Slice { element, value_space: space, access: AccessMode::ReadOnly }, &mut budget).unwrap(), schema);
            assert_eq!(layouts.lease.persistent.get(), credit);
        }
        assert_eq!(layouts.keys, keys);
        assert_eq!(layouts.original_schema(&owner, DESCRIPTOR, &mut budget).unwrap(), None);
        assert!(layouts.row_id(RowKey { ty: DESCRIPTOR, role: RowRole::DescriptorLength }, &mut budget).is_err());
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 223);
    }
}

#[test]
fn actual_stored_loan_cannot_be_reinterpreted_with_an_empty_original_write_census() {
    let reached = std::cell::Cell::new(false);
    let result = with_boundary_schema_source_v29(backing_owner_v29(false), |plan, budget| {
        let requests = plan.storage_demands.unwrap().requests(plan.instances, budget)?.0;
        assert!(!requests.is_empty());
        assert!(!plan.representation_demands.is_empty());
        assert!(plan.cells.rows.iter().any(|row| matches!(row.kind, SourceBackingKindV29::Object(_))));
        assert!(plan.nodes.iter().any(|row| matches!(row.kind, SourceReferenceNodeKindV29::Loan(_))));
        source_reference_check_boundary_write_census_v29(plan, requests, budget)?;
        reached.set(true);
        // The original source/profile/plan and all actual cells were built
        // successfully. Only the checker input is now maliciously omitted.
        source_reference_check_boundary_write_census_v29(plan, &[], budget)
    });
    assert!(reached.get());
    assert!(result.is_err());
}

#[test]
fn original_by_value_descriptor_entry_gets_a_schema_without_inventing_a_global_region_or_cell() {
    with_boundary_schema_source_v29(transient_descriptor_owner_v29(), |plan, budget| {
        let pair = fixture_type_v29(plan.instances.owner(), TWO_DESCRIPTORS);
        let descriptor = fixture_type_v29(plan.instances.owner(), DESCRIPTOR);
        let instance = plan.instances.root();
        let entry = plan.entries[instance.index()].unwrap();
        let node = plan.states[entry][2].node.unwrap();
        assert!(!plan.cells.rows.iter().any(|row| row.instance == instance && row.local.index() == 2));
        let schema = source_reference_selected_value_schema_v29(plan, node, pair, budget)?.unwrap();
        let layouts = plan.storage_root.as_ref().unwrap().source_layouts(plan.instances, budget)?;
        let rows = layouts.rows(plan.instances.owner(), budget)?;
        let StorageLayoutKindV1::Record(fields) = &rows[schema.0 as usize].kind
        else { panic!("actual by-value descriptor record"); };
        assert_eq!(fields.len(), 2);
        for field in fields {
            let StorageLayoutKindV1::Slice { value_space, data, .. } = rows[field.layout.0 as usize].kind
            else { panic!("actual descriptor field"); };
            assert_eq!(value_space, AddressSpace::Generic,
                "original by-value ABI bytes do not imply a Global pointee");
            assert!(matches!(rows[data.layout.0 as usize].kind,
                StorageLayoutKindV1::Pointer(pointer) if pointer.value_space == AddressSpace::Generic
                    && pointer.encoded_space == AddressSpace::Generic));
        }
        drop(rows);
        let direct = plan.states[entry][1].node.unwrap();
        assert_eq!(source_reference_selected_value_schema_v29(plan, direct, descriptor, budget)?, None,
            "direct slice keeps its existing descriptor transport");
        Ok(())
    }).unwrap();
}

#[test]
fn boundary_schema_root_error_and_panic_cleanup_preserve_the_original_caller_floor() {
    for panic in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
            with_boundary_schema_source_v29(boundary_schema_owner_v29(PAIR), |plan, budget| {
                let row = (0..plan.boundary_values.len()).map(|index| plan.boundary_value(index, budget).unwrap())
                    .find(|row| row.role == SourceReferenceBoundaryRoleV29::Argument(0)).unwrap();
                assert!(source_reference_boundary_schema_v29(plan, row.site, row.role, budget)?.is_some());
                reached.set(true);
                if panic { panic!("boundary root callback panic"); }
                Err(source_reference_error_v29("boundary root callback error"))
            })));
        assert!(reached.get());
        assert!(result.is_ok(), "production root must consume the callback panic after cleanup");
        assert!(result.unwrap().is_err());
    }
}

pub(super) struct SelectedNicheRowsV29 {
    pub variants: [(u32, StorageLayoutIdV1); 2],
    pub pointer: StorageLayoutIdV1,
    pub mixed: StorageLayoutIdV1,
    pub nested: Option<(StorageLayoutIdV1, StorageLayoutIdV1)>,
}

pub(super) fn selected_niche_rows_v29(
    fixture: &SelectedNicheFixtureV29,
    layouts: &SourceStorageLayoutsV29<'_>,
    space: AddressSpace,
    budget: &mut Budget<'_>,
) -> SelectedNicheRowsV29 {
    let owner = &fixture.owner;
    let word = layouts.original_schema(owner, WORD, budget).unwrap().unwrap();
    let mixed = layouts.select_schema(owner, fixture.mixed,
        SourceStorageSelectionV29::Aggregate { variant: None, fields: &[(1, word)] }, budget).unwrap();
    let pointer = layouts.select_schema(owner, fixture.pointer, if fixture.slice {
        SourceStorageSelectionV29::Slice { element: mixed, value_space: space, access: AccessMode::ReadOnly }
    } else {
        SourceStorageSelectionV29::Pointer { pointee: mixed, value_space: space, access: AccessMode::ReadOnly }
    }, budget).unwrap();
    let (child, nested) = if let Some((tuple, array)) = fixture.nested {
        let tuple = layouts.select_schema(owner, tuple,
            SourceStorageSelectionV29::Aggregate { variant: None, fields: &[(0, pointer)] }, budget).unwrap();
        let array = layouts.select_schema(owner, array, SourceStorageSelectionV29::Array { element: tuple }, budget).unwrap();
        (array, Some((tuple, array)))
    } else { (pointer, None) };
    let empty = layouts.select_schema(owner, fixture.enumeration,
        SourceStorageSelectionV29::Aggregate { variant: Some(0), fields: &[] }, budget).unwrap();
    let payload = layouts.select_schema(owner, fixture.enumeration,
        SourceStorageSelectionV29::Aggregate { variant: Some(1), fields: &[(1, child)] }, budget).unwrap();
    SelectedNicheRowsV29 { variants: [(0, empty), (1, payload)], pointer, mixed, nested }
}

#[test]
fn selected_pointer_niche_uses_actual_selected_only_payload_representation_and_original_encoding() {
    for slice in [false, true] {
        for nested in [false, true] {
            for space in [AddressSpace::Generic, AddressSpace::Global, AddressSpace::Private] {
                let fixture = selected_niche_fixture_v29(slice, nested);
                let owner = &fixture.owner;
                let mut work = work();
                let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
                budget.reserve_storage(149).unwrap();
                let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
                let selected = selected_niche_rows_v29(&fixture, &layouts, space, &mut budget);
                assert_eq!(layouts.original_schema(owner, fixture.mixed, &mut budget).unwrap(), None);
                assert_eq!(layouts.original_schema(owner, fixture.enumeration, &mut budget).unwrap(), None);
                let original_key = RowKey { ty: fixture.enumeration, role: RowRole::Tag };
                assert!(layouts.enum_auxiliary(original_key, &mut budget).is_err(),
                    "original helper must not invent the missing original pointee row");
                let before = layouts.lease.persistent.get();
                let id = layouts.select_schema(owner, fixture.enumeration,
                    SourceStorageSelectionV29::Enum { variants: &selected.variants }, &mut budget).unwrap();
                assert!(layouts.lease.persistent.get() > before);
                let stable = layouts.lease.persistent.get();
                assert_eq!(layouts.select_schema(owner, fixture.enumeration,
                    SourceStorageSelectionV29::Enum { variants: &selected.variants }, &mut budget).unwrap(), id);
                assert_eq!(layouts.lease.persistent.get(), stable);
                let rows = layouts.rows(owner, &mut budget).unwrap();
                let StorageLayoutKindV1::Variants { encoding, variants } = &rows[id.0 as usize].kind
                else { panic!("selected variant table"); };
                assert_eq!(variants[0].layout, selected.variants[0].1);
                assert_eq!(variants[1].layout, selected.variants[1].1);
                assert_eq!(variants.iter().map(|v| v.discriminant).collect::<Vec<_>>(), vec![0, 1]);
                let tag = encoding.tag();
                assert_eq!(tag.offset, if nested { if slice { 16 } else { 8 } } else { 0 });
                assert!(matches!(encoding, StorageVariantEncodingV1::Niche {
                    untagged_variant: 1, first_niche_variant: 0, last_niche_variant: 0, niche_start: 0, .. }));
                let pointer = if slice {
                    let StorageLayoutKindV1::Slice { data, .. } = rows[selected.pointer.0 as usize].kind
                    else { panic!("actual descriptor"); };
                    data.layout
                } else { selected.pointer };
                assert_eq!(rows[tag.layout.0 as usize], rows[pointer.0 as usize]);
                let StorageLayoutKindV1::Pointer(actual) = rows[tag.layout.0 as usize].kind
                else { panic!("actual pointer tag"); };
                assert_eq!((actual.pointee, actual.value_space, actual.encoded_space, actual.access, actual.stored_bits),
                    (selected.mixed, space, AddressSpace::Generic, AccessMode::ReadOnly, 64));
                drop(rows);
                assert_eq!(layouts.schema_key(tag.layout, &mut budget).unwrap(), original_key);
                layouts.release(&mut budget).unwrap();
                assert_eq!(budget.storage(), 149);
            }
        }
    }
}

#[test]
fn selected_niche_rejects_changed_roster_path_geometry_pointer_contract_and_cycles() {
    for mutation in 0..11 {
        let fixture = selected_niche_fixture_v29(true, true);
        let owner = &fixture.owner;
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(151).unwrap();
        let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
        let mut selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
        let positive = layouts.selected_niche_pointer_row(fixture.enumeration, &selected.variants, &mut budget).unwrap();
        assert!(matches!(positive.kind, StorageLayoutKindV1::Pointer(_)));
        let count = layouts.physical.borrow().rows.len();
        let credit = layouts.lease.persistent.get();
        {
            let mut physical = layouts.physical.borrow_mut();
            let StorageLayoutKindV1::Slice { data, .. } = physical.rows[selected.pointer.0 as usize].kind
            else { panic!("actual source slice"); };
            match mutation {
                0 => selected.variants.swap(0, 1),
                1 => selected.variants[1].1 = selected.variants[0].1,
                2 => {
                    let StorageLayoutKindV1::Record(fields) = &mut physical.rows[selected.variants[1].1.0 as usize].kind
                    else { panic!("payload"); };
                    fields[0].offset = 8;
                }
                3 => {
                    let StorageLayoutKindV1::Pointer(pointer) = &mut physical.rows[data.layout.0 as usize].kind
                    else { panic!("pointer"); };
                    pointer.access = AccessMode::ReadWrite;
                }
                4 => {
                    let StorageLayoutKindV1::Pointer(pointer) = &mut physical.rows[data.layout.0 as usize].kind
                    else { panic!("pointer"); };
                    pointer.encoded_space = AddressSpace::Private;
                }
                5 => {
                    let StorageLayoutKindV1::Slice { data, .. } = &mut physical.rows[selected.pointer.0 as usize].kind
                    else { panic!("slice"); };
                    data.offset = 8;
                }
                6 => {
                    let (_, array) = selected.nested.unwrap();
                    let StorageLayoutKindV1::Array { stride, .. } = &mut physical.rows[array.0 as usize].kind
                    else { panic!("array"); };
                    *stride += 8;
                }
                7 => {
                    let (tuple, array) = selected.nested.unwrap();
                    physical.depths[tuple.0 as usize] = physical.depths[array.0 as usize];
                }
                8 => {
                    let (_, array) = selected.nested.unwrap();
                    let StorageLayoutKindV1::Array { element, .. } = &mut physical.rows[array.0 as usize].kind
                    else { panic!("array"); };
                    *element = array;
                }
                9 => {
                    let StorageLayoutKindV1::Pointer(pointer) = &mut physical.rows[data.layout.0 as usize].kind
                    else { panic!("pointer"); };
                    pointer.pointee = StorageLayoutIdV1(0);
                }
                10 => {
                    let StorageLayoutKindV1::Pointer(pointer) = &mut physical.rows[data.layout.0 as usize].kind
                    else { panic!("pointer"); };
                    pointer.stored_bits = 32;
                }
                _ => unreachable!(),
            }
        }
        assert!(layouts.select_schema(owner, fixture.enumeration,
            SourceStorageSelectionV29::Enum { variants: &selected.variants }, &mut budget).is_err(), "mutation {mutation}");
        assert_eq!(layouts.physical.borrow().rows.len(), count);
        assert_eq!(layouts.lease.persistent.get(), credit);
        assert!(layouts.release(&mut budget).is_err());
        assert_eq!(budget.storage(), 151);
    }
}

#[test]
fn selected_niche_foreign_owner_and_active_physical_builder_refuse_without_publication() {
    for foreign in [false, true] {
        let fixture = selected_niche_fixture_v29(false, false);
        let other = selected_niche_fixture_v29(false, false);
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(157).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&fixture.owner, &[WORD], &mut budget).unwrap();
        let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Private, &mut budget);
        let count = layouts.physical.borrow().rows.len();
        if foreign {
            assert!(layouts.select_schema(&other.owner, fixture.enumeration,
                SourceStorageSelectionV29::Enum { variants: &selected.variants }, &mut budget).is_err());
        } else {
            let held = layouts.physical.borrow_mut();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                layouts.selected_niche_pointer_row(fixture.enumeration, &selected.variants, &mut budget)));
            assert!(result.is_ok());
            assert!(result.unwrap().is_err());
            drop(held);
        }
        assert_eq!(layouts.physical.borrow().rows.len(), count);
        let _ = layouts.release(&mut budget);
        assert_eq!(budget.storage(), 157);
    }
}

fn recursive_backing_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    backing_owner_v29(true)
}

fn backing_owner_v29(recursive: bool) -> ProductionSemanticSsaOwnerV1 {
    backing_owner_with_observed_store_v29(recursive, true)
}

fn backing_owner_with_observed_store_v29(recursive: bool, memory_backed: bool) -> ProductionSemanticSsaOwnerV1 {
    let template = owner();
    let source = template.source_semantic();
    let original = &source.functions()[0];
    let span = original.blocks()[0].source();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |destination: SemanticPlaceV1, value| {
        SemanticStatementV1::new(
            span,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), value),
            )),
        )
    };
    let mut types = source.types().to_vec();
    if !recursive {
        types[RECURSIVE.index() as usize] = declaration(
            13,
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![REFERENCE]).unwrap()),
        );
    }
    let closure = FixtureTypeClosureV29::new(&types,
        &[UNIT, WORD, RECURSIVE, if recursive { LINK } else { REFERENCE }]);
    assert_eq!((closure.id(UNIT), closure.id(WORD)), (UNIT, WORD), "cloned scalar root ABI IDs");
    let record = closure.id(RECURSIVE);
    let pointer = closure.id(if recursive { LINK } else { REFERENCE });
    let mut locals = original.locals()[..2].to_vec();
    let temporary_types = if recursive {
        [record, pointer]
    } else {
        [pointer, record]
    };
    for (tag, ty) in [201, 202].into_iter().zip(temporary_types) {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            span,
        ));
    }
    let mut statements = if recursive {
        vec![
            assign(
                place(3, pointer),
                SemanticRvalueKindV1::AddressOf {
                    place: place(2, record),
                    mutability: SemanticMutabilityV1::Immutable,
                },
            ),
            assign(
                place(2, record),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![SemanticOperandV1::Copy(place(3, pointer))],
                    )
                    .unwrap(),
                ),
            ),
        ]
    } else {
        let borrow = SemanticOperandV1::Copy(place(2, pointer));
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), pointer).unwrap()],
            pointer,
        ).unwrap();
        // An ordinary field assignment is now a valid SSA update. The backing
        // tests require an actual memory write; retain both source forms.
        let update = if memory_backed {
            SemanticStatementV1::new(span, SemanticStatementKindV1::Store(
                SemanticMemoryStoreV1::new(field, borrow.clone(), SemanticVolatilityV1::NonVolatile, None)))
        } else {
            assign(field, SemanticRvalueKindV1::Use(borrow.clone()))
        };
        vec![
            assign(
                place(2, pointer),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(1, WORD),
                },
            ),
            assign(
                place(3, record),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![borrow.clone()],
                    )
                    .unwrap(),
                ),
            ),
            update,
        ]
    };
    statements.push(assign(
        place(0, UNIT),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))),
    ));
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([203; 32]),
        span,
        statements,
        SemanticTerminatorV1::new(span, SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([204; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([205; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([206; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([207; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([208; 32]),
        span,
        original.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        closure.types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(ROOT)],
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
    .unwrap()
}

fn with_actual_backing_v29<R>(
    mut owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(&SourceReferencePlanV29<'_, '_>, &mut Budget<'_>) -> R,
) -> R {
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(89).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
    let roots = fixture.roots();
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            budget
                .reserve_storage(std::mem::size_of::<
                    kernel_argument_abi_v18::CapturedKernelArgumentAbiV18,
                >())
                .unwrap();
            let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
                &owner,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .unwrap();
            let descriptor = profile.descriptor_root(&owner, 0, budget).unwrap();
            let demands =
                source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, budget)
                    .unwrap();
            let mut layouts = SourceStorageLayoutsV29::new_with_limits(
                &owner,
                demands.types(&owner, budget).unwrap(),
                fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                    rows: 1024,
                    edges: 4096,
                    containment_depth: 256,
                    object_bytes: 1024,
                },
                budget,
            )
            .unwrap();
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            let result = with_source_storage_descriptor_demands_root_v29(
                &mut layouts,
                instances,
                Some(descriptor),
                lens,
                budget,
                |plan, _, budget| Ok(consume(plan, budget)),
            )
            .unwrap();
            layouts.release(budget).unwrap();
            demands.discard(budget).unwrap();
            drop(profile);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 89 + capture.retained_storage());
    drop(roots);
    drop(fixture);
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 89);
    result
}

#[test]
fn actual_recursive_object_writes_reuse_closed_original_rows() {
    with_actual_backing_v29(recursive_backing_owner_v29(), |plan, budget| {
        let owner = plan.instances.owner();
        let recursive = fixture_type_v29(owner, RECURSIVE);
        let layouts = plan
            .storage_root
            .as_ref()
            .unwrap()
            .source_layouts(plan.instances, budget)
            .unwrap();
        assert_eq!(plan.raw_origins.len(), 1);
        assert!(
            plan.representation_demands
                .iter()
                .any(|row| row.local.index() == 2)
        );
        let cell = plan
            .cells
            .rows
            .iter()
            .find(|cell| cell.local.index() == 2)
            .unwrap();
        let SourceBackingKindV29::Object(schema) = cell.kind else {
            panic!("recursive object was scalarized")
        };
        assert_eq!(
            schema,
            layouts
                .original_schema(owner, recursive, budget)
                .unwrap()
                .unwrap()
        );
        let rows = layouts.rows(owner, budget).unwrap();
        let StorageLayoutKindV1::Record(fields) = &rows[schema.0 as usize].kind else {
            panic!("record")
        };
        let StorageLayoutKindV1::Pointer(pointer) = rows[fields[0].layout.0 as usize].kind else {
            panic!("link")
        };
        assert_eq!(pointer.pointee, schema);
        assert_eq!(
            (
                pointer.value_space,
                pointer.encoded_space,
                pointer.stored_bits
            ),
            (AddressSpace::Generic, AddressSpace::Generic, 64)
        );
    });
}

#[test]
fn storing_a_stable_scalar_loan_selects_an_actual_object_pointee_family() {
    with_actual_backing_v29(backing_owner_v29(false), |plan, budget| {
        let word = fixture_type_v29(plan.instances.owner(), WORD);
        let (loan, source) = plan
            .loans
            .iter()
            .enumerate()
            .find(|(_, loan)| plan.origins[loan.origin].local.index() == 1)
            .unwrap();
        assert_eq!(
            source.representation,
            SourceReferenceRepresentationV29::StableReferent
        );
        assert!(matches!(
            plan.cells.strategies[loan],
            SourceReferenceCellStrategyV29::Object(_)
        ));
        let (_, referent) = plan.backing_cell(loan, budget).unwrap().unwrap();
        let SourceBackingKindV29::Object(pointee) = referent.kind else {
            panic!("stored reference has no real object")
        };
        assert_eq!(
            (referent.instance, referent.local.index(), referent.ty),
            (plan.instances.root(), 1, word)
        );
        assert!(plan.scalar_cell(loan, budget).is_err());
        let physical = source_reference_object_pointer_type_v29(plan, loan, budget).unwrap();
        assert_eq!(
            physical,
            Type::pointer(
                Type::StorageObject(pointee),
                AddressSpace::Private,
                AccessMode::ReadOnly
            )
        );
        let holder = plan
            .cells
            .rows
            .iter()
            .find(|row| row.local.index() == 3)
            .unwrap();
        let SourceBackingKindV29::Object(holder) = holder.kind else {
            panic!("stored-reference holder")
        };
        let layouts = plan
            .storage_root
            .as_ref()
            .unwrap()
            .source_layouts(plan.instances, budget)
            .unwrap();
        let rows = layouts.rows(plan.instances.owner(), budget).unwrap();
        let StorageLayoutKindV1::Record(fields) = &rows[holder.0 as usize].kind else {
            panic!("holder record")
        };
        let StorageLayoutKindV1::Pointer(pointer) = rows[fields[0].layout.0 as usize].kind else {
            panic!("holder pointer field")
        };
        assert_eq!(pointer.pointee, pointee);
        assert_eq!(
            (
                pointer.value_space,
                pointer.encoded_space,
                pointer.stored_bits
            ),
            (AddressSpace::Private, AddressSpace::Generic, 64)
        );
    });
}

// Solver stress inputs are inert equations, not source or physical authority.
// Their type/field identities come from the admitted and selected fixture above.
fn recursive_equations_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    count: usize,
    budget: &mut Budget<'_>,
) -> SourceBackingEquationsV29 {
    assert!(count >= 2 && count % 2 == 0);
    let recursive = fixture_type_v29(plan.instances.owner(), RECURSIVE);
    let link = fixture_type_v29(plan.instances.owner(), LINK);
    assert!(
        plan.cells.rows.iter().any(
            |cell| cell.ty == recursive && matches!(cell.kind, SourceBackingKindV29::Object(_))
        )
    );
    let mut equations = SourceBackingEquationsV29::new(plan, budget).unwrap();
    for index in 0..count {
        equations
            .insert(if index % 2 == 0 { recursive } else { link }, None, budget)
            .unwrap();
    }
    for index in 0..count {
        if index % 2 == 0 {
            reserve_execution_cfg_map_entry_v29::<u32, usize>(0, budget).unwrap();
            equations.rows[index].children.insert(0, index + 1);
        } else {
            equations.rows[index].pointee = Some((index + 1) % count);
            equations.rows[index].space = Some(AddressSpace::Private);
        }
    }
    equations
}

#[test]
fn recursive_equation_traversal_has_independent_exact_and_one_short_work() {
    with_actual_backing_v29(recursive_backing_owner_v29(), |plan, _| {
        // Header vectors: 6 + nodes + loans; first two row pushes: 7;
        // one paid BTree entry: 32. Traversal: 9 vector work + 4 init,
        // 12 containment-edge + 11 pointee-edge + 2 final visits = 38.
        let construction = 45 + plan.nodes.len() + plan.loans.len();
        let exact = construction + 38;
        for short in [false, true] {
            let mut work =
                fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(exact - usize::from(short));
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(97).unwrap();
            assert!(matches!(
                budget.charge_work(usize::MAX),
                Err(ArgumentResourceV1::Work(_))
            ));
            let mut equations = recursive_equations_v29(plan, 2, &mut budget);
            assert_eq!(budget.work(), construction);
            let result = equations.retain_recursive_original(&mut budget);
            if short {
                assert!(matches!(
                    result,
                    Err(Error::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    ))
                ));
                assert_eq!(budget.work(), exact - 1);
            } else {
                result.unwrap();
                assert_eq!(budget.work(), exact);
                assert!(equations.rows.iter().all(|row| row.original));
            }
            drop(equations);
            budget.release_storage(budget.storage() - 97).unwrap();
            assert_eq!(budget.storage(), 97);
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    });
}

#[test]
fn recursive_equations_are_iterative_and_cannot_erase_a_changed_pointee_contract() {
    with_actual_backing_v29(recursive_backing_owner_v29(), |plan, _| {
        let word = fixture_type_v29(plan.instances.owner(), WORD);
        let link = fixture_type_v29(plan.instances.owner(), LINK);
        for changed in [false, true] {
            let mut work = work();
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(101).unwrap();
            let count = if changed { 2 } else { 8192 };
            let mut equations = recursive_equations_v29(plan, count, &mut budget);
            let before = budget.work();
            equations.retain_recursive_original(&mut budget).unwrap();
            // Each pair contributes 4 init + 12/11 edge + 2 final work.
            assert_eq!(budget.work() - before, 9 + (count / 2) * 29);
            if changed {
                equations.rows[0].ty = word;
            }
            let result = equations.propagate_original(
                plan.instances.owner().source_semantic().types(),
                &mut budget,
            );
            if changed {
                assert!(matches!(
                    result,
                    Err(Error::Unsupported {
                        detail: "selected backing differs from its original source object or representation",
                        ..
                    })
                ));
            } else {
                result.unwrap();
                for row in equations.rows.iter().filter(|row| row.ty == link) {
                    assert_eq!(row.space, Some(AddressSpace::Generic));
                }
            }
            drop(equations);
            budget.release_storage(budget.storage() - 101).unwrap();
            assert_eq!(budget.storage(), 101);
        }
    });
}

#[test]
fn recursive_equations_refuse_incompatible_concrete_pointer_representation() {
    let mut declarations = types();
    declarations[RECURSIVE.index() as usize] = declaration(13,
        SemanticTypeLayoutV1::aggregate(Some(4), 4,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap()).unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![LINK]).unwrap()));
    declarations[LINK.index() as usize] = declaration(14,
        SemanticTypeLayoutV1::new_with_backend_repr(Some(4), 4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(3, 4, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()))), false).unwrap(),
        SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1::new_with_kind(RECURSIVE,
            SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable, 3, 32,
            SemanticPointerMetadataV1::None).unwrap()));
    let admitted = request(declarations).admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default()).unwrap();
    with_actual_backing_v29(owner, |plan, _| {
        // Only the constraint solver is exercised here. This inert graph is
        // never admitted as an actual source write, pointer or allocation.
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(127).unwrap();
        let mut equations = SourceBackingEquationsV29::new(plan, &mut budget).unwrap();
        let object = equations.insert(RECURSIVE, None, &mut budget).unwrap();
        let pointer = equations.insert(LINK, None, &mut budget).unwrap();
        assert_eq!((object, pointer), (0, 1));
        reserve_execution_cfg_map_entry_v29::<u32, usize>(0, &mut budget).unwrap();
        equations.rows[object].children.insert(0, pointer);
        equations.rows[pointer].pointee = Some(object);
        equations.rows[pointer].space = Some(AddressSpace::Private);
        assert_eq!(equations.rows[1].space, Some(AddressSpace::Private));
        equations.retain_recursive_original(&mut budget).unwrap();
        assert!(matches!(equations.propagate_original(
            plan.instances.owner().source_semantic().types(), &mut budget),
            Err(Error::Unsupported { detail:
                "recursive backing cannot discard an incompatible original pointer representation", .. })));
        drop(equations);
        budget.release_storage(budget.storage() - 127).unwrap();
        assert_eq!(budget.storage(), 127);
    });
}

#[test]
fn selected_instance_backing_lookup_keeps_original_local_and_generation_coordinates() {
    with_actual_backing_v29(recursive_backing_owner_v29(), |plan, budget| {
        let cell = *plan
            .cells
            .rows
            .iter()
            .find(|cell| cell.local.index() == 2)
            .unwrap();
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            let (index, actual) = layouts
                .backing_cell(cell.instance, cell.local, cell.generation, budget)?
                .unwrap();
            assert_eq!(actual.instance, cell.instance);
            assert_eq!(actual.local, cell.local);
            assert_eq!(actual.generation, cell.generation);
            assert_eq!(actual.kind, cell.kind);
            assert_eq!(actual.ty, cell.ty);
            assert_eq!(plan.cells.rows[index].local, cell.local);
            assert!(
                layouts
                    .backing_cell(cell.instance, cell.local, cell.generation + 1, budget)?
                    .is_none()
            );
            assert!(
                layouts
                    .backing_cell(
                        ProductionCallInstanceIdV1(usize::MAX),
                        cell.local,
                        cell.generation,
                        budget
                    )
                    .is_err()
            );
            assert!(
                layouts
                    .backing_cell(
                        cell.instance,
                        SemanticLocalIdV1::from_index(u32::MAX),
                        cell.generation,
                        budget
                    )
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    });
}

fn transient_descriptor_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    transient_descriptor_with_observed_stores_v29(true)
}

fn transient_descriptor_with_observed_stores_v29(memory_backed: bool) -> ProductionSemanticSsaOwnerV1 {
    let template = owner();
    let source = template.source_semantic();
    let original = &source.functions()[0];
    let mut types = source.types().to_vec();
    let scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    types[DESCRIPTOR.index() as usize] = declaration(
        12,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(scalar, length),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SLICE,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let closure = FixtureTypeClosureV29::new(&types, &[UNIT, DESCRIPTOR, TWO_DESCRIPTORS]);
    let descriptor = closure.id(DESCRIPTOR);
    let pair = closure.id(TWO_DESCRIPTORS);
    let unit = closure.id(UNIT);
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let shared = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(8),
    )
    .unwrap();
    let aggregate = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesNone),
            true,
            false,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        32,
        Some(8),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([181; 32]),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                descriptor,
                SemanticAbiPassModeV1::Pair {
                    first: shared,
                    second: attributes,
                },
            )),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                pair,
                SemanticAbiPassModeV1::Indirect {
                    attributes: aggregate,
                    metadata_attributes: None,
                    on_stack: false,
                },
            )),
        ],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let span = original.blocks()[0].source();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            span,
        )
    };
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let field = |local, field| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), descriptor)
                    .unwrap(),
            ],
            descriptor,
        )
        .unwrap()
    };
    let assign = |place: SemanticPlaceV1, value| {
        SemanticStatementV1::new(
            span,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place.clone(),
                SemanticRvalueV1::new(place.ty(), value),
            )),
        )
    };
    let global = || SemanticOperandV1::Copy(place(1, descriptor));
    let update = |destination, value| {
        if memory_backed {
            SemanticStatementV1::new(span, SemanticStatementKindV1::Store(
                SemanticMemoryStoreV1::new(destination, value, SemanticVolatilityV1::NonVolatile, None)))
        } else {
            assign(destination, SemanticRvalueKindV1::Use(value))
        }
    };
    let entry = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([186; 32]),
        span,
        vec![
            assign(
                place(3, pair),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![global(), global()],
                    )
                    .unwrap(),
                ),
            ),
            update(field(3, 0), SemanticOperandV1::Copy(field(2, 0))),
            update(field(3, 0), global()),
        ],
        SemanticTerminatorV1::new(
            span,
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(1),
            )),
        ),
    )
    .unwrap();
    let exit = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([187; 32]),
        span,
        vec![assign(
            place(0, unit),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                unit,
                SemanticConstantValueV1::ZeroSized,
            ))),
        )],
        SemanticTerminatorV1::new(span, SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([188; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([189; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([190; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([191; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([192; 32]),
        span,
        abi,
        vec![
            local(182, unit, SemanticLocalRoleV1::Return),
            local(183, descriptor, SemanticLocalRoleV1::Argument(0)),
            local(184, pair, SemanticLocalRoleV1::Argument(1)),
            local(185, pair, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![entry, exit],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        closure.types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(ROOT)],
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
    .unwrap()
}

#[test]
fn authentic_original_writes_select_backing_even_when_the_transient_value_disappears() {
    for profiled in [true, false] {
        let mut owner = transient_descriptor_owner_v29();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(71).unwrap();
        let capture = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
        let roots = fixture.roots();
        production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut budget,
            |instances, budget| {
                let floor = budget.storage();
                budget.reserve_storage(std::mem::size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>()).unwrap();
                let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(&owner,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget).unwrap();
                let descriptor = profiled.then(|| profile.descriptor_root(&owner, 0, budget).unwrap());
                let demands = source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, budget).unwrap();
                let mut layouts = SourceStorageLayoutsV29::new(&owner, demands.types(&owner, budget).unwrap(), budget).unwrap();
                let lens = demands.root_lens(&owner, 0, budget).unwrap();
                with_source_storage_descriptor_demands_root_v29(&mut layouts, instances, descriptor, lens, budget,
                    |plan, root, budget| {
                        let original = plan.storage_demands.unwrap().requests(instances, budget)?.0;
                        assert!(original.iter().any(|row| row.local.index() == 3
                            && row.kind == source_storage_demands_v29::DemandKindV29::WholeBackingCandidate));
                        let writes = plan.representation_demands.iter().filter(|row| row.local.index() == 3).collect::<Vec<_>>();
                        assert_eq!(writes.len(), 3);
                        assert!(writes[0].projections.is_empty());
                        assert_eq!(writes[1].projections.len(), 1);
                        assert_eq!(writes[2].projections.len(), 1);
                        let final_block = plan.blocks.iter().find(|row| row.instance == instances.root() && row.block.index() == 1).unwrap();
                        let node = plan.states[final_block.entry][3].node.unwrap();
                        let SourceReferenceNodeKindV29::Aggregate { first, count: 2 } = plan.nodes[node].kind else {
                            panic!("final original object was not retained")
                        };
                        let final_field = plan.children[first];
                        if profiled {
                            assert_eq!(plan.descriptor_sets[plan.nodes[final_field].descriptor.unwrap()].representation, AddressSpace::Global);
                            let transient = &plan.nodes[writes[1].node];
                            assert!(transient.descriptor.is_none(), "by-value fields have no invented argument origin");
                            let Type::Slice(physical) = source_descriptor_node_type_v29(
                                plan, Some(writes[1].node), transient.ty, budget,
                            )? else { panic!("original transient descriptor"); };
                            assert_eq!(physical.address_space, AddressSpace::Generic);
                        } else {
                            assert!(plan.nodes[final_field].descriptor.is_none());
                        }
                        let cell = plan.cells.rows.iter().find(|cell| cell.instance == instances.root() && cell.local.index() == 3).unwrap();
                        let SourceBackingKindV29::Object(schema) = cell.kind else { panic!("retained aggregate has no selected backing") };
                        let rows = root.arena.layouts.rows(&owner, budget)?;
                        let StorageLayoutKindV1::Record(fields) = &rows[schema.0 as usize].kind else { panic!("selected aggregate row") };
                        assert_eq!(fields.len(), 2);
                        let spaces = fields.iter().map(|field| match rows[field.layout.0 as usize].kind {
                            StorageLayoutKindV1::Slice { value_space, .. } => value_space,
                            _ => panic!("selected original descriptor field"),
                        }).collect::<Vec<_>>();
                        assert_eq!(spaces, vec![AddressSpace::Generic,
                            if profiled { AddressSpace::Global } else { AddressSpace::Generic }]);
                        assert_eq!((fields[0].offset, fields[1].offset, rows[schema.0 as usize].size), (0, 16, 32));
                        drop(rows);
                        Ok(())
                    }).unwrap();
                layouts.release(budget).unwrap();
                demands.discard(budget).unwrap();
                drop(profile);
                budget.release_storage(budget.storage() - floor).unwrap();
                Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            }).unwrap();
        assert_eq!(budget.storage(), 71 + capture.retained_storage());
        drop(roots);
        drop(fixture);
        drop(owner);
        budget.release_storage(capture.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 71);
    }
}

#[test]
fn ordinary_field_updates_and_explicit_stores_keep_distinct_original_ssa_and_backing() {
    for memory_backed in [false, true] {
        for (owner, updates) in [
            (backing_owner_with_observed_store_v29(false, memory_backed), 1),
            (transient_descriptor_with_observed_stores_v29(memory_backed), 2),
        ] {
            let source = &owner.source_semantic().functions()[0];
            let mut observed = 0;
            for statement in source.blocks()[0].statements() {
                match statement.kind() {
                    SemanticStatementKindV1::Store(store) if store.destination().local().index() == 3 => {
                        assert!(memory_backed);
                        assert_eq!(store.destination().projections().len(), 1);
                        assert!(matches!(store.value(), SemanticOperandV1::Copy(_)));
                        observed += 1;
                    }
                    SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().local().index() == 3
                            && !assignment.destination().projections().is_empty() => {
                        assert!(!memory_backed);
                        assert_eq!(assignment.destination().projections().len(), 1);
                        assert!(matches!(assignment.value().kind(),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_))));
                        observed += 1;
                    }
                    _ => {}
                }
            }
            assert_eq!(observed, updates);
            let plan = owner.plan_for_function(ROOT).unwrap().plan();
            assert_eq!(plan.promoted_variables().iter().any(|local| local.get() == 3), !memory_backed);
            let completed = std::cell::Cell::new(false);
            with_boundary_schema_source_v29(owner, |plan, budget| {
                let root = plan.instances.root();
                let requests = plan.storage_demands.unwrap().requests(plan.instances, budget)?.0;
                assert_eq!(requests.iter().any(|row| row.local.index() == 3
                    && row.kind == source_storage_demands_v29::DemandKindV29::WholeBackingCandidate), memory_backed);
                assert_eq!(plan.cells.rows.iter().any(|row| row.instance == root && row.local.index() == 3),
                    memory_backed);
                completed.set(true);
                Ok(())
            }).unwrap();
            assert!(completed.get());
        }
    }
}

fn descriptor_schema(
    layouts: &SourceStorageLayoutsV29<'_>,
    owner: &ProductionSemanticSsaOwnerV1,
    space: AddressSpace,
    budget: &mut Budget<'_>,
) -> Result<StorageLayoutIdV1, Error> {
    let element = layouts.row_for(owner, WORD, budget)?;
    layouts.select_schema(
        owner,
        DESCRIPTOR,
        SourceStorageSelectionV29::Slice {
            element,
            value_space: space,
            access: AccessMode::ReadOnly,
        },
        budget,
    )
}

#[test]
fn selected_schema_keeps_original_geometry_and_distinguishes_closed_representations() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(37).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[DESCRIPTOR], &mut budget).unwrap();
    let original = layouts.rows(&owner, &mut budget).unwrap().to_vec();
    let generic = descriptor_schema(&layouts, &owner, AddressSpace::Generic, &mut budget).unwrap();
    assert_eq!(
        generic,
        layouts.row_for(&owner, DESCRIPTOR, &mut budget).unwrap()
    );
    let global = descriptor_schema(&layouts, &owner, AddressSpace::Global, &mut budget).unwrap();
    assert_ne!(generic, global);
    let paid = (budget.storage(), layouts.lease.persistent.get());
    assert_eq!(
        descriptor_schema(&layouts, &owner, AddressSpace::Global, &mut budget).unwrap(),
        global
    );
    assert_eq!((budget.storage(), layouts.lease.persistent.get()), paid);
    {
        let rows = layouts.rows(&owner, &mut budget).unwrap();
        assert_eq!(&rows[..original.len()], original);
        assert_eq!(rows.len(), original.len() + 2);
        let StorageLayoutKindV1::Slice {
            value_space,
            data,
            length,
            ..
        } = rows[global.0 as usize].kind
        else {
            panic!("selected descriptor lost its actual row kind")
        };
        assert_eq!(
            (value_space, data.offset, length.offset),
            (AddressSpace::Global, 0, 8)
        );
        let StorageLayoutKindV1::Pointer(pointer) = rows[data.layout.0 as usize].kind else {
            panic!("selected descriptor lost its data representation")
        };
        assert_eq!(
            (
                pointer.value_space,
                pointer.encoded_space,
                pointer.stored_bits
            ),
            (AddressSpace::Global, AddressSpace::Generic, 64)
        );
        assert_eq!(
            (
                rows[global.0 as usize].size,
                rows[global.0 as usize].alignment
            ),
            (16, 8)
        );
    }
    let SemanticTypeShapeV1::Pointer(source) =
        owner.source_semantic().types()[DESCRIPTOR.index() as usize].shape()
    else {
        panic!("original descriptor declaration changed")
    };
    assert_eq!(source.address_space(), 0);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn selected_schema_refuses_outstanding_row_views_without_a_panic_or_published_id() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts = SourceStorageLayoutsV29::new(&owner, &[DESCRIPTOR], &mut budget).unwrap();
    let rows = layouts.rows(&owner, &mut budget).unwrap();
    let length = rows.len();
    let first = descriptor_schema(&layouts, &owner, AddressSpace::Global, &mut budget).unwrap_err();
    assert!(matches!(
        first,
        Error::Unsupported {
            detail: "source storage schema publication has an outstanding row view",
            ..
        }
    ));
    assert_eq!(rows.len(), length);
    drop(rows);
    let work = budget.work();
    assert!(matches!(
        descriptor_schema(&layouts, &owner, AddressSpace::Generic, &mut budget),
        Err(Error::Unsupported {
            detail: "source storage schema publication has an outstanding row view",
            ..
        })
    ));
    assert_eq!(budget.work(), work);
    assert!(layouts.release(&mut budget).is_err());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn selected_schema_storage_denial_cannot_publish_or_refund_the_original_table() {
    const LIMIT: usize = 64 * 1024 * 1024;
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(137).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[DESCRIPTOR], &mut budget).unwrap();
    let original = layouts.lease.persistent.get();
    assert_eq!(layouts.rows(&owner, &mut budget).unwrap().len(), 4);
    let pressure = LIMIT - budget.storage();
    budget.reserve_storage(pressure).unwrap();
    let result = descriptor_schema(&layouts, &owner, AddressSpace::Global, &mut budget);
    assert!(matches!(result, Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_)))));
    assert_eq!(layouts.lease.persistent.get(), original);
    let physical = layouts.physical.try_borrow().unwrap();
    assert_eq!(physical.rows.len(), 4);
    assert!(physical.selected_keys.is_empty());
    drop(physical);
    assert!(layouts.release(&mut budget).is_err());
    assert_eq!(budget.storage(), 137 + pressure);
    budget.release_storage(pressure).unwrap();
    assert_eq!(budget.storage(), 137);
}

#[test]
fn selected_descriptor_limits_are_exact_before_table_growth() {
    use fe2o3_kernel_ir::StorageLayoutLimitsV1;
    // Original closure: word, slice, data and length = four rows/four edges.
    // A Global representation adds one pointer and one slice = two/four more.
    let exact = StorageLayoutLimitsV1 {
        rows: 6,
        edges: 8,
        containment_depth: 2,
        object_bytes: 16,
    };
    for (limits, accepted) in [
        (exact, true),
        (StorageLayoutLimitsV1 { rows: 5, ..exact }, false),
        (StorageLayoutLimitsV1 { edges: 7, ..exact }, false),
    ] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(53).unwrap();
        let layouts =
            SourceStorageLayoutsV29::new_with_limits(&owner, &[DESCRIPTOR], limits, &mut budget)
                .unwrap();
        let result = descriptor_schema(&layouts, &owner, AddressSpace::Global, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(layouts.rows(&owner, &mut budget).unwrap().len(), 6);
            layouts.release(&mut budget).unwrap();
        } else {
            let rows = layouts.physical.try_borrow().unwrap();
            assert_eq!(rows.rows.len(), 5);
            assert_eq!(rows.selected_keys.len(), 1);
            drop(rows);
            assert!(layouts.release(&mut budget).is_err());
        }
        assert_eq!(budget.storage(), 53);
    }
    for limits in [
        StorageLayoutLimitsV1 {
            containment_depth: 1,
            ..exact
        },
        StorageLayoutLimitsV1 {
            object_bytes: 15,
            ..exact
        },
    ] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(53).unwrap();
        assert!(
            SourceStorageLayoutsV29::new_with_limits(&owner, &[DESCRIPTOR], limits, &mut budget)
                .is_err()
        );
        assert_eq!(budget.storage(), 53);
    }
}

#[test]
fn root_scratch_cleanup_keeps_schema_credit_and_second_root_reuses_exact_rows() {
    root_custody_tests::instances(|instances, budget| {
        let before = budget.storage();
        let mut layouts =
            SourceStorageLayoutsV29::new(instances.owner(), &[PAIR, DESCRIPTOR], budget).unwrap();
        let table = budget.storage();
        let old_persistent = layouts.lease.persistent.get();
        let mut first = None;
        for turn in 0..2 {
            with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                let state = root.new_state(instances.root(), local_for(PAIR), budget)?;
                let path = root.root_path(PAIR, budget)?;
                root.mutate(
                    state,
                    path,
                    SourceStorageRootMutationV29::Initialize,
                    budget,
                )?;
                let id = descriptor_schema(
                    root.arena.layouts,
                    instances.owner(),
                    AddressSpace::Global,
                    budget,
                )?;
                if let Some(previous) = first {
                    assert_eq!(id, previous);
                } else {
                    first = Some(id);
                }
                assert!(root.is_initialized(state, path, budget)?);
                Ok(())
            })
            .unwrap();
            assert!(layouts.lease.root.get().is_none());
            let persistent = layouts.lease.persistent.get();
            assert!(persistent > old_persistent);
            assert_eq!(layouts.lease.owned.get(), persistent);
            assert_eq!(budget.storage(), table + persistent - old_persistent);
            if turn == 1 {
                let rows = layouts.rows(instances.owner(), budget).unwrap();
                assert_eq!(rows.len(), layouts.keys.len() + 2);
            }
        }
        layouts.release(budget).unwrap();
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn root_callback_error_and_panic_do_not_refund_persistent_schemas() {
    root_custody_tests::instances(|instances, budget| {
        for panic in [false, true] {
            let before = budget.storage();
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[PAIR, DESCRIPTOR], budget)
                    .unwrap();
            let table = budget.storage();
            let persistent = layouts.lease.persistent.get();
            let result: Result<(), Error> =
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    let _ = root.new_state(instances.root(), local_for(PAIR), budget)?;
                    descriptor_schema(
                        root.arena.layouts,
                        instances.owner(),
                        AddressSpace::Global,
                        budget,
                    )?;
                    if panic {
                        panic!("schema callback panic");
                    }
                    Err(error("schema callback failure").into())
                });
            let first = result.unwrap_err();
            assert!(matches!(first, Error::Unsupported { detail, .. }
                if detail == if panic { "source reference callback panicked" } else { "schema callback failure" }));
            assert!(layouts.lease.persistent.get() > persistent);
            assert_eq!(layouts.lease.owned.get(), layouts.lease.persistent.get());
            assert_eq!(
                budget.storage(),
                table + layouts.lease.persistent.get() - persistent
            );
            assert!(layouts.release(budget).is_err());
            assert_eq!(budget.storage(), before);
        }
    });
}

fn settle_component_query_credit_v29(
    layouts: &SourceStorageLayoutsV29<'_>,
    credit: SourceStorageEmissionCreditV29,
    budget: &mut Budget<'_>,
) {
    let scratch = credit.into_root_credit(layouts, budget).unwrap();
    assert!(layouts.permits_root_emission_refund(layouts.owner, scratch, budget));
    budget.release_storage(scratch).unwrap();
}

fn component_mapping_owner_v29() -> (ProductionSemanticSsaOwnerV1, SemanticTypeIdV1, SemanticTypeIdV1, SemanticTypeIdV1) {
    let template = selected_niche_fixture_v29(false, false);
    let mut declarations = template.owner.source_semantic().types().to_vec();
    let SemanticTypeShapeV1::Tuple(fields) = declarations[template.mixed.index() as usize].shape()
        else { panic!("source mixed record") };
    let nominal = fields.fields()[0];
    let mixed = SemanticTypeIdV1::from_index(declarations.len() as u32);
    declarations.push(declaration(140,
        SemanticTypeLayoutV1::aggregate(Some(16), 8,
            SemanticAggregateLayoutV1::new(vec![0, 0, 8, 8, 9], vec![]).unwrap()).unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(
            vec![nominal, WORD, nominal, BYTE, nominal]).unwrap())));
    let single = SemanticTypeIdV1::from_index(declarations.len() as u32);
    declarations.push(declaration(141,
        SemanticTypeLayoutV1::aggregate(Some(8), 8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap()).unwrap(),
        SemanticTypeShapeV1::enum_type(BYTE, vec![
            SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![WORD]).unwrap()),
        ]).unwrap()));
    let admitted = request(declarations).admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default()).unwrap();
    (owner, mixed, nominal, single)
}

#[test]
fn selected_component_mapper_preserves_original_and_compressed_field_ordinals() {
    let (owner, mixed, nominal, _) = component_mapping_owner_v29();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(277).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[WORD, BYTE], &mut budget).unwrap();
    let word = layouts.row_for(&owner, WORD, &mut budget).unwrap();
    let byte = layouts.row_for(&owner, BYTE, &mut budget).unwrap();
    let schema = layouts.select_schema(&owner, mixed, SourceStorageSelectionV29::Aggregate {
        variant: None, fields: &[(1, word), (3, byte)],
    }, &mut budget).unwrap();
    let persistent = layouts.lease.persistent.get();
    let credit = layouts.capture_emission_credit(&owner, &mut budget).unwrap();
    for (field, physical, ty, expected, offset) in [(1, 0, WORD, word, 0), (3, 1, BYTE, byte, 8)] {
        let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()];
        let before = budget.work();
        assert_eq!(layouts.project_selected_schema(&owner, mixed, schema, &path, &mut budget).unwrap(), (ty, expected));
        assert_eq!(budget.work() - before, 7 + field as usize, "preserve the original one-pass query charge");
        let mut visited = 0;
        let result = layouts.visit_selected_components(&owner, mixed, schema, &path, &mut budget, |step, _| {
            visited += 1;
            assert!(std::ptr::eq(step.projection, &path[0]));
            assert_eq!((step.source_type, step.source_schema), (mixed, schema));
            assert_eq!((step.result_type, step.result_schema), (ty, Some(expected)));
            assert_eq!(step.kind, SourceSelectedComponentKindV29::Field {
                original: field, physical, byte_offset: offset,
            });
            assert!(layouts.physical.try_borrow_mut().is_err(), "the observed physical owner remains borrowed");
            Ok(())
        }).unwrap();
        assert_eq!(visited, 1);
        assert_eq!(result, SourceSelectedProjectionV29::Physical { ty, schema: expected });
    }
    for field in [0, 2, 4] {
        let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), nominal).unwrap()];
        let error = layouts.project_selected_schema(&owner, mixed, schema, &path, &mut budget).unwrap_err();
        assert!(matches!(error, Error::Unsupported { detail: "source-nominal field has no physical pointer view", .. }));
        let mut visited = 0;
        let result = layouts.visit_selected_components(&owner, mixed, schema, &path, &mut budget, |step, _| {
            visited += 1;
            assert_eq!(step.kind, SourceSelectedComponentKindV29::OmittedNominal { original: field });
            assert_eq!((step.result_type, step.result_schema), (nominal, None));
            Ok(())
        }).unwrap();
        assert_eq!(visited, 1);
        assert_eq!(result, SourceSelectedProjectionV29::OmittedNominal { ty: nominal, position: 0 });
    }
    assert_eq!(layouts.lease.persistent.get(), persistent);
    settle_component_query_credit_v29(&layouts, credit, &mut budget);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 277);
}

#[test]
fn selected_component_mapper_keeps_single_variant_metadata_and_union_overlap_inert() {
    let (owner, _, _, single) = component_mapping_owner_v29();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(281).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[single, UNION], &mut budget).unwrap();
    let single_schema = layouts.row_for(&owner, single, &mut budget).unwrap();
    let union_schema = layouts.row_for(&owner, UNION, &mut budget).unwrap();
    let credit = layouts.capture_emission_credit(&owner, &mut budget).unwrap();
    let downcast = [SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), single).unwrap()];
    let error = layouts.project_selected_schema(&owner, single, single_schema, &downcast, &mut budget).unwrap_err();
    assert!(matches!(error, Error::Unsupported { detail: "selected source path ends at an untyped enum payload", .. }));
    let mut visited = 0;
    assert_eq!(layouts.visit_selected_components(&owner, single, single_schema, &downcast, &mut budget, |step, _| {
        visited += 1;
        assert_eq!(step.kind, SourceSelectedComponentKindV29::Variant { original: 0, physical: false });
        assert_eq!(step.result_schema, Some(single_schema));
        Ok(())
    }).unwrap(), SourceSelectedProjectionV29::Payload { ty: single, schema: single_schema, variant: 0 });
    assert_eq!(visited, 1);
    for (field, ty) in [(0, WORD), (1, BYTE)] {
        let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()];
        let mut visited = 0;
        layouts.visit_selected_components(&owner, UNION, union_schema, &path, &mut budget, |step, _| {
            visited += 1;
            assert_eq!(step.kind, SourceSelectedComponentKindV29::Field {
                original: field, physical: field as usize, byte_offset: 0,
            });
            Ok(())
        }).unwrap();
        assert_eq!(visited, 1);
    }
    settle_component_query_credit_v29(&layouts, credit, &mut budget);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 281);
}

#[test]
fn selected_component_mapper_keeps_nested_array_and_actual_pointer_niche_representation() {
    for slice in [false, true] {
        let fixture = selected_niche_fixture_v29(slice, true);
        let owner = &fixture.owner;
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(283).unwrap();
        let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
        let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
        let schema = layouts.select_schema(owner, fixture.enumeration,
            SourceStorageSelectionV29::Enum { variants: &selected.variants }, &mut budget).unwrap();
        let (tuple_type, array_type) = fixture.nested.unwrap();
        let (tuple_schema, array_schema) = selected.nested.unwrap();
        let path = [
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), fixture.enumeration).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), array_type).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::ConstantIndex { offset: 1, minimum_length: 2, from_end: false }, tuple_type).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), fixture.pointer).unwrap(),
        ];
        let persistent = layouts.lease.persistent.get();
        let credit = layouts.capture_emission_credit(owner, &mut budget).unwrap();
        let mut visited = 0;
        let result = layouts.visit_selected_components(owner, fixture.enumeration, schema, &path, &mut budget, |step, _| {
            assert!(std::ptr::eq(step.projection, &path[visited]));
            match visited {
                0 => {
                    assert_eq!(step.kind, SourceSelectedComponentKindV29::Variant { original: 1, physical: true });
                    assert_eq!(step.result_schema, Some(selected.variants[1].1));
                }
                1 => {
                    assert_eq!(step.kind, SourceSelectedComponentKindV29::Field { original: 1, physical: 0, byte_offset: 0 });
                    assert_eq!(step.result_schema, Some(array_schema));
                }
                2 => {
                    assert_eq!(step.kind, SourceSelectedComponentKindV29::Index { length: 2, stride: if slice { 16 } else { 8 } });
                    assert_eq!(step.result_schema, Some(tuple_schema));
                }
                3 => assert_eq!(step.result_schema, Some(selected.pointer)),
                _ => panic!("unexpected extra component"),
            }
            visited += 1;
            Ok(())
        }).unwrap();
        assert_eq!(visited, 4);
        assert_eq!(result, SourceSelectedProjectionV29::Physical { ty: fixture.pointer, schema: selected.pointer });
        let rows = layouts.rows(owner, &mut budget).unwrap();
        let StorageLayoutKindV1::Variants { encoding, .. } = rows[schema.0 as usize].kind else { panic!("actual enum") };
        let actual_pointer = if slice {
            let StorageLayoutKindV1::Slice { data, .. } = rows[selected.pointer.0 as usize].kind else { panic!("actual slice") };
            data.layout
        } else { selected.pointer };
        assert_eq!(rows[encoding.tag().layout.0 as usize], rows[actual_pointer.0 as usize]);
        let StorageLayoutKindV1::Pointer(pointer) = rows[actual_pointer.0 as usize].kind else { panic!("actual pointer") };
        assert_eq!((pointer.pointee, pointer.value_space, pointer.encoded_space, pointer.access),
            (selected.mixed, AddressSpace::Global, AddressSpace::Generic, AccessMode::ReadOnly));
        drop(rows);
        assert_eq!(layouts.lease.persistent.get(), persistent);
        settle_component_query_credit_v29(&layouts, credit, &mut budget);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 283);
    }
}

#[test]
fn selected_component_mapper_rejects_invalid_suffixes_before_any_callback() {
    let (owner, mixed, nominal, _) = component_mapping_owner_v29();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(293).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[WORD, BYTE], &mut budget).unwrap();
    let word = layouts.row_for(&owner, WORD, &mut budget).unwrap();
    let byte = layouts.row_for(&owner, BYTE, &mut budget).unwrap();
    let schema = layouts.select_schema(&owner, mixed, SourceStorageSelectionV29::Aggregate {
        variant: None, fields: &[(1, word), (3, byte)],
    }, &mut budget).unwrap();
    let count = layouts.physical.borrow().rows.len();
    let credit = layouts.capture_emission_credit(&owner, &mut budget).unwrap();
    for mode in 0..6 {
        let projection = |kind, ty| SemanticProjectionV1::new(kind, ty).unwrap();
        let (path, length) = match mode {
            0 => ([projection(SemanticProjectionKindV1::Field(1), WORD),
                projection(SemanticProjectionKindV1::Field(0), WORD)], 2),
            1 => ([projection(SemanticProjectionKindV1::Field(0), nominal),
                projection(SemanticProjectionKindV1::Field(0), nominal)], 2),
            2 => ([projection(SemanticProjectionKindV1::Field(0), WORD),
                projection(SemanticProjectionKindV1::Field(0), WORD)], 1),
            3 => ([projection(SemanticProjectionKindV1::Field(1), BYTE),
                projection(SemanticProjectionKindV1::Field(1), BYTE)], 1),
            4 => ([projection(SemanticProjectionKindV1::Field(5), WORD),
                projection(SemanticProjectionKindV1::Field(5), WORD)], 1),
            5 => ([projection(SemanticProjectionKindV1::Downcast(0), mixed),
                projection(SemanticProjectionKindV1::Downcast(0), mixed)], 1),
            _ => unreachable!(),
        };
        let mut called = false;
        assert!(layouts.visit_selected_components(&owner, mixed, schema, &path[..length], &mut budget, |_, _| {
            called = true; Ok(())
        }).is_err(), "mode {mode}");
        assert!(!called, "invalid source path must fail before callback mode {mode}");
        assert_eq!(layouts.physical.borrow().rows.len(), count);
    }
    settle_component_query_credit_v29(&layouts, credit, &mut budget);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 293);
}

#[test]
fn selected_component_mapper_authenticates_owner_row_and_active_table_before_callback() {
    for mode in 0..5 {
        let fixture = selected_niche_fixture_v29(false, false);
        let other = selected_niche_fixture_v29(false, false);
        let owner = &fixture.owner;
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(307).unwrap();
        let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
        let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
        let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), WORD).unwrap()];
        let credit = layouts.capture_emission_credit(owner, &mut budget).unwrap();
        if mode == 4 {
            let mut physical = layouts.physical.borrow_mut();
            let StorageLayoutKindV1::Record(fields) = &mut physical.rows[selected.mixed.0 as usize].kind else { panic!("mixed record") };
            fields[0].layout = selected.pointer;
        }
        let held = if mode == 3 { Some(layouts.physical.borrow_mut()) } else { None };
        let mut called = false;
        let result = layouts.visit_selected_components(
            if mode == 0 { &other.owner } else { owner },
            if mode == 1 { WORD } else { fixture.mixed },
            if mode == 2 { StorageLayoutIdV1(u32::MAX) } else { selected.mixed },
            &path, &mut budget, |_, _| { called = true; Ok(()) });
        assert!(result.is_err(), "mode {mode}");
        assert!(!called);
        drop(held);
        settle_component_query_credit_v29(&layouts, credit, &mut budget);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 307);
    }
}

#[test]
fn selected_component_mapper_propagates_callback_error_and_unwind_without_schema_growth() {
    for panic_callback in [false, true] {
        let fixture = selected_niche_fixture_v29(false, false);
        let owner = &fixture.owner;
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(311).unwrap();
        let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
        let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
        let count = layouts.physical.borrow().rows.len();
        let persistent = layouts.lease.persistent.get();
        let credit = layouts.capture_emission_credit(owner, &mut budget).unwrap();
        let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), WORD).unwrap()];
        let mut visited = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
            layouts.visit_selected_components(owner, fixture.mixed, selected.mixed, &path, &mut budget, |_, _| {
                visited += 1;
                if panic_callback { panic!("selected component callback panic probe"); }
                Err(error("selected component callback error probe"))
            })));
        assert_eq!(visited, 1);
        if panic_callback { assert!(result.is_err()); }
        else { assert!(matches!(result.unwrap(), Err(Error::Unsupported {
            detail: "selected component callback error probe", ..
        }))); }
        assert_eq!(layouts.physical.borrow().rows.len(), count);
        assert_eq!(layouts.lease.persistent.get(), persistent);
        settle_component_query_credit_v29(&layouts, credit, &mut budget);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 311);
    }
}

#[test]
fn selected_component_mapper_has_independent_two_pass_work_and_live_header_boundaries() {
    let fixture = selected_niche_fixture_v29(false, false);
    let owner = &fixture.owner;
    let headers = 2 * size_of::<SourceSelectedProjectionV29>()
        + 2 * size_of::<Result<SourceSelectedProjectionV29, Error>>()
        + size_of::<SourceSelectedComponentV29<'_>>() + size_of::<Result<(), Error>>();
    assert_eq!(source_selected_projection_headers_v29().unwrap(), headers);
    for storage_cut in [false, true] {
        for slack in [0, 1] {
            const LIMIT: usize = 20_000_000;
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(313).unwrap();
            let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
            let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
            let count = layouts.physical.borrow().rows.len();
            let persistent = layouts.lease.persistent.get();
            let credit = layouts.capture_emission_credit(owner, &mut budget).unwrap();
            let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), WORD).unwrap()];
            // Owner check + two (owner/key + projection + two original fields
            // + result key) traversals + final equality: 1 + 2*(2+3+2+1) + 1.
            let required_work = 18;
            if storage_cut {
                budget.reserve_storage(LIMIT - budget.storage() - (headers - slack)).unwrap();
            } else {
                budget.charge_work(LIMIT - budget.work() - (required_work - slack)).unwrap();
            }
            let before_storage = budget.storage();
            let before_work = budget.work();
            let mut calls = 0;
            let result = layouts.visit_selected_components(owner, fixture.mixed, selected.mixed, &path, &mut budget, |_, _| {
                calls += 1; Ok(())
            });
            if slack == 0 {
                assert!(result.is_ok());
                assert_eq!(calls, 1);
                assert_eq!(budget.work(), before_work + required_work);
                assert_eq!(budget.storage(), before_storage + headers);
            } else if storage_cut {
                let Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) = result else { panic!("expected header cut") };
                assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
                assert_eq!(calls, 0);
                assert_eq!(budget.work(), before_work + 1);
                assert_eq!(budget.storage(), before_storage);
            } else {
                let Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error))) = result else { panic!("expected final comparison cut") };
                assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
                assert_eq!(calls, 1, "source validation completed before this later resource denial");
                assert_eq!(budget.work(), before_work + 17);
                assert_eq!(budget.storage(), before_storage + headers);
            }
            assert_eq!(layouts.physical.borrow().rows.len(), count);
            assert_eq!(layouts.lease.persistent.get(), persistent);
            settle_component_query_credit_v29(&layouts, credit, &mut budget);
            let cleanup = layouts.release(&mut budget);
            assert_eq!(cleanup.is_ok(), slack == 0);
            assert_eq!(budget.storage(), 313);
        }
    }
}

#[test]
fn selected_component_mapper_preserves_empty_paths_and_original_dynamic_index_metadata() {
    let fixture = selected_niche_fixture_v29(false, true);
    let owner = &fixture.owner;
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(317).unwrap();
    let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
    let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
    let (tuple_type, array_type) = fixture.nested.unwrap();
    let (tuple_schema, array_schema) = selected.nested.unwrap();
    let credit = layouts.capture_emission_credit(owner, &mut budget).unwrap();
    let mut calls = 0;
    let empty = layouts.visit_selected_components(owner, array_type, array_schema, &[], &mut budget,
        |_, _| { calls += 1; Ok(()) }).unwrap();
    assert_eq!(empty, SourceSelectedProjectionV29::Physical { ty: array_type, schema: array_schema });
    assert_eq!(calls, 0);
    let selector = SemanticLocalIdV1::from_index(17);
    let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Index(selector), tuple_type).unwrap()];
    let result = layouts.visit_selected_components(owner, array_type, array_schema, &path, &mut budget, |step, _| {
        calls += 1;
        assert!(std::ptr::eq(step.projection, &path[0]));
        assert_eq!(step.projection.kind(), SemanticProjectionKindV1::Index(selector));
        assert_eq!(step.kind, SourceSelectedComponentKindV29::Index { length: 2, stride: 8 });
        assert_eq!((step.source_type, step.source_schema), (array_type, array_schema));
        assert_eq!((step.result_type, step.result_schema), (tuple_type, Some(tuple_schema)));
        Ok(())
    }).unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result, SourceSelectedProjectionV29::Physical { ty: tuple_type, schema: tuple_schema });
    // This owner has no original instance/local17. The mapping carries the
    // borrowed selector, not a value, bounds fact or storage-read permission.
    settle_component_query_credit_v29(&layouts, credit, &mut budget);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 317);
}

#[test]
fn selected_component_mapper_rejects_an_underfunded_original_lease_before_callbacks() {
    let fixture = selected_niche_fixture_v29(false, false);
    let owner = &fixture.owner;
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(331).unwrap();
    let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
    let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
    let credit = layouts.capture_emission_credit(owner, &mut budget).unwrap();
    let required = layouts.lease.floor + layouts.lease.owned.get();
    let stolen = budget.storage() - required + 1;
    budget.release_storage(stolen).unwrap();
    let before = budget.work();
    let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), WORD).unwrap()];
    let mut calls = 0;
    let error = layouts.visit_selected_components(owner, fixture.mixed, selected.mixed, &path, &mut budget,
        |_, _| { calls += 1; Ok(()) }).unwrap_err();
    assert!(matches!(error, Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)));
    assert_eq!(calls, 0);
    assert_eq!(budget.work(), before);
    budget.reserve_storage(stolen).unwrap();
    assert!(layouts.visit_selected_components(owner, fixture.mixed, selected.mixed, &path, &mut budget,
        |_, _| { calls += 1; Ok(()) }).is_err(), "repairing credit does not erase the original failure");
    assert_eq!(calls, 0);
    settle_component_query_credit_v29(&layouts, credit, &mut budget);
    assert!(layouts.release(&mut budget).is_err());
    assert_eq!(budget.storage(), 331);
}

#[test]
fn selected_component_strict_wrapper_pays_only_its_additional_live_envelopes() {
    let fixture = selected_niche_fixture_v29(false, false);
    let owner = &fixture.owner;
    let additional = size_of::<SourceSelectedProjectionV29>()
        + size_of::<Result<SourceSelectedProjectionV29, Error>>()
        + size_of::<SourceSelectedComponentV29<'_>>() + size_of::<Result<(), Error>>()
        + size_of::<std::thread::Result<Result<(SemanticTypeIdV1, StorageLayoutIdV1), Error>>>()
        + size_of::<Result<(), Error>>();
    let visitor = 2 * size_of::<SourceSelectedProjectionV29>()
        + 2 * size_of::<Result<SourceSelectedProjectionV29, Error>>()
        + size_of::<SourceSelectedComponentV29<'_>>() + size_of::<Result<(), Error>>();
    assert_eq!(source_selected_strict_headers_v29().unwrap(), additional);
    assert_eq!(source_selected_projection_headers_v29().unwrap(), visitor);
    for short in [false, true] {
        const LIMIT: usize = 20_000_000;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(337).unwrap();
        let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
        let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
        let filler = LIMIT - budget.storage() - additional + usize::from(short);
        budget.reserve_storage(filler).unwrap();
        let storage = budget.storage();
        let work = budget.work();
        let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), WORD).unwrap()];
        let result = layouts.project_selected_schema(owner, fixture.mixed, selected.mixed, &path, &mut budget);
        if short {
            let Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) = result
                else { panic!("strict extra-envelope one-short boundary"); };
            assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
            assert_eq!(budget.storage(), storage);
            assert_eq!(budget.work() - work, 2, "original owner/key check precedes storage publication");
        } else {
            assert_eq!(result.unwrap().0, WORD);
            assert_eq!(budget.storage(), storage, "strict temporaries are retired before the old tuple escapes");
            assert_eq!(budget.work() - work, 8, "unchanged strict owner/key/projection/field/result work");
        }
        budget.release_storage(filler).unwrap();
        assert_eq!(layouts.release(&mut budget).is_err(), short);
        assert_eq!(budget.storage(), 337);
    }
}

#[test]
fn selected_strict_projection_settles_only_its_lease_headers_on_success_error_and_unwind() {
    let fixture = selected_niche_fixture_v29(false, false);
    let owner = &fixture.owner;
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(349).unwrap();
    let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
    let row = layouts.row_for(owner, WORD, &mut budget).unwrap();
    let headers = source_selected_strict_headers_v29().unwrap();
    for mode in 0..3 {
        let before = (budget.storage(), budget.work(), layouts.lease.owned.get(), layouts.lease.persistent.get());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
            layouts.with_selected_strict_headers(&mut budget, |budget| {
                assert_eq!(budget.storage(), before.0 + headers);
                assert_eq!(layouts.lease.owned.get(), before.2 + headers);
                assert_eq!(layouts.lease.persistent.get(), before.3);
                match mode {
                    0 => Ok((WORD, row)),
                    1 => Err(error("strict projection cleanup sentinel")),
                    _ => panic!("strict projection original panic"),
                }
            })));
        match mode {
            0 => assert_eq!(result.unwrap().unwrap(), (WORD, row)),
            1 => assert!(format!("{:?}", result.unwrap()).contains("strict projection cleanup sentinel")),
            _ => assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&"strict projection original panic")),
        }
        assert_eq!((budget.storage(), budget.work(), layouts.lease.owned.get(), layouts.lease.persistent.get()), before);
    }
    // No caller emission-credit capture was established for any strict query.
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 349);
}

#[test]
fn selected_strict_projection_refusal_restores_direct_caller_floor_without_credit_scope() {
    let fixture = selected_niche_fixture_v29(false, false);
    let owner = &fixture.owner;
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(353).unwrap();
    let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
    let selected = selected_niche_rows_v29(&fixture, &layouts, AddressSpace::Global, &mut budget);
    let SemanticTypeShapeV1::Tuple(fields) = owner.source_semantic().types()[fixture.mixed.index() as usize].shape()
        else { panic!("original mixed field roster"); };
    let nominal = fields.fields()[0];
    let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), nominal).unwrap()];
    let before = (budget.storage(), layouts.lease.owned.get(), layouts.lease.persistent.get());
    let result = layouts.project_selected_schema(owner, fixture.mixed, selected.mixed, &path, &mut budget);
    assert!(matches!(result, Err(Error::Unsupported { detail: "source-nominal field has no physical pointer view", .. })));
    assert_eq!((budget.storage(), layouts.lease.owned.get(), layouts.lease.persistent.get()), before);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 353);
}

#[test]
fn selected_strict_projection_custody_loss_forbids_header_refund_and_preserves_original_error() {
    for original_error in [false, true] {
        let fixture = selected_niche_fixture_v29(false, false);
        let owner = &fixture.owner;
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(359).unwrap();
        let layouts = SourceStorageLayoutsV29::new(owner, &[WORD], &mut budget).unwrap();
        let row = layouts.row_for(owner, WORD, &mut budget).unwrap();
        let headers = source_selected_strict_headers_v29().unwrap();
        let before = (budget.storage(), layouts.lease.owned.get(), layouts.lease.persistent.get());
        let result = layouts.with_selected_strict_headers(&mut budget, |budget| {
            budget.release_storage(1).unwrap();
            if original_error { Err(error("strict original failure before invalid cleanup")) }
            else { Ok((WORD, row)) }
        });
        if original_error {
            assert!(format!("{result:?}").contains("strict original failure before invalid cleanup"));
        } else {
            assert!(matches!(result, Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
        }
        assert_eq!(budget.storage(), before.0 + headers - 1, "custody loss cannot release any header credit");
        assert_eq!(layouts.lease.owned.get(), before.1 + headers);
        assert_eq!(layouts.lease.persistent.get(), before.2);
        budget.reserve_storage(1).unwrap();
        assert!(layouts.project_selected_schema(owner, WORD, row, &[], &mut budget).is_err(),
            "restoring bytes does not clear the original accounting failure");
        assert!(layouts.release(&mut budget).is_err());
        assert_eq!(budget.storage(), 359);
    }
}
