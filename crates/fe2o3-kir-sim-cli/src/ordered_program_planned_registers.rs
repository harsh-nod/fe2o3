//! Static declared demand intervals, not an allocator trace or physical state.
use super::{FixedOutput, InspectResult, Report, quoted_name};
use fe2o3_kernel_ir::{Gfx942OrderedProgramV1, Gfx942ProgramInstructionV1, Gfx942ProgramRoleV1};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::io::{self, Write};

const MAX_VALUES: usize = 19; // Three entry inputs plus sixteen distinct writes.
const MAX_USES: usize = 33; // Two reads per instruction plus the region-result handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
struct Value {
    id: u8,
    role: &'static str,
    binding: u8,
    def: u8,
    last_use: Option<u8>,
    overwritten: Option<u8>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
struct Use {
    at: u8,
    value: u8,
    kind: &'static str,
}
struct Rows<T, const N: usize> {
    rows: [Option<T>; N],
    len: usize,
}
impl<T: Copy, const N: usize> Rows<T, N> {
    fn new() -> Self {
        Self {
            rows: [None; N],
            len: 0,
        }
    }
    fn push(&mut self, value: T) -> InspectResult<()> {
        if self.len == N {
            return Err("planned register row bound exceeded");
        }
        self.rows[self.len] = Some(value);
        self.len += 1;
        Ok(())
    }
}
impl<T: Serialize, const N: usize> Serialize for Rows<T, N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for row in &self.rows[..self.len] {
            seq.serialize_element(row.as_ref().ok_or_else(|| {
                <S::Error as serde::ser::Error>::custom("planned register row missing")
            })?)?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct Plan {
    steps: u8,
    result_boundary: u8,
    values: Rows<Value, MAX_VALUES>,
    uses: Rows<Use, MAX_USES>,
}
fn role_name(role: Gfx942ProgramRoleV1) -> &'static str {
    match role {
        Gfx942ProgramRoleV1::Input0 => "input0",
        Gfx942ProgramRoleV1::Input1 => "input1",
        Gfx942ProgramRoleV1::Input2 => "input2",
        Gfx942ProgramRoleV1::Scratch => "scratch",
        Gfx942ProgramRoleV1::Output => "out",
    }
}
impl Plan {
    fn value_mut(&mut self, id: u8) -> InspectResult<&mut Value> {
        self.values
            .rows
            .get_mut(usize::from(id))
            .and_then(Option::as_mut)
            .ok_or("planned register definition missing")
    }
    fn use_value(&mut self, id: u8, at: u8, kind: &'static str) -> InspectResult<()> {
        let value = self.value_mut(id)?;
        if at < value.def || value.overwritten.is_some() {
            return Err("planned register use ordering mismatch");
        }
        value.last_use = Some(at);
        self.uses.push(Use {
            at,
            value: id,
            kind,
        })
    }
    fn derive(program: &Gfx942OrderedProgramV1) -> InspectResult<Self> {
        let steps = program.program().count();
        if !(1..=16).contains(&steps) {
            return Err("planned register step bound exceeded");
        }
        let mut plan = Self {
            steps,
            result_boundary: 2 * steps + 1,
            values: Rows::new(),
            uses: Rows::new(),
        };
        let mut current = [None; 5];
        for (id, role) in [
            Gfx942ProgramRoleV1::Input0,
            Gfx942ProgramRoleV1::Input1,
            Gfx942ProgramRoleV1::Input2,
        ]
        .into_iter()
        .enumerate()
        {
            plan.values.push(Value {
                id: id as u8,
                role: role_name(role),
                binding: program.registers().binding(role),
                def: 0,
                last_use: None,
                overwritten: None,
            })?;
            current[role as usize] = Some(id as u8);
        }
        for (index, instruction) in program.program().instructions().enumerate() {
            let read_at = 2 * index as u8 + 1;
            let (destination, operands, count) = match instruction {
                Gfx942ProgramInstructionV1::Move {
                    destination,
                    source,
                } => (destination, [(source, "move"), (source, "move")], 1),
                Gfx942ProgramInstructionV1::Binary {
                    destination,
                    left,
                    right,
                    ..
                } => (destination, [(left, "left"), (right, "right")], 2),
            };
            // Capture BOTH operand versions before installing the destination.
            // Self-moves and same-role binary operands therefore read the old version.
            for (role, kind) in &operands[..count] {
                let id =
                    current[*role as usize].ok_or("planned register read before definition")?;
                plan.use_value(id, read_at, kind)?;
            }
            let role = destination.role();
            let write_at = read_at + 1;
            if let Some(old) = current[role as usize] {
                plan.value_mut(old)?.overwritten = Some(write_at);
            }
            let id = plan.values.len as u8;
            plan.values.push(Value {
                id,
                role: role_name(role),
                binding: program.registers().binding(role),
                def: write_at,
                last_use: None,
                overwritten: None,
            })?;
            current[role as usize] = Some(id);
        }
        let output = current[Gfx942ProgramRoleV1::Output as usize]
            .ok_or("planned register output definition missing")?;
        // This is the region's logical result handoff even when the enclosing
        // KIR result is unused. It is not a physical register read observation.
        plan.use_value(output, plan.result_boundary, "region_result")?;
        Ok(plan)
    }
}
#[derive(Serialize)]
struct Normalized<'a> {
    schema: &'static str,
    authority: &'static str,
    provenance: &'static str,
    canonical: &'a super::CanonicalIdentity,
    kernel: &'a str,
    function: &'a str,
    coordinate: &'a super::Coordinate,
    raw_block_id: u32,
    input_value_ids: [u32; 3],
    result_value_id: u32,
    declared_target: &'static str,
    declared_wave_width: u32,
    declared_source_ids: &'a super::DeclaredSourceIds,
    register_plan: &'a super::RegisterPlan,
    boundary_convention: &'static str,
    interval_convention: &'static str,
    plan: &'a Plan,
    physical_allocation_observed: bool,
    physical_register_values_available: bool,
    instruction_microsteps_available: bool,
    source_authentication: bool,
    artifact_authority: bool,
    production_resume_authority: bool,
    hardware_execution: bool,
    fixed_plan_bytes: usize,
    fixed_projection_bytes: usize,
    fixed_representations_limit_bytes: usize,
    accounting_scope: &'static str,
}
// This bounds these two retained representations, not compiler-generated stack
// frames/copies, serde internals, the canonical owner or total process RSS.
const FIXED_REPRESENTATIONS_LIMIT_BYTES: usize = 4096;
const PLAN_BYTES: usize = std::mem::size_of::<Plan>();
const PROJECTION_BYTES: usize = std::mem::size_of::<Normalized<'static>>();
const _: () = assert!(PLAN_BYTES + PROJECTION_BYTES <= FIXED_REPRESENTATIONS_LIMIT_BYTES);
fn normalized<'a>(observed: &'a Report<'_>, plan: &'a Plan) -> Normalized<'a> {
    Normalized {
        schema: "fe2o3-declared-register-demand-v1",
        authority: "observation_only",
        provenance: "derived_from_declared_ordered_program_not_physical_allocation",
        canonical: &observed.canonical,
        kernel: observed.kernel,
        function: observed.function,
        coordinate: &observed.coordinate,
        raw_block_id: observed.raw_block_id,
        input_value_ids: observed.input_value_ids,
        result_value_id: observed.result_value_id,
        declared_target: observed.declared_target,
        declared_wave_width: observed.declared_wave_width,
        declared_source_ids: &observed.declared_source_ids,
        register_plan: &observed.register_plan,
        boundary_convention: "0=entry;2*i+1=read;2*i+2=write;2*n+1=region_result",
        interval_convention: "inclusive def..last_use;null last_use=unused;overwritten is not free",
        plan,
        physical_allocation_observed: false,
        physical_register_values_available: false,
        instruction_microsteps_available: false,
        source_authentication: false,
        artifact_authority: false,
        production_resume_authority: false,
        hardware_execution: false,
        fixed_plan_bytes: PLAN_BYTES,
        fixed_projection_bytes: PROJECTION_BYTES,
        fixed_representations_limit_bytes: FIXED_REPRESENTATIONS_LIMIT_BYTES,
        accounting_scope: "fixed plan/projection representations and 8192-byte output are separate from the dropped CPU preflight plan's 64-MiB resident budget; not total stack, allocator or RSS accounting",
    }
}
fn write_text(output: &mut FixedOutput, observed: &Report<'_>, plan: &Plan) -> io::Result<()> {
    output
        .write_all(b"Planned declared-register demand (not LLVM allocation or hardware time)\n")?;
    write!(
        output,
        "canonical: V{} sha256=",
        observed.canonical.wire_version
    )?;
    for byte in observed.canonical.sha256.0 {
        write!(output, "{byte:02x}")?;
    }
    writeln!(output, " bytes={}", observed.canonical.bytes)?;
    output.write_all(b"kernel: ")?;
    quoted_name(output, observed.kernel)?;
    output.write_all(b"\nfunction: ")?;
    quoted_name(output, observed.function)?;
    writeln!(
        output,
        "\ncoordinate: function={} block={} operation={} raw_block_id={}",
        observed.coordinate.function_ordinal,
        observed.coordinate.block_ordinal,
        observed.coordinate.operation_ordinal,
        observed.raw_block_id
    )?;
    writeln!(
        output,
        "target: {}; wave={}; steps={}",
        observed.declared_target, observed.declared_wave_width, plan.steps
    )?;
    output.write_all(b"0=entry; step i reads at 2*i+1 then writes at 2*i+2; final column=region result\nLegend: D=definition r=operand read R=result handoff -=demand span x=overwritten .=no demand\n              boundary ")?;
    for at in 0..=plan.result_boundary {
        write!(output, "{}", at % 10)?;
    }
    output.write_all(b"\n")?;
    for row in &plan.values.rows[..plan.values.len] {
        let value = row
            .as_ref()
            .ok_or_else(|| io::Error::other("planned register row missing"))?;
        write!(
            output,
            "  #{:02} v{:02} {:7}    ",
            value.id, value.binding, value.role
        )?;
        for at in 0..=plan.result_boundary {
            let used = plan.uses.rows[..plan.uses.len]
                .iter()
                .flatten()
                .find(|used| used.value == value.id && used.at == at);
            let cell = if at == value.def {
                'D'
            } else if let Some(used) = used {
                if used.kind == "region_result" {
                    'R'
                } else {
                    'r'
                }
            } else if value.overwritten == Some(at) {
                'x'
            } else if at > value.def && value.last_use.is_some_and(|last| at < last) {
                '-'
            } else {
                '.'
            };
            write!(output, "{cell}")?;
        }
        match value.last_use {
            Some(last) => writeln!(output, "  def={} last={last}", value.def)?,
            None => writeln!(output, "  def={} unused", value.def)?,
        }
    }
    writeln!(
        output,
        "Storage: plan={PLAN_BYTES}B projection={PROJECTION_BYTES}B; representation ceiling={FIXED_REPRESENTATIONS_LIMIT_BYTES}B, separate from the 8192B output and CPU preflight budget (not total stack/RSS)."
    )?;
    output.write_all(b"Intervals are inclusive declared demand, not allocation/free events. The result handoff does not prove surrounding use.\nUnavailable: LLVM allocation, captured values, instruction microsteps, source authentication, artifact/resume/hardware authority.\n")
}
pub(super) fn render(
    output: &mut FixedOutput,
    observed: &Report<'_>,
    json: bool,
) -> InspectResult<()> {
    let plan = Plan::derive(observed.declared_instruction_steps.0)?;
    if json {
        serde_json::to_writer(&mut *output, &normalized(observed, &plan))
            .map_err(|_| "bounded planned-register JSON serialization failed")?;
        output
            .write_all(b"\n")
            .map_err(|_| "planned-register output bound exceeded")
    } else {
        write_text(output, observed, &plan)
            .map_err(|_| "bounded planned-register text serialization failed")
    }
}
#[cfg(all(test, target_os = "linux"))]
#[path = "ordered_program_planned_registers_tests.rs"]
mod tests;
