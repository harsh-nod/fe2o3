//! Closed repeat spelling recognition against the already observed typed program.
//! No source owner, parser, const evaluator, or alternate executable is created.
use super::{
    BoundedText, Coordinates, Error, HELPER_CAP, OrderedCompositionTypedEditV1, Result, SOURCE_CAP,
    identifier, role,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, Gfx942ProgramBinaryOpcodeV1 as Op,
    Gfx942ProgramInstructionV1 as Step, Gfx942U32ProgramV1 as Program,
};
use std::fmt::Write as _;

const MAX_STEPS: usize = 16;
const MAX_REPETITIONS: usize = 15;
const MAX_CANDIDATES: usize = 45;
pub(super) const MATCH_WORK: usize = 512 * 1024;
pub(super) const MATCH_SCRATCH: usize = 16 * 1024;
// One <=64KiB scan, then <=45 bounded template writes/comparisons plus fixed
// descriptor work. These are conservative logical units, not elapsed time.
const WORK_BOUND: usize = SOURCE_CAP + MAX_CANDIDATES * (2 * HELPER_CAP + 256);
// Compact bytes, one live template allocation, bounded descriptor copies,
// returned/moved program headers and formatting/counter bookkeeping.
const SCRATCH_BOUND: usize =
    2 * HELPER_CAP + 4 * size_of::<Program>() + 4 * size_of::<[u16; MAX_STEPS]>() + 1024;
const _: () = assert!(WORK_BOUND <= MATCH_WORK && SCRATCH_BOUND <= MATCH_SCRATCH);

pub(super) fn require(
    original: &str,
    coordinates: &Coordinates,
    actual: OrderedCompositionTypedEditV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    // No repeat source normalization, candidate enumeration or allocation until
    // this exact original ledger has admitted all extra work and scratch.
    budget.with_prepaid_scope(budget.storage(), 0, MATCH_WORK, MATCH_SCRATCH, |_| {
        if original.len() > SOURCE_CAP {
            return Err(Error::refused("publisher repeat source bound exceeded"));
        }
        let mut names = [""; 3];
        for (index, range) in coordinates.arguments.iter().enumerate() {
            names[index] = original
                .get(range.clone())
                .filter(|name| identifier(name))
                .ok_or_else(|| Error::refused("publisher repeat parameter range differs"))?;
        }
        let selected = original
            .get(coordinates.selected.clone())
            .ok_or_else(|| Error::refused("publisher repeat macro range differs"))?;
        if recognize(selected, names, actual)? {
            Ok(())
        } else {
            Err(Error::refused(
                "publisher supports only exact flat or literal-repeat program spelling",
            ))
        }
    })
}

fn compact(source: &str, bytes: &mut [u8; HELPER_CAP]) -> Result<usize> {
    if source.len() > SOURCE_CAP {
        return Err(Error::refused("publisher repeat source bound exceeded"));
    }
    let mut length = 0;
    for byte in source.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        let slot = bytes
            .get_mut(length)
            .ok_or_else(|| Error::refused("publisher repeat normalized source bound exceeded"))?;
        *slot = byte;
        length += 1;
    }
    Ok(length)
}

/// Re-expand the proposed prefix/period independently. Include count and all
/// zero padding in the comparison to the observed program, not only an output
/// value or a prefix of its descriptors.
fn expanded(
    actual: &Program,
    initial: usize,
    repeated: usize,
    repetitions: usize,
) -> Result<Option<Program>> {
    if initial == 0 || repeated == 0 || !(1..=MAX_REPETITIONS).contains(&repetitions) {
        return Err(Error::refused(
            "publisher repeat decomposition bound differs",
        ));
    }
    let count = repeated
        .checked_mul(repetitions)
        .and_then(|n| initial.checked_add(n))
        .filter(|&n| n <= MAX_STEPS && n == usize::from(actual.count()))
        .ok_or_else(|| Error::refused("publisher repeat expanded count differs"))?;
    let boundary = initial
        .checked_add(repeated)
        .filter(|&n| n <= count)
        .ok_or_else(|| Error::refused("publisher repeat block boundary differs"))?;
    let source = actual.descriptors();
    let mut words = [0_u16; MAX_STEPS];
    words[..initial].copy_from_slice(&source[..initial]);
    for copy in 0..repetitions {
        let start = initial + copy * repeated;
        words[start..start + repeated].copy_from_slice(&source[initial..boundary]);
    }
    if words != *actual.descriptors() {
        return Ok(None);
    }
    Program::from_descriptors(count as u8, words)
        .map(Some)
        .map_err(|_| Error::refused("publisher repeat expansion is not a valid program"))
}

