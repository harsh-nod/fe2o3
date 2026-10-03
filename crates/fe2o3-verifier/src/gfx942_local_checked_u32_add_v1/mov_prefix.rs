//! Borrowed conditional coordinates for a complete MOV-prefix/ADD interval.
//! Entry equality, reachability, continuation and compiler lineage stay unresolved.

use super::*;
use fe2o3_kernel_analysis::{
    Gfx942MovPrefixAddErrorV1, Gfx942MovPrefixAddU32V1, Gfx942SMovB32V1, Gfx942U32OriginV1,
};

/// This object retains the exact authenticated input owners, not a proof that
/// the caller's source local equals the derived SGPR at span entry.
#[derive(Debug)]
pub struct Gfx942LocalMovPrefixCheckedU32AddObligationV1<'a> {
    inputs: &'a ValidatedCompilerProofInputsV4,
    execution: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    coordinates: PrefixCoordinates,
}

impl Gfx942LocalMovPrefixCheckedU32AddObligationV1<'_> {
    pub const fn inputs(&self) -> &ValidatedCompilerProofInputsV4 {
        self.inputs
    }

    pub const fn execution(&self) -> &AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.execution
    }

    pub const fn anchor(&self) -> VerifiedSemanticU32InductionKirAnchorV1 {
        self.coordinates.source.anchor
    }

    pub fn function_symbol(&self) -> &str {
        &self.coordinates.source.function
    }

    pub const fn literal(&self) -> u32 {
        self.coordinates.source.literal
    }

    /// Both source equalities are hypotheses at `machine_span().first_offset()`.
    /// Reaching this entry with those values is also an unresolved obligation.
    pub const fn unresolved_entry_relation(&self) -> UnresolvedCheckedU32AddEntryRelationV1 {
        self.coordinates.entry
    }

    pub const fn conditional_results(&self) -> ConditionalCheckedU32AddResultsV1 {
        self.coordinates.results
    }

    pub const fn machine_input_definition(&self) -> Gfx942ReachingDefinitionV1 {
        self.coordinates.definition
    }

    pub const fn machine_span(&self) -> &Gfx942MovPrefixAddU32V1 {
        &self.coordinates.machine
    }
}

/// Extend only the physical interval: the existing exact MIR/KIR source profile
/// and its unresolved entry equalities are unchanged. No authority is returned.
pub fn check_gfx942_local_mov_prefix_checked_u32_add_v1<'a>(
    inputs: &'a ValidatedCompilerProofInputsV4,
    execution: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    anchor_index: usize,
    first_offset: u64,
    add_offset: u64,
) -> Result<
    Gfx942LocalMovPrefixCheckedU32AddObligationV1<'a>,
    Gfx942LocalMovPrefixCheckedU32AddErrorV1,
> {
    use Gfx942LocalMovPrefixCheckedU32AddErrorV1 as E;
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes())
        .map_err(|error| E::Source(Gfx942LocalCheckedU32AddErrorV1::KernelIr(error)))?;
    let source = source_coordinates(inputs, &module, anchor_index).map_err(E::Source)?;
    let coordinates = prefix_machine_coordinates(
        source,
        execution.request(),
        execution.analysis(),
        first_offset,
        add_offset,
    )?;
    Ok(Gfx942LocalMovPrefixCheckedU32AddObligationV1 {
        inputs,
        execution,
        coordinates,
    })
}

#[derive(Debug)]
struct PrefixCoordinates {
    source: SourceCoordinates,
    machine: Gfx942MovPrefixAddU32V1,
    entry: UnresolvedCheckedU32AddEntryRelationV1,
    results: ConditionalCheckedU32AddResultsV1,
    definition: Gfx942ReachingDefinitionV1,
}

