//! Observation-only roster transport. Native admission remains in the application backend.
use super::*;

pub(crate) const REQUEST_ENV: &str = "FE2O3_GENUINE_GPU_ROSTER";
pub(crate) const SELECTION_ENV: &str = "FE2O3_GENUINE_GPU_ROSTER_SELECTION";
pub(crate) const SELECTION_TAG: &str = "FE2O3_GPU_ROSTER_SELECTION_V1=";
const MAX_DEVICES: usize = 8;
const SELECTION_SCHEMA: &str = "fe2o3.genuine-gpu-roster-selection.v1";
const REPORT_SCHEMA: &str = "fe2o3.genuine-device-roster.v1";

pub(crate) fn requested_ids() -> Result<Vec<u64>, String> {
    parse_ids(&std::env::var(REQUEST_ENV).map_err(|error| format!("{REQUEST_ENV}: {error}"))?)
}

fn parse_ids(record: &str) -> Result<Vec<u64>, String> {
    if record.len() > 4096 {
        return Err("GPU roster input exceeds its byte bound".into());
    }
    let values: Vec<String> = serde_json::from_str(record).map_err(|error| error.to_string())?;
    let ids = values
        .iter()
        .map(|uid| super::parse_uid(uid))
        .collect::<Result<Vec<_>, _>>()?;
    validate_ids(&ids)?;
    Ok(ids)
}

