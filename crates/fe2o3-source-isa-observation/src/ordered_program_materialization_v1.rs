//! Checked local bitselect-graph materialization, never reverse compilation.
//!
//! This module is a child of multilevel_authoring_v1: it reads the immutable
//! typed owner rather than trusting serialized operation summaries. Explicit
//! source names and byte ranges remain caller assertions, not recovered SSA
//! names or HIR/source custody. No source replacement or frontend admission
//! occurs here; both must be checked separately against the retained baseline.

use std::{fmt, fmt::Write as _, str};

use fe2o3_kernel_ir::{
    BinaryOp, Gfx942OrderedProgramRegistersV1, Gfx942ProgramBinaryOpcodeV1,
    Gfx942ProgramDestinationV1, Gfx942ProgramInstructionV1, Gfx942ProgramRoleV1,
    Gfx942U32ProgramV1, OperationKind, ScalarType, Type, ValueId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    AuthoringErrorV1, AuthoringRegionSelectorV1, AuthoringSnapshotSummaryV1, AuthoringSnapshotV1,
    AuthoringSourceSpanV1, BoundedText, MAX_AUTHORING_SOURCE_BYTES_V1, bounded_report, hex,
    valid_helper_name,
};
use crate::source_edit_v1::{
    MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1, SourceEditErrorV1, SourceEditRangeV1,
    validate_source_edit_path_v1,
};

/// Explicit association, not a name inferred from a numeric SSA label.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedProgramValueBindingV1 {
    pub value: u32,
    pub identifier: String,
}

/// A byte commitment and a caller-proposed enclosing source range.
///
/// Neither the file identity nor the byte hash proves that an identifier names
/// the selected SSA value, or that the range is a replaceable Rust expression.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedProgramSourceBindingV1 {
    pub relative_path: String,
    pub expected_source_sha256: String,
    pub expected_source_bytes: u32,
    pub source_file_identity: String,
    pub source_display_path: String,
    pub source_range: SourceEditRangeV1,
    /// Exact role order: a, b, mask for b ^ ((a ^ b) & mask).
    pub inputs: [OrderedProgramValueBindingV1; 3],
    pub output: OrderedProgramValueBindingV1,
}

/// Untrusted register requests; construction reuses the checked kernel-IR type.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedProgramRegisterRequestV1 {
    pub scratch: u8,
    pub output: u8,
    pub inputs: [u8; 3],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedProgramMaterializationRequestV1 {
    pub selector: AuthoringRegionSelectorV1,
    pub source: OrderedProgramSourceBindingV1,
    pub registers: OrderedProgramRegisterRequestV1,
}

/// Inert checked plan. Private fields and no Deserialize prevent caller reports
/// from being promoted into checked plans. The contained program is not a
/// canonical owner, source occurrence receipt, or production-resume token.
///
/// ~~~compile_fail
/// use fe2o3_source_isa_observation::multilevel_authoring_v1::
///     ordered_program_materialization_v1::OrderedProgramMaterializationPlanV1;
/// let _: OrderedProgramMaterializationPlanV1 = serde_json::from_str("{}").unwrap();
/// ~~~
///
/// ~~~compile_fail
/// use fe2o3_source_isa_observation::multilevel_authoring_v1::
///     ordered_program_materialization_v1::OrderedProgramMaterializationPlanV1;
/// fn change(plan: &mut OrderedProgramMaterializationPlanV1) {
///     plan.expression = "unreviewed_source".into();
/// }
/// ~~~
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OrderedProgramMaterializationPlanV1 {
    schema: &'static str,
    baseline: AuthoringSnapshotSummaryV1,
    request: OrderedProgramMaterializationRequestV1,
    selected_source_spans: [AuthoringSourceSpanV1; 3],
    intermediate_values: [u32; 2],
    program_count: u8,
    /// Source descriptors, not AMD machine encodings.
    program_descriptors: [u16; 16],
    #[serde(skip)]
    program: Gfx942U32ProgramV1,
    #[serde(skip)]
    registers: Gfx942OrderedProgramRegistersV1,
    expression: String,
    statement: String,
    graph_relation: &'static str,
    source_binding_status: &'static str,
    source_application: &'static str,
    frontend_profile_admission: &'static str,
    required_wave_width: u32,
    required_workgroup_size: [u32; 3],
    requires_fresh_frontend_admission: bool,
    grants_source_authentication: bool,
    grants_proof_authority: bool,
    grants_production_resume: bool,
    grants_load_or_launch: bool,
    reversible_rust_roundtrip: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderedProgramMaterializationErrorV1 {
    Authoring(AuthoringErrorV1),
    Source(SourceEditErrorV1),
    UnsupportedGraph,
    InvalidValueBindings,
    InvalidSourceIdentifier,
    InvalidRegisters,
    InvalidProgramContract,
}

impl fmt::Display for OrderedProgramMaterializationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authoring(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::UnsupportedGraph => formatter.write_str(
                "selection must be exactly a pure u32 XOR/AND/XOR bitselect with three live-ins and one final live-out",
            ),
            Self::InvalidValueBindings => formatter.write_str(
                "explicit source bindings must match the exact ordered graph boundary",
            ),
            Self::InvalidSourceIdentifier => formatter.write_str(
                "source names must be distinct non-keyword ASCII identifiers of at most 64 bytes",
            ),
            Self::InvalidRegisters => formatter.write_str(
                "five explicit physical bindings must be distinct registers in v0..v63",
            ),
            Self::InvalidProgramContract => formatter.write_str(
                "literal bitselect program no longer satisfies the checked descriptor contract",
            ),
        }
    }
}

