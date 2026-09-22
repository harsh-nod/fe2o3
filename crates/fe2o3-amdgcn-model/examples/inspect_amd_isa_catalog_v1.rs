//! Author-facing inspection of immutable AMD ISA metadata, never execution.
//!
//! Names and opcodes are inventory observations, not encoding, legality, source
//! admission, simulation, proof, artifact, or hardware qualification.

use std::env;
use std::ffi::OsString;
use std::fmt::{self, Write as _};
use std::io::{self, Write as _};

use fe2o3_amdgcn_model::{
    AmdIsaInstructionEncodingV1, AmdIsaInstructionV1, AmdIsaMetadataKindV1, AmdIsaSpecCatalogV1,
    amd_isa_catalog_for_target_v1,
};

const MAX_ARGUMENT_BYTES: usize = 128;
const MAX_INSTRUCTIONS: usize = 4096;
const MAX_ALTERNATIVES: usize = 128;
const MAX_OPERANDS: usize = 64;
const MAX_ALIASES: usize = 128;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const OUTPUT_LIMIT: &str = "catalog observation exceeds its bounded output limit";
const USAGE: &str = "\
usage: inspect_amd_isa_catalog_v1 --help
       inspect_amd_isa_catalog_v1 TARGET
       inspect_amd_isa_catalog_v1 TARGET --list
       inspect_amd_isa_catalog_v1 TARGET INSTRUCTION_OR_ALIAS

TARGET is exactly gfx942 or gfx950, without features, suffixes or whitespace.
Names use exact upstream spelling; only explicit upstream aliases resolve.
Output is bounded human-readable metadata, not a stable wire format or an encoder.
";
const BOUNDARY: &str = "\
catalogued: pinned metadata inventory only
encodable: not established by this catalog; no encoding selection or legality check
authorable: encoding-level authoring unavailable in the reviewed overlay
simulated: encoding-level simulation unavailable in the reviewed overlay
qualified: proof and hardware qualification unavailable in the reviewed overlay
source_marker_coverage: separate family-level observation, never an encoding choice
conditions: declarations only; expressions are not evaluated
effects: source-declared flags only; missing explicit EXEC is not an EXEC guarantee
registers: operand widths are not legal allocation, lifetime or resource bounds
authority: no source authentication, production resume, artifact, load or launch
";

#[derive(Debug, Eq, PartialEq)]
enum Request {
    Help,
    Inspect { target: String, mode: Mode },
}

#[derive(Debug, Eq, PartialEq)]
enum Mode {
    Summary,
    List,
    Instruction(String),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("inspect_amd_isa_catalog_v1: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let request = arguments(env::args_os().skip(1))?;
    // Construct the complete bounded report before writing anything to stdout.
    // An I/O error can still cause a partial operating-system write.
    let observation = render(&request, MAX_OUTPUT_BYTES)?;
    io::stdout()
        .lock()
        .write_all(observation.as_bytes())
        .map_err(|error| format!("write catalog observation: {error}"))
}

fn argument(value: OsString) -> Result<String, String> {
    let value = value
        .into_string()
        .map_err(|_| "arguments must be valid UTF-8".to_owned())?;
    if value.is_empty() || value.len() > MAX_ARGUMENT_BYTES {
        return Err("arguments must contain 1..=128 UTF-8 bytes".into());
    }
    Ok(value)
}

fn arguments(mut args: impl Iterator<Item = OsString>) -> Result<Request, String> {
    let first = argument(args.next().ok_or_else(|| USAGE.to_owned())?)?;
    let second = args.next();
    // Consume at most three arguments, never collect an unbounded iterator.
    if args.next().is_some() {
        return Err(USAGE.into());
    }
    if first == "--help" && second.is_none() {
        return Ok(Request::Help);
    }
    amd_isa_catalog_for_target_v1(&first).map_err(|error| error.to_string())?;
    let mode = match second.map(argument).transpose()? {
        None => Mode::Summary,
        Some(value) if value == "--list" => Mode::List,
        Some(value) if value.starts_with('-') => return Err(USAGE.into()),
        Some(value) => Mode::Instruction(value),
    };
    Ok(Request::Inspect {
        target: first,
        mode,
    })
}

struct BoundedText {
    text: String,
    limit: usize,
}

impl BoundedText {
    fn new(limit: usize) -> Result<Self, String> {
        if limit == 0 || limit > MAX_OUTPUT_BYTES {
            return Err(OUTPUT_LIMIT.into());
        }
        Ok(Self {
            text: String::new(),
            limit,
        })
    }

