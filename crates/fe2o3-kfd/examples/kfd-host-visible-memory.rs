use std::path::Path;
use std::process::Command;

use fe2o3_kfd::topology::discover_default_topology;
use fe2o3_kfd::{
    DeviceSelector, HOST_VISIBLE_MEMORY_PROFILE_SHA256_V1, HostVisibleMemoryPhase, OpenedKfd,
};

const CHILD_ENV: &str = "FE2O3_KFD_MEMORY_LIVE_CHILD";
const DEFAULT_REQUESTED_BYTES: usize = 4097;
const USAGE: &str = "usage: kfd-host-visible-memory (--all|<selected-unique-id>) [requested-bytes]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GpuSelection {
    All,
    UniqueId(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    selection: GpuSelection,
    requested: usize,
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

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let selected = args.next().ok_or_else(|| USAGE.to_owned())?;
    let selection = if selected == "--all" {
        GpuSelection::All
    } else {
        GpuSelection::UniqueId(parse_u64(&selected)?)
    };
    let requested = args
        .next()
        .map(|value| {
            value
                .parse()
                .map_err(|error| format!("invalid requested byte count `{value}`: {error}"))
        })
        .transpose()?
        .unwrap_or(DEFAULT_REQUESTED_BYTES);
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument `{extra}`; {USAGE}"));
    }
    Ok(Options {
        selection,
        requested,
    })
}

fn run_child(unique_id: u64, requested: usize) -> Result<(), Box<dyn std::error::Error>> {
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut session = device.acquire_host_visible_memory_session()?;
    let layout = session.allocate(requested)?;
    session.with_bytes_mut(|bytes| {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (index as u8).wrapping_mul(17).wrapping_add(3);
        }
    })?;
    session.verify_dontfork_child_negative()?;
    session.with_bytes(|bytes| {
        assert_eq!(bytes[0], 3);
        assert_eq!(
            bytes[bytes.len() - 1],
            ((bytes.len() - 1) as u8).wrapping_mul(17).wrapping_add(3)
        );
    })?;
    session.map_to_gpu()?;
    assert_eq!(session.phase(), HostVisibleMemoryPhase::GpuAccessible);
    session.unmap_from_gpu()?;
    session.with_bytes(|bytes| assert_eq!(bytes[0], 3))?;
    session.release()?;
    assert_eq!(session.phase(), HostVisibleMemoryPhase::Released);
    let model = session.model_journal_summary();
    println!(
        "profile_sha256={} unique_id={unique_id:016x} requested={} backing={} dontfork_child=absent map_unmap=success release=success model_vms={} model_reservations={} model_allocations={} model_mappings={}",
        HOST_VISIBLE_MEMORY_PROFILE_SHA256_V1,
        layout.requested_bytes(),
        layout.backing_bytes(),
        model.vm_records(),
        model.reservation_records(),
        model.allocation_records(),
        model.mapping_records(),
    );
    Ok(())
}

fn run_isolated_child(
    executable: &Path,
    unique_id: u64,
    requested: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new(executable)
        .arg(unique_id.to_string())
        .arg(requested.to_string())
        .env(CHILD_ENV, "1")
        .status()?;
    if !status.success() {
        return Err(format!(
            "isolated memory child for unique ID {unique_id:016x} failed with {status}"
        )
        .into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_options(std::env::args().skip(1))?;
    if std::env::var_os(CHILD_ENV).is_some() {
        let GpuSelection::UniqueId(unique_id) = options.selection else {
            return Err("isolated memory child requires one explicit unique ID".into());
        };
        return run_child(unique_id, options.requested);
    }

    let unique_ids = match options.selection {
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
        run_isolated_child(&executable, unique_id, options.requested)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_REQUESTED_BYTES, GpuSelection, Options, parse_options};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn explicit_unique_ids_accept_decimal_and_hex() {
        assert_eq!(
            parse_options(args(&["42"])).unwrap(),
            Options {
                selection: GpuSelection::UniqueId(42),
                requested: DEFAULT_REQUESTED_BYTES,
            }
        );
        assert_eq!(
            parse_options(args(&["0x2a", "8193"])).unwrap(),
            Options {
                selection: GpuSelection::UniqueId(42),
                requested: 8193,
            }
        );
    }

    #[test]
    fn all_uses_the_same_default_and_override() {
        assert_eq!(
            parse_options(args(&["--all"])).unwrap(),
            Options {
                selection: GpuSelection::All,
                requested: DEFAULT_REQUESTED_BYTES,
            }
        );
        assert_eq!(
            parse_options(args(&["--all", "12289"])).unwrap(),
            Options {
                selection: GpuSelection::All,
                requested: 12289,
            }
        );
    }

    #[test]
    fn malformed_or_ambiguous_arguments_are_rejected() {
        assert!(parse_options(args(&[])).is_err());
        assert!(parse_options(args(&["not-an-id"])).is_err());
        assert!(parse_options(args(&["--all", "not-bytes"])).is_err());
        assert!(parse_options(args(&["42", "4097", "extra"])).is_err());
    }
}
