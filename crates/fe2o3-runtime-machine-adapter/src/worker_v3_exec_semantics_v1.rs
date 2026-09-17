//! Bind machine observations to the exact selected Worker V3 artifact.

use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, WorkerV3SemanticMachineRefinementRequestV1,
};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1, Gfx942ExecExecutionErrorV1,
    Gfx942ExecLimitsV1, Gfx942ExecObservationV1, Gfx942ExecSliceV1, Gfx942ExecStateV1,
    execute_authenticated_gfx942_exec_slice_v1,
};

/// Refusal to associate authenticated machine analysis with a live proof request.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3ExecSemanticsErrorV1 {
    /// The analyzer inspected different complete artifact bytes.
    Artifact,
    /// The selected entry is missing, duplicated, or has a different symbol.
    Entry,
    /// The analyzer and inspected descriptor disagree on the selected code range.
    Range,
    /// The selected ISA is not the exact in-bounds artifact range.
    Isa,
    /// The requested control slice lies outside the selected kernel entry.
    Slice,
    /// The bounded interpreter rejected an instruction, live-in, or resource limit.
    Execution(Gfx942ExecExecutionErrorV1),
}

impl core::fmt::Display for WorkerV3ExecSemanticsErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Worker V3 executable-control subject mismatch: {self:?}")
    }
}

impl std::error::Error for WorkerV3ExecSemanticsErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Execution(error) => Some(error),
            _ => None,
        }
    }
}

/// Observe a supported EXEC/control slice of this exact selected executable.
///
/// This performs CPU interpretation, not GPU dispatch. Every live-in is an
/// explicit caller assumption. The observation retains the authenticated
/// analyzer identity but proves neither its policy's deployment approval nor
/// source semantics, memory safety, floating-point behavior, or kernel completion.
/// It cannot be substituted for Worker V3 semantic-machine refinement evidence.
pub fn observe_worker_v3_gfx942_exec_slice_v1<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3SemanticMachineRefinementRequestV1<'_, '_, K>,
    execution: &AuthenticatedPhysicalMachineAnalysisExecutionV1,
    slice: Gfx942ExecSliceV1<'_>,
    initial: &Gfx942ExecStateV1,
    limits: Gfx942ExecLimitsV1,
) -> Result<Gfx942ExecObservationV1, WorkerV3ExecSemanticsErrorV1> {
    check_worker_v3_gfx942_exec_subject_v1(request, execution)?;
    let binding = request.verification_request().descriptor_binding();
    check_selected_slice(
        K::EXPORT_NAME,
        binding.entry_file_offset(),
        binding.entry_size(),
        slice,
    )?;
    execute_authenticated_gfx942_exec_slice_v1(execution, slice, initial, limits)
        .map_err(WorkerV3ExecSemanticsErrorV1::Execution)
}

fn check_selected_slice(
    symbol: &str,
    offset: u64,
    size: u64,
    slice: Gfx942ExecSliceV1<'_>,
) -> Result<(), WorkerV3ExecSemanticsErrorV1> {
    let end = offset
        .checked_add(size)
        .ok_or(WorkerV3ExecSemanticsErrorV1::Range)?;
    // The stop instruction belongs to the same entry but is not executed.
    if slice.function_symbol() != symbol
        || slice.start_offset() < offset
        || slice.stop_offset() >= end
    {
        return Err(WorkerV3ExecSemanticsErrorV1::Slice);
    }
    Ok(())
}

/// Check complete artifact bytes and the exact selected symbol and code range.
///
/// The existing analyzer owner authenticates its execution; this comparison
/// only associates that execution with this request. It does not approve the
/// analyzer deployment policy, prove source/machine refinement, establish the
/// initial register state, or authorize loading or launching the artifact.
pub fn check_worker_v3_gfx942_exec_subject_v1<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3SemanticMachineRefinementRequestV1<'_, '_, K>,
    execution: &AuthenticatedPhysicalMachineAnalysisExecutionV1,
) -> Result<(), WorkerV3ExecSemanticsErrorV1> {
    let verification = request.verification_request();
    let binding = verification.descriptor_binding();
    let symbol = verification.descriptor().entry_name().as_str();
    if symbol != K::EXPORT_NAME {
        return Err(WorkerV3ExecSemanticsErrorV1::Entry);
    }
    let mut entries = execution
        .analysis()
        .effects()
        .entry_points()
        .iter()
        .filter(|entry| entry.symbol() == symbol);
    let entry = entries.next().ok_or(WorkerV3ExecSemanticsErrorV1::Entry)?;
    if entries.next().is_some() {
        return Err(WorkerV3ExecSemanticsErrorV1::Entry);
    }
    check_exact_subject(
        MachineSubject {
            payload: request.finalized_hsaco_bytes(),
            symbol,
            offset: binding.entry_file_offset(),
            size: binding.entry_size(),
        },
        MachineSubject {
            payload: execution.request().exact_payload_bytes(),
            symbol: entry.symbol(),
            offset: entry.code_offset(),
            size: entry.code_size(),
        },
        request.selected_isa_bytes(),
    )
}

struct MachineSubject<'a> {
    payload: &'a [u8],
    symbol: &'a str,
    offset: u64,
    size: u64,
}

fn check_exact_subject(
    expected: MachineSubject<'_>,
    actual: MachineSubject<'_>,
    selected_isa: &[u8],
) -> Result<(), WorkerV3ExecSemanticsErrorV1> {
    use WorkerV3ExecSemanticsErrorV1 as Error;
    if expected.payload != actual.payload {
        return Err(Error::Artifact);
    }
    if expected.symbol != actual.symbol {
        return Err(Error::Entry);
    }
    if expected.offset != actual.offset || expected.size != actual.size || expected.size == 0 {
        return Err(Error::Range);
    }
    let start = usize::try_from(expected.offset).map_err(|_| Error::Range)?;
    let size = usize::try_from(expected.size).map_err(|_| Error::Range)?;
    let end = start.checked_add(size).ok_or(Error::Range)?;
    let exact = expected.payload.get(start..end).ok_or(Error::Range)?;
    if exact != selected_isa {
        return Err(Error::Isa);
    }
    Ok(())
}

#[cfg(test)]
#[path = "worker_v3_exec_semantics_v1_tests.rs"]
mod tests;