fn prefix_machine_coordinates(
    source: SourceCoordinates,
    request: &PhysicalMachineEffectRequestV1,
    analysis: &PhysicalMachineAnalysisEvidenceV1,
    first_offset: u64,
    add_offset: u64,
) -> Result<PrefixCoordinates, Gfx942LocalMovPrefixCheckedU32AddErrorV1> {
    use Gfx942LocalMovPrefixCheckedU32AddErrorV1 as E;
    if analysis.effects().request_identity() != request.identity()
        || analysis.effects().payload_identity() != request.payload_identity()
        || request
            .entries()
            .iter()
            .filter(|entry| entry.symbol() == source.function)
            .count()
            != 1
    {
        return Err(E::MachineBinding);
    }
    let trace = analysis.trace();
    let machine =
        Gfx942MovPrefixAddU32V1::decode(trace, &source.function, first_offset, add_offset)
            .map_err(E::Span)?;
    let register = match machine.terminal_origins() {
        [
            Gfx942U32OriginV1::EntrySgpr(register),
            Gfx942U32OriginV1::Constant(literal),
        ]
        | [
            Gfx942U32OriginV1::Constant(literal),
            Gfx942U32OriginV1::EntrySgpr(register),
        ] if literal == source.literal => register,
        _ => return Err(E::LiteralMismatch),
    };
    let dataflow = Gfx942MachineDataflowV1::derive(trace).map_err(E::Dataflow)?;
    let definitions = dataflow
        .reaching_definitions_before(
            &source.function,
            first_offset,
            Gfx942RegisterUnitV1::Sgpr(u16::from(register)),
        )
        .map_err(E::Dataflow)?;
    let [definition] = definitions.as_slice() else {
        return Err(E::AmbiguousMachineDefinition);
    };
    if let Gfx942ReachingDefinitionV1::Instruction { offset } = *definition {
        if (first_offset..=add_offset).contains(&offset) {
            return Err(E::UnsupportedMachineDefinition);
        }
        let mut definitions = trace.instructions().iter().filter(|instruction| {
            instruction.function_symbol() == source.function
                && instruction.instruction_offset() == offset
        });
        let instruction = definitions.next().ok_or(E::MachineBinding)?;
        if definitions.next().is_some() {
            return Err(E::MachineBinding);
        }
        let destination = Gfx942SMovB32V1::decode(instruction)
            .map(|instruction| instruction.destination())
            .or_else(|_| {
                Gfx942SAddU32V1::decode(instruction).map(|instruction| instruction.destination())
            })
            .map_err(|_| E::UnsupportedMachineDefinition)?;
        if destination != register
            || !dataflow
                .instruction_dominates(&source.function, offset, first_offset)
                .map_err(E::Dataflow)?
        {
            return Err(E::UnsupportedMachineDefinition);
        }
    }
    let entry = UnresolvedCheckedU32AddEntryRelationV1 {
        semantic_function: source.anchor.semantic_function(),
        semantic_local: source.local,
        kernel_ir_value: source.lhs,
        machine_source_sgpr: register,
        machine_instruction_offset: first_offset,
    };
    let results = ConditionalCheckedU32AddResultsV1 {
        semantic_result_local: source.result_local,
        kernel_ir_value: source.anchor.value_result(),
        kernel_ir_overflow: source.anchor.overflow_result(),
        machine_destination_sgpr: machine.terminal_add().destination(),
    };
    Ok(PrefixCoordinates {
        source,
        machine,
        entry,
        results,
        definition: *definition,
    })
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Gfx942LocalMovPrefixCheckedU32AddErrorV1 {
    Source(Gfx942LocalCheckedU32AddErrorV1),
    Span(Gfx942MovPrefixAddErrorV1),
    MachineBinding,
    LiteralMismatch,
    Dataflow(Gfx942MachineDataflowErrorV1),
    AmbiguousMachineDefinition,
    UnsupportedMachineDefinition,
}

impl fmt::Display for Gfx942LocalMovPrefixCheckedU32AddErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported local MOV-prefix checked-u32-add obligation: {self:?}"
        )
    }
}

impl Error for Gfx942LocalMovPrefixCheckedU32AddErrorV1 {}

#[cfg(test)]
mod tests;
