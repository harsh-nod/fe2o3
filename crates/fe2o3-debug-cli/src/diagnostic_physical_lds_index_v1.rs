//! V22-only bounded inert observation index. No transcript import or new interpreter.
//! Workspace was reserved on the original ledger before capture. Its allocation
//! dies before JSONL serving; the conservative reservation lives until teardown.
use super::*;
use serde::{Serialize, Serializer};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
#[path = "diagnostic_physical_lds_index_rows_v1.rs"]
mod rows;
pub(super) const BYTES: usize = 8 * 1024 * 1024;
pub(super) const PAYLOAD_BYTES: usize = BYTES - 1024;
pub(super) const RECORDS: usize = 16384;
pub(super) const ALLOCATIONS: usize = 8;
pub(super) const BINDINGS: usize = 768;
pub(super) const PENDING: usize = 8;
pub(super) const ROW_BYTES: usize = 16384;
pub(super) const TEMP_STORAGE: usize = BYTES + 65536;
pub(super) const PREPAID_WORK: usize = RECORDS * (BINDINGS + ALLOCATIONS + 256) + 4 * BYTES;
pub(super) const CONFIGURATION_CAPS: [usize; 9] = [
    BYTES,
    PAYLOAD_BYTES,
    RECORDS,
    ALLOCATIONS,
    BINDINGS,
    PENDING,
    ROW_BYTES,
    TEMP_STORAGE,
    PREPAID_WORK,
];
const SCHEMA: &str = "fe2o3-physical-lds-cpu-capture-index-v1";
const DOMAIN: &[u8] = b"fe2o3-debug-physical-lds-v22-capture-index-v1\0";
const BUDGET: &str = "kir_v22_debug_capture_index_budget";
const LIMIT: &str = "kir_v22_debug_capture_index_limit";
const SHAPE: &str = "kir_v22_debug_capture_index_shape";
const OUTPUT: &str = "kir_v22_debug_capture_index_output";
const _: () = assert!(TEMP_STORAGE == 8_454_144 && PREPAID_WORK == 50_462_720);

#[derive(Clone, Copy)]
struct IndexDigest([u8; 32]);
impl Serialize for IndexDigest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut text = [0u8; 64];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (byte, digits) in self.0.iter().zip(text.chunks_exact_mut(2)) {
            digits[0] = HEX[usize::from(byte >> 4)];
            digits[1] = HEX[usize::from(byte & 15)];
        }
        let text = std::str::from_utf8(&text).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(text)
    }
}
#[derive(Serialize)]
struct ContentIdentity {
    sha256: IndexDigest,
    byte_length: u64,
}
#[derive(Serialize)]
struct Header {
    configuration_identity: OpaqueIdentityV1,
    canonical: ContentIdentity,
    request: ContentIdentity,
    outcome: &'static str,
    capture_stop: Option<&'static str>,
    record_count: usize,
    simulated: bool,
    hardware_observed: bool,
    performance_prediction: bool,
    provenance: &'static str,
    wave_interpretation: &'static str,
    unavailable: [&'static str; 6],
}
fn stop(
    value: Option<fe2o3_kir_sim::PhysicalLdsExchangeDebugCaptureStopV22>,
) -> Option<&'static str> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use fe2o3_kir_sim::PhysicalLdsExchangeDebugCaptureStopV22 as Stop;
    value.map(|s| match s {
        Stop::RecordLimit => "record_limit",
        Stop::SnapshotUnavailable => "snapshot_unavailable",
        Stop::Resource(Resource::Work(_)) => "resource_work",
        Stop::Resource(Resource::Storage(_)) => "resource_storage",
        Stop::Resource(Resource::Allocation) => "resource_allocation",
        Stop::Resource(Resource::Arithmetic) => "resource_arithmetic",
        Stop::Resource(Resource::Accounting) => "resource_accounting",
    })
}
struct Bounded {
    bytes: Vec<u8>,
    limit: usize,
}
impl Bounded {
    fn new(limit: usize) -> Result<Self, &'static str> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(limit).map_err(|_| BUDGET)?;
        Ok(Self { bytes, limit })
    }
}
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::ErrorKind::OutOfMemory.into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct RowBuffer {
    bytes: [u8; ROW_BYTES],
    len: usize,
}
impl RowBuffer {
    fn new() -> Self {
        Self {
            bytes: [0; ROW_BYTES],
            len: 0,
        }
    }
    fn clear(&mut self) {
        self.len = 0;
    }
    fn slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}
