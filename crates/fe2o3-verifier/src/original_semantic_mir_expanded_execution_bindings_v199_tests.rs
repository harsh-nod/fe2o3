use super::super::source_function::tile_fixture_tests::run_fixture_with_plan;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ExecutionTileLayoutV1 as Layout,
};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticRustTypeKindV1 as RustType, SemanticTypeShapeV1 as Shape,
};
use fe2o3_mir_model::{SsaBlockIdV1 as Block, SsaEdgeIdV1 as Edge, SsaResolvedEventV1 as Event};
use std::collections::BTreeSet;

const LIMIT: usize = 200_000_000;

fn exercise(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let target = TileTargetV176::derive(slots, out)?;
    let before = out.budget.storage();
    let bindings = ExpandedExecutionBindingsV199::derive(plan, slots, &target, out)?;
    assert_eq!(
        size_of::<ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_>>(),
        3 * size_of::<&()>() + size_of::<usize>()
    );
    let retained = 3 * size_of::<&()>() + size_of::<usize>();
    let results = 2 * size_of::<Result<ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_>>>();
    let producers = 2 * size_of::<Owner>();
    let site_alignment = std::mem::align_of::<usize>().max(std::mem::align_of::<Operation>());
    let sites = 4 * (size_of::<usize>() + size_of::<Operation>()).next_multiple_of(site_alignment);
    let loans =
        2 * size_of::<Option<Recipe>>() + size_of::<Option<execution_loans::ExecutionOperand>>();
    let coordinates = 2 * size_of::<Definition>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>();
    let outer_frame = size_of::<&ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_>>()
        + 2 * size_of::<usize>()
        + size_of::<Value>()
        + size_of::<&mut Writer<'_, '_>>()
        + size_of::<Result<()>>();
    assert_eq!(
        out.budget.storage() - before,
        retained
            + results
            + producers
            + sites
            + loans
            + coordinates
            + 32 * size_of::<usize>()
            + size_of::<bool>()
            + size_of::<&str>()
            + outer_frame
    );
    bindings.check_owner(plan, slots, &target, out)?;
    let source = plan.source(out)?;
    let semantic = source.source_semantic(out.budget)?;
    let archive = source.source_ssa(out.budget)?;
    let root = plan.root(0, out)?;
    let mut seen = BTreeSet::new();
    let (mut owned, mut borrowed, mut payloads) = (0usize, 0usize, 0usize);
    for instance in 0..root.instances.len() {
        let row = plan.instance(0, instance, out)?;
        if !row.active {
            continue;
        }
        let function = &semantic.functions()[row.function.index() as usize];
        let ssa = archive.plan_for_function(row.function).unwrap().plan();
        let mut values = Vec::new();
        values.extend(
            ssa.entry_definitions()
                .iter()
                .map(|entry| (entry.variable(), entry.value())),
        );
        for (block, declaration) in function.blocks().iter().enumerate() {
            let block = Block::new(block as u32);
            if let Some(events) = ssa.resolved_events(block) {
                for (_, event) in events {
                    if let Event::Define { variable, value } = event {
                        values.push((*variable, *value));
                    }
                }
            }
            for ordinal in 0..declaration.terminator().kind().edge_count() {
                if let Some(entries) = ssa.edge_definitions(Edge::new(block, ordinal as u32)) {
                    values.extend(
                        entries
                            .iter()
                            .map(|entry| (entry.variable(), entry.value())),
                    );
                }
            }
        }
        for (variable, value) in values {
            let ty = function.locals()[variable.get() as usize].ty();
            let declaration = &semantic.types()[ty.index() as usize];
            let referenced = match declaration.shape() {
                Shape::Pointer(pointer) => Some(pointer.pointee()),
                _ => None,
            };
            let role = if let RustType::Execution(role) = declaration.rust_type_kind() {
                Some((role, false))
            } else if let Some(pointee) = referenced {
                match semantic.types()[pointee.index() as usize].rust_type_kind() {
                    RustType::Execution(role) => Some((role, true)),
                    _ => None,
                }
            } else {
                None
            };
            let Some((role, is_borrowed)) = role else {
                continue;
            };
            if !seen.insert((instance, value)) {
                continue;
            }
            let before = out.text.len();
            match role {
                SourceRole::KernelContext | SourceRole::Workgroup => {
                    bindings.emit_conjunct(0, instance, value, out)?;
                    let text = &out.text[before..];
                    assert!(text.starts_with("invocation_execution_related_v199(source, target, "));
                    assert!(text.contains(&format!("source_owner: {}", row.function.index())));
                    assert!(text.contains(if is_borrowed {
                        ", recipe: Some(InvocationSourceExecutionRecipeV168"
                    } else {
                        ", recipe: None"
                    }));
                    assert!(text.contains(if role == SourceRole::Workgroup {
                        ", lease_recipe: Some(InvocationSourceExecutionRecipeV168"
                    } else {
                        ", lease_recipe: None"
                    }));
                    if role == SourceRole::Workgroup {
                        let endpoint = slots
                            .correspondence(out)?
                            .ssa_typed_endpoint_v36(0, instance, value, out.budget)?;
                        let owner = endpoint.execution_owner_v199(out.budget)?.unwrap();
                        let original = execution_loans::call_argument(
                            slots,
                            plan,
                            0,
                            owner.identity.instance,
                            owner.identity.block.index() as usize,
                            0,
                            out,
                        )?
                        .unwrap();
                        assert!(original.moved && original.recipe.mutable);
                        assert_eq!(original.recipe.role, Role::Context);
                        let recipe = original.recipe;
                        assert!(out.text[before..].contains(&format!(
                            ", lease_recipe: Some(InvocationSourceExecutionRecipeV168 {{ reference_type: {}, source_type: {}, role: InvocationSourceExecutionRoleV168::Context, mutable: true, instance: {}, block: {}, statement: {} }})",
                            recipe.reference_type, recipe.source_type, recipe.instance, recipe.block,
                            recipe.statement)));
                    }
                    if !is_borrowed && role == SourceRole::KernelContext {
                        let endpoint = slots
                            .correspondence(out)?
                            .ssa_typed_endpoint_v36(0, instance, value, out.budget)?;
                        let owner = endpoint.execution_owner_v199(out.budget)?.unwrap();
                        for identity in [
                            Identity {
                                source_type:
                                    fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1::from_index(
                                        u32::MAX,
                                    ),
                                ..owner.identity
                            },
                            Identity {
                                block:
                                    fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(
                                        u32::MAX,
                                    ),
                                ..owner.identity
                            },
                            Identity {
                                destination:
                                    fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                                        u32::MAX,
                                    ),
                                ..owner.identity
                            },
                        ] {
                            assert!(matches!(
                                bindings.site(0, identity, KirRole::Context, out),
                                Err(Error::Statement(
                                    "execution relation differs from its source producer or actual scope slot"
                                ))
                            ));
                        }
                    }
                    let end = out.text.len();
                    bindings.emit_mapped_conjunct_v205(0, instance, value, out)?;
                    let original = out.text[before..end]
                        .strip_prefix("invocation_execution_related_v199(source, target, ")
                        .unwrap();
                    let mapped = out.text[end..]
                        .strip_prefix(
                            "invocation_execution_mapped_v205(source, target, execution_map, ",
                        )
                        .unwrap();
                    assert_eq!(original, mapped);
                    if is_borrowed {
                        borrowed += 1;
                    } else {
                        owned += 1;
                    }
                }
                SourceRole::MaskedTileU32 { .. } | SourceRole::LaneFragmentU32 { .. } => {
                    match bindings.emit_conjunct(0, instance, value, out) {
                        Err(Error::Statement(
                            "tile payload needs its separate aggregate leaf relation",
                        )) => (),
                        Err(error) => return Err(error),
                        Ok(()) => panic!("payload silently treated as a scope token"),
                    }
                    assert_eq!(out.text.len(), before);
                    match bindings.emit_mapped_conjunct_v205(0, instance, value, out) {
                        Err(Error::Statement(
                            "tile payload needs its separate aggregate leaf relation",
                        )) => (),
                        Err(error) => return Err(error),
                        Ok(()) => panic!("execution map silently admitted a tile payload"),
                    }
                    assert_eq!(out.text.len(), before);
                    payloads += 1;
                }
            }
        }
    }
    assert!(owned > 0 && borrowed > 0 && payloads > 0);
    assert!(SHARED.contains("scope.parent == Some(context.identity)"));
    assert!(SHARED.contains("context.child == Some(scope.identity)"));
    assert!(SHARED.contains("invocation_source_execution_lease_current_v170"));
    assert!(SHARED.contains("binding.lease_recipe == Some(lease.recipe)"));
    assert!(!SHARED.contains("origin_version == scope.identity.epoch"));
    Ok(())
}

