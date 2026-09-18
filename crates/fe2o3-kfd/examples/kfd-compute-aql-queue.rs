//! Isolated MI300X CREATE/doorbell-map/DESTROY validation without MMIO stores.

use std::path::Path;
use std::process::Command;

use fe2o3_kfd::topology::discover_default_topology;
use fe2o3_kfd::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1,
    DeviceSelector, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1, Gfx942DeviceBackingBudgetV1,
    Gfx942DeviceBackingUsageV1, Gfx942HostVisibleBackingBudgetV1, Gfx942HostVisibleBackingUsageV1,
    Gfx942SdmaMemoryPoolObservationV1, OpenedKfd, PrimaryQueueReleaseCustodyV1,
};

const CHILD_ENV: &str = "FE2O3_KFD_COMPUTE_AQL_QUEUE_CHILD";
const USAGE: &str = "usage: kfd-compute-aql-queue [--retained-release] (--all|<selected-unique-id>) | --retained-release-sdma (generic|0|1) <selected-unique-id>";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    selection: GpuSelection,
    release: ReleaseMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseMode {
    Legacy,
    Primary,
    SingleSdma(Option<u32>),
}

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

fn parse_selection(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let selected = args.next().ok_or_else(|| USAGE.to_owned())?;
    let (release, selected) = match selected.as_str() {
        "--retained-release" => (
            ReleaseMode::Primary,
            args.next().ok_or_else(|| USAGE.to_owned())?,
        ),
        "--retained-release-sdma" => {
            let engine = match args.next().as_deref() {
                Some("generic") => None,
                Some("0") => Some(0),
                Some("1") => Some(1),
                _ => return Err(USAGE.to_owned()),
            };
            (
                ReleaseMode::SingleSdma(engine),
                args.next().ok_or_else(|| USAGE.to_owned())?,
            )
        }
        _ => (ReleaseMode::Legacy, selected),
    };
    let selection = if selected == "--all" {
        if matches!(release, ReleaseMode::SingleSdma(_)) {
            return Err("single SDMA validation requires one explicit unique ID".to_owned());
        }
        GpuSelection::All
    } else {
        GpuSelection::UniqueId(parse_u64(&selected)?)
    };
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument `{extra}`; {USAGE}"));
    }
    Ok(Options { selection, release })
}

fn release_single_sdma(
    mut queue: ComputeAqlQueueSessionV1,
    engine: Option<u32>,
) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
    let primary_id = queue.observation().queue_id();
    let device_before = queue
        .device_backing_usage_v1()
        .expect("configured device account");
    let host_before = queue
        .host_visible_backing_usage_v1()
        .expect("configured host account");
    assert_eq!(
        device_before,
        Gfx942DeviceBackingUsageV1 {
            budget: Gfx942DeviceBackingBudgetV1::new(4096, 1).unwrap(),
            used_backing_bytes: 0,
            used_allocation_records: 0,
            reserved_records: 0,
            retained_records: 0,
            quarantined_records: 0,
            poisoned: false,
        }
    );
    assert_eq!(
        host_before.budget,
        Gfx942HostVisibleBackingBudgetV1::new(16 * 1024 * 1024, 32).unwrap()
    );
    assert!(host_before.used_backing_bytes > 0 && host_before.used_allocation_records > 0);
    assert_eq!(host_before.reserved_records, 0);
    assert_eq!(host_before.retained_records, 0);
    assert_eq!(host_before.quarantined_records, 0);
    assert!(!host_before.poisoned);
    let sdma = match engine {
        Some(index) => queue.enable_gfx942_sdma_copy_engine_on_engine_index(index)?,
        None => queue.enable_sdma_copy_engine()?,
    };
    assert_eq!(sdma.engine_index, engine);
    assert_ne!(sdma.queue_id, primary_id);
    assert_eq!(queue.device_backing_usage_v1(), Some(device_before));
    // Only the ordinary coherent completion page joins the host account.
    assert_eq!(
        queue.host_visible_backing_usage_v1(),
        Some(Gfx942HostVisibleBackingUsageV1 {
            used_backing_bytes: host_before.used_backing_bytes + 4096,
            used_allocation_records: host_before.used_allocation_records + 1,
            ..host_before
        })
    );
    assert_eq!(
        queue.sdma_memory_pool_observation()?,
        Gfx942SdmaMemoryPoolObservationV1::default()
    );
    assert!(queue.supports_retained_primary_release_v1()?);
    queue.preflight_primary_release_v1()?;
    let mut custody = PrimaryQueueReleaseCustodyV1::new(queue);
    let destroyed = custody.release_in_place()?;
    assert_eq!(destroyed.queue_id(), primary_id);
    assert_eq!(destroyed.released_resources(), 8);
    let empty_host = Gfx942HostVisibleBackingUsageV1 {
        used_backing_bytes: 0,
        used_allocation_records: 0,
        ..host_before
    };
    assert_eq!(custody.device_backing_usage_v1(), Some(device_before));
    assert_eq!(custody.host_visible_backing_usage_v1(), Some(empty_host));
    assert!(matches!(
        custody.release_in_place(),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "primary release is one-shot"
        ))
    ));
    assert_eq!(custody.device_backing_usage_v1(), Some(device_before));
    assert_eq!(custody.host_visible_backing_usage_v1(), Some(empty_host));
    drop(custody);
    let profile = match engine {
        None => "generic",
        Some(0) => "0",
        Some(1) => "1",
        _ => unreachable!(),
    };
    println!(
        "retained_single_sdma_release=complete engine={profile} primary_queue_id={primary_id} sdma_queue_id={} resources_returned=8 device_backing=refunded host_backing=refunded retry=rejected public_root_drop=completed packets=0 mmio_stores=0",
        sdma.queue_id
    );
    Ok(destroyed)
}

