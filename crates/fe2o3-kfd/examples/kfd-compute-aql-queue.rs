//! Isolated MI300X CREATE/doorbell-map/DESTROY validation without MMIO stores.

use std::path::Path;
use std::process::Command;

use fe2o3_kfd::topology::discover_default_topology;
use fe2o3_kfd::{DeviceSelector, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1, OpenedKfd};

const CHILD_ENV: &str = "FE2O3_KFD_COMPUTE_AQL_QUEUE_CHILD";
const USAGE: &str = "usage: kfd-compute-aql-queue (--all|<selected-unique-id>)";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GpuSelection {
    All,
    UniqueId(u64),
}

fn parse_u64(value: &str) -> Result<u64, String> {
    if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
            .map_err(|error| format!("invalid selected unique ID `{value}`: {error}"))
    } else {
        value
            .parse()
            .map_err(|error| format!("invalid selected unique ID `{value}`: {error}"))
    }
}

fn parse_selection(args: impl IntoIterator<Item = String>) -> Result<GpuSelection, String> {
    let mut args = args.into_iter();
    let selected = args.next().ok_or_else(|| USAGE.to_owned())?;
    let selection = if selected == "--all" {
        GpuSelection::All
    } else {
        GpuSelection::UniqueId(parse_u64(&selected)?)
    };
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument `{extra}`; {USAGE}"));
    }
    Ok(selection)
}

fn run_child(unique_id: u64) -> Result<(), Box<dyn std::error::Error>> {
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut queue = device.create_compute_aql_queue(4096)?;
    let observation = queue.observation();
    assert_eq!(
        observation.queue_id(),
        0,
        "isolated process must receive queue ID zero"
    );
    assert_eq!(observation.ring_bytes(), 4096);
    assert_eq!(observation.doorbell_slice_bytes(), 8192);
    assert!(observation.doorbell_byte_offset() < 8192);
    assert_eq!(observation.doorbell_byte_offset() % 8, 0);
    assert!((1..=255).contains(&observation.event_id()));
    assert_eq!(observation.cwsr_shadow_pages(), 8);
    queue.verify_doorbell_dontfork()?;
    queue.verify_exception_shadows_dontfork()?;
    let destroyed = queue.destroy()?;
    assert_eq!(destroyed.queue_id(), 0);
    assert_eq!(destroyed.released_resources(), 5);
    println!(
        "profile_sha256={} unique_id={unique_id:016x} queue_id={} event_id={} cwsr_shadow_pages={} runtime=enabled-before-create-then-disabled ring=4096 roles=ring,control,eop,cwsr,completion-signals gtt_policy=accepted doorbell_slice={} doorbell_byte_offset={} dontfork=confirmed mmio_stores=0 packets=0 destroy=queue-then-event-then-runtime-confirmed resources_returned={}",
        GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1,
        observation.queue_id(),
        observation.event_id(),
        observation.cwsr_shadow_pages(),
        observation.doorbell_slice_bytes(),
        observation.doorbell_byte_offset(),
        destroyed.released_resources(),
    );
    Ok(())
}

fn run_isolated_child(executable: &Path, unique_id: u64) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new(executable)
        .arg(unique_id.to_string())
        .env(CHILD_ENV, "1")
        .status()?;
    if !status.success() {
        return Err(format!(
            "isolated compute-AQL queue child for unique ID {unique_id:016x} failed with {status}"
        )
        .into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selection = parse_selection(std::env::args().skip(1))?;
    if std::env::var_os(CHILD_ENV).is_some() {
        let GpuSelection::UniqueId(unique_id) = selection else {
            return Err("isolated compute-AQL queue child requires one explicit unique ID".into());
        };
        return run_child(unique_id);
    }

    let unique_ids = match selection {
        GpuSelection::All => {
            let unique_ids = discover_default_topology()?
                .topology()
                .gpu_nodes()
                .iter()
                .map(|gpu| gpu.unique_id())
                .collect::<Vec<_>>();
            if unique_ids.is_empty() {
                return Err("no topology GPU available for --all".into());
            }
            unique_ids
        }
        GpuSelection::UniqueId(unique_id) => vec![unique_id],
    };
    let executable = std::env::current_exe()?;
    for unique_id in unique_ids {
        run_isolated_child(&executable, unique_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GpuSelection, parse_selection};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn explicit_unique_ids_accept_decimal_and_hex() {
        assert_eq!(
            parse_selection(args(&["42"])).unwrap(),
            GpuSelection::UniqueId(42)
        );
        assert_eq!(
            parse_selection(args(&["0x2a"])).unwrap(),
            GpuSelection::UniqueId(42)
        );
    }

    #[test]
    fn all_is_an_explicit_selection() {
        assert_eq!(
            parse_selection(args(&["--all"])).unwrap(),
            GpuSelection::All
        );
    }

    #[test]
    fn malformed_or_ambiguous_arguments_are_rejected() {
        assert!(parse_selection(args(&[])).is_err());
        assert!(parse_selection(args(&["not-an-id"])).is_err());
        assert!(parse_selection(args(&["42", "extra"])).is_err());
        assert!(parse_selection(args(&["--all", "extra"])).is_err());
    }
}
