//! Isolated MI300X CREATE/doorbell-map/DESTROY validation without MMIO stores.

use std::path::Path;
use std::process::Command;

use fe2o3_kfd::topology::discover_default_topology;
use fe2o3_kfd::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1,
    DeviceSelector, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1, GFX942_SDMA_MAX_IN_FLIGHT_V1,
    Gfx942DeviceBackingBudgetV1, Gfx942DeviceBackingUsageV1, Gfx942HostVisibleBackingBudgetV1,
    Gfx942HostVisibleBackingUsageV1, Gfx942SdmaMemoryPoolObservationV1,
    Gfx942SdmaQueueObservationV1, OpenedKfd, PrimaryQueueReleaseCustodyV1,
};

const CHILD_ENV: &str = "FE2O3_KFD_COMPUTE_AQL_QUEUE_CHILD";
const USAGE: &str = "usage: kfd-compute-aql-queue [--retained-release] (--all|<selected-unique-id>) | --retained-release-sdma (generic|0|1) <selected-unique-id> | --retained-release-striped-sdma (2|4|6|8|10|12|14|16) <selected-unique-id>";

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
    StripedSdma(u32),
}

impl ReleaseMode {
    fn sdma_queue_count(self) -> usize {
        match self {
            Self::Legacy | Self::Primary => 0,
            Self::SingleSdma(_) => 1,
            Self::StripedSdma(count) => count as usize,
        }
    }