impl Write for RowBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.bytes.len().saturating_sub(self.len) {
            return Err(io::ErrorKind::OutOfMemory.into());
        }
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Caller prepaid original-ledger workspace/work; only actual immutable records
/// are observed. Kept private so raw caller records cannot manufacture an index.
fn encode(backend: &Backend<LdsSession>, input: &LdsInput) -> Result<Bounded, &'static str> {
    if backend.session.records_len() > RECORDS {
        return Err(LIMIT);
    }
    if backend.session.identity().digest() != input.canonical().identity().digest()
        || backend.configuration != configuration(input, true)?
        || backend.session.outcome() != PhysicalEntryDebugOutcomeV20::Completed
        || backend.session.capture_error().is_some()
    {
        return Err(SHAPE);
    }
    let header = Header {
        configuration_identity: backend.configuration,
        canonical: ContentIdentity {
            sha256: IndexDigest(*input.canonical().identity().digest()),
            byte_length: input.canonical().identity().canonical_length(),
        },
        request: ContentIdentity {
            sha256: IndexDigest(*input.request_digest()),
            byte_length: input.request_bytes() as u64,
        },
        outcome: "completed",
        capture_stop: stop(backend.session.capture_stop()),
        record_count: backend.session.records_len(),
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        provenance: "simulated_observation",
        wave_interpretation: "logical_visualization",
        unavailable: [
            "source_variables",
            "physical_registers",
            "physical_exec",
            "pending_store_queue",
            "publication_bitmap",
            "hardware_observation",
        ],
    };
    let catalog = rows::Catalog::scan(&backend.session)?;
    let mut payload = Bounded::new(PAYLOAD_BYTES)?;
    serde_json::to_writer(&mut payload, &header).map_err(|_| LIMIT)?;
    // Header is a closed typed struct; append two fields without a payload tree.
    if payload.bytes.pop() != Some(b'}') {
        return Err(SHAPE);
    }
    payload.write_all(b",\"allocations\":").map_err(|_| LIMIT)?;
    serde_json::to_writer(&mut payload, catalog.entries()).map_err(|_| LIMIT)?;
    payload.write_all(b",\"records\":[").map_err(|_| LIMIT)?;
    let mut row_bytes = RowBuffer::new();
    let mut previous_ordinal = None;
    for i in 0..backend.session.records_len() {
        let record = backend.session.record(i).ok_or(SHAPE)?;
        if previous_ordinal.is_some_and(|old| old >= record.ordinal()) {
            return Err(SHAPE);
        }
        previous_ordinal = Some(record.ordinal());
        let row = rows::Row::new(i, record, &catalog)?;
        row_bytes.clear();
        serde_json::to_writer(&mut row_bytes, &row).map_err(|_| LIMIT)?;
        if i != 0 {
            payload.write_all(b",").map_err(|_| LIMIT)?;
        }
        payload.write_all(row_bytes.slice()).map_err(|_| LIMIT)?;
    }
    payload.write_all(b"]}").map_err(|_| LIMIT)?;
    Ok(payload)
}
#[derive(Serialize)]
struct Identity {
    sha256: IndexDigest,
    payload_bytes: usize,
}
#[derive(Serialize)]
struct Envelope {
    schema: &'static str,
    identity: Identity,
}
fn envelope(payload: &[u8]) -> Result<RowBuffer, &'static str> {
    if payload.len() > PAYLOAD_BYTES {
        return Err(LIMIT);
    }
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(payload);
    let identity = Identity {
        sha256: IndexDigest(digest.finalize().into()),
        payload_bytes: payload.len(),
    };
    let mut header = RowBuffer::new();
    serde_json::to_writer(
        &mut header,
        &Envelope {
            schema: SCHEMA,
            identity,
        },
    )
    .map_err(|_| LIMIT)?;
    if header.len == 0 || header.bytes[header.len - 1] != b'}' {
        return Err(SHAPE);
    }
    header.len -= 1;
    header.write_all(b",\"payload\":").map_err(|_| LIMIT)?;
    if header.len + 2 > 1024 || header.len + payload.len() + 2 > BYTES {
        return Err(LIMIT);
    }
    Ok(header)
}
fn write_fresh(path: &std::path::Path, header: &[u8], payload: &[u8]) -> Result<(), &'static str> {
    // Explicit opt-in only; never overwrite an existing file or follow its symlink.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| OUTPUT)?;
    file.write_all(header)
        .and_then(|()| file.write_all(payload))
        .and_then(|()| file.write_all(b"}\n"))
        .and_then(|()| file.flush())
        .map_err(|_| OUTPUT)
}
pub(super) fn export(
    backend: &mut Backend<LdsSession>,
    input: &LdsInput,
    path: &std::path::Path,
) -> Result<(), &'static str> {
    // The sole production caller reserved TEMP_STORAGE before creating capture.
    backend
        .session
        .charge_query_work(PREPAID_WORK)
        .map_err(|_| BUDGET)?;
    let payload = encode(backend, input)?;
    let header = envelope(&payload.bytes)?;
    write_fresh(path, header.slice(), &payload.bytes)
}
#[cfg(test)]
#[path = "diagnostic_physical_lds_index_v1_tests.rs"]
mod tests;