    fn line(&mut self, value: fmt::Arguments<'_>) -> Result<(), String> {
        self.write_fmt(value)
            .and_then(|()| self.write_char('\n'))
            .map_err(|_| OUTPUT_LIMIT.to_owned())
    }
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self
            .text
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.limit)
        {
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

fn inventory(catalog: AmdIsaSpecCatalogV1) -> Result<Vec<AmdIsaInstructionV1>, String> {
    if catalog.instruction_count() > MAX_INSTRUCTIONS {
        return Err("catalog instruction inventory exceeds the inspector bound".into());
    }
    let mut rows = catalog
        .instructions()
        .take(MAX_INSTRUCTIONS + 1)
        .collect::<Vec<_>>();
    if rows.len() != catalog.instruction_count() {
        return Err("catalog instruction inventory is incomplete".into());
    }
    rows.sort_by_key(|row| row.name());
    if rows.windows(2).any(|pair| pair[0].name() >= pair[1].name()) {
        return Err("catalog instruction names are not unique".into());
    }
    Ok(rows)
}

fn alternatives(
    instruction: AmdIsaInstructionV1,
) -> Result<Vec<AmdIsaInstructionEncodingV1>, String> {
    let rows = instruction.encodings();
    if rows.len() > MAX_ALTERNATIVES {
        return Err("instruction alternatives exceed the inspector bound".into());
    }
    let mut rows = rows.collect::<Vec<_>>();
    // Keep all alternatives. Sorting neither chooses nor merges an encoding.
    rows.sort_by_key(|row| (row.name(), row.condition(), row.opcode()));
    Ok(rows)
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Counts {
    alternatives: usize,
    missing_conditions: usize,
    repeated_conditions: usize,
    source_marker_families: usize,
}

fn counts(rows: &[AmdIsaInstructionV1]) -> Result<Counts, String> {
    let mut count = Counts::default();
    for instruction in rows {
        if instruction.reviewed_source_marker_family() {
            count.source_marker_families += 1;
        }
        for encoding in alternatives(*instruction)? {
            count.alternatives += 1;
            if !encoding.condition_definition_available() {
                count.missing_conditions += 1;
            }
            if encoding.condition_declaration_count() > 1 {
                count.repeated_conditions += 1;
            }
        }
    }
    // All counters are bounded by MAX_INSTRUCTIONS * MAX_ALTERNATIVES.
    Ok(count)
}

fn available(present: bool) -> &'static str {
    if present { "available" } else { "unavailable" }
}

fn render(request: &Request, limit: usize) -> Result<String, String> {
    let mut output = BoundedText::new(limit)?;
    let Request::Inspect { target, mode } = request else {
        output
            .write_str(USAGE)
            .map_err(|_| OUTPUT_LIMIT.to_owned())?;
        return Ok(output.text);
    };
    let catalog = amd_isa_catalog_for_target_v1(target).map_err(|error| error.to_string())?;
    let rows = inventory(catalog)?;
    let count = counts(&rows)?;
    let identity = catalog.identity();
    output.line(format_args!("AMD ISA catalog inspection (metadata only)"))?;
    output.line(format_args!("target_profile: {target}"))?;
    output.line(format_args!("architecture: {:?}", identity.architecture))?;
    output.line(format_args!("catalog_format: {}", identity.format))?;
    output.line(format_args!("archive_sha256: {}", identity.archive_sha256))?;
    output.line(format_args!("member: {}", identity.member))?;
    output.line(format_args!("member_sha256: {}", identity.member_sha256))?;
    output.line(format_args!(
        "metadata_sha256: {}",
        identity.metadata_sha256
    ))?;
    output.line(format_args!(
        "reviewed_coverage_sha256: {}",
        identity.reviewed_coverage_sha256
    ))?;
    output.line(format_args!(
        "grants_authority: {}",
        catalog.grants_authority()
    ))?;
    output
        .write_str(BOUNDARY)
        .map_err(|_| OUTPUT_LIMIT.to_owned())?;
    output.line(format_args!("instruction_families: {}", rows.len()))?;
    output.line(format_args!(
        "encoding_forms: {}",
        catalog.entries(AmdIsaMetadataKindV1::Encoding).len()
    ))?;
    output.line(format_args!(
        "operand_types: {}",
        catalog.entries(AmdIsaMetadataKindV1::OperandType).len()
    ))?;
    output.line(format_args!(
        "data_formats: {}",
        catalog.entries(AmdIsaMetadataKindV1::DataFormat).len()
    ))?;
    output.line(format_args!(
        "instruction_alternatives: {}",
        count.alternatives
    ))?;
    output.line(format_args!(
        "alternatives_with_missing_condition_definition: {}",
        count.missing_conditions
    ))?;
    output.line(format_args!(
        "alternatives_with_multiple_condition_declarations: {}",
        count.repeated_conditions
    ))?;
    output.line(format_args!(
        "reviewed_gfx942_u32_source_marker_families: {}",
        count.source_marker_families
    ))?;
    match mode {
        Mode::Summary => {}
        Mode::List => {
            output.line(format_args!(
                "Canonical inventory (family coverage only, never an encoding support list):"
            ))?;
            for instruction in rows {
                output.line(format_args!(
                    "instruction {:?} aliases={} alternatives={} reviewed_source_marker_family={}",
                    instruction.name(),
                    instruction.aliases().len(),
                    instruction.encodings().len(),
                    instruction.reviewed_source_marker_family()
                ))?;
            }
        }
        Mode::Instruction(name) => {
            let instruction = catalog.lookup(name).ok_or_else(|| {
                "instruction or explicit alias is unavailable in this exact catalog".to_owned()
            })?;
            describe(&mut output, instruction)?;
        }
    }
    Ok(output.text)
}

fn describe(output: &mut BoundedText, instruction: AmdIsaInstructionV1) -> Result<(), String> {
    output.line(format_args!(
        "queried_name: {:?}",
        instruction.queried_name()
    ))?;
    output.line(format_args!(
        "canonical_instruction: {:?}",
        instruction.name()
    ))?;
    output.line(format_args!(
        "lookup_kind: {}",
        if instruction.queried_name() == instruction.name() {
            "canonical"
        } else {
            "explicit_upstream_alias"
        }
    ))?;
    output.line(format_args!(
        "reviewed_gfx942_u32_source_marker_family: {}",
        available(instruction.reviewed_source_marker_family())
    ))?;
    if instruction.aliases().len() > MAX_ALIASES {
        return Err("instruction aliases exceed the inspector bound".into());
    }
    let mut aliases = instruction.aliases().to_vec();
    aliases.sort_unstable();
    output.line(format_args!("aliases: {aliases:?}"))?;
    let flags = instruction.flags();
    output.line(format_args!(
        "declared_flags: branch={} conditional_branch={} indirect_branch={} terminator={} immediately_executed={}",
        flags.is_branch(),
        flags.is_conditional_branch(),
        flags.is_indirect_branch(),
        flags.is_program_terminator(),
        flags.is_immediately_executed()
    ))?;
    for encoding in alternatives(instruction)? {
        let definition = encoding.definition();
        output.line(format_args!(
            "encoding {:?} condition={:?} opcode={} condition_declarations={} condition_definition={} encoding_definition={} encoding_bit_count={:?}",
            encoding.name(),
            encoding.condition(),
            encoding.opcode(),
            encoding.condition_declaration_count(),
            available(encoding.condition_definition_available()),
            available(definition.is_some()),
            definition.and_then(|row| row.bit_count())
        ))?;
        let operands = encoding.operands();
        if operands.len() > MAX_OPERANDS {
            return Err("encoding operands exceed the inspector bound".into());
        }
        let mut operands = operands.collect::<Vec<_>>();
        operands.sort_by_key(|row| row.order());
        for operand in operands {
            output.line(format_args!(
                "  operand order={} input={} output={} implicit={} binary_microcode_required={} field={:?} type={:?} type_definition={} format={:?} format_definition={} bits={}",
                operand.order(),
                operand.is_input(),
                operand.is_output(),
                operand.is_implicit(),
                operand.binary_microcode_required(),
                operand.field_name(),
                operand.operand_type_name(),
                available(operand.operand_type().is_some()),
                operand.data_format_name(),
                available(operand.data_format().is_some()),
                operand.bit_count()
            ))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "inspect_amd_isa_catalog_v1/tests.rs"]
mod tests;
