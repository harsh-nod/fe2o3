//! Bounded read-only ordered-program inspection shared by CLI and example.
//! Canonical admission, CPU preflight, and structural/output accounting remain
//! separate; declared register bindings are not observed physical values.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::{AdmittedSimulationInputV1, load_debug_simulation_input_v17};
use fe2o3_kernel_ir::{
    FunctionRole, Gfx942OrderedProgramV1, Gfx942ProgramBinaryOpcodeV1, Gfx942ProgramInstructionV1,
    Module, OperationKind, ScalarType, TargetCapability, WorkgroupSize,
};
use fe2o3_kir_sim::{SimulationArgumentV1, SimulationLimitsV1};
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

const MAX_CANONICAL_BYTES: u64 = 64 * 1024;
const MAX_BLOCKS: usize = 128;
const MAX_OPERATIONS: usize = 4096;
const MAX_SSA_DEFINITIONS: usize = 8192;
const MAX_CAPABILITIES: usize = 256;
const MAX_NAMES_BYTES: usize = 16 * 1024;
const MAX_ID_BYTES: usize = 1024;
const MAX_REQUEST_DATA_BYTES: usize = 4 * 1024 * 1024;
const OUTPUT_BYTES: usize = 8192;
const CPU_PREFLIGHT_RESIDENT_BYTES: usize = 64 * 1024 * 1024;

type InspectResult<T> = Result<T, &'static str>;

#[derive(Default, Serialize)]
struct StructuralCounts {
    blocks: usize,
    operations: usize,
    ssa_definitions: usize,
    capability_entries: usize,
    name_bytes: usize,
}

fn charge(total: &mut usize, amount: usize, maximum: usize) -> InspectResult<()> {
    let next = total
        .checked_add(amount)
        .ok_or("inspection count overflow")?;
    if next > maximum {
        return Err("inspection structural limit exceeded");
    }
    *total = next;
    Ok(())
}

fn name(counts: &mut StructuralCounts, value: &str) -> InspectResult<()> {
    if value.len() > MAX_ID_BYTES {
        return Err("inspection identity/name is too long");
    }
    charge(&mut counts.name_bytes, value.len(), MAX_NAMES_BYTES)
}

fn capabilities(
    counts: &mut StructuralCounts,
    values: &BTreeSet<TargetCapability>,
) -> InspectResult<()> {
    // Charge the complete collection before visiting any of its entries.
    charge(
        &mut counts.capability_entries,
        values.len(),
        MAX_CAPABILITIES,
    )?;
    for capability in values {
        if let TargetCapability::Extension {
            namespace,
            name: extension,
        } = capability
        {
            name(counts, namespace)?;
            name(counts, extension)?;
        }
    }
    Ok(())
}

struct ProgramView<'a> {
    program: &'a Gfx942OrderedProgramV1,
    block_ordinal: usize,
    operation_ordinal: usize,
    raw_block_id: u32,
    result_value_id: u32,
    counts: StructuralCounts,
}

/// Structural precheck only: its caller must retain the admitted immutable owner.
/// This deliberately narrow profile rejects extra functions, roots or programs.
fn find_program(module: &Module) -> InspectResult<ProgramView<'_>> {
    let [kernel] = module.kernels.as_slice() else {
        return Err("inspection requires exactly one kernel");
    };
    let [function] = module.functions.as_slice() else {
        return Err("inspection requires exactly one function");
    };
    let mut counts = StructuralCounts::default();
    for identity in [
        module.id.as_str(),
        kernel.id.as_str(),
        kernel.entry.as_str(),
        function.id.as_str(),
    ] {
        name(&mut counts, identity)?;
    }
    if kernel.entry != function.id || function.role != FunctionRole::KernelEntry {
        return Err("inspection requires the single function to be the kernel entry");
    }
    if kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1)) {
        return Err("inspection requires an explicit 64x1x1 workgroup");
    }
    for set in [
        &module.required_capabilities,
        &kernel.required_capabilities,
        &function.required_capabilities,
    ] {
        capabilities(&mut counts, set)?;
    }
    let body = function
        .body
        .as_ref()
        .ok_or("inspection requires a defined entry body")?;
    if function.signature.parameters.len() > 64 || function.signature.results.len() > 64 {
        return Err("inspection signature limit exceeded");
    }
    charge(
        &mut counts.ssa_definitions,
        body.parameters.len(),
        MAX_SSA_DEFINITIONS,
    )?;
    charge(&mut counts.blocks, body.blocks.len(), MAX_BLOCKS)?;
    let mut selected = None;
    for (block_ordinal, block) in body.blocks.iter().enumerate() {
        charge(
            &mut counts.ssa_definitions,
            block.parameters.len(),
            MAX_SSA_DEFINITIONS,
        )?;
        charge(
            &mut counts.operations,
            block.operations.len(),
            MAX_OPERATIONS,
        )?;
        for (operation_ordinal, operation) in block.operations.iter().enumerate() {
            charge(
                &mut counts.ssa_definitions,
                operation.results.len(),
                MAX_SSA_DEFINITIONS,
            )?;
            if let OperationKind::Gfx942OrderedProgram(program) = &operation.kind {
                if selected.is_some() {
                    return Err("inspection requires exactly one ordered program");
                }
                let [result] = operation.results.as_slice() else {
                    return Err("ordered program result arity disagrees with admitted contract");
                };
                if result.ty.as_scalar() != Some(ScalarType::U32)
                    || !operation.has_complete_effect_summary()
                {
                    return Err("ordered program result/effect disagrees with admitted contract");
                }
                selected = Some((
                    program,
                    block_ordinal,
                    operation_ordinal,
                    block.id.0,
                    result.id.0,
                ));
            }
        }
    }
    let (program, block_ordinal, operation_ordinal, raw_block_id, result_value_id) =
        selected.ok_or("inspection requires exactly one ordered program")?;
    Ok(ProgramView {
        program,
        block_ordinal,
        operation_ordinal,
        raw_block_id,
        result_value_id,
        counts,
    })
}

