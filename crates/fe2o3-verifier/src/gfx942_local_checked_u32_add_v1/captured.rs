//! Content-bound genuine lowering capture, not a source-value simulation theorem.

use super::*;
use fe2o3_lower_mir_kernel::ProductionCheckedU32AddCaptureV1;

/// The original conditional machine obligation and its actual lowering occurrence.
///
/// The capture borrows its own compiler owner. The separately decoded V4 inputs
/// must contain exactly that owner's MIR and typed V8 bytes. This content join
/// does not authenticate compiler origin or discharge either entry-value equality.
#[derive(Debug)]
pub struct Gfx942CapturedMovPrefixCheckedU32AddObligationV1<'a> {
    local: Gfx942LocalMovPrefixCheckedU32AddObligationV1<'a>,
    capture: ProductionCheckedU32AddCaptureV1<'a>,
}

impl Gfx942CapturedMovPrefixCheckedU32AddObligationV1<'_> {
    pub const fn local_obligation(&self) -> &Gfx942LocalMovPrefixCheckedU32AddObligationV1<'_> {
        &self.local
    }

    pub const fn capture(&self) -> &ProductionCheckedU32AddCaptureV1<'_> {
        &self.capture
    }
}

/// Joins actual source/SSA/emission custody to the existing conditional span.
///
/// No detached IDs or second compiler owner can stand in for the capture's
/// original owner. Exact bytes are required; versions are never reserialized.
/// Source and KIR state semantics, physical entry values, protected compiler
/// execution and whole-program continuation remain separate obligations.
pub fn check_gfx942_captured_mov_prefix_checked_u32_add_v1<'a>(
    inputs: &'a ValidatedCompilerProofInputsV4,
    execution: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    capture: ProductionCheckedU32AddCaptureV1<'a>,
    anchor_index: usize,
    first_offset: u64,
    add_offset: u64,
) -> Result<
    Gfx942CapturedMovPrefixCheckedU32AddObligationV1<'a>,
    Gfx942CapturedMovPrefixCheckedU32AddErrorV1,
> {
    use Gfx942CapturedMovPrefixCheckedU32AddErrorV1 as E;
    let owner = capture.owner();
    if owner.semantic().semantic().canonical_encoding()
        != inputs.semantic_mir().canonical_encoding()
    {
        return Err(E::SemanticOwner);
    }
    let kernel_ir = owner.canonical_kernel_ir_v8().ok_or(E::KernelIrOwner)?;
    let input_kernel_ir = inputs.kernel_ir().as_v8().ok_or(E::KernelIrOwner)?;
    if kernel_ir.identity() != input_kernel_ir.identity()
        || kernel_ir.canonical_bytes() != input_kernel_ir.canonical_bytes()
    {
        return Err(E::KernelIrOwner);
    }
    let local = check_gfx942_local_mov_prefix_checked_u32_add_v1(
        inputs,
        execution,
        anchor_index,
        first_offset,
        add_offset,
    )
    .map_err(E::Local)?;
    let request = capture.request();
    let anchor = local.anchor();
    let entry = local.unresolved_entry_relation();
    let results = local.conditional_results();
    if !inputs.semantic_mir().roots().contains(&request.root())
        || request.function().index() != anchor.semantic_function()
        || request.block().index() != anchor.semantic_block()
        || request.statement() != anchor.semantic_statement()
        || capture.lhs_local().index() != entry.semantic_local
        || capture.tuple_local().index() != results.semantic_result_local
        || capture.use_event().checked_add(1) != Some(capture.define_event())
        || capture.kernel_ir_function() != local.function_symbol()
        || capture.block().0 != anchor.kernel_ir_block()
        || capture.operation() != anchor.kernel_ir_operation()
        || capture.operand().0 != entry.kernel_ir_value
        || capture.value().0 != results.kernel_ir_value
        || capture.overflow().0 != results.kernel_ir_overflow
        || capture.literal() != local.literal()
    {
        return Err(E::Coordinates);
    }
    Ok(Gfx942CapturedMovPrefixCheckedU32AddObligationV1 { local, capture })
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Gfx942CapturedMovPrefixCheckedU32AddErrorV1 {
    SemanticOwner,
    KernelIrOwner,
    Coordinates,
    Local(Gfx942LocalMovPrefixCheckedU32AddErrorV1),
}

impl fmt::Display for Gfx942CapturedMovPrefixCheckedU32AddErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "mismatched captured checked-u32-add obligation: {self:?}"
        )
    }
}

impl Error for Gfx942CapturedMovPrefixCheckedU32AddErrorV1 {}