fn run_child(unique_id: u64, release: ReleaseMode) -> Result<(), Box<dyn std::error::Error>> {
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut queue = if matches!(release, ReleaseMode::SingleSdma(_)) {
        device.create_compute_aql_queue_with_backing_budgets_v1(
            4096,
            Gfx942DeviceBackingBudgetV1::new(4096, 1),
            Gfx942HostVisibleBackingBudgetV1::new(16 * 1024 * 1024, 32),
        )?
    } else {
        device.create_compute_aql_queue(4096)?
    };
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
    assert_eq!(observation.cwsr_shadow_pages(), 24);
    queue.verify_doorbell_dontfork()?;
    queue.verify_exception_shadows_dontfork()?;
    let destroyed = if let ReleaseMode::SingleSdma(engine) = release {
        release_single_sdma(queue, engine)?
    } else if release == ReleaseMode::Primary {
        assert!(queue.supports_retained_primary_release_v1()?);
        queue.preflight_primary_release_v1()?;
        let mut custody = PrimaryQueueReleaseCustodyV1::new(queue);
        let destroyed = custody.release_in_place()?;
        assert_eq!(destroyed.queue_id(), observation.queue_id());
        assert_eq!(destroyed.released_resources(), 5);
        drop(custody);
        println!(
            "retained_primary_release=complete public_root_drop=completed packets=0 mmio_stores=0"
        );
        destroyed
    } else {
        queue.destroy()?
    };
    assert_eq!(destroyed.queue_id(), 0);
    let resources = if matches!(release, ReleaseMode::SingleSdma(_)) {
        8
    } else {
        5
    };
    assert_eq!(destroyed.released_resources(), resources);
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

fn run_isolated_child(
    executable: &Path,
    unique_id: u64,
    release: ReleaseMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new(executable);
    match release {
        ReleaseMode::Legacy => {}
        ReleaseMode::Primary => {
            command.arg("--retained-release");
        }
        ReleaseMode::SingleSdma(engine) => {
            command.arg("--retained-release-sdma").arg(match engine {
                None => "generic".to_owned(),
                Some(index) => index.to_string(),
            });
        }
    }
    let status = command
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
    let options = parse_selection(std::env::args().skip(1))?;
    if std::env::var_os(CHILD_ENV).is_some() {
        let GpuSelection::UniqueId(unique_id) = options.selection else {
            return Err("isolated compute-AQL queue child requires one explicit unique ID".into());
        };
        return run_child(unique_id, options.release);
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
        run_isolated_child(&executable, unique_id, options.release)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GpuSelection, Options, ReleaseMode, parse_selection};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn explicit_unique_ids_accept_decimal_and_hex() {
        assert_eq!(
            parse_selection(args(&["42"])).unwrap(),
            Options {
                selection: GpuSelection::UniqueId(42),
                release: ReleaseMode::Legacy
            }
        );
        assert_eq!(
            parse_selection(args(&["0x2a"])).unwrap(),
            Options {
                selection: GpuSelection::UniqueId(42),
                release: ReleaseMode::Legacy
            }
        );
    }

    #[test]
    fn all_is_an_explicit_selection() {
        assert_eq!(
            parse_selection(args(&["--all"])).unwrap(),
            Options {
                selection: GpuSelection::All,
                release: ReleaseMode::Legacy
            }
        );
    }

    #[test]
    fn malformed_or_ambiguous_arguments_are_rejected() {
        assert!(parse_selection(args(&[])).is_err());
        assert!(parse_selection(args(&["not-an-id"])).is_err());
        assert!(parse_selection(args(&["42", "extra"])).is_err());
        assert!(parse_selection(args(&["--all", "extra"])).is_err());
        assert!(parse_selection(args(&["--retained-release"])).is_err());
        assert!(parse_selection(args(&["42", "--retained-release"])).is_err());
        assert!(
            parse_selection(args(&["--retained-release", "--retained-release", "42"])).is_err()
        );
    }

    #[test]
    fn retained_release_requires_explicit_mode_and_device_selection() {
        assert_eq!(
            parse_selection(args(&["--retained-release", "0x2a"])).unwrap(),
            Options {
                selection: GpuSelection::UniqueId(42),
                release: ReleaseMode::Primary
            }
        );
        assert_eq!(
            parse_selection(args(&["--retained-release", "--all"])).unwrap(),
            Options {
                selection: GpuSelection::All,
                release: ReleaseMode::Primary
            }
        );
    }

    #[test]
    fn single_sdma_requires_a_known_profile_and_one_explicit_device() {
        for (profile, engine) in [("generic", None), ("0", Some(0)), ("1", Some(1))] {
            for id in ["42", "0x2a"] {
                assert_eq!(
                    parse_selection(args(&["--retained-release-sdma", profile, id])).unwrap(),
                    Options {
                        selection: GpuSelection::UniqueId(42),
                        release: ReleaseMode::SingleSdma(engine),
                    }
                );
            }
            assert!(parse_selection(args(&["--retained-release-sdma", profile, "--all"])).is_err());
            assert!(parse_selection(args(&["--retained-release-sdma", profile])).is_err());
            assert!(
                parse_selection(args(&["--retained-release-sdma", profile, "42", "extra"]))
                    .is_err()
            );
        }
        for profile in ["2", "-1", "none", "0x0", "--retained-release"] {
            assert!(parse_selection(args(&["--retained-release-sdma", profile, "42"])).is_err());
        }
        assert!(parse_selection(args(&["--retained-release-sdma"])).is_err());
    }
}