fn preflight_limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: MAX_CANONICAL_BYTES as usize,
        max_reachable_functions: 1,
        max_reachable_operations: MAX_OPERATIONS,
        max_invocations: 64,
        max_workgroups: 1,
        max_scheduled_slots: 64,
        max_steps: 1 << 20,
        max_call_depth: 1,
        max_ssa_values: MAX_SSA_DEFINITIONS,
        max_allocations: 16,
        max_allocation_bytes: 1024 * 1024,
        max_total_bytes: MAX_REQUEST_DATA_BYTES,
        max_resident_bytes: CPU_PREFLIGHT_RESIDENT_BYTES,
        max_events: 1,
        max_memory_access_records: 1,
    }
}

fn inspect(input: &AdmittedSimulationInputV1) -> InspectResult<ProgramView<'_>> {
    if input.module.identity().wire_version() != 17
        || input.module.identity().canonical_length() > MAX_CANONICAL_BYTES
    {
        return Err("inspection requires exact V17 within its 64 KiB post-admission profile");
    }
    let view = find_program(input.module.module())?;
    let request = &input.request;
    if request.kernel.as_str().len() > MAX_ID_BYTES
        || request.kernel != input.module.module().kernels[0].id
    {
        return Err("inspection request must select the single admitted kernel");
    }
    if request.grid.0 != [64, 1, 1] || request.workgroup.0 != [64, 1, 1] {
        return Err("inspection requires one complete 64x1x1 workgroup request");
    }
    if request.arguments.len() > 32 || request.shared_buffers.len() > 8 {
        return Err("inspection request collection limit exceeded");
    }
    let mut data_bytes = 0;
    for buffer in request
        .arguments
        .iter()
        .filter_map(|argument| match argument {
            SimulationArgumentV1::Buffer(buffer) => Some(buffer),
            _ => None,
        })
        .chain(request.shared_buffers.iter().map(|shared| &shared.buffer))
    {
        charge(
            &mut data_bytes,
            buffer.bytes().len(),
            MAX_REQUEST_DATA_BYTES,
        )?;
        charge(
            &mut data_bytes,
            buffer.initialized().len(),
            MAX_REQUEST_DATA_BYTES,
        )?;
    }
    // Reuses exact declared-target/wave, launch, argument and operation checks.
    // Success remains logical CPU eligibility, not source or hardware authority.
    let plan = input
        .module
        .preflight(request, input.simulation_target(), preflight_limits())
        .map_err(|_| "CPU preflight rejected the exact declared profile/request")?;
    drop(plan);
    Ok(view)
}

#[derive(Clone, Copy)]
struct HexIdentity([u8; 32]);

impl Serialize for HexIdentity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut output = [0_u8; 64];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (index, byte) in self.0.iter().copied().enumerate() {
            output[index * 2] = HEX[usize::from(byte >> 4)];
            output[index * 2 + 1] = HEX[usize::from(byte & 15)];
        }
        serializer.serialize_str(std::str::from_utf8(&output).expect("hex is ASCII"))
    }
}