#[test]
fn expanded_execution_map_support_keeps_dynamic_identity_and_history_separate() {
    let source = include_str!("original_semantic_mir_execution_correspondence_v205.vrs");
    assert!(source.contains(
        "execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>"
    ));
    for guard in [
        "source.logical.versions[origin.local] == origin.version",
        "source.machine.frames.active.contains(origin.frame)",
        "capability.identity == identity",
        "execution_map[scope.identity] == origin",
        "== invocation_execution_reference_origin_v205(lease)",
        "invocation_execution_related_v199(source, target, binding)",
    ] {
        assert!(
            source.contains(guard),
            "missing dynamic identity guard: {guard}"
        );
    }
    for law in [
        "empty_map_grants_no_binding",
        "stale_source_version_is_not_current",
        "departed_source_frame_is_not_current",
        "stale_target_identity_is_not_an_entry",
        "wrong_context_lease_is_not_related",
        "mapped_binding_keeps_original_conjunct",
    ] {
        assert!(source.contains(&format!("proof fn invocation_execution_{law}_v205(")));
    }
    assert_eq!(source.matches("proof fn ").count(), 6);
    assert_eq!(source.matches("forall|").count(), 2);
    assert_eq!(source.matches("#![trigger ").count(), 2);
    for unsupported in [
        "assume(",
        "admit(",
        "external_body",
        "Map::new",
        "epoch == origin.version",
    ] {
        assert!(!source.contains(unsupported));
    }
}

