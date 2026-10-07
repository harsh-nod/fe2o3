use fe2o3_kfd::topology::discover_default_topology;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

pub(super) mod roster;

pub(super) const SELECTION_ENV: &str = "FE2O3_GENUINE_GPU_SELECTION";
pub(super) const SELECTION_TAG: &str = "FE2O3_GPU_SELECTION_V1=";
const SELECTION_SCHEMA: &str = "fe2o3.genuine-gpu-selection.v1";
const REPORT_SCHEMA: &str = "fe2o3.genuine-two-gpu.v1";
const CONTROL_SCHEMA: &str = "fe2o3.genuine-two-gpu-control.v1";
const CLIENT_UID: u32 = 1000;
const CLIENT_GID: u32 = 1000;

#[derive(Clone, Copy, Debug)]
pub(super) enum Control {
    SecondCoverageReject,
    PeerDeadlineBeforeSubmit,
}

impl Control {
    pub(super) fn token(self) -> &'static str {
        match self {
            Self::SecondCoverageReject => "second-coverage-reject",
            Self::PeerDeadlineBeforeSubmit => "peer-deadline-before-submit",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    schema: String,
    // Hex strings survive jq without its floating-point u64 precision loss.
    devices: [String; 2],
    render_minors: [u16; 2],
}

#[derive(Clone, Copy)]
struct ObservedGpu {
    uid: u64,
    gpu_id: u64,
    minor: u16,
}

pub(super) fn requested_ids() -> Result<[u64; 2], String> {
    let values = ["FE2O3_GENUINE_GPU_UID0", "FE2O3_GENUINE_GPU_UID1"]
        .map(|name| std::env::var(name).map_err(|error| format!("{name}: {error}")));
    let [first, second] = values;
    parse_ids([&first?, &second?])
}

fn parse_ids(values: [&str; 2]) -> Result<[u64; 2], String> {
    let ids = [parse_uid(values[0])?, parse_uid(values[1])?];
    if ids.contains(&0) || ids[0] == ids[1] {
        return Err("two nonzero distinct GPU UIDs are required".into());
    }
    Ok(ids)
}

fn parse_uid(value: &str) -> Result<u64, String> {
    let digits = value
        .strip_prefix("0x")
        .ok_or("GPU UID requires 0x prefix")?;
    if digits.is_empty() || digits.len() > 16 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid GPU UID digits".to_owned());
    }
    u64::from_str_radix(digits, 16).map_err(|error| error.to_string())
}

pub(super) fn discover(ids: [u64; 2]) -> Result<Selection, String> {
    let snapshot = discover_default_topology().map_err(|e| format!("GPU topology: {e}"))?;
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
            .map_err(|e| format!("directed native route: {e}"))
    })
}

// This pure selection step grants no native authority; the backend independently
// admits both devices. Inject only the observation-level route check for CPU tests.
fn select(
    ids: [u64; 2],
    nodes: &[ObservedGpu],
    renders: &[(u64, u16)],
    mut route: impl FnMut(u32, u32) -> Result<(), String>,
) -> Result<Selection, String> {
    if ids.contains(&0) || ids[0] == ids[1] {
        return Err("two nonzero distinct GPU UIDs are required".into());
    }
    let mut selected = Vec::with_capacity(2);
    for uid in ids {
        let mut matching = nodes.iter().filter(|node| node.uid == uid);
        let node = matching.next().ok_or("requested GPU UID is absent")?;
        if matching.next().is_some()
            || !(128..=255).contains(&node.minor)
            || renders
                .iter()
                .filter(|(id, minor)| *id == uid && *minor == node.minor)
                .count()
                != 1
        {
            return Err("ambiguous or uncorrelated render node".into());
        }
        selected.push(*node);
    }
    if selected[0].minor == selected[1].minor {
        return Err("GPU UIDs resolve to the same render node".into());
    }
    let gpu_ids = [selected[0].gpu_id, selected[1].gpu_id]
        .map(|id| u32::try_from(id).map_err(|_| "kernel GPU ID exceeds u32".to_owned()));
    let [first, second] = gpu_ids;
    let (first, second) = (first?, second?);
    route(first, second)?;
    route(second, first)?;
    Ok(Selection {
        schema: SELECTION_SCHEMA.into(),
        devices: ids.map(|id| format!("{id:#018x}")),
        render_minors: [selected[0].minor, selected[1].minor],
    })
}

pub(super) fn revalidate(ids: [u64; 2]) -> Result<Vec<u32>, String> {
    let current = discover(ids)?;
    match_selection(
        &std::env::var(SELECTION_ENV).map_err(|e| e.to_string())?,
        &current,
    )?;
    current.required_groups()
}

fn match_selection(record: &str, current: &Selection) -> Result<(), String> {
    let original: Selection =
        serde_json::from_str(record).map_err(|e| format!("selection record: {e}"))?;
    if original != *current {
        return Err("GPU selection changed across private namespaces".into());
    }
    Ok(())
}

impl Selection {
    pub(super) fn required_groups(&self) -> Result<Vec<u32>, String> {
        required_groups_for_minors(&self.render_minors)
    }
}

