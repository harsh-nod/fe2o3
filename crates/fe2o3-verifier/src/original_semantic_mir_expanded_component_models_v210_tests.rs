//! Complete generated component predicates, not an admitted paired step proof.
use super::super::super::{
    byte_bindings::SourceByteBindings,
    expanded_execution::ExpandedExecutionBindingsV199,
    paired::ExpandedScalarBindingsV196,
    slots::SourceTagPairsV40,
    tile_target::{TileMicroCutsV180, TileTargetV176},
};
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::target_view_contracts_v38::TargetByteViewContractsV38 as TargetContracts;
use fe2o3_kernel_ir::{ExecutionRoleV15, FormalIndexWidth, Type};
use fe2o3_mir_model::{SsaBlockIdV1 as Block, SsaEdgeIdV1 as Edge, SsaResolvedEventV1 as Event};
use std::fmt::Write as _;

const LIMIT: usize = 512 * 1024 * 1024;

fn generate(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    tile: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let width = FormalIndexWidth::Bits64;
    let relation = slots.correspondence(out)?;
    let original = TargetContracts::derive(relation.inventory(out.budget)?, width, out)?;
    let tags = SourceTagPairsV40::derive(slots, &original, out)?;
    let target = TileTargetV176::derive(slots, out)?;
    let contracts = TargetContracts::derive(target.inventory(out)?, width, out)?;
    let cuts = TileMicroCutsV180::derive(&target, plan, out)?;
    let bytes = SourceByteBindings::derive_expanded_v188(&target, out)?;
    let values = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
    let execution = ExpandedExecutionBindingsV199::derive(plan, slots, &target, out)?;
    super::super::super::emit_model_prelude_v187(out)?;
    slots.emit_source_tag_contracts(0, out)?;
    tags.emit_expanded_v190(&target, &contracts, width, 0, 1, out)?;
    slots.emit(out)?;
    super::generate_actual_tile_source_v168(plan, slots, tile, out)?;
    bytes.emit(out)?;
    writeln!(out, "spec fn invocation_runtime_index_bytes_v36() -> int {{ 8 }}\nspec fn invocation_runtime_little_endian_v36() -> bool {{ true }}")
        .map_err(|_| out.error())?;
    target.emit(width, out)?;
    cuts.emit(out)?;

    let source = relation.source(out.budget)?;
    let archive = source.source_ssa(out.budget)?;
    let semantic = source.source_semantic(out.budget)?;
    let launches = source.source_launch(out.budget)?;
    let (mut scalars, mut leaves, mut scopes, mut payloads, mut issues) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for root in 0..source.root_count(out.budget)? {
        let (function, _) = source.root(root, out.budget)?;
        let launch = launches
            .roots()
            .iter()
            .find(|launch| launch.selected_root() == function)
            .expect("fixture has an authenticated source launch");
        let [x, y, z] = launch.layout().global_extents();
        assert!(x > 0 && y > 0 && z > 0, "fixture launch must be finite");
        writeln!(out, "spec fn invocation_runtime_launch_{root}_v36() -> (int, Seq<int>) {{ ({}, seq![{x}int, {y}int, {z}int]) }}", launch.source_rank())
            .map_err(|_| out.error())?;
        super::super::super::emit_execution_v37(relation, root, out)?;
        for instance in 0..plan.root(root, out)?.instances.len() {
            let row = plan.instance(root, instance, out)?;
            if !row.active {
                continue;
            }
            let ssa = archive.plan_for_function(row.function).unwrap().plan();
            let function = &semantic.functions()[row.function.index() as usize];
            let mut definitions = std::collections::BTreeSet::new();
            definitions.extend(ssa.entry_definitions().iter().map(|entry| entry.value()));
            for (block, declaration) in function.blocks().iter().enumerate() {
                let block = Block::new(block.try_into().unwrap());
                for (_, event) in ssa.resolved_events(block).into_iter().flatten() {
                    if let Event::Define { value, .. } = event {
                        definitions.insert(*value);
                    }
                }
                for edge in 0..declaration.terminator().kind().edge_count() {
                    definitions.extend(
                        ssa.edge_definitions(Edge::new(block, edge.try_into().unwrap()))
                            .into_iter()
                            .flatten()
                            .map(|entry| entry.value()),
                    );
                }
            }
            for (ordinal, value) in definitions.into_iter().enumerate() {
                let endpoint =
                    relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
                let ty = endpoint.physical_type(out.budget)?;
                let nominal = &semantic.types()[endpoint.source_type(out.budget)?.index() as usize];
                let role = match nominal.rust_type_kind() {
                    SemanticRustTypeKindV1::Execution(role) => Some(role),
                    _ => match nominal.shape() {
                        SemanticTypeShapeV1::Pointer(pointer) => {
                            match semantic.types()[pointer.pointee().index() as usize]
                                .rust_type_kind()
                            {
                                SemanticRustTypeKindV1::Execution(role) => Some(role),
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                };
                let scope = matches!(
                    role,
                    Some(
                        SemanticExecutionRoleV29::KernelContext
                            | SemanticExecutionRoleV29::Workgroup
                    )
                );
                let scalar = ssa
                    .entry_definitions()
                    .iter()
                    .any(|entry| entry.value() == value)
                    && matches!(
                        ty,
                        Some(Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_))
                    );
                let payload = matches!(
                    ty,
                    Some(Type::Execution(
                        ExecutionRoleV15::MaskedTileU32 { .. }
                            | ExecutionRoleV15::LaneFragmentU32 { .. }
                    ))
                ) && endpoint.execution_borrow_v163(out.budget)?.is_none();
                if !scalar && !payload && !scope {
                    continue;
                }
                // Each SSA definition gets a separate predicate. Conjoining
                // different dynamic cuts would incorrectly require dead values.
                write!(out, "spec fn expanded_source_component_{root}_{instance}_{ordinal}_v210(source: InvocationSourceByteStateV36, target: MemoryStateV30, map: InvocationByteMapV36, execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>) -> bool {{ true")
                    .map_err(|_| out.error())?;
                if scope {
                    write!(out, " && ").map_err(|_| out.error())?;
                    execution.emit_mapped_conjunct_v205(root, instance, value, out)?;
                    scopes += 1;
                } else if scalar {
                    values.emit_source_conjunct(plan, root, instance, value, width, out)?;
                    scalars += 1;
                } else {
                    write!(out, " && ").map_err(|_| out.error())?;
                    execution.emit_payload_lease_conjunct_v209(root, instance, value, out)?;
                    payloads += 1;
                    let ty = endpoint.source_type(out.budget)?;
                    let count = slots.aggregate_leaf_count(ty, out)?.unwrap();
                    for leaf in 0..count {
                        values.emit_source_leaf_conjunct(
                            plan, root, instance, value, leaf, width, out,
                        )?;
                        leaves += 1;
                    }
                }
                writeln!(out, " }}").map_err(|_| out.error())?;
                if matches!(
                    nominal.rust_type_kind(),
                    SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::KernelContext)
                ) && endpoint.execution_borrow_v163(out.budget)?.is_none()
                {
                    write!(out, "spec fn expanded_source_context_issue_{root}_{instance}_{ordinal}_v211(source: InvocationSourceByteStateV36, target: MemoryStateV30, execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>) -> InvocationContextIssueCoupledV211 {{ ")
                        .map_err(|_| out.error())?;
                    execution.emit_context_issue_step_v211(root, instance, value, out)?;
                    writeln!(out, " }}").map_err(|_| out.error())?;
                    issues += 1;
                }
            }
        }
    }
    assert!(scalars > 0 && leaves > 0 && scopes > 0 && payloads > 0 && issues > 0);
    super::super::super::paired::emit_source_cut_values_v213(plan, slots, &target, width, out)?;
    super::super::super::support_closure::retain_referenced(out)?;
    writeln!(out, "}}").map_err(|_| out.error())?;
    assert!(
        out.text
            .contains("spec fn invocation_source_byte_block_0_v36(")
    );
    assert!(out.text.contains("spec fn byte_micro_step_0_v30("));
    assert!(out.text.contains("spec fn expanded_source_component_0_0_"));
    assert!(!out.text.contains("proof fn invocation_paired_"));
    assert!(!out.text.contains("assume("));
    Ok(())
}

#[test]
fn expanded_source_components_emit_complete_source_and_actual_target_models() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture_with_plan(layout, LIMIT, LIMIT, generate)
            .0
            .unwrap();
    }
}