impl std::error::Error for OrderedProgramMaterializationErrorV1 {}
impl From<AuthoringErrorV1> for OrderedProgramMaterializationErrorV1 {
    fn from(value: AuthoringErrorV1) -> Self {
        Self::Authoring(value)
    }
}
impl From<SourceEditErrorV1> for OrderedProgramMaterializationErrorV1 {
    fn from(value: SourceEditErrorV1) -> Self {
        Self::Source(value)
    }
}
type Result<T> = std::result::Result<T, OrderedProgramMaterializationErrorV1>;

/// Checks the exact selected typed graph and renders one literal ordered unit.
///
/// Only this operand order is accepted: t0 = a ^ b; t1 = t0 & mask;
/// result = b ^ t1. Commuted, duplicated-input, effectful, wider, dead-result or
/// externally used intermediate graphs are not silently normalized into it.
/// The retained source range is a preview association, NOT permission to replace.
pub fn prepare_ordered_program_materialization_v1(
    snapshot: &AuthoringSnapshotV1,
    request: &OrderedProgramMaterializationRequestV1,
    original: &[u8],
) -> Result<OrderedProgramMaterializationPlanV1> {
    snapshot.check_selector(&request.selector)?;
    if request.selector.target != "gfx942:xnack-" {
        return Err(AuthoringErrorV1::IncompatibleTarget.into());
    }
    if request.selector.operations.len() != 3 {
        return Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph);
    }
    let region = snapshot.select_region(&request.selector)?;
    let function = request.selector.operations[0].function;
    let mut graph = [(ValueId(0), ValueId(0), ValueId(0)); 3];
    for (index, coordinate) in request.selector.operations.iter().enumerate() {
        let operation = snapshot.operation(*coordinate)?;
        let OperationKind::Binary { op, lhs, rhs } = &operation.kind else {
            return Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph);
        };
        let [result] = operation.results.as_slice() else {
            return Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph);
        };
        let expected = if index == 1 {
            BinaryOp::BitAnd
        } else {
            BinaryOp::BitXor
        };
        if *op != expected
            || result.ty != Type::Scalar(ScalarType::U32)
            || [*lhs, *rhs].iter().any(|value| {
                snapshot.value_type(function, *value) != Some(&Type::Scalar(ScalarType::U32))
            })
            || !region.operations[index].complete_local_effect_summary
            || !region.operations[index].local_memory_effects.is_empty()
        {
            return Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph);
        }
        graph[index] = (*lhs, *rhs, result.id);
    }
    let (a, b, first) = graph[0];
    let (and_left, mask, second) = graph[1];
    let (xor_left, xor_right, output) = graph[2];
    let values = [a, b, mask, first, second, output];
    if and_left != first
        || xor_left != b
        || xor_right != second
        || values
            .iter()
            .enumerate()
            .any(|(index, value)| values[..index].contains(value))
        || region
            .live_in
            .iter()
            .map(|value| value.value)
            .ne([a.0, b.0, mask.0])
        || region
            .live_out
            .iter()
            .map(|value| value.value)
            .ne([output.0])
    {
        return Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph);
    }
    if request
        .source
        .inputs
        .iter()
        .map(|value| value.value)
        .ne([a.0, b.0, mask.0])
        || request.source.output.value != output.0
    {
        return Err(OrderedProgramMaterializationErrorV1::InvalidValueBindings);
    }
    let names = [
        request.source.inputs[0].identifier.as_str(),
        request.source.inputs[1].identifier.as_str(),
        request.source.inputs[2].identifier.as_str(),
        request.source.output.identifier.as_str(),
    ];
    if names
        .iter()
        .enumerate()
        .any(|(index, name)| !valid_helper_name(name) || names[..index].contains(name))
    {
        return Err(OrderedProgramMaterializationErrorV1::InvalidSourceIdentifier);
    }
    validate_baseline(&request.source, &request.source.relative_path, original)?;
    let source = str::from_utf8(original).map_err(|_| SourceEditErrorV1::InvalidUtf8)?;
    let mut selected_spans = Vec::with_capacity(3);
    for operation in &region.operations {
        let span = match operation.source_spans.as_slice() {
            [] => return Err(SourceEditErrorV1::MissingSourceSpan.into()),
            [span] => span,
            _ => return Err(SourceEditErrorV1::AmbiguousSourceSpan.into()),
        };
        if span.file_identity != request.source.source_file_identity
            || span.display_path != request.source.source_display_path
        {
            return Err(SourceEditErrorV1::SourceSpanSubstitution.into());
        }
        let start = span_offset(&span.byte_start)?;
        let end = span_offset(&span.byte_end)?;
        validate_nonempty_range(source, SourceEditRangeV1 { start, end })?;
        if start < request.source.source_range.start || end > request.source.source_range.end {
            return Err(SourceEditErrorV1::InvalidByteRange.into());
        }
        selected_spans.push(span.clone());
    }
    let registers = Gfx942OrderedProgramRegistersV1::new(
        request.registers.scratch,
        request.registers.output,
        request.registers.inputs,
    )
    .map_err(|_| OrderedProgramMaterializationErrorV1::InvalidRegisters)?;
    let program = bitselect_program()?;
    let (expression, statement) = render(&request.source, registers)?;
    bounded_report(OrderedProgramMaterializationPlanV1 {
        schema: "fe2o3-ordered-program-materialization-plan-v1",
        baseline: snapshot.summary(),
        request: request.clone(),
        selected_source_spans: selected_spans.try_into()
            .map_err(|_| OrderedProgramMaterializationErrorV1::UnsupportedGraph)?,
        intermediate_values: [first.0, second.0],
        program_count: program.count(),
        program_descriptors: *program.descriptors(),
        program,
        registers,
        expression,
        statement,
        graph_relation: "exact_typed_u32_b_xor_a_xor_b_and_mask_no_external_intermediate_use",
        source_binding_status: "explicit_caller_names_and_bytes_not_authenticated_ssa_to_source_bindings",
        source_application: "unavailable_requires_checked_hir_range_and_scope_binding",
        frontend_profile_admission: "not_performed_requires_direct_root_target_wave_launch_and_occurrence_checks",
        required_wave_width: 64,
        required_workgroup_size: [64, 1, 1],
        requires_fresh_frontend_admission: true,
        grants_source_authentication: false,
        grants_proof_authority: false,
        grants_production_resume: false,
        grants_load_or_launch: false,
        reversible_rust_roundtrip: false,
    }).map_err(Into::into)
}