    fn released_resources(self) -> usize {
        5 + 3 * self.sdma_queue_count()
    }
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
        "--retained-release-striped-sdma" => {
            let count = match args.next().as_deref() {
                Some(count @ ("2" | "4" | "6" | "8" | "10" | "12" | "14" | "16")) => {
                    count.parse().expect("closed decimal queue-count roster")
                }
                _ => return Err(USAGE.to_owned()),
            };
            (
                ReleaseMode::StripedSdma(count),
                args.next().ok_or_else(|| USAGE.to_owned())?,
            )
        }
        _ => (ReleaseMode::Legacy, selected),
    };
    let selection = if selected == "--all" {
        if release.sdma_queue_count() != 0 {
            return Err("SDMA validation requires one explicit unique ID".to_owned());
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

fn retained_host_usage(
    budget: Gfx942HostVisibleBackingBudgetV1,
    bytes: u64,
    records: usize,
) -> Gfx942HostVisibleBackingUsageV1 {
    Gfx942HostVisibleBackingUsageV1 {
        budget,
        used_backing_bytes: bytes,
        used_allocation_records: records as u64,
        reserved_records: 0,
        retained_records: records,
        quarantined_records: 0,
        poisoned: false,
    }
}

fn validate_sdma_observations(
    observations: &[Gfx942SdmaQueueObservationV1],
    primary_id: u32,
    release: ReleaseMode,
) {
    assert_eq!(observations.len(), release.sdma_queue_count());
    for (index, observation) in observations.iter().enumerate() {
        let expected_engine = match release {
            ReleaseMode::SingleSdma(engine) => engine,
            ReleaseMode::StripedSdma(_) => Some((index % 2) as u32),
            _ => unreachable!("SDMA observations require an SDMA release mode"),
        };
        assert_eq!(observation.engine_index, expected_engine);
        assert_eq!(observation.ring_bytes, 4096);
        assert_eq!(
            usize::from(observation.maximum_in_flight),
            GFX942_SDMA_MAX_IN_FLIGHT_V1
        );
        assert_ne!(observation.queue_id, primary_id);
        assert!(
            observations[..index]
                .iter()
                .all(|previous| previous.queue_id != observation.queue_id)
        );
    }
}

fn release_sdma(
    mut queue: ComputeAqlQueueSessionV1,
    release: ReleaseMode,
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
    assert_eq!(
        host_before,
        retained_host_usage(
            host_before.budget,
            host_before.used_backing_bytes,
            usize::try_from(host_before.used_allocation_records).unwrap(),
        )
    );
    let sdma = match release {
        ReleaseMode::SingleSdma(Some(index)) => {
            vec![queue.enable_gfx942_sdma_copy_engine_on_engine_index(index)?]
        }
        ReleaseMode::SingleSdma(None) => vec![queue.enable_sdma_copy_engine()?],
        ReleaseMode::StripedSdma(count) => queue.enable_gfx942_striped_sdma_copy_engines(count)?,
        _ => unreachable!("SDMA release requires an SDMA release mode"),
    };
    validate_sdma_observations(&sdma, primary_id, release);
    assert_eq!(queue.device_backing_usage_v1(), Some(device_before));
    // Only each ordinary coherent completion page joins the host account.
    assert_eq!(
        queue.host_visible_backing_usage_v1(),
        Some(retained_host_usage(
            host_before.budget,
            host_before.used_backing_bytes + 4096 * sdma.len() as u64,
            host_before.retained_records + sdma.len(),
        ))
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
    assert_eq!(
        usize::from(destroyed.released_resources()),
        release.released_resources()
    );
    let empty_host = retained_host_usage(host_before.budget, 0, 0);
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
    match release {
        ReleaseMode::SingleSdma(engine) => {
            let profile = match engine {
                None => "generic",
                Some(0) => "0",
                Some(1) => "1",
                _ => unreachable!(),
            };
            println!(
                "retained_single_sdma_release=complete engine={profile} primary_queue_id={primary_id} sdma_queue_id={} resources_returned=8 device_backing=refunded host_backing=refunded retry=rejected public_root_drop=completed packets=0 mmio_stores=0",
                sdma[0].queue_id
            );
        }
        ReleaseMode::StripedSdma(count) => {
            let ids = sdma
                .iter()
                .map(|queue| queue.queue_id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let engines = sdma
                .iter()
                .map(|queue| queue.engine_index.unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",");
            // No publication occurs in this probe, so creation's cursor cannot advance.
            println!(
                "retained_striped_sdma_release=complete queue_count={count} cursor=initial-0-no-advance primary_queue_id={primary_id} sdma_queue_ids={ids} engine_placement={engines} host_delta_bytes={} host_delta_records={count} resources_returned={} device_backing=refunded host_backing=refunded retry=rejected public_root_drop=completed packets=0 mmio_stores=0",
                4096 * u64::from(count),
                destroyed.released_resources()
            );
        }
        _ => unreachable!(),
    }
    Ok(destroyed)
}

fn run_child(unique_id: u64, release: ReleaseMode) -> Result<(), Box<dyn std::error::Error>> {
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut queue = if release.sdma_queue_count() != 0 {
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
    let destroyed = if release.sdma_queue_count() != 0 {
        release_sdma(queue, release)?
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
    assert_eq!(
        usize::from(destroyed.released_resources()),
        release.released_resources()
    );
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
        ReleaseMode::StripedSdma(count) => {
            command
                .arg("--retained-release-striped-sdma")
                .arg(count.to_string());
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
    use super::{
        GFX942_SDMA_MAX_IN_FLIGHT_V1, Gfx942HostVisibleBackingBudgetV1,
        Gfx942HostVisibleBackingUsageV1, Gfx942SdmaQueueObservationV1, GpuSelection, Options,
        ReleaseMode, parse_selection, retained_host_usage, validate_sdma_observations,
    };

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn host_usage_oracle_counts_live_retained_records_and_empty_refund() {
        let budget = Gfx942HostVisibleBackingBudgetV1::new(16 * 1024 * 1024, 32).unwrap();
        for records in 0..=17 {
            assert_eq!(
                retained_host_usage(budget, records * 4096, records as usize),
                Gfx942HostVisibleBackingUsageV1 {
                    budget,
                    used_backing_bytes: records * 4096,
                    used_allocation_records: records,
                    reserved_records: 0,
                    retained_records: records as usize,
                    quarantined_records: 0,
                    poisoned: false,
                }
            );
        }
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

    #[test]
    fn striped_sdma_accepts_every_balanced_count_and_one_explicit_device() {
        for count in [2, 4, 6, 8, 10, 12, 14, 16] {
            for id in ["42", "0x2a"] {
                assert_eq!(
                    parse_selection(args(&[
                        "--retained-release-striped-sdma",
                        &count.to_string(),
                        id
                    ]))
                    .unwrap(),
                    Options {
                        selection: GpuSelection::UniqueId(42),
                        release: ReleaseMode::StripedSdma(count)
                    }
                );
            }
        }
    }

    #[test]
    fn striped_sdma_rejects_unsupported_or_ambiguous_arguments() {
        for count in [
            "0",
            "1",
            "3",
            "15",
            "17",
            "18",
            "-2",
            "+2",
            "02",
            "0x2",
            "generic",
            "4294967296",
        ] {
            assert!(
                parse_selection(args(&["--retained-release-striped-sdma", count, "42"])).is_err()
            );
        }
        for count in ["2", "4", "6", "8", "10", "12", "14", "16"] {
            assert!(
                parse_selection(args(&["--retained-release-striped-sdma", count, "--all"]))
                    .is_err()
            );
            assert!(parse_selection(args(&["--retained-release-striped-sdma", count])).is_err());
            assert!(
                parse_selection(args(&[
                    "--retained-release-striped-sdma",
                    count,
                    "42",
                    "extra"
                ]))
                .is_err()
            );
        }
        assert!(parse_selection(args(&["--retained-release-striped-sdma"])).is_err());
    }

    fn striped_observations(count: u32) -> Vec<Gfx942SdmaQueueObservationV1> {
        (0..count)
            .map(|index| Gfx942SdmaQueueObservationV1 {
                queue_id: 7 + index * 3,
                ring_bytes: 4096,
                maximum_in_flight: GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
                engine_index: Some(index % 2),
            })
            .collect()
    }

    #[test]
    fn observation_oracle_accepts_sparse_ids_and_all_admitted_profiles() {
        for count in [2, 4, 6, 8, 10, 12, 14, 16] {
            validate_sdma_observations(
                &striped_observations(count),
                0,
                ReleaseMode::StripedSdma(count),
            );
        }
        for engine in [None, Some(0), Some(1)] {
            let mut observations = striped_observations(1);
            observations[0].engine_index = engine;
            validate_sdma_observations(&observations, 0, ReleaseMode::SingleSdma(engine));
        }
    }

    #[test]
    fn observation_oracle_rejects_each_malformed_field_and_roster() {
        for count in [2, 4, 6, 8, 10, 12, 14, 16] {
            for fault in 0..8 {
                let mut observations = striped_observations(count);
                let last = observations.last_mut().unwrap();
                match fault {
                    0 => last.queue_id = 0,
                    1 => last.queue_id = 7,
                    2 => last.ring_bytes = 8192,
                    3 => last.maximum_in_flight -= 1,
                    4 => last.engine_index = Some(0),
                    5 => last.engine_index = None,
                    6 => {
                        observations.pop();
                    }
                    7 => observations.clear(),
                    _ => unreachable!(),
                }
                assert!(
                    std::panic::catch_unwind(|| validate_sdma_observations(
                        &observations,
                        0,
                        ReleaseMode::StripedSdma(count)
                    ))
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn resource_oracle_counts_primary_and_all_sdma_owners() {
        for release in [ReleaseMode::Legacy, ReleaseMode::Primary] {
            assert_eq!(release.sdma_queue_count(), 0);
            assert_eq!(release.released_resources(), 5);
        }
        for engine in [None, Some(0), Some(1)] {
            let release = ReleaseMode::SingleSdma(engine);
            assert_eq!(release.sdma_queue_count(), 1);
            assert_eq!(release.released_resources(), 8);
        }
        for (count, resources) in [
            (2, 11),
            (4, 17),
            (6, 23),
            (8, 29),
            (10, 35),
            (12, 41),
            (14, 47),
            (16, 53),
        ] {
            let release = ReleaseMode::StripedSdma(count);
            assert_eq!(release.sdma_queue_count(), count as usize);
            assert_eq!(release.released_resources(), resources);
        }
    }
}