#[derive(Serialize)]
struct CanonicalIdentity {
    wire_version: u16,
    sha256: HexIdentity,
    bytes: u64,
}
#[derive(Serialize)]
struct Coordinate {
    function_ordinal: usize,
    block_ordinal: usize,
    operation_ordinal: usize,
}
#[derive(Serialize)]
struct RegisterPlan {
    scratch: u8,
    output: u8,
    inputs: [u8; 3],
    vgpr_high_water: u8,
}
#[derive(Serialize)]
struct Step {
    instruction: &'static str,
    output: u8,
    inputs: StepInputs,
}
struct StepInputs {
    values: [u8; 2],
    count: usize,
}
impl Serialize for StepInputs {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.values[..self.count].serialize(serializer)
    }
}
struct DeclaredSteps<'a>(&'a Gfx942OrderedProgramV1);
impl Serialize for DeclaredSteps<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let registers = self.0.registers();
        let mut sequence = serializer.serialize_seq(Some(usize::from(self.0.program().count())))?;
        for instruction in self.0.program().instructions() {
            let step = match instruction {
                Gfx942ProgramInstructionV1::Move {
                    destination,
                    source,
                } => Step {
                    instruction: "v_mov_b32_e32",
                    output: registers.binding(destination.role()),
                    inputs: StepInputs {
                        values: [registers.binding(source), 0],
                        count: 1,
                    },
                },
                Gfx942ProgramInstructionV1::Binary {
                    opcode,
                    destination,
                    left,
                    right,
                } => Step {
                    instruction: match opcode {
                        Gfx942ProgramBinaryOpcodeV1::Add => "v_add_u32_e32",
                        Gfx942ProgramBinaryOpcodeV1::Subtract => "v_sub_u32_e32",
                        Gfx942ProgramBinaryOpcodeV1::And => "v_and_b32_e32",
                        Gfx942ProgramBinaryOpcodeV1::Or => "v_or_b32_e32",
                        Gfx942ProgramBinaryOpcodeV1::Xor => "v_xor_b32_e32",
                    },
                    output: registers.binding(destination.role()),
                    inputs: StepInputs {
                        values: [registers.binding(left), registers.binding(right)],
                        count: 2,
                    },
                },
            };
            sequence.serialize_element(&step)?;
        }
        sequence.end()
    }
}
#[derive(Serialize)]
struct DeclaredProgram<'a> {
    count: u8,
    descriptors: &'a [u16; 16],
}
#[derive(Serialize)]
struct DeclaredSourceIds {
    frontend_unit: HexIdentity,
    function: HexIdentity,
    contract: HexIdentity,
    statement: HexIdentity,
}
#[derive(Serialize)]
struct Report<'a> {
    kind: &'static str,
    authority: &'static str,
    canonical: CanonicalIdentity,
    kernel: &'a str,
    function: &'a str,
    coordinate: Coordinate,
    raw_block_id: u32,
    input_value_ids: [u32; 3],
    result_value_id: u32,
    declared_target: &'static str,
    declared_wave_width: u32,
    profile: &'static str,
    register_plan: RegisterPlan,
    declared_program: DeclaredProgram<'a>,
    declared_instruction_steps: DeclaredSteps<'a>,
    declared_source_ids: DeclaredSourceIds,
    memory_effect: &'static str,
    ordered_region_effect: bool,
    pure_or_movable: bool,
    logical_observation_granularity: &'static str,
    source_authentication: bool,
    source_map_available: bool,
    physical_register_values_available: bool,
    instruction_microsteps_available: bool,
    register_lifetime_or_final_allocation_proof: bool,
    proof_authority: bool,
    artifact_authority: bool,
    production_resume_authority: bool,
    hardware_execution: bool,
    cpu_preflight_passed: bool,
    inspection_counts: &'a StructuralCounts,
    inspection_max_canonical_bytes_after_admission: u64,
    cpu_preflight_resident_limit_bytes: usize,
    output_buffer_bytes: usize,
    accounting_scope: &'static str,
}