fn validate_ids(ids: &[u64]) -> Result<(), String> {
    if !(2..=MAX_DEVICES).contains(&ids.len())
        || ids
            .iter()
            .enumerate()
            .any(|(index, uid)| *uid == 0 || ids[..index].contains(uid))
    {
        return Err("roster requires two through eight nonzero distinct GPU UIDs".into());
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    schema: String,
    devices: Vec<String>,
    render_minors: Vec<u16>,
    observed_host_devices: Vec<String>,
    excluded_unselected_devices: Vec<String>,
    occupancy: String,
    scope: String,
}

pub(crate) fn discover(ids: &[u64]) -> Result<Selection, String> {
    validate_ids(ids)?;
    let snapshot =
        discover_default_topology().map_err(|error| format!("GPU roster topology: {error}"))?;
    let nodes: Vec<_> = snapshot
        .topology()
        .gpu_nodes()
        .iter()
        .map(|node| ObservedGpu {
            uid: node.unique_id(),
            gpu_id: node.gpu_id(),
            minor: node.drm_render_minor(),
        })
        .collect();
    let renders: Vec<_> = snapshot
        .render_nodes()
        .iter()
        .map(|node| (node.unique_id(), node.drm_render_minor()))
        .collect();
    select(ids, &nodes, &renders, |source, destination| {
        snapshot
            .topology()
            .admit_gfx942_xgmi_route(source, destination)
            .map(|_| ())
            .map_err(|error| format!("directed roster route: {error}"))
    })
}

fn select(
    ids: &[u64],
    nodes: &[ObservedGpu],
    renders: &[(u64, u16)],
    mut route: impl FnMut(u32, u32) -> Result<(), String>,
) -> Result<Selection, String> {
    validate_ids(ids)?;
    if nodes.len() > 256
        || nodes.iter().enumerate().any(|(index, node)| {
            node.uid == 0 || nodes[..index].iter().any(|prior| prior.uid == node.uid)
        })
    {
        return Err("host topology UID inventory is ambiguous or exceeds its bound".into());
    }
    let mut selected = Vec::<ObservedGpu>::with_capacity(ids.len());
    for uid in ids {
        let node = *nodes
            .iter()
            .find(|node| node.uid == *uid)
            .ok_or("requested roster GPU UID is absent")?;
        if !(128..=255).contains(&node.minor)
            || renders
                .iter()
                .filter(|(id, minor)| *id == *uid && *minor == node.minor)
                .count()
                != 1
            || renders
                .iter()
                .filter(|(id, minor)| *id == *uid || *minor == node.minor)
                .count()
                != 1
            || selected
                .iter()
                .any(|prior| prior.minor == node.minor || prior.gpu_id == node.gpu_id)
            || node.gpu_id == 0
            || u32::try_from(node.gpu_id).is_err()
        {
            return Err("roster render node or kernel GPU ID is uncorrelated or aliased".into());
        }
        selected.push(node);
    }
    for (source, left) in selected.iter().enumerate() {
        for (destination, right) in selected.iter().enumerate() {
            if source != destination {
                route(left.gpu_id as u32, right.gpu_id as u32)?;
            }
        }
    }
    let mut host_ids: Vec<_> = nodes.iter().map(|node| node.uid).collect();
    host_ids.sort_unstable();
    Ok(Selection {
        schema: SELECTION_SCHEMA.into(),
        devices: ids.iter().map(|uid| format!("{uid:#018x}")).collect(),
        render_minors: selected.iter().map(|node| node.minor).collect(),
        observed_host_devices: host_ids.iter().map(|uid| format!("{uid:#018x}")).collect(),
        excluded_unselected_devices: host_ids
            .iter()
            .filter(|uid| !ids.contains(uid))
            .map(|uid| format!("{uid:#018x}"))
            .collect(),
        occupancy: "not-measured".into(),
        scope: "selected-admitted-context-only".into(),
    })
}

impl Selection {
    pub(crate) fn required_groups(&self) -> Result<Vec<u32>, String> {
        let directory = std::fs::symlink_metadata("/dev/dri").map_err(|error| error.to_string())?;
        if !directory.is_dir() || directory.file_type().is_symlink() {
            return Err("render directory must be an actual directory, not an alias".into());
        }
        super::required_groups_for_minors(&self.render_minors)
    }
}

pub(crate) fn revalidate(ids: &[u64]) -> Result<Vec<u32>, String> {
    let current = discover(ids)?;
    let encoded = std::env::var(SELECTION_ENV).map_err(|error| error.to_string())?;
    match_selection(&encoded, &current)?;
    current.required_groups()
}

fn match_selection(encoded: &str, current: &Selection) -> Result<(), String> {
    if encoded.len() > 32768 {
        return Err("roster selection exceeds its byte bound".into());
    }
    let original: Selection = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
    if original != *current {
        return Err("roster or host topology changed across namespaces".into());
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShardReport {
    device: String,
    elements: usize,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema: String,
    devices: Vec<String>,
    shards: Vec<ShardReport>,
    directed_peer_pairs: Vec<[String; 2]>,
    native_peer_completions: usize,
    roster_scope: String,
    ordering: String,
    copy_compute_overlap: String,
    peer_tag_origin: String,
    shutdown: String,
}

fn expected_report(ids: &[u64]) -> Result<Report, String> {
    validate_ids(ids)?;
    let devices: Vec<_> = ids.iter().map(|uid| format!("{uid:#018x}")).collect();
    Ok(Report {
        schema: REPORT_SCHEMA.into(),
        shards: devices
            .iter()
            .enumerate()
            .map(|(index, device)| ShardReport {
                device: device.clone(),
                elements: 65 + index,
            })
            .collect(),
        directed_peer_pairs: devices
            .iter()
            .enumerate()
            .flat_map(|(source, left)| {
                devices
                    .iter()
                    .enumerate()
                    .filter(move |(destination, _)| *destination != source)
                    .map(move |(_, right)| [left.clone(), right.clone()])
            })
            .collect(),
        native_peer_completions: ids.len() * (ids.len() - 1),
        devices,
        roster_scope: "all-admitted-context-devices".into(),
        ordering: "compute-complete-before-peer-submit".into(),
        copy_compute_overlap: "not-measured".into(),
        peer_tag_origin: "host".into(),
        shutdown: "released".into(),
    })
}

pub(crate) fn verify_report(stdout: &[u8], ids: &[u64]) -> Result<(), String> {
    if stdout.len() > 1024 * 1024 {
        return Err("roster stdout exceeds capture bound".into());
    }
    let text = std::str::from_utf8(stdout).map_err(|error| error.to_string())?;
    let mut records = text
        .lines()
        .filter(|line| line.trim_start().starts_with('{') || line.contains("fe2o3.genuine-"));
    let line = records.next().ok_or("missing roster execution report")?;
    if records.next().is_some() {
        return Err("duplicate or mixed roster execution report".into());
    }
    if line.len() > 32768 {
        return Err("roster report exceeds its byte bound".into());
    }
    let report: Report = serde_json::from_str(line).map_err(|error| error.to_string())?;
    if report != expected_report(ids)? {
        return Err("roster report does not match the complete requested campaign".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