impl OrderedProgramMaterializationPlanV1 {
    pub fn baseline(&self) -> &AuthoringSnapshotSummaryV1 {
        &self.baseline
    }
    pub fn request(&self) -> &OrderedProgramMaterializationRequestV1 {
        &self.request
    }
    pub fn selected_source_spans(&self) -> &[AuthoringSourceSpanV1; 3] {
        &self.selected_source_spans
    }
    pub fn program(&self) -> &Gfx942U32ProgramV1 {
        &self.program
    }
    pub const fn registers(&self) -> Gfx942OrderedProgramRegistersV1 {
        self.registers
    }
    /// Expression-only draft for a future independently checked HIR replacement.
    pub fn expression(&self) -> &str {
        &self.expression
    }
    /// Complete let-statement draft, using only the explicitly supplied output name.
    pub fn statement(&self) -> &str {
        &self.statement
    }

    /// Revalidates the original immutable owner and independent source commitment.
    /// It still does not establish names, HIR boundaries, or filesystem custody.
    pub fn validate_current(
        &self,
        snapshot: &AuthoringSnapshotV1,
        relative_path: &str,
        original: &[u8],
    ) -> Result<()> {
        snapshot.check_selector(&self.request.selector)?;
        if snapshot.summary() != self.baseline {
            return Err(AuthoringErrorV1::StaleBundleIdentity.into());
        }
        validate_baseline(&self.request.source, relative_path, original)
    }
}

