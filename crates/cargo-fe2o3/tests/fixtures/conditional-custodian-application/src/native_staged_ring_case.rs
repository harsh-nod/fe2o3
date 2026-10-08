//! Closed report oracle, not graph, version, device, or native authority.
use crate::native_shards_case::{self, Case};
use std::ffi::OsString;

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--native-v5-staged-ring")) {
        return Err("explicit native staged-ring selector required".into());
    }
    native_shards_case::parse(std::iter::once(OsString::from("--native-v5-shards")).chain(args))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compute {
    Succeeded,
    RejectedBeforePublication,
    DeviceUnavailableBeforeActivation,
    ReadbackFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transfer {
    Succeeded,
    DependencyFailed,
    SettledFailure,
}

impl Compute {
    fn token(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::RejectedBeforePublication => "rejected-before-publication",
            Self::DeviceUnavailableBeforeActivation => "device-unavailable-before-activation",
            Self::ReadbackFailed => "readback-failed-after-native-settlement",
        }
    }
}

impl Transfer {
    fn token(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::DependencyFailed => "dependency-failed-not-issued",
            Self::SettledFailure => "failed-after-original-graph-retirement",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub source: (u64, u16),
    pub destination: (u64, u16),
    pub nodes: [u32; 5],
    pub compute: Compute,
    pub staging: Transfer,
    pub copy: Transfer,
    pub checked_values: usize,
    pub checked_copy_bytes: usize,
    pub staged_version_committed: bool,
    pub copy_input_available: bool,
    pub copy_version_committed: bool,
}

pub fn nodes(index: usize) -> [u32; 5] {
    let first = (index * 5 + 1) as u32;
    [first, first + 1, first + 2, first + 3, first + 4]
}

pub fn check_bytes(case: &Case, index: usize, bytes: &[u8]) -> Result<(), String> {
    if index >= case.devices.len()
        || bytes.len() != case.elements(index) * 4
        || bytes
            .chunks_exact(4)
            .enumerate()
            .any(|(offset, raw)| raw != (offset as u32).to_le_bytes())
    {
        return Err("staged-ring copy extent or exact decoded values differ".into());
    }
    Ok(())
}

pub fn report(
    case: &Case,
    observed: &[(u64, u16)],
    rows: &[Row],
    local_failure: bool,
) -> Result<String, String> {
    case.check_roster(observed)?;
    if rows.len() != observed.len() {
        return Err("one exact terminal ring row per admitted device required".into());
    }
    let mut copied = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let computed = row.compute == Compute::Succeeded;
        let staged = row.staging == Transfer::Succeeded;
        let copied_successfully = row.copy == Transfer::Succeeded;
        if row.source != observed[index]
            || row.destination != observed[(index + 1) % observed.len()]
            || row.nodes != nodes(index)
            || row.checked_values != if computed { case.elements(index) } else { 0 }
            || row.checked_copy_bytes
                != if copied_successfully {
                    case.elements(index) * 4
                } else {
                    0
                }
            || (!computed
                && (row.staging != Transfer::DependencyFailed
                    || row.copy != Transfer::DependencyFailed))
            || (computed && row.staging == Transfer::DependencyFailed)
            || (!staged && row.copy != Transfer::DependencyFailed)
            || (staged && row.copy == Transfer::DependencyFailed)
            || row.staged_version_committed != staged
            || row.copy_input_available != (row.copy != Transfer::DependencyFailed)
            || row.copy_version_committed != copied_successfully
        {
            return Err("terminal ring identity, dependency, result, or version mismatch".into());
        }
        copied = copied
            .checked_add(row.checked_copy_bytes)
            .ok_or("copied-byte overflow")?;
    }
    if local_failure != rows.iter().any(|row| row.compute != Compute::Succeeded) {
        return Err("settled original compute result differs from exact roster".into());
    }
    let successful = rows
        .iter()
        .filter(|row| row.copy == Transfer::Succeeded)
        .count();
    let values = rows.iter().map(|row| format!(
        "{{\"source_uid\":\"0x{:016x}\",\"source_render_minor\":{},\"destination_uid\":\"0x{:016x}\",\"destination_render_minor\":{},\"nodes\":{:?},\"compute\":\"{}\",\"staging\":\"{}\",\"copy\":\"{}\",\"checked_values\":{},\"checked_copy_bytes\":{},\"staged_version_committed\":{},\"copy_input_available\":{},\"copy_version_committed\":{}}}",
        row.source.0, row.source.1, row.destination.0, row.destination.1, row.nodes,
        row.compute.token(), row.staging.token(), row.copy.token(), row.checked_values,
        row.checked_copy_bytes, row.staged_version_committed, row.copy_input_available,
        row.copy_version_committed,
    )).collect::<Vec<_>>().join(",");
    Ok(format!(
        "\"target\":\"gfx942:xnack-\",\"coverage\":\"all-admitted-context\",\"admitted_devices\":{},\"ring\":[{}],\"successful_transfers\":{},\"failed_transfers\":{},\"checked_copy_bytes\":{},\"settled_compute_failure\":{},\"graph_retired\":true,\"transport\":\"coherent-host-read-staging-then-destination-sdma\",\"direct_native_data_transfer\":false,\"xgmi_transfer\":false,\"measured_compute_transfer_overlap\":false",
        rows.len(),
        values,
        successful,
        rows.len() - successful,
        copied,
        local_failure,
    ))
}

#[cfg(test)]
#[path = "native_staged_ring_case/tests.rs"]
mod tests;