#[test]
fn expanded_execution_bindings_rejoin_original_owners_and_actual_scope_slots() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture_with_plan(layout, LIMIT, LIMIT, |plan, slots, _, out| {
            exercise(plan, slots, out)
        })
        .0
        .unwrap();
    }
}

#[test]
fn expanded_execution_bindings_have_exact_and_one_short_resources() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let run = |work, storage| {
            run_fixture_with_plan(layout, work, storage, |plan, slots, _, out| {
                exercise(plan, slots, out)
            })
        };
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        run(measured.1, measured.3).0.unwrap();
        assert!(matches!(run(measured.1 - 1, measured.3).0,
            Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
        assert!(matches!(run(measured.1, measured.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
    }
}

#[test]
fn expanded_execution_bindings_require_the_retained_actual_target_owner() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let result = run_fixture_with_plan(layout, LIMIT, LIMIT, |plan, slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let distinct = TileTargetV176::derive(slots, out)?;
            let bindings = ExpandedExecutionBindingsV199::derive(plan, slots, &target, out)?;
            bindings.check_owner(plan, slots, &target, out)?;
            bindings.check_owner(plan, slots, &distinct, out)
        })
        .0;
        assert!(matches!(
            result,
            Err(Error::Statement(
                "execution relation differs from its source producer or actual scope slot"
            ))
        ));
    }
}

#[test]
fn expanded_execution_bindings_keep_foreign_and_refunded_accounts_closed() {
    for foreign in [false, true] {
        let mut reached = false;
        let result = run_fixture_with_plan(Layout::Blocked, LIMIT, LIMIT, |plan, slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let bindings = ExpandedExecutionBindingsV199::derive(plan, slots, &target, out)?;
            reached = true;
            let error = if foreign {
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut writer = Writer::new(&mut budget)?;
                bindings.check(&mut writer).unwrap_err()
            } else {
                out.budget.release_storage(1)?;
                bindings.check(out).unwrap_err()
            };
            assert!(matches!(
                error,
                Error::Resource(Resource::Accounting)
                    | Error::Source(SourceError::Resource(Resource::Accounting))
            ));
            assert!(bindings.check(out).is_err());
            Err(error)
        });
        assert!(reached && result.0.is_err());
    }
}