#[test]
fn expanded_execution_maps_and_payload_laws_are_registered_in_the_production_prelude() {
    let shared = super::super::super::expanded_execution::SHARED;
    let prelude = super::super::super::source_bytes::SOURCE_BYTES_V36;
    assert_eq!(prelude.matches(shared).count(), 1);
    for name in [
        "struct InvocationExecutionOriginV205",
        "spec fn invocation_execution_mapped_v205(",
        "spec fn invocation_execution_payload_related_v209(",
        "proof fn invocation_execution_empty_map_grants_no_binding_v205(",
        "proof fn invocation_execution_payload_missing_context_is_not_related_v209(",
        "spec fn invocation_context_issue_coupled_v211(",
        "proof fn invocation_context_issue_fresh_has_exact_updates_v211(",
    ] {
        assert_eq!(prelude.matches(name).count(), 1);
    }
}

#[test]
#[ignore = "complete component models; no full source refinement or production authority"]
fn diagnostic_complete_expanded_source_component_models_export_v210() {
    use sha2::{Digest, Sha256};
    use std::io::{BufWriter, Write as _};
    for (layout, label) in [(Layout::Blocked, "blocked"), (Layout::Striped, "striped")] {
        run_fixture_with_plan(layout, LIMIT, LIMIT, |plan, slots, tile, out| {
            generate(plan, slots, tile, out)?;
            assert!(out.text.len() <= 16 * 1024 * 1024);
            let mut output = BufWriter::new(std::io::stdout().lock());
            write!(output, "{{\"kind\":\"fe2o3-expanded-source-component-model-v210\",\"scope\":\"necessary source value components only\",\"layout\":\"{label}\",\"bytes\":{},\"sha256\":\"", out.text.len()).unwrap();
            for byte in Sha256::digest(out.text.as_bytes()) {
                write!(output, "{byte:02x}").unwrap();
            }
            write!(output, "\",\"model_hex\":\"").unwrap();
            for byte in out.text.as_bytes() {
                write!(output, "{byte:02x}").unwrap();
            }
            writeln!(output, "\"}}").unwrap();
            output.flush().unwrap();
            Ok(())
        }).0.unwrap();
    }
}
