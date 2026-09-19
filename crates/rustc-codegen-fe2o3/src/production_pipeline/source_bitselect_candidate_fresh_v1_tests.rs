//! Fresh source admission uses the actual candidate callback, not a report.

use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::fresh_header;
use fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1;

#[path = "source_bitselect_candidate_machine_pipeline_v1_tests.rs"]
mod machine;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_fresh_source_bitselect_candidate(
        self,
        input: RetainedInput,
    ) -> Result<Value, String> {
        self.observe_fresh_source_bitselect_candidate_with(input, registers(), |_| Ok(()))
            .map(|(observation, ())| observation)
    }

    fn observe_fresh_source_bitselect_candidate_with<R>(
        self,
        mut input: RetainedInput,
        expected_registers: Gfx942OrderedProgramRegistersV1,
        observe: impl FnOnce(&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17) -> Result<R, String>,
    ) -> Result<(Value, R), String> {
        let mut meter = ScanMeter::default();
        let header = fresh_header(self.stage.tcx, &self.stage.closure, &input, &mut meter)?;
        // This is the existing source-bound pre-ranked route, not V8 ranked
        // compilation, a canonical-byte inverse, or a production continuation.
        let owner = self.observe_ordered_program_v32()?;
        let materialized = owner.materialized();
        let semantic = materialized.semantic_ssa().source_semantic();
        if semantic.wire_version() != SemanticMirWireVersionV1::V32 {
            return Err("source-candidate fresh semantic version is not V32".into());
        }
        let [root] = semantic.roots() else {
            return Err("source-candidate fresh semantic root roster".into());
        };
        let function = semantic
            .functions()
            .get(root.index() as usize)
            .ok_or("source-candidate fresh semantic root absent")?;
        let identities = header.identities;
        if function.identity() != identities.function()
            || function.item_definition_identity() != identities.item_definition()
            || function.monomorphization_identity() != identities.monomorphization()
            || function.generic_type_arguments_identity() != identities.generic_type_arguments()
            || function.const_generic_arguments_identity() != identities.const_generic_arguments()
            || function.role() != SemanticFunctionRoleV1::KernelRoot
        {
            return Err("source-candidate fresh sealed Instance identity mismatch".into());
        }
        let correspondence = materialized.correspondence();
        let mapping = unique(
            correspondence.lowered_functions(),
            &mut meter,
            |row| row.correspondence_owner() == *root && row.semantic_function() == *root,
            "source-candidate fresh function mapping",
        )?;
        let executable = materialized.executable();
        let module = executable.module();
        let kir = unique(
            &module.functions,
            &mut meter,
            |row| &row.id == mapping.kernel_ir_function(),
            "source-candidate fresh KIR function",
        )?;
        let body = kir
            .body
            .as_ref()
            .ok_or("source-candidate fresh body absent")?;
        let mut inputs = [ValueId(0); 3];
        for (index, ordinal) in header.ordinals.iter().enumerate() {
            meter.rows(function.locals().len())?;
            let mut local = None;
            for (local_index, declaration) in function.locals().iter().enumerate() {
                if declaration.role() == SemanticLocalRoleV1::Argument(*ordinal) {
                    let local_index = u32::try_from(local_index)
                        .map_err(|_| "source-candidate fresh local conversion")?;
                    if local
                        .replace(SemanticLocalIdV1::from_index(local_index))
                        .is_some()
                    {
                        return Err("source-candidate ambiguous fresh parameter".into());
                    }
                }
            }
            let local = local.ok_or("source-candidate fresh semantic parameter absent")?;
            inputs[index] = unique(
                correspondence.parameter_bindings(),
                &mut meter,
                |row| {
                    row.correspondence_owner() == *root
                        && row.semantic_function() == *root
                        && row.semantic_local() == local
                },
                "source-candidate fresh parameter correspondence",
            )?
            .kernel_ir_value();
            meter.rows(body.parameters.len())?;
            let mut count = 0;
            for (position, value) in body.parameters.iter().enumerate() {
                if *value == inputs[index] {
                    if kir.signature.parameters.get(position)
                        != Some(&Type::Scalar(ScalarType::U32))
                    {
                        return Err("source-candidate fresh input type changed".into());
                    }
                    count += 1;
                }
            }
            if count != 1 {
                return Err("source-candidate fresh input is not a single parameter".into());
            }
        }
        let mut selected = None;
        meter.rows(body.blocks.len())?;
        for block in &body.blocks {
            meter.rows(block.operations.len())?;
            for operation in &block.operations {
                if let OperationKind::Gfx942OrderedProgram(program) = &operation.kind {
                    if selected.replace(program).is_some() {
                        return Err("source-candidate multiple fresh ordered programs".into());
                    }
                    let [result] = operation.results.as_slice() else {
                        return Err("source-candidate fresh ordered result arity".into());
                    };
                    if result.ty != Type::Scalar(ScalarType::U32) {
                        return Err("source-candidate fresh ordered result type".into());
                    }
                }
            }
        }
        let program = selected.ok_or("source-candidate fresh ordered program absent")?;
        meter.storage(2 * 64 * 1024)?;
        meter.scan(2 * 64 * 1024)?;
        let (expected, _) = render_bitselect_expression_v1(
            header.names.each_ref().map(String::as_str),
            expected_registers,
        )
        .map_err(|e| e.to_string())?;
        if program.inputs() != &inputs
            || program.registers() != expected_registers
            || program.program() != &expected
            || program.source().function != *header.identities.function().as_bytes()
        {
            return Err("source-candidate fresh program/role binding differs".into());
        }
        // Bounded independent Boolean oracle; not formal or whole-kernel proof.
        let mut state = 0x9e37_79b9u32;
        let mut checked = 0usize;
        for iteration in 0..128 {
            let mut values = [0u32; 3];
            for value in &mut values {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *value = state;
            }
            if iteration < 4 {
                values = match iteration {
                    0 => [0, u32::MAX, 0],
                    1 => [u32::MAX, 0, u32::MAX],
                    2 => [0xaaaa_aaaa, 0x5555_5555, 0x0f0f_0f0f],
                    _ => [0, 0, 0],
                };
            }
            meter.scan(16)?;
            let [a, b, mask] = values;
            if program.program().evaluate(values) != ((a & mask) | (b & !mask)) {
                return Err("source-candidate fresh program Boolean oracle mismatch".into());
            }
            checked += 1;
        }
        let additional = observe(executable)?;
        input.recheck()?;
        Ok((
            json!({
                "stage": "fresh_actual_source_mir32_kir17_candidate",
                "semantic_version": "V32", "kernel_ir_version": "V17",
                "semantic_sha256": semantic.semantic_sha256().as_bytes(),
                "kernel_ir_sha256": executable.identity().digest(),
                "candidate_sha256": <[u8;32]>::from(Sha256::digest(input.original())),
                "descriptors": program.program().active_descriptors(),
                "fresh_input_values": inputs.map(|value| value.0),
                "fresh_source_parameter_ordinals": header.ordinals,
                "boolean_oracle_cases": checked,
                "scan_accounting": meter, "io_envelope_accounting": input.io,
                "fresh_frontend_admitted": true, "pre_ranked_diagnostic": true,
                "ranked_checks": false, "functional_proof": false, "production_resume": false,
                "source_map_available": false, "hardware_observed": false,
                "grants_artifact_or_launch_authority": false,
            }),
            additional,
        ))
    }
}