fn bitselect_program() -> Result<Gfx942U32ProgramV1> {
    use Gfx942ProgramBinaryOpcodeV1::{And, Xor};
    use Gfx942ProgramDestinationV1::{Output, Scratch};
    use Gfx942ProgramRoleV1::{Input0, Input1, Input2, Scratch as ScratchValue};
    Gfx942U32ProgramV1::from_instructions(&[
        Gfx942ProgramInstructionV1::Binary {
            opcode: Xor,
            destination: Scratch,
            left: Input0,
            right: Input1,
        },
        Gfx942ProgramInstructionV1::Binary {
            opcode: And,
            destination: Scratch,
            left: ScratchValue,
            right: Input2,
        },
        Gfx942ProgramInstructionV1::Binary {
            opcode: Xor,
            destination: Output,
            left: Input1,
            right: ScratchValue,
        },
    ])
    .map_err(|_| OrderedProgramMaterializationErrorV1::InvalidProgramContract)
}

fn render(
    source: &OrderedProgramSourceBindingV1,
    registers: Gfx942OrderedProgramRegistersV1,
) -> Result<(String, String)> {
    let mut text = BoundedText::new(MAX_AUTHORING_SOURCE_BYTES_V1);
    writeln!(text, "fe2o3_device::amdgpu_ordered_program! {{")
        .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    writeln!(text, "    gfx942_xnack_off_wave64;").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    writeln!(
        text,
        "    scratch({}); out({});",
        registers.scratch(),
        registers.output()
    )
    .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    for (register, input) in registers.inputs().iter().zip(&source.inputs) {
        writeln!(text, "    in({register}) = {};", input.identifier)
            .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    }
    text.write_str("    xor(scratch, input0, input1);\n    and(scratch, scratch, input2);\n    xor(out, input1, scratch);\n}")
        .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    let expression = text.text;
    let mut statement = BoundedText::new(MAX_AUTHORING_SOURCE_BYTES_V1);
    writeln!(
        statement,
        "let {}: u32 = {expression};",
        source.output.identifier
    )
    .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    Ok((expression, statement.text))
}

fn validate_baseline(
    binding: &OrderedProgramSourceBindingV1,
    relative_path: &str,
    original: &[u8],
) -> Result<()> {
    validate_source_edit_path_v1(relative_path)?;
    if relative_path != binding.relative_path {
        return Err(SourceEditErrorV1::PathMismatch.into());
    }
    if original.is_empty() {
        return Err(SourceEditErrorV1::EmptySource.into());
    }
    if original.len() > MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 {
        return Err(SourceEditErrorV1::ResourceLimit.into());
    }
    let source = str::from_utf8(original).map_err(|_| SourceEditErrorV1::InvalidUtf8)?;
    if binding.expected_source_sha256.len() != 64
        || !binding
            .expected_source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SourceEditErrorV1::InvalidDigest.into());
    }
    let digest: [u8; 32] = Sha256::digest(original).into();
    if original.len() != binding.expected_source_bytes as usize
        || hex(&digest) != binding.expected_source_sha256
    {
        return Err(SourceEditErrorV1::StaleSource.into());
    }
    validate_nonempty_range(source, binding.source_range)
}

fn validate_nonempty_range(source: &str, range: SourceEditRangeV1) -> Result<()> {
    let start = range.start as usize;
    let end = range.end as usize;
    if start >= end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err(SourceEditErrorV1::InvalidByteRange.into());
    }
    Ok(())
}

fn span_offset(value: &str) -> Result<u32> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| SourceEditErrorV1::InvalidByteRange)?;
    if parsed.to_string() != value {
        return Err(SourceEditErrorV1::InvalidByteRange.into());
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "ordered_program_materialization_v1_tests.rs"]
mod tests;
