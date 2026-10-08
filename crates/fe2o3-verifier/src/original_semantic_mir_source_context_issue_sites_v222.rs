//! Sites come from the same retained controls that emit actual source dispatch.
use super::super::expanded_execution::ExpandedExecutionBindingsV199;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1;
use fe2o3_mir_model::{SsaArgumentV1, SsaBlockIdV1, SsaEdgeIdV1, SsaValueV1};

#[derive(Clone, Copy)]
pub(in super::super) struct ContextIssueSiteV222 {
    pub(in super::super) pc: usize,
    pub(in super::super) statements: usize,
    pub(in super::super) destination: usize,
    pub(in super::super) source_type: SemanticTypeIdV1,
    pub(in super::super) continuation: usize,
}

fn headers() -> usize {
    size_of::<ContextIssueSiteV222>()
        + 2 * size_of::<Result<Option<ContextIssueSiteV222>>>()
        + size_of::<Option<SsaValueV1>>()
        + size_of::<SsaEdgeIdV1>()
        + size_of::<std::slice::Iter<'_, SsaArgumentV1>>()
        + size_of::<std::slice::Iter<'_, BodyBlock>>()
        + size_of::<Result<()>>()
        + 10 * size_of::<usize>()
        + 12 * size_of::<&()>()
}

