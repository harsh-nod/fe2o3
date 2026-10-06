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

fn payload_refusal(result: Result<()>) -> Result<()> {
    match result {
        Err(Error::Statement(
            "execution relation differs from its source producer or actual scope slot",
        )) => Ok(()),
        Err(error) => Err(error),
        Ok(()) => panic!("non-payload or borrowed payload admitted without its relation"),
    }
}

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
    let payload_frame = size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<Result<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>>()
        + size_of::<Owner>()
        + size_of::<Identity>()
        + 2 * size_of::<Site>()
        + size_of::<execution_loans::ExecutionOperand>()
        + size_of::<Option<execution_loans::ExecutionOperand>>()
        + size_of::<Result<()>>()
        + 12 * size_of::<usize>();
    let issue_frame = size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<Owner>()
        + size_of::<Site>()
        + size_of::<Definition>()
        + size_of::<Option<usize>>()
        + size_of::<Result<()>>()
        + 10 * size_of::<usize>();
    let issue_segment_frame = size_of::<super::super::source_function::ContextIssueSiteV222>()
        + size_of::<Site>()
        + size_of::<Owner>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<Result<()>>()
        + 12 * size_of::<usize>()
        + 12 * size_of::<&()>();
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
            + payload_frame
            + issue_frame
            + issue_segment_frame
    );
    bindings.check_owner(plan, slots, &target, out)?;
    let source = plan.source(out)?;
    let semantic = source.source_semantic(out.budget)?;
    let archive = source.source_ssa(out.budget)?;
    let root = plan.root(0, out)?;
    let mut seen = BTreeSet::new();
    let (mut owned, mut borrowed, mut payloads, mut payload_leases, mut issues) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut transported_contexts = 0;
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
                    let end = out.text.len();
                    payload_refusal(
                        bindings.emit_payload_lease_conjunct_v209(0, instance, value, out),
                    )?;
                    assert_eq!(out.text.len(), end);
                    if !is_borrowed && role == SourceRole::KernelContext {
                        let endpoint = slots
                            .correspondence(out)?
                            .ssa_typed_endpoint_v36(0, instance, value, out.budget)?;
                        let owner = endpoint.execution_owner_v199(out.budget)?.unwrap();
                        let site = bindings.site(0, owner.identity, KirRole::Context, out)?;
                        let local = endpoint.source_local(out.budget)?;
                        if owner.identity.instance == instance
                            && owner.identity.destination == local
                        {
                            bindings.emit_context_issue_step_v211(0, instance, value, out)?;
                            assert_eq!(
                                &out.text[end..],
                                format!(
                                    "invocation_context_issue_coupled_v211(source, target, execution_map, InvocationSourceContextIssueV161 {{ destination: {}, source_type: {} }}, MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}, {})",
                                    row.locals.start + owner.identity.destination.index() as usize,
                                    owner.identity.source_type.index(),
                                    site.operation.block.function.0,
                                    site.operation.block.block,
                                    site.operation.operation,
                                    site.definition
                                )
                            );
                            issues += 1;
                        } else {
                            payload_refusal(
                                bindings.emit_context_issue_step_v211(0, instance, value, out),
                            )?;
                            assert_eq!(out.text.len(), end);
                            transported_contexts += 1;
                        }
                    } else {
                        payload_refusal(
                            bindings.emit_context_issue_step_v211(0, instance, value, out),
                        )?;
                        assert_eq!(out.text.len(), end);
                    }
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
                    if is_borrowed {
                        payload_refusal(
                            bindings.emit_payload_lease_conjunct_v209(0, instance, value, out),
                        )?;
                        assert_eq!(out.text.len(), before);
                    } else {
                        let endpoint = slots
                            .correspondence(out)?
                            .ssa_typed_endpoint_v36(0, instance, value, out.budget)?;
                        let owner = endpoint.execution_owner_v199(out.budget)?.unwrap();
                        let workgroup = owner.workgroup.unwrap();
                        let original = execution_loans::call_argument(
                            slots,
                            plan,
                            0,
                            workgroup.instance,
                            workgroup.block.index() as usize,
                            0,
                            out,
                        )?
                        .unwrap();
                        let context = bindings.site(0, owner.context, KirRole::Context, out)?;
                        let local =
                            row.locals.start + endpoint.source_local(out.budget)?.index() as usize;
                        bindings.emit_payload_lease_conjunct_v209(0, instance, value, out)?;
                        let text = &out.text[before..];
                        let recipe = original.recipe;
                        assert!(original.moved && recipe.mutable && recipe.role == Role::Context);
                        assert!(text.starts_with("invocation_execution_payload_related_v209(source, target, execution_map, "));
                        assert!(text.contains(&format!(
                            "local: {local}, source_type: {}",
                            owner.identity.source_type.index()
                        )));
                        assert!(
                            text.contains(&format!("context_definition: {}", context.definition))
                        );
                        assert!(text.contains(&format!(
                            "lease_recipe: InvocationSourceExecutionRecipeV168 {{ reference_type: {}, source_type: {}, role: InvocationSourceExecutionRoleV168::Context, mutable: true, instance: {}, block: {}, statement: {} }}",
                            recipe.reference_type, recipe.source_type, recipe.instance, recipe.block,
                            recipe.statement)));
                        payload_leases += 1;
                    }
                    payloads += 1;
                }
            }
        }
    }
    assert!(owned > 0 && borrowed > 0 && payloads > 0 && payload_leases > 0 && issues > 0);
    assert!(
        transported_contexts > 0,
        "transported Context is not a fresh issuance"
    );
    assert!(SHARED.contains("scope.parent == Some(context.identity)"));
    assert!(SHARED.contains("context.child == Some(scope.identity)"));
    assert!(SHARED.contains("invocation_source_execution_lease_current_v170"));
    assert!(SHARED.contains("binding.lease_recipe == Some(lease.recipe)"));
    assert!(!SHARED.contains("origin_version == scope.identity.epoch"));
    assert!(
        SHARED.contains("let source_next = invocation_source_context_issue_v161(source, issue);")
    );
    assert!(
        SHARED.contains(
            "let target_next = byte_execution_step_v178(target, site, 0, destination, -1);"
        )
    );
    assert!(SHARED.contains("version: source_next.source.logical.versions[issue.destination]"));
    assert!(
        SHARED
            .contains("frame: source.machine.frames.active.last(), source_type: issue.source_type")
    );
    assert!(SHARED.contains("invocation_context_issue_fresh_has_exact_updates_v211"));
    Ok(())
}

