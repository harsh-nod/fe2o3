//! Shared fixed tooling bounds and request/file mechanics; no typed-owner erasure.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::mem::size_of;

pub(super) const KIR_BYTES: usize = 128 * 1024;
pub(super) const REQUEST_BYTES: usize = 16 * 1024;
#[derive(Clone, Copy)]
pub(super) enum Profile {
    EntryV20,
    GlobalCopyV21,
    LdsExchangeV22,
}
pub(super) fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: KIR_BYTES,
        max_reachable_functions: 1,
        max_reachable_operations: 128,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 32_768,
        max_call_depth: 1,
        max_ssa_values: 768,
        max_allocations: 8,
        max_allocation_bytes: 4096,
        max_total_bytes: 8192,
        max_resident_bytes: 64 * 1024 * 1024,
        max_events: 1,
        max_memory_access_records: 1024,
    }
}
/// Same conservative allowance and checked arithmetic as the original V20 loader.
/// Actual profile-specific typed header is included, never a caller-supplied estimate.
pub(super) fn envelope(header: usize) -> Result<usize, Resource> {
    let cells = size_of::<RequestArgument>()
        + size_of::<RequestSharedBuffer>()
        + size_of::<SimulationArgumentV1>()
        + size_of::<SharedBufferV1>()
        + 128;
    REQUEST_BYTES
        .checked_mul(cells)
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(2 * (KIR_BYTES + 1)))
        .and_then(|n| n.checked_add(2 * (REQUEST_BYTES + 1)))
        .and_then(|n| {
            header
                .checked_add(128 * 1024)
                .and_then(|h| n.checked_add(h))
        })
        .ok_or(Resource::Arithmetic)
}
pub(super) fn read_inputs(
    kir: &Path,
    request: &Path,
    profile: Profile,
) -> Result<(Vec<u8>, Vec<u8>), Failure> {
    let (code, label, request_label) = match profile {
        Profile::EntryV20 => (
            InputCode::KirV20,
            "diagnostic physical-entry KIR V20",
            "physical-entry simulation request",
        ),
        Profile::LdsExchangeV22 => (
            InputCode::KirV22,
            "diagnostic physical-LDS-exchange KIR V22",
            "physical-LDS-exchange simulation request",
        ),
        Profile::GlobalCopyV21 => (
            InputCode::KirV21,
            "diagnostic physical-global-copy KIR V21",
            "physical-global-copy simulation request",
        ),
    };
    Ok((
        secure_read(kir, KIR_BYTES, code, label)?,
        secure_read(request, REQUEST_BYTES, InputCode::Request, request_label)?,
    ))
}
pub(super) struct RequestRefused;
pub(super) fn parse_request(
    bytes: &[u8],
    profile: Profile,
) -> Result<SimulationRequestV1, RequestRefused> {
    let document: RequestDocument = serde_json::from_slice(bytes).map_err(|_| RequestRefused)?;
    let parsed = prepare_request(document).map_err(|_| RequestRefused)?;
    let (arguments, buffers) = match profile {
        Profile::EntryV20 => (5, 1),
        Profile::GlobalCopyV21 | Profile::LdsExchangeV22 => (2, 2),
    };
    if parsed.arguments.len() != arguments
        || parsed.shared_buffers.len() > buffers
        || parsed.grid.0[0] > 128
        || parsed.grid.0[1..] != [1, 1]
        || match profile {
            Profile::EntryV20 | Profile::GlobalCopyV21 => parsed.workgroup.0 != [64, 1, 1],
            Profile::LdsExchangeV22 => {
                parsed.workgroup.0 != [128, 1, 1] || parsed.grid.0 != [128, 1, 1]
            }
        }
    {
        return Err(RequestRefused);
    }
    Ok(parsed)
}