impl SourceByteProgram<'_, '_, '_> {
    pub(in super::super) fn context_issue_site_v222(
        &self,
        root: usize,
        instance: usize,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<ContextIssueSiteV222>> {
        self.source_slots(out)?;
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(6)?;
        let range = &self.roots.get(root).ok_or_else(mismatch)?.0;
        if instance >= range.len() {
            return Err(mismatch());
        }
        let function = self.functions[range.start + instance]
            .as_ref()
            .ok_or_else(mismatch)?;
        let control = function.control.get(block).ok_or_else(mismatch)?;
        let End::Tile(call) = &control.end else {
            return Ok(None);
        };
        Ok(call
            .context_issue_v222()
            .map(
                |(destination, source_type, continuation)| ContextIssueSiteV222 {
                    pc: function.blocks.start + block,
                    statements: control.statements,
                    destination,
                    source_type,
                    continuation,
                },
            ))
    }

    pub(in super::super) fn emit_context_issue_segments_v222(
        &self,
        plan: &InvocationPlan<'_, '_>,
        execution: &ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        let slots = self.source_slots(out)?;
        let source = slots.correspondence(out)?.source(out.budget)?;
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(1)?;
        if !std::ptr::eq(source, plan.source(out)?) {
            return Err(mismatch());
        }
        let archive = source.source_ssa(out.budget)?;
        let mut count = 0usize;
        for root in 0..self.roots.len() {
            for instance in 0..self.roots[root].0.len() {
                out.budget.charge_work(2)?;
                let Some(function) = &self.functions[self.roots[root].0.start + instance] else {
                    continue;
                };
                let row = plan.instance(root, instance, out)?;
                let ssa = archive
                    .plan_for_function(row.function)
                    .ok_or_else(mismatch)?
                    .plan();
                for block in 0..function.control.len() {
                    out.budget.charge_work(1)?;
                    let Some(site) = self.context_issue_site_v222(root, instance, block, out)?
                    else {
                        continue;
                    };
                    let local = site
                        .destination
                        .checked_sub(row.locals.start)
                        .ok_or(Resource::Arithmetic)?;
                    let edge = SsaEdgeIdV1::new(
                        SsaBlockIdV1::new(u32::try_from(block).map_err(|_| Resource::Arithmetic)?),
                        0,
                    );
                    let definitions = ssa.edge_definitions(edge).ok_or_else(mismatch)?;
                    let mut value = None;
                    for definition in definitions {
                        out.budget.charge_work(2)?;
                        if definition.variable().get() as usize == local {
                            if value.is_some() {
                                return Err(mismatch());
                            }
                            value = Some(definition.value());
                        }
                    }
                    execution.emit_context_issue_segment_v222(
                        self,
                        root,
                        instance,
                        block,
                        value.ok_or_else(mismatch)?,
                        out,
                    )?;
                    count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
            }
        }
        self.source_slots(out)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::tile_target::TileTargetV176;
    use super::super::tile_fixture_tests::run_fixture_with_plan;
    use super::*;
    use fe2o3_kernel_ir::ExecutionTileLayoutV1;
    use sha2::{Digest, Sha256};

    fn check_composition_contracts() {
        let source = include_str!("original_semantic_mir_context_issue_segment_v222.rs");
        // The complete 188915 contracts survive the shared proof decomposition.
        for (name, bytes, digest) in [
            (
                "invocation_context_issue_initial_map_segment_{root}_{instance}_{block}_v232",
                1536,
                "2db05d7cf86a00268d473d99b3e96e00bfaff25cb2d3e5634a6da2a732f5a245",
            ),
            (
                "invocation_context_issue_current_map_segment_{root}_{instance}_{block}_v240",
                1713,
                "731989e952b6767209199d69f19121b136543f9c2b193f703be36430d3494727",
            ),
        ] {
            let marker = format!("proof fn {name}(");
            assert_eq!(source.matches(&marker).count(), 1);
            let header = source
                .split_once(&marker)
                .unwrap()
                .1
                .split_once("\n{{\n")
                .unwrap()
                .0;
            assert_eq!(header.len(), bytes);
            assert_eq!(format!("{:x}", Sha256::digest(header.as_bytes())), digest);
        }
    }

    fn check_pc_composition(generated: &str, continuation: usize) {
        let (_, body) = generated.split_once("\n{\n").unwrap();
        let helper = format!(
            "invocation_execution_map_source_pc_v303(\n        coupled.source.source, coupled.target, coupled.execution_map, {continuation});"
        );
        assert_eq!(body.matches(&helper).count(), 1);
        assert!(
            body.find(
                "assert(invocation_source_byte_state_well_formed_v36(coupled.source.source));"
            )
            .unwrap()
                < body.find(&helper).unwrap()
        );
        assert!(body.contains("assert(invocation_execution_map_current_v205(continued, coupled.target, coupled.execution_map));"));
        assert!(!body.contains("assert forall|key: MemoryExecutionReferenceV178|"));
        assert!(!body.contains("hide("));
    }

    #[test]
    fn context_issue_segments_keep_original_dispatch_and_target_microstep() {
        check_composition_contracts();
        for layout in [
            ExecutionTileLayoutV1::Blocked,
            ExecutionTileLayoutV1::Striped,
        ] {
            run_fixture_with_plan(layout, 512 * 1024 * 1024, 512 * 1024 * 1024, |plan, slots, _, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let target = TileTargetV176::derive(slots, out)?;
                let execution = ExpandedExecutionBindingsV199::derive(plan, slots, &target, out)?;
                let start = out.text.len();
                let count = program.emit_context_issue_segments_v222(plan, &execution, out)?;
                let generated = out.text[start..].to_owned();
                let mut expected = 0;
                let mut other_controls = 0;
                for root in 0..program.roots.len() {
                    for instance in 0..program.roots[root].0.len() {
                        let Some(function) = &program.functions[program.roots[root].0.start + instance] else {
                            continue;
                        };
                        for (block, control) in function.control.iter().enumerate() {
                            let original = match &control.end {
                                End::Tile(call) => call.context_issue_v222(),
                                _ => None,
                            };
                            let actual = program.context_issue_site_v222(root, instance, block, out)?;
                            let Some((destination, source_type, continuation)) = original else {
                                assert!(actual.is_none());
                                other_controls += 1;
                                continue;
                            };
                            let actual = actual.expect("original issue must have an authenticated site");
                            assert_eq!((actual.pc, actual.statements, actual.destination, actual.source_type, actual.continuation),
                                (function.blocks.start + block, control.statements, destination, source_type, continuation));
                            let marker = format!("proof fn invocation_context_issue_segment_{root}_{instance}_{block}_v222(");
                            assert_eq!(generated.matches(&marker).count(), 1);
                            let body = generated.split(&marker).nth(1).unwrap().split("// Fresh issuance from an empty witness only").next().unwrap();
                            let requires = body.split("    ensures").next().unwrap();
                            assert_eq!(requires.matches(&format!(
                                "byte_inputs_{root}_v55(target.state, little_endian)"
                            )).count(), 1);
                            for text in [
                                format!("source.source.machine.pc == {}", actual.pc),
                                format!("source.next_statement == {}", actual.statements),
                                format!("source.observations.len() == {}", actual.statements),
                                format!("destination: {destination}, source_type: {}", source_type.index()),
                                format!("invocation_source_micro_finish_{root}_{instance}_v36(source, little_endian)"),
                                format!("byte_micro_step_{root}_v30(target, little_endian)"),
                                format!("invocation_source_byte_pc_v36(coupled.source.source, {continuation})"),
                            ] { assert!(body.contains(&text), "{text}"); }
                            assert!(body.contains("source.observations.push(InvocationSourceStatementObservationV36"));
                            assert!(body.contains("target.observations.push(actual.observation)"));
                            assert!(body.contains("effect: MemoryOperationEffectV30::Pure"));
                            let initial_marker = format!("proof fn invocation_context_issue_initial_map_segment_{root}_{instance}_{block}_v232(");
                            assert_eq!(generated.matches(&initial_marker).count(), 1);
                            let initial = generated.split(&initial_marker).nth(1).unwrap().split("// Fresh issuance with a current input witness").next().unwrap();
                            check_pc_composition(initial, continuation);
                            let (input, output) = initial.split_once("    ensures").unwrap();
                            assert!(!input.contains("execution_map:"));
                            assert!(!input.contains("invocation_execution_map_current_v205"));
                            assert_eq!(input.matches("invocation_context_issue_fresh_enabled_v211(source.source, target.state,").count(), 1);
                            assert_eq!(input.matches(&format!("byte_inputs_{root}_v55(target.state, little_endian)")).count(), 1);
                            for text in [
                                format!("destination: {destination}, source_type: {}", source_type.index()),
                                format!("invocation_context_issue_segment_{root}_{instance}_{block}_v222(\n        source, target, Map::empty(), little_endian)"),
                                format!("invocation_source_micro_finish_{root}_{instance}_v36(source, little_endian)"),
                                format!("byte_micro_step_{root}_v30(target, little_endian)"),
                                format!("invocation_source_byte_pc_v36(coupled.source.source, {continuation})"),
                                format!("assert(coupled.source.source.machine.pc == {})", actual.pc),
                            ] { assert!(initial.contains(&text), "{text}"); }
                            assert!(output.contains("original.source, actual.next.state, coupled.execution_map"));
                            assert!(output.contains("invocation_context_issue_initializes_current_map_v211("));
                            assert!(output.contains("invocation_execution_map_source_pc_v303("));
                            let current_marker = format!("proof fn invocation_context_issue_current_map_segment_{root}_{instance}_{block}_v240(");
                            assert_eq!(generated.matches(&current_marker).count(), 1);
                            let current = generated.split(&current_marker).nth(1).unwrap().split("// Exact endpoint replay only.").next().unwrap();
                            check_pc_composition(current, continuation);
                            let (input, output) = current.split_once("    ensures").unwrap();
                            assert_eq!(input.matches("execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>").count(), 1);
                            assert_eq!(input.matches("invocation_execution_map_current_v205(source.source, target.state, execution_map)").count(), 1);
                            assert_eq!(input.matches("invocation_context_issue_fresh_enabled_v211(source.source, target.state,").count(), 1);
                            assert_eq!(input.matches(&format!("byte_inputs_{root}_v55(target.state, little_endian)")).count(), 1);
                            assert!(!input.contains("coupled") && !input.contains("original.source") && !input.contains("actual.next"));
                            assert!(!current.contains("Map::empty()"));
                            for text in [
                                format!("source.source.machine.pc == {}", actual.pc),
                                format!("source.next_statement == {}", actual.statements),
                                format!("destination: {destination}, source_type: {}", source_type.index()),
                                format!("invocation_context_issue_segment_{root}_{instance}_{block}_v222(\n        source, target, execution_map, little_endian)"),
                                format!("invocation_source_micro_finish_{root}_{instance}_v36(source, little_endian)"),
                                format!("byte_micro_step_{root}_v30(target, little_endian)"),
                                format!("invocation_source_byte_pc_v36(coupled.source.source, {continuation})"),
                            ] { assert!(current.contains(&text), "{text}"); }
                            assert_eq!(output.matches("invocation_context_issue_fresh_preserves_frame_v259(").count(), 1);
                            assert!(!output.contains("invocation_context_issue_fresh_preserves_current_map_v238("));
                            assert!(output.contains("invocation_context_issue_fresh_has_exact_updates_v211("));
                            assert!(output.contains("assert(coupled.source.source.machine == (MemoryStateV30 {"));
                            assert!(output.contains("original.source, actual.next.state, coupled.execution_map"));
                            assert!(output.contains("invocation_execution_map_source_pc_v303("));
                            expected += 1;
                        }
                    }
                }
                assert!(expected > 0 && other_controls > 0);
                assert_eq!(count, expected);
                assert_eq!(generated.matches("proof fn ").count(), 3 * expected);
                assert!(!generated.contains("assume("));
                Ok(())
            }).0.unwrap();
        }
    }
}