#[test]
fn expanded_transition_support_uses_actual_steps_and_input_only_guards() {
    let source = include_str!("original_semantic_mir_expanded_transition_v259.vrs");
    assert_eq!(source.matches("proof fn ").count(), 4);
    let frame = source
        .split_once("proof fn invocation_context_issue_fresh_preserves_frame_v259(")
        .unwrap()
        .1
        .split_once("\n{\n")
        .unwrap()
        .0;
    let requires = frame
        .split_once("    requires ")
        .unwrap()
        .1
        .split_once("    ensures ")
        .unwrap()
        .0;
    assert_eq!(
        requires,
        "invocation_context_issue_fresh_enabled_v211(source, target, issue, site, destination),\n        invocation_execution_map_current_v205(source, target, execution_map),\n"
    );
    for exact in [
        "values: source.machine.values.update(issue.destination, MemoryValueV30::Undefined)",
        "values: target.values.update(destination, next.target.values[destination])",
        "after.slots == source.slots && after.objects == source.objects",
        "next.source.observations == Seq::empty()",
        "0 <= dependency < target.values.len(), dependency != destination",
        "invocation_source_checked_pair_v44(",
        "let after = invocation_source_checked_v42(source, destination, source_type,",
        "0, 32, false, left_operand, right_operand, root, instance, little_endian)",
        "invocation_source_byte_evaluate_v36(source, left_operand, 32, root, instance, little_endian)",
        "invocation_source_byte_evaluate_v36(source, right_operand, 32, root, instance, little_endian)",
        "invocation_source_aggregate_complete_v42(aggregate)",
    ] {
        assert!(
            source.contains(exact),
            "missing actual transition fact: {exact}"
        );
    }
    for forbidden in ["assume(", "admit(", "external_body", "assume_specification"] {
        assert!(!source.contains(forbidden));
    }
}

#[test]
fn expanded_context_fresh_preservation_keeps_input_guards_and_all_witness_rows() {
    let source = include_str!("original_semantic_mir_context_issue_coupling_v211.vrs");
    let (_, law) = source
        .split_once("proof fn invocation_context_issue_fresh_preserves_current_map_v238(")
        .unwrap();
    let (header, body) = law.split_once("\n{\n").unwrap();
    let (_, contract) = header.split_once("    requires ").unwrap();
    let (requires, ensures) = contract.split_once("    ensures ").unwrap();
    assert_eq!(requires, "invocation_context_issue_fresh_enabled_v211(source, target, issue, site, destination),\n        invocation_execution_map_current_v205(source, target, execution_map),\n");
    assert!(ensures.contains("next.updated && invocation_execution_map_current_v205(\n            next.source.source, next.target, next.execution_map)"));
    assert!(body.contains("source, target, execution_map, issue, site, destination"));
    assert!(body.contains("execution_map[key].local != issue.destination"));
    assert!(body.contains("key.definition != destination"));
    assert!(body.contains("lease.origin != issue.destination"));
    assert!(body.contains("next.execution_map[key] == execution_map[key]"));
    assert!(body.contains("next.execution_map[left].frame == next.execution_map[right].frame"));
    assert!(source.contains("source.logical.execution_references == Map::empty()"));
    assert!(source.contains("target.values[destination] == MemoryValueV30::Undefined"));
    for forbidden in ["assume(", "admit(", "external_body", "assume_specification"] {
        assert!(!law.contains(forbidden));
    }
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
fn expanded_execution_payload_lease_support_binds_actual_context_origin() {
    let source = include_str!("original_semantic_mir_expanded_payload_lease_v209.vrs");
    for guard in [
        "source.logical.aggregates[binding.local].source_type == binding.source_type",
        "invocation_source_execution_aggregate_current_v170(source, binding.local)",
        "lease.recipe == binding.lease_recipe",
        "context.site == binding.context_site",
        "execution_map.contains_key(context.identity)",
        "== invocation_execution_reference_origin_v205(lease)",
    ] {
        assert!(
            source.contains(guard),
            "missing payload parent guard: {guard}"
        );
    }
    for law in [
        "wrong_recipe_is_not_related",
        "wrong_context_origin_is_not_related",
        "missing_context_is_not_related",
        "relation_keeps_source_lease_current",
    ] {
        assert!(source.contains(&format!(
            "proof fn invocation_execution_payload_{law}_v209("
        )));
    }
    assert_eq!(source.matches("proof fn ").count(), 4);
    for invalid in [
        "assume(",
        "admit(",
        "external_body",
        "epoch == lease.origin_version",
    ] {
        assert!(!source.contains(invalid));
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