fn instruction(out: &mut BoundedText, descriptor: u16) -> Result<()> {
    let step = Step::from_descriptor(descriptor)
        .map_err(|_| Error::refused("publisher repeat instruction differs"))?;
    match step {
        Step::Move {
            destination,
            source,
        } => {
            write!(out, "mov({},{});", role(destination.role()), role(source))
        }
        Step::Binary {
            opcode,
            destination,
            left,
            right,
        } => {
            let opcode = match opcode {
                Op::Add => "add",
                Op::Subtract => "sub",
                Op::And => "and",
                Op::Or => "or",
                Op::Xor => "xor",
            };
            write!(
                out,
                "{opcode}({},{},{});",
                role(destination.role()),
                role(left),
                role(right)
            )
        }
    }
    .map_err(|_| Error::refused("publisher repeat template bound exceeded"))
}

fn template(
    names: [&str; 3],
    actual: OrderedCompositionTypedEditV1,
    initial: usize,
    repeated: usize,
    repetitions: usize,
) -> Result<String> {
    let mut out = BoundedText::new(HELPER_CAP)?;
    let [a, b, c] = actual.registers.inputs();
    write!(
        out,
        "fe2o3_device::amdgpu_ordered_program!{{gfx942_xnack_off_wave64;scratch({});out({});in({a})={};in({b})={};in({c})={};init{{",
        actual.registers.scratch(), actual.registers.output(), names[0], names[1], names[2],
    )
    .map_err(|_| Error::refused("publisher repeat header bound exceeded"))?;
    for &descriptor in &actual.program.descriptors()[..initial] {
        instruction(&mut out, descriptor)?;
    }
    write!(out, "}}repeat({repetitions}){{")
        .map_err(|_| Error::refused("publisher repeat count bound exceeded"))?;
    for &descriptor in &actual.program.descriptors()[initial..initial + repeated] {
        instruction(&mut out, descriptor)?;
    }
    out.push("}}")?;
    Ok(out.value)
}

fn recognize(
    selected: &str,
    names: [&str; 3],
    actual: OrderedCompositionTypedEditV1,
) -> Result<bool> {
    let mut bytes = [0_u8; HELPER_CAP];
    let length = compact(selected, &mut bytes)?;
    if !names.iter().all(|name| identifier(name)) {
        return Err(Error::refused("publisher repeat parameters differ"));
    }
    let count = usize::from(actual.program.count());
    if !(2..=MAX_STEPS).contains(&count) {
        return Ok(false);
    }
    let mut candidates = 0;
    let mut matched = false;
    // For count<=16, sum(floor((count-1)/r), r=1..15) <=45. No
    // source-dependent loop bound, dynamic parser or arbitrary const evaluation.
    for repetitions in 1..=MAX_REPETITIONS {
        for repeated in 1..=(count - 1) / repetitions {
            candidates += 1;
            if candidates > MAX_CANDIDATES {
                return Err(Error::refused("publisher repeat candidate bound exceeded"));
            }
            let initial = count - repeated * repetitions;
            if expanded(&actual.program, initial, repeated, repetitions)? != Some(actual.program) {
                continue;
            }
            let candidate = template(names, actual, initial, repeated, repetitions)?;
            if candidate.as_bytes() == &bytes[..length] {
                if matched {
                    return Err(Error::refused("publisher repeat source is ambiguous"));
                }
                matched = true;
            }
        }
    }
    Ok(matched)
}

#[cfg(test)]
#[path = "production_ordered_composition_publish_repeat_v1_tests.rs"]
mod tests;
