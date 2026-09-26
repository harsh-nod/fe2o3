use super::*;

const CONSTRUCTION_LIMIT: usize = 20_000_000;

fn construction_field(local: u32, field: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
        ty,
    )
    .unwrap()
}

fn construction_checked_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = scoped_root_tests::fixtures::checked_slot_owner();
    let source = original.source_semantic();
    let mut functions = source.functions().to_vec();
    let helper = &functions[3];
    let pair = helper.locals()[3].ty();
    let SemanticTypeShapeV1::Tuple(fields) = source.types()[pair.index() as usize].shape() else {
        panic!("checked fixture tuple");
    };
    assert_eq!(fields.fields()[0], U32);
    let boolean = fields.fields()[1];
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 4);
    locals.push(local(137, boolean, SemanticLocalRoleV1::Temporary));
    let goto = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    functions[3] = function(
        130,
        helper.role(),
        helper.abi().clone(),
        locals,
        vec![
            block(140, helper.blocks()[0].statements().to_vec(), goto(1)),
            block(
                141,
                vec![
                    assign(
                        place(0, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(construction_field(
                            3, 0, U32,
                        ))),
                    ),
                    assign(
                        place(4, boolean),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(construction_field(
                            3, 1, boolean,
                        ))),
                    ),
                ],
                goto(2),
            ),
            block(142, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    construction_rebuild(source, functions)
}

fn construction_rebuild(
    source: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    functions: Vec<SemanticFunctionDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
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

fn construction_with_instances(
    mut owner: ProductionSemanticSsaOwnerV1,
    inspect: impl FnOnce(
        &ExecutionInstancesV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(CONSTRUCTION_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, CONSTRUCTION_LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let floor = budget.storage();
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(inspect(
                instances, budget,
            ))
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}

fn construction_with_storage(
    mut owner: ProductionSemanticSsaOwnerV1,
    inspect: impl for<'owner, 'root, 'view, 'source, 'work> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        source_storage_v29::SourceStorageRootV29<'view, 'root, 'source>,
        &'view ExecutionInstancesV29<'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<
        (),
        ProductionSemanticKirErrorV1,
    >,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(CONSTRUCTION_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, CONSTRUCTION_LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget).unwrap(),
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let floor = budget.storage();
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let floor = budget.storage();
            let result = source_storage_v29::with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |plan, root, budget| inspect(plan, root, instances, budget).map_err(Into::into),
            );
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
    layouts.release(&mut budget).unwrap();
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}

fn construction_block_state<'a>(
    plan: &'a SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    block: u32,
) -> &'a [SourceReferenceLocalV29] {
    let mut matching = plan
        .blocks
        .iter()
        .filter(|row| row.instance == instance && row.block.index() == block);
    let state = matching.next().unwrap().entry;
    assert!(matching.next().is_none());
    &plan.states[state]
}

#[test]
fn actual_intrinsic_normal_returns_initialize_only_the_original_nominal_destinations() {
    construction_with_storage(
        scoped_root_tests::fixtures::repeated_slot_owner(),
        |plan, root, instances, budget| {
            plan.check_owner(instances, budget)?;
            let root_id = instances.root();
            let entry = &plan.states[plan.entries[root_id.index()].unwrap()];
            let initial = plan.storage_snapshots[entry[2].storage.unwrap()];
            assert!(!root.snapshot_initialized(initial, &[], budget).unwrap());
            let issued = construction_block_state(plan, root_id, 1);
            let issued = plan.storage_snapshots[issued[2].storage.unwrap()];
            assert!(root.snapshot_initialized(issued, &[], budget).unwrap());
            let moved = construction_block_state(plan, root_id, 2);
            let moved = plan.storage_snapshots[moved[2].storage.unwrap()];
            assert!(!root.snapshot_initialized(moved, &[], budget).unwrap());
            let helper = instances
                .calls(root_id)
                .unwrap()
                .iter()
                .find_map(|call| call.child())
                .unwrap();
            assert_eq!(instances.instance(helper).unwrap().function(), HELPER);
            let initial = &plan.states[plan.entries[helper.index()].unwrap()];
            assert!(root
                .snapshot_initialized(
                    plan.storage_snapshots[initial[1].storage.unwrap()],
                    &[],
                    budget
                )
                .unwrap());
            assert!(!root
                .snapshot_initialized(
                    plan.storage_snapshots[initial[4].storage.unwrap()],
                    &[],
                    budget
                )
                .unwrap());
            let derived = construction_block_state(plan, helper, 1);
            assert!(root
                .snapshot_initialized(
                    plan.storage_snapshots[derived[4].storage.unwrap()],
                    &[],
                    budget
                )
                .unwrap());
            let moved = construction_block_state(plan, helper, 2);
            assert!(!root
                .snapshot_initialized(
                    plan.storage_snapshots[moved[4].storage.unwrap()],
                    &[],
                    budget
                )
                .unwrap());
            assert_eq!(
                instances.owner().source_semantic().types()[CONTEXT.index() as usize]
                    .layout()
                    .size_bytes(),
                Some(0)
            );
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn actual_checked_binary_constructs_both_tuple_children_in_each_original_instance() {
    construction_with_storage(
        construction_checked_owner(),
        |plan, root, instances, budget| {
            let mut checked = 0;
            let mut distinct = BTreeSet::new();
            for (ordinal, instance) in instances.instances().iter().enumerate() {
                if instance.function().index() != 3 {
                    continue;
                }
                let id = instances.id_at(ordinal).unwrap();
                assert!(distinct.insert(id.index()));
                let ty = instance.declaration().locals()[3].ty();
                let SemanticTypeShapeV1::Tuple(fields) =
                    instances.owner().source_semantic().types()[ty.index() as usize].shape()
                else {
                    panic!("tuple");
                };
                let initial = &plan.states[plan.entries[id.index()].unwrap()];
                assert!(!root
                    .snapshot_initialized(
                        plan.storage_snapshots[initial[3].storage.unwrap()],
                        &[],
                        budget
                    )
                    .unwrap());
                let after = construction_block_state(plan, id, 1);
                let snapshot = plan.storage_snapshots[after[3].storage.unwrap()];
                assert!(root.snapshot_initialized(snapshot, &[], budget).unwrap());
                for (field, ty) in fields.fields().iter().enumerate() {
                    let projection = SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Field(field as u32),
                        *ty,
                    )
                    .unwrap();
                    assert!(root
                        .snapshot_initialized(snapshot, &[projection], budget)
                        .unwrap());
                }
                let read = construction_block_state(plan, id, 2);
                for (local, ty) in [(0, U32), (4, fields.fields()[1])] {
                    assert_eq!(plan.nodes[read[local].node.unwrap()].ty, ty);
                    assert!(root
                        .snapshot_initialized(
                            plan.storage_snapshots[read[local].storage.unwrap()],
                            &[],
                            budget
                        )
                        .unwrap());
                }
                checked += 1;
            }
            assert_eq!(checked, 2);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn arbitrary_plain_context_zero_width_and_workgroup_nodes_do_not_gain_construction_authority() {
    construction_with_storage(lifecycle_owner(false), |_, root, instances, budget| {
        let floor = budget.storage();
        let statistics = root.snapshot_statistics(budget).unwrap();
        let root_id = instances.root();
        let helper = instances
            .calls(root_id)
            .unwrap()
            .iter()
            .find_map(|call| call.child())
            .unwrap();
        let mut builder = SourceReferenceBuilderV29::new_with_root(
            instances,
            SourceReferenceStorageV29::ScalarCells,
            Some(root),
            budget,
        )?;
        for (instance, local, bytes) in [(root_id, 2usize, 0), (helper, 4usize, 16)] {
            let declaration = instances.instance(instance).unwrap().declaration();
            let ty = declaration.locals()[local].ty();
            assert_eq!(
                instances.owner().source_semantic().types()[ty.index() as usize]
                    .layout()
                    .size_bytes(),
                Some(bytes)
            );
            let storage = builder
                .initial_storage_snapshot(
                    instance,
                    SemanticLocalIdV1::from_index(local as u32),
                    false,
                    budget,
                )?
                .unwrap();
            let mut state = source_reference_scratch_v29(declaration.locals().len(), budget)?;
            state.resize(
                declaration.locals().len(),
                SourceReferenceLocalV29::default(),
            );
            state[local].storage = Some(storage);
            let index = builder.plan.states.len();
            emission_push_v1(&mut builder.plan.states, state, budget)?;
            builder.frames[instance.index()] = Some(index);
            let site = SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(0),
                statement: None,
            };
            builder.effect_site = Some(site);
            let node = builder.plain(ty, budget)?;
            let result = builder.assign(site, &place(local as u32, ty), node, budget);
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source storage value needs its original typed construction state",
                        ..
                    })
                ),
                "{result:?}"
            );
            let after = builder.local(instance, SemanticLocalIdV1::from_index(local as u32))?;
            assert_eq!(after.storage, Some(storage));
            assert_eq!(after.node, None);
            let snapshot = builder.plan.storage_snapshots[storage];
            assert!(!builder
                .storage_root
                .as_ref()
                .unwrap()
                .snapshot_initialized(snapshot, &[], budget)
                .unwrap());
        }
        assert_eq!(
            builder
                .storage_root
                .as_ref()
                .unwrap()
                .snapshot_statistics(budget)
                .unwrap(),
            statistics
        );
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    })
    .unwrap();
}

fn construction_no_effects(builder: &SourceReferenceBuilderV29<'_, '_, '_>) {
    assert!(builder.plan.nodes.is_empty());
    assert!(builder.plan.states.is_empty());
    assert!(builder.plan.origins.is_empty());
    assert!(builder.plan.loans.is_empty());
    assert!(builder.plan.accesses.is_empty());
    assert!(builder.plan.storage_snapshots.is_empty());
    assert!(builder.frames.iter().all(Option::is_none));
    assert!(builder.visited.iter().all(|visited| !visited));
    assert_eq!(builder.effect_ordinal, 0);
    assert!(builder.effect_sites.is_empty());
}

fn construction_refused(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source construction differs from its original successful producer",
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn construction_validation_rejects_wrong_sites_inactive_occurrences_and_defined_helpers_before_effects(
) {
    construction_with_instances(
        scoped_root_tests::fixtures::checked_slot_owner(),
        |instances, budget| {
            let root = instances.root();
            let calls = instances.calls(root).unwrap();
            let issuer = calls.iter().find(|call| call.child().is_none()).unwrap();
            let defined = calls.iter().find(|call| call.child().is_some()).unwrap();
            let original = SourceReferenceSiteV29 {
                instance: root,
                block: issuer.occurrence().block,
                statement: None,
            };
            for (active, site, call) in [
                (None, original, issuer),
                (
                    Some(original),
                    SourceReferenceSiteV29 {
                        statement: Some(0),
                        ..original
                    },
                    issuer,
                ),
                (
                    Some(original),
                    SourceReferenceSiteV29 {
                        block: defined.occurrence().block,
                        ..original
                    },
                    issuer,
                ),
                (
                    Some(original),
                    SourceReferenceSiteV29 {
                        instance: defined.child().unwrap(),
                        ..original
                    },
                    issuer,
                ),
                (
                    Some(SourceReferenceSiteV29 {
                        block: defined.occurrence().block,
                        ..original
                    }),
                    SourceReferenceSiteV29 {
                        block: defined.occurrence().block,
                        ..original
                    },
                    defined,
                ),
            ] {
                let floor = budget.storage();
                let mut builder = SourceReferenceBuilderV29::new_with_storage(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                )?;
                builder.effect_site = active;
                construction_refused(builder.assign_constructed(
                    site,
                    SourceReferenceConstructionV29::Intrinsic(call),
                    budget,
                ));
                construction_no_effects(&builder);
                drop(builder);
                budget.release_storage(budget.storage() - floor)?;
            }
            let helpers = instances
                .instances()
                .iter()
                .enumerate()
                .filter(|(_, row)| row.function().index() == 3)
                .map(|(ordinal, _)| instances.id_at(ordinal).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(helpers.len(), 2);
            let declaration = instances.instance(helpers[0]).unwrap().declaration();
            assert!(std::ptr::eq(
                declaration,
                instances.instance(helpers[1]).unwrap().declaration()
            ));
            let SemanticStatementKindV1::Assign(assignment) =
                declaration.blocks()[0].statements()[5].kind()
            else {
                panic!("checked source");
            };
            let cloned = assignment.clone();
            let original = SourceReferenceSiteV29 {
                instance: helpers[0],
                block: SemanticBlockIdV1::from_index(0),
                statement: Some(5),
            };
            for (active, site, assignment) in [
                (None, original, assignment),
                (
                    Some(original),
                    SourceReferenceSiteV29 {
                        instance: helpers[1],
                        ..original
                    },
                    assignment,
                ),
                (
                    Some(original),
                    SourceReferenceSiteV29 {
                        statement: None,
                        ..original
                    },
                    assignment,
                ),
                (Some(original), original, &cloned),
            ] {
                let floor = budget.storage();
                let mut builder = SourceReferenceBuilderV29::new_with_storage(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                )?;
                builder.effect_site = active;
                construction_refused(builder.assign_constructed(
                    site,
                    SourceReferenceConstructionV29::CheckedBinary(assignment),
                    budget,
                ));
                construction_no_effects(&builder);
                drop(builder);
                budget.release_storage(budget.storage() - floor)?;
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn construction_rejects_reconstructed_call_rosters_and_equal_fresh_sources() {
    construction_with_instances(
        scoped_root_tests::fixtures::checked_slot_owner(),
        |instances, budget| {
            let original = instances
                .calls(instances.root())
                .unwrap()
                .iter()
                .find(|call| call.child().is_none())
                .unwrap();
            let site = SourceReferenceSiteV29 {
                instance: instances.root(),
                block: original.occurrence().block,
                statement: None,
            };
            let reject = |other: &ExecutionInstancesV29<'_>,
                              budget: &mut ArgumentBudgetV1<'_>| {
                let row = other
                    .calls(other.root())
                    .unwrap()
                    .iter()
                    .find(|call| call.child().is_none())
                    .unwrap();
                assert!(!std::ptr::eq(original, row));
                assert_eq!(original.occurrence(), row.occurrence());
                let floor = budget.storage();
                let mut builder = SourceReferenceBuilderV29::new_with_storage(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                )?;
                builder.effect_site = Some(site);
                construction_refused(builder.assign_constructed(
                    site,
                    SourceReferenceConstructionV29::Intrinsic(row),
                    budget,
                ));
                construction_no_effects(&builder);
                drop(builder);
                budget.release_storage(budget.storage() - floor)?;
                Ok::<_, ProductionSemanticKirErrorV1>(())
            };
            with_production_call_instances_v1(instances.owner(), ROOT, budget, |other, budget| {
                let row = other
                    .calls(other.root())
                    .unwrap()
                    .iter()
                    .find(|call| call.child().is_none())
                    .unwrap();
                assert!(std::ptr::eq(original.source(), row.source()));
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(reject(
                    other, budget,
                ))
            })
            .unwrap()?;
            let mut fresh = scoped_root_tests::fixtures::checked_slot_owner();
            assert_eq!(
                fresh.source_semantic_sha256(),
                instances.owner().source_semantic_sha256()
            );
            let receipt = fresh
                .try_capture_occurrences_with_budget_v1(budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            with_production_call_instances_v1(&fresh, ROOT, budget, |other, budget| {
                let row = other
                    .calls(other.root())
                    .unwrap()
                    .iter()
                    .find(|call| call.child().is_none())
                    .unwrap();
                assert!(!std::ptr::eq(original.source(), row.source()));
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(reject(
                    other, budget,
                ))
            })
            .unwrap()?;
            drop(fresh);
            budget.release_storage(receipt.retained_storage())?;
            Ok(())
        },
    )
    .unwrap();
}

fn construction_cleanup_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = lifecycle_owner(false);
    let source = original.source_semantic();
    let mut functions = source.functions().to_vec();
    let helper = &functions[HELPER.index() as usize];
    let mut blocks = helper.blocks().to_vec();
    let SemanticTerminatorKindV1::Call(call) = blocks[0].terminator().kind() else {
        panic!("derive");
    };
    assert!(matches!(call.arguments()[0], SemanticOperandV1::Move(_)));
    let cleanup = SemanticDirectCallV1::new_callable(
        call.callee(),
        call.arguments().to_vec(),
        call.destination().cloned(),
        SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallUnwind,
            SemanticBlockIdV1::from_index(blocks.len() as u32),
        )),
    )
    .unwrap();
    blocks[0] = block(
        90,
        blocks[0].statements().to_vec(),
        SemanticTerminatorKindV1::Call(cleanup),
    );
    blocks.push(block(149, vec![], SemanticTerminatorKindV1::UnwindResume));
    functions[HELPER.index() as usize] = function(
        96,
        helper.role(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        blocks,
    );
    construction_rebuild(source, functions)
}

#[test]
fn construction_refuses_cleanup_before_evaluating_its_move_argument() {
    construction_with_instances(construction_cleanup_owner(), |instances, budget| {
        let helper = instances
            .calls(instances.root())
            .unwrap()
            .iter()
            .find_map(|call| call.child())
            .unwrap();
        let call = instances
            .calls(helper)
            .unwrap()
            .iter()
            .find(|call| call.source().callee() == DERIVE)
            .unwrap();
        assert!(matches!(
            call.source().unwind(),
            SemanticUnwindActionV1::Cleanup(_)
        ));
        assert!(matches!(
            call.source().arguments()[0],
            SemanticOperandV1::Move(_)
        ));
        let site = SourceReferenceSiteV29 {
            instance: helper,
            block: call.occurrence().block,
            statement: None,
        };
        let floor = budget.storage();
        let mut builder = SourceReferenceBuilderV29::new_with_storage(
            instances,
            SourceReferenceStorageV29::ScalarCells,
            budget,
        )?;
        builder.effect_site = Some(site);
        // No operand frame is installed: touching the Move would produce a
        // different source-state error before the exact constructor refusal.
        construction_refused(builder.assign_constructed(
            site,
            SourceReferenceConstructionV29::Intrinsic(call),
            budget,
        ));
        construction_no_effects(&builder);
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn construction_refuses_trap_with_an_original_uninhabited_destination() {
    let original = lifecycle_owner(false);
    let source = original.source_semantic();
    let mut types = source.types().to_vec();
    let never = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Empty,
            SemanticBackendReprV1::memory(true),
            None,
            true,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Never,
        None,
    );
    let trap_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([150; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        value_abi(&types, never),
    )
    .unwrap();
    let mut callables = source.callables().to_vec();
    let trap = SemanticCallableIdV1::from_index(callables.len() as u32);
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([150; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([150; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([150; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([150; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([150; 32]),
            super::source(),
            trap_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Trap,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([150; 32]),
    });
    let mut functions = source.functions().to_vec();
    let helper = &functions[HELPER.index() as usize];
    let mut locals = helper.locals().to_vec();
    let destination = place(locals.len() as u32, never);
    locals.push(local(151, never, SemanticLocalRoleV1::Temporary));
    let mut blocks = helper.blocks().to_vec();
    assert_eq!(blocks.len(), 3);
    assert!(matches!(
        blocks[2].terminator().kind(),
        SemanticTerminatorKindV1::Return
    ));
    // Keep the original derive/callback closure; only its final return gains
    // the Trap whose construction preflight is under test.
    blocks[2] = block(
        95,
        blocks[2].statements().to_vec(),
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                trap,
                vec![],
                Some(SemanticCallDestinationV1::new(
                    destination,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(3),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    blocks.push(block(152, vec![], SemanticTerminatorKindV1::Return));
    functions[HELPER.index() as usize] =
        function(96, helper.role(), helper.abi().clone(), locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    construction_with_instances(owner, |instances, budget| {
        let helper = instances
            .calls(instances.root())
            .unwrap()
            .iter()
            .find_map(|call| call.child())
            .unwrap();
        let call = instances
            .calls(helper)
            .unwrap()
            .iter()
            .find(|call| call.source().callee() == trap)
            .unwrap();
        assert!(
            instances.owner().source_semantic().types()[never.index() as usize]
                .layout()
                .is_uninhabited()
        );
        let site = SourceReferenceSiteV29 {
            instance: helper,
            block: call.occurrence().block,
            statement: None,
        };
        let floor = budget.storage();
        let mut builder = SourceReferenceBuilderV29::new_with_storage(
            instances,
            SourceReferenceStorageV29::ScalarCells,
            budget,
        )?;
        builder.effect_site = Some(site);
        construction_refused(builder.assign_constructed(
            site,
            SourceReferenceConstructionV29::Intrinsic(call),
            budget,
        ));
        construction_no_effects(&builder);
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    })
    .unwrap();
}

fn construction_resource(error: ProductionSemanticKirErrorV1) -> ArgumentResourceV1 {
    let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) = error else {
        panic!("expected resource refusal: {error:?}");
    };
    error
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConstructionBoundary {
    Exact,
    WorkShort,
    StorageShort,
}

fn construction_resource_case(intrinsic: bool, boundary: ConstructionBoundary, prior: bool) {
    let mut owner = scoped_root_tests::fixtures::checked_slot_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(CONSTRUCTION_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, CONSTRUCTION_LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let caller_floor = budget.storage();
    let expected_work_failure = with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        let mut builder = SourceReferenceBuilderV29::new_with_storage(instances, SourceReferenceStorageV29::ScalarCells, budget).unwrap();
        let helpers = instances.instances().iter().enumerate().find(|(_, row)| row.function().index() == 3).unwrap().0;
        let helper = instances.id_at(helpers).unwrap();
        let declaration = instances.instance(helper).unwrap().declaration();
        let SemanticStatementKindV1::Assign(assignment) = declaration.blocks()[0].statements()[5].kind() else { panic!("checked producer"); };
        let clone = assignment.clone();
        let defined = instances.calls(instances.root()).unwrap().iter().find(|call| call.child().is_some()).unwrap();
        let site = if intrinsic {
            SourceReferenceSiteV29 { instance: instances.root(), block: defined.occurrence().block, statement: None }
        } else {
            SourceReferenceSiteV29 { instance: helper, block: SemanticBlockIdV1::from_index(0), statement: Some(5) }
        };
        builder.effect_site = Some(site);
        // Independent schedule: custody5, producer validation8, then only the
        // intrinsic branch scans its original caller's exact call-row roster.
        let required = 5 + 8 + if intrinsic { instances.calls(site.instance).unwrap().len() } else { 0 };
        let word = std::mem::size_of::<usize>();
        assert_eq!(std::mem::size_of::<SourceReferenceConstructionV29<'_, '_>>(), 2 * word);
        assert_eq!(std::mem::size_of::<(&SemanticPlaceV1, usize)>(), 2 * word);
        let headers = 4 * word + std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
        assert_eq!(source_reference_construction_headers_v29().unwrap(), headers);
        let remaining_work = required - usize::from(boundary == ConstructionBoundary::WorkShort);
        budget.charge_work(CONSTRUCTION_LIMIT - budget.work() - remaining_work).unwrap();
        let remaining_storage = headers - usize::from(boundary == ConstructionBoundary::StorageShort);
        budget.reserve_storage(CONSTRUCTION_LIMIT - budget.storage() - remaining_storage).unwrap();
        let start = (budget.work(), budget.storage());
        if prior {
            assert!(budget.charge_work(remaining_work + 33).is_err());
            assert!(budget.reserve_storage(remaining_storage + 29).is_err());
            assert!(builder.plan.failure.first_error().is_none());
        }
        let invoke = |builder: &mut SourceReferenceBuilderV29<'_, '_, '_>, budget: &mut ArgumentBudgetV1<'_>| {
            let construction = if intrinsic { SourceReferenceConstructionV29::Intrinsic(defined) }
                else { SourceReferenceConstructionV29::CheckedBinary(&clone) };
            builder.assign_constructed(site, construction, budget)
        };
        let result = invoke(&mut builder, budget);
        let first = if boundary == ConstructionBoundary::Exact {
            construction_refused(result);
            assert!(builder.plan.failure.first_error().is_none());
            assert_eq!((budget.work(), budget.storage()), (start.0 + required, start.1 + headers));
            None
        } else {
            let error = construction_resource(result.unwrap_err());
            match boundary {
                ConstructionBoundary::WorkShort => {
                    assert!(matches!(error, ArgumentResourceV1::Work(error) if error.actual() == CONSTRUCTION_LIMIT + 1 && error.limit() == CONSTRUCTION_LIMIT));
                    let last = if intrinsic { instances.calls(site.instance).unwrap().len() } else { 8 };
                    assert_eq!((budget.work(), budget.storage()), (start.0 + required - last, start.1 + headers));
                }
                ConstructionBoundary::StorageShort => {
                    assert!(matches!(error, ArgumentResourceV1::Storage(error) if error.actual() == CONSTRUCTION_LIMIT + 1 && error.limit() == CONSTRUCTION_LIMIT));
                    assert_eq!((budget.work(), budget.storage()), (start.0 + 5, start.1));
                }
                ConstructionBoundary::Exact => unreachable!(),
            }
            assert_eq!(construction_resource(builder.plan.failure.first_error().unwrap()), error);
            let unchanged = (budget.work(), budget.storage(), budget.peak_storage(), budget.failed_storage());
            assert_eq!(construction_resource(invoke(&mut builder, budget).unwrap_err()), error);
            assert_eq!((budget.work(), budget.storage(), budget.peak_storage(), budget.failed_storage()), unchanged);
            Some(error)
        };
        construction_no_effects(&builder);
        let expected_storage = if prior { Some(CONSTRUCTION_LIMIT + 29) }
            else if boundary == ConstructionBoundary::StorageShort { Some(CONSTRUCTION_LIMIT + 1) } else { None };
        assert_eq!(budget.failed_storage(), expected_storage);
        assert_eq!(first.is_some(), boundary != ConstructionBoundary::Exact);
        drop(builder);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), expected_storage);
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(if prior { Some(CONSTRUCTION_LIMIT + 33) }
            else if boundary == ConstructionBoundary::WorkShort { Some(CONSTRUCTION_LIMIT + 1) } else { None })
    }).unwrap();
    assert_eq!(budget.storage(), caller_floor);
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    drop(budget);
    assert_eq!(work.failed_work(), expected_work_failure);
}

#[test]
fn construction_preflight_has_independent_exact_and_one_short_resources_and_sticky_failure() {
    for intrinsic in [false, true] {
        for boundary in [
            ConstructionBoundary::Exact,
            ConstructionBoundary::WorkShort,
            ConstructionBoundary::StorageShort,
        ] {
            construction_resource_case(intrinsic, boundary, false);
        }
    }
}

#[test]
fn construction_preflight_preserves_prior_global_denials_without_confusing_its_own_failure() {
    for intrinsic in [false, true] {
        for boundary in [
            ConstructionBoundary::Exact,
            ConstructionBoundary::WorkShort,
            ConstructionBoundary::StorageShort,
        ] {
            construction_resource_case(intrinsic, boundary, true);
        }
    }
}