fn required_groups_for_minors(minors: &[u16]) -> Result<Vec<u32>, String> {
    let mut groups = Vec::new();
    let kfd = checked_node(Path::new("/dev/kfd"), None)?;
    let major = rustix::fs::major(kfd.rdev());
    let expected = Path::new("/sys/devices/virtual/kfd/kfd");
    if major == 0
        || rustix::fs::minor(kfd.rdev()) != 0
        || std::fs::canonicalize(format!("/sys/dev/char/{major}:0")).map_err(|e| e.to_string())?
            != expected
        || std::fs::canonicalize("/sys/class/kfd/kfd").map_err(|e| e.to_string())? != expected
        || std::fs::read_to_string(expected.join("dev")).map_err(|e| e.to_string())?
            != format!("{major}:0\n")
    {
        return Err("KFD character device differs from sysfs".into());
    }
    let nodes = std::iter::once(Ok(kfd))
        .chain(minors.iter().copied().map(|minor| {
            checked_node(
                Path::new(&format!("/dev/dri/renderD{minor}")),
                Some((226, u32::from(minor))),
            )
        }))
        .collect::<Result<Vec<_>, _>>()?;
    for metadata in &nodes {
        if let Some(group) = required_group(metadata.uid(), metadata.gid(), metadata.mode())?
            && !groups.contains(&group)
        {
            groups.push(group);
        }
    }
    groups.sort_unstable();
    if nodes
        .iter()
        .any(|node| !read_write_allowed(node.uid(), node.gid(), node.mode(), &groups))
    {
        return Err("combined GPU group membership denies access to a selected node".into());
    }
    Ok(groups)
}

fn read_write_allowed(uid: u32, gid: u32, mode: u32, groups: &[u32]) -> bool {
    let mask = if uid == CLIENT_UID {
        0o600
    } else if gid == CLIENT_GID || groups.contains(&gid) {
        0o060
    } else {
        0o006
    };
    mode & mask == mask
}

fn checked_node(path: &Path, expected: Option<(u32, u32)>) -> Result<std::fs::Metadata, String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.file_type().is_char_device()
        || expected.is_some_and(|pair| {
            pair != (
                rustix::fs::major(metadata.rdev()),
                rustix::fs::minor(metadata.rdev()),
            )
        })
    {
        return Err(format!(
            "{} is not the exact character device",
            path.display()
        ));
    }
    Ok(metadata)
}

fn required_group(uid: u32, gid: u32, mode: u32) -> Result<Option<u32>, String> {
    let (mask, supplemental) = if uid == CLIENT_UID {
        (0o600, None)
    } else if gid == CLIENT_GID {
        (0o060, None)
    } else if mode & 0o006 == 0o006 {
        (0o006, None)
    } else if gid != 0 && gid != u32::MAX {
        (0o060, Some(gid))
    } else {
        return Err("GPU access would require a privileged or invalid group".into());
    };
    if mode & mask != mask {
        return Err("GPU node is not readable and writable by application credentials".into());
    }
    Ok(supplemental)
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct HardwareReport {
    schema: String,
    devices: [String; 2],
    elements: u32,
    native_peer_completions: u32,
    shutdown: String,
}

pub(super) fn verify_report(stdout: &[u8], ids: [u64; 2]) -> Result<(), String> {
    let text = std::str::from_utf8(stdout).map_err(|e| e.to_string())?;
    let mut reports = text
        .lines()
        .filter(|line| line.trim_start().starts_with('{') || line.contains(REPORT_SCHEMA));
    let line = reports.next().ok_or("missing two-GPU execution report")?;
    if reports.next().is_some() {
        return Err("duplicate two-GPU execution report".into());
    }
    let report: HardwareReport =
        serde_json::from_str(line).map_err(|e| format!("hardware report: {e}"))?;
    let expected = HardwareReport {
        schema: REPORT_SCHEMA.into(),
        devices: ids.map(|id| format!("{id:#018x}")),
        elements: 65,
        native_peer_completions: 2,
        shutdown: "released".into(),
    };
    if report != expected {
        return Err("two-GPU report does not match the requested successful execution".into());
    }
    Ok(())
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ControlReport {
    schema: String,
    devices: [String; 2],
    mode: String,
    shutdown: String,
}

pub(super) fn verify_control_report(
    stdout: &[u8],
    ids: [u64; 2],
    control: Control,
    code: Option<i32>,
) -> Result<(), String> {
    // Expected rejection is a successful test; all production post-spawn checks still run.
    // Status alone is insufficient: only the distinct control record can qualify this case.
    if code != Some(0) {
        return Err("negative control requires a successful production application route".into());
    }
    let text = std::str::from_utf8(stdout).map_err(|e| e.to_string())?;
    let mut reports = text.lines().filter(|line| {
        line.trim_start().starts_with('{') || line.contains("fe2o3.genuine-two-gpu")
    });
    let line = reports
        .next()
        .ok_or("missing two-GPU negative-control report")?;
    if reports.next().is_some() {
        return Err("duplicate or mixed two-GPU control/success records".into());
    }
    let report: ControlReport =
        serde_json::from_str(line).map_err(|e| format!("control report: {e}"))?;
    if report
        != (ControlReport {
            schema: CONTROL_SCHEMA.into(),
            devices: ids.map(|id| format!("{id:#018x}")),
            mode: control.token().into(),
            shutdown: "released".into(),
        })
    {
        return Err("two-GPU control record does not match the requested case".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