fn report<'a>(input: &'a AdmittedSimulationInputV1, view: &'a ProgramView<'a>) -> Report<'a> {
    let identity = input.module.identity();
    let module = input.module.module();
    let registers = view.program.registers();
    let source = view.program.source();
    Report {
        kind: "diagnostic_ordered_program_inspection_example",
        authority: "observation_only",
        canonical: CanonicalIdentity {
            wire_version: identity.wire_version(),
            sha256: HexIdentity(*identity.digest()),
            bytes: identity.canonical_length(),
        },
        kernel: module.kernels[0].id.as_str(),
        function: module.functions[0].id.as_str(),
        coordinate: Coordinate {
            function_ordinal: 0,
            block_ordinal: view.block_ordinal,
            operation_ordinal: view.operation_ordinal,
        },
        raw_block_id: view.raw_block_id,
        input_value_ids: view.program.inputs().map(|value| value.0),
        result_value_id: view.result_value_id,
        declared_target: "gfx942:xnack-",
        declared_wave_width: 64,
        profile: "closed_u32_program_e32_v1",
        register_plan: RegisterPlan {
            scratch: registers.scratch(),
            output: registers.output(),
            inputs: registers.inputs(),
            vgpr_high_water: registers.vgpr_high_water(),
        },
        declared_program: DeclaredProgram {
            count: view.program.program().count(),
            descriptors: view.program.program().descriptors(),
        },
        declared_instruction_steps: DeclaredSteps(view.program),
        declared_source_ids: DeclaredSourceIds {
            frontend_unit: HexIdentity(source.frontend_unit),
            function: HexIdentity(source.function),
            contract: HexIdentity(source.contract),
            statement: HexIdentity(source.statement),
        },
        memory_effect: "NoMemory",
        ordered_region_effect: true,
        pure_or_movable: false,
        logical_observation_granularity: "whole_program_before_after",
        source_authentication: false,
        source_map_available: false,
        physical_register_values_available: false,
        instruction_microsteps_available: false,
        register_lifetime_or_final_allocation_proof: false,
        proof_authority: false,
        artifact_authority: false,
        production_resume_authority: false,
        hardware_execution: false,
        cpu_preflight_passed: true,
        inspection_counts: &view.counts,
        inspection_max_canonical_bytes_after_admission: MAX_CANONICAL_BYTES,
        cpu_preflight_resident_limit_bytes: CPU_PREFLIGHT_RESIDENT_BYTES,
        output_buffer_bytes: OUTPUT_BYTES,
        accounting_scope: "shared canonical admission, CPU resident accounting, and this borrowed structural/output bound are separate; not a combined allocator or RSS cap",
    }
}

struct FixedOutput {
    bytes: [u8; OUTPUT_BYTES],
    len: usize,
}
impl FixedOutput {
    fn new() -> Self {
        Self {
            bytes: [0; OUTPUT_BYTES],
            len: 0,
        }
    }
    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}
impl Write for FixedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self
            .len
            .checked_add(bytes.len())
            .filter(|end| *end <= OUTPUT_BYTES)
            .ok_or_else(|| io::Error::other("inspection JSON output bound exceeded"))?;
        self.bytes[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn run() -> InspectResult<()> {
    let mut args = std::env::args_os();
    let _ = args.next();
    let kir = args
        .next()
        .ok_or("usage: fe2o3-program-inspect KIR_PATH REQUEST_PATH")?;
    let request = match args.next() {
        None if kir == "--help" => {
            return io::stdout()
                .lock()
                .write_all(
                    b"usage: fe2o3-program-inspect KIR_PATH REQUEST_PATH\n\
Read-only diagnostic KIR V17 inspection: one gfx942 Wave64 ordered program.\n\
Reports declared registers/instructions, not physical values or execution authority.\n",
                )
                .map_err(|_| "inspection output write failed");
        }
        Some(request) => request,
        None => return Err("usage: fe2o3-program-inspect KIR_PATH REQUEST_PATH"),
    };
    if args.next().is_some()
        || kir.as_encoded_bytes().len() > 4096
        || request.as_encoded_bytes().len() > 4096
    {
        return Err("exactly two paths of at most 4096 bytes are required");
    }
    let input = load_debug_simulation_input_v17(Path::new(&kir), Path::new(&request))
        .map_err(|_| "shared secure V17/request admission rejected the input")?;
    let view = inspect(&input)?;
    let mut output = FixedOutput::new();
    serde_json::to_writer(&mut output, &report(&input, &view))
        .map_err(|_| "bounded inspection JSON serialization failed")?;
    output
        .write_all(b"\n")
        .map_err(|_| "inspection output bound exceeded")?;
    io::stdout()
        .lock()
        .write_all(output.as_bytes())
        .map_err(|_| "inspection output write failed")
}

/// Run the bounded diagnostic inspector using the process arguments and streams.
/// This loads and preflights inputs; it never executes the kernel or invokes a compiler.
pub fn run_ordered_program_inspector() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("inspection refused: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "../tests/fixtures/diagnostic_kir_v17.rs"]
mod fixture;

#[cfg(all(test, target_os = "linux"))]
#[path = "ordered_program_inspection_tests.rs"]
mod tests;
