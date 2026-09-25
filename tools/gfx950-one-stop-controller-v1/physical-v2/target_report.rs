//! Independent target report parsing. Declared hashes/closure are not source custody.
use crate::syntax::{Refusal, decimal};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trace {
    last_returned: String,
    native_effects: String,
    zero_gpu_fd_fence: bool,
    owned_descriptors_closed: bool,
    owner_failure_publication_possible: (),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Facts {
    source_sha256: String,
    source_bytes: u64,
    artifact_sha256: String,
    artifact_bytes: u64,
    trap_sha256: String,
    trap_bytes: u64,
    mapped_backing_bytes: String,
    metadata_retained_bytes: String,
    closure_sha256: String,
    queue_backing_bytes: String,
    projected_native_bytes: String,
    projected_logical_bytes: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema: String,
    profile: String,
    qualification: String,
    status: String,
    process_id: u32,
    trace: Trace,
    facts: Facts,
    failure: (),
    completion_requires_exit_zero: bool,
    valid_packet_publications: u32,
    doorbell_stores: u32,
    attached_debugger: (),
    loaded_success: (),
    event_acknowledged: (),
    ttmp_readback: (),
    sampler_exclusion: (),
    unique_gpu_debug_stop: (),
    physical_capture: bool,
    family_cleanup_established: bool,
    operational_qualification: bool,
    proc_snapshot_proves_general_exclusion: bool,
    source_or_launch_authority: bool,
}
pub(crate) fn check(bytes: &[u8], pid: u32) -> Result<(), Refusal> {
    if bytes.is_empty() || bytes.len() > 64 * 1024 {
        return Err(Refusal::Bound);
    }
    let r: Report = serde_json::from_slice(bytes).map_err(|_| Refusal::Shape)?;
    let Facts {
        source_sha256,
        source_bytes,
        artifact_sha256,
        artifact_bytes,
        trap_sha256,
        trap_bytes,
        mapped_backing_bytes,
        metadata_retained_bytes,
        closure_sha256,
        queue_backing_bytes,
        projected_native_bytes,
        projected_logical_bytes,
    } = r.facts;
    // Deserialize unit requires an explicit JSON null, not missing/Boolean.
    let _ = (
        r.failure,
        r.attached_debugger,
        r.loaded_success,
        r.event_acknowledged,
        r.ttmp_readback,
        r.sampler_exclusion,
        r.unique_gpu_debug_stop,
        r.trace.owner_failure_publication_possible,
    );
    if r.schema != "fe2o3-gfx950-one-stop-target-observation-v1"
        || r.profile != "fe2o3.gfx950.one-stop-target.v1"
        || r.qualification != "UNQUALIFIED_NATIVE_TARGET_NOT_TO_RUN"
        || r.status != "local_complete"
        || r.process_id != pid
        || pid == 0
        || r.trace.last_returned != "local_complete"
        || r.trace.native_effects != "local_complete"
        || !r.trace.zero_gpu_fd_fence
        || !r.trace.owned_descriptors_closed
        || !r.completion_requires_exit_zero
        || r.valid_packet_publications != 1
        || r.doorbell_stores != 1
        || r.physical_capture
        || r.family_cleanup_established
        || r.operational_qualification
        || r.proc_snapshot_proves_general_exclusion
        || r.source_or_launch_authority
        || source_bytes != 1767
        || artifact_bytes != 5312
        || trap_bytes != 1116
        || source_sha256 != "e567e9b414f00822fbf6436984a452d9940630eccec40a0e86b2c5c191598017"
        || artifact_sha256 != "cd3daab76db347104166c898a58dcc29c197ee1eac8b258b10a2c793e8779c87"
        || trap_sha256 != "4ffea893ee53e018a629c19254855721882444517155251f1f45dd3519bc81fe"
    {
        return Err(Refusal::Artifact);
    }
    let mapped = decimal(mapped_backing_bytes.as_bytes())?;
    let metadata = decimal(metadata_retained_bytes.as_bytes())?;
    let queue = decimal(queue_backing_bytes.as_bytes())?;
    let native = decimal(projected_native_bytes.as_bytes())?;
    let logical = decimal(projected_logical_bytes.as_bytes())?;
    if mapped != 12288
        || queue != 190296064
        || metadata == 0
        || metadata > 256 * 1024 * 1024
        || native
            != mapped
                .checked_add(queue)
                .and_then(|n| n.checked_add(4096))
                .ok_or(Refusal::Bound)?
        || logical == 0
        || logical > 256 * 1024 * 1024
        || closure_sha256.len() != 64
        || !closure_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || closure_sha256.bytes().all(|b| b == b'0')
    {
        return Err(Refusal::Artifact);
    }
    Ok(())
}
