#![recursion_limit = "256"]

use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    Module, TargetCapability, VerifiedCanonicalKernelIrV7, WaveWidth,
};
use fe2o3_semantic_import::{
    CaptureUnavailableReasonV1, ProfilerSourceKindV4, ProfilerUnavailableFactV4, TruthOriginV1,
    decode_profiler_bundle_v4,
};

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TestProcessIdentity {
    pid: i32,
    start_time: u64,
    process_group: i32,
    session: i32,
}

#[derive(Debug)]
struct TestProcessStat {
    state: char,
    process_group: i32,
    session: i32,
    start_time: u64,
}

struct PinnedTestProcess {
    identity: TestProcessIdentity,
    descriptor: rustix::fd::OwnedFd,
}

impl Drop for PinnedTestProcess {
    fn drop(&mut self) {
        let _ = rustix::process::pidfd_send_signal(&self.descriptor, rustix::process::Signal::KILL);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TestProcessPublication {
    leader: TestProcessIdentity,
    descendant: TestProcessIdentity,
}

fn parse_test_process_identity_fields(fields: &[&str]) -> Result<TestProcessIdentity, String> {
    if fields.len() != 4 {
        return Err("unexpected test process identity width".to_owned());
    }
    Ok(TestProcessIdentity {
        pid: fields[0]
            .parse()
            .map_err(|_| "invalid published test PID".to_owned())?,
        start_time: fields[1]
            .parse()
            .map_err(|_| "invalid published test start time".to_owned())?,
        process_group: fields[2]
            .parse()
            .map_err(|_| "invalid published test process group".to_owned())?,
        session: fields[3]
            .parse()
            .map_err(|_| "invalid published test session".to_owned())?,
    })
}

fn parse_test_process_publication(source: &str) -> Result<TestProcessPublication, String> {
    let fields: Vec<_> = source.split_whitespace().collect();
    if fields.len() != 8 {
        return Err("unexpected test process publication width".to_owned());
    }
    Ok(TestProcessPublication {
        leader: parse_test_process_identity_fields(&fields[..4])?,
        descendant: parse_test_process_identity_fields(&fields[4..])?,
    })
}

fn read_test_process_stat(pid: i32) -> Result<Option<TestProcessStat>, String> {
    let source = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to read test process {pid} stat: {error}")),
    };
    let (identity, fields) = source
        .trim_end()
        .rsplit_once(") ")
        .ok_or_else(|| format!("test process {pid} stat has no final command delimiter"))?;
    let observed_pid = identity
        .split_once(" (")
        .ok_or_else(|| format!("test process {pid} stat has no command prefix"))?
        .0
        .parse::<i32>()
        .map_err(|_| format!("test process {pid} stat has an invalid PID"))?;
    if observed_pid != pid {
        return Err(format!(
            "test process {pid} stat reported PID {observed_pid}"
        ));
    }
    let fields: Vec<_> = fields.split_whitespace().collect();
    if fields.len() < 20 {
        return Err(format!("test process {pid} stat is truncated"));
    }
    let parse_i32 = |index: usize, label: &str| {
        fields[index]
            .parse::<i32>()
            .map_err(|_| format!("test process {pid} stat has an invalid {label}"))
    };
    Ok(Some(TestProcessStat {
        state: fields[0]
            .parse::<char>()
            .map_err(|_| format!("test process {pid} stat has an invalid state"))?,
        process_group: parse_i32(2, "process group")?,
        session: parse_i32(3, "session")?,
        start_time: fields[19]
            .parse::<u64>()
            .map_err(|_| format!("test process {pid} stat has an invalid start time"))?,
    }))
}

fn test_process_is_terminal(stat: &TestProcessStat) -> bool {
    matches!(stat.state, 'Z' | 'X' | 'x')
}

fn validate_live_test_process(identity: TestProcessIdentity) -> Result<TestProcessStat, String> {
    let Some(before) = read_test_process_stat(identity.pid)? else {
        return Err(format!(
            "test process {} disappeared before pinning",
            identity.pid
        ));
    };
    if before.start_time != identity.start_time
        || before.process_group != identity.process_group
        || before.session != identity.session
    {
        return Err(format!(
            "test process {} changed identity before pinning: {before:?}",
            identity.pid
        ));
    }
    if test_process_is_terminal(&before) {
        return Err(format!(
            "test process {} was already terminal before pinning: {before:?}",
            identity.pid
        ));
    }
    Ok(before)
}

fn pin_live_test_process(identity: TestProcessIdentity) -> Result<PinnedTestProcess, String> {
    validate_live_test_process(identity)?;
    let pid = rustix::process::Pid::from_raw(identity.pid)
        .ok_or_else(|| format!("invalid test process PID {}", identity.pid))?;
    let descriptor = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())
        .map_err(|error| format!("failed to pin live test process {}: {error}", identity.pid))?;
    validate_live_test_process(identity)?;
    Ok(PinnedTestProcess {
        identity,
        descriptor,
    })
}

fn current_test_session() -> i32 {
    // SAFETY: getsid with zero reads the calling process's session without mutation.
    let session = unsafe { libc::getsid(0) };
    assert!(session > 0, "failed to read the test process session");
    session
}

fn pin_published_test_descendant(
    publication: TestProcessPublication,
    expected_session: i32,
) -> Result<PinnedTestProcess, String> {
    let leader = validate_live_test_process(publication.leader)?;
    let process = pin_live_test_process(publication.descendant)?;
    if publication.leader.process_group != publication.leader.pid
        || publication.leader.session != expected_session
        || publication.descendant.process_group != publication.leader.pid
        || publication.descendant.session != expected_session
        || publication.descendant.pid == publication.leader.pid
    {
        return Err(format!(
            "published process topology did not match the collector-group contract: publication={publication:?} leader={leader:?}"
        ));
    }
    Ok(process)
}

fn monitor_test_process_publication(
    publication_path: PathBuf,
    release_path: PathBuf,
    expected_session: i32,
) -> std::thread::JoinHandle<Result<PinnedTestProcess, String>> {
    std::thread::Builder::new()
        .name("fe2o3-cli-test-descendant-pin-v1".to_owned())
        .spawn(move || {
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(2))
                .ok_or("test publication deadline overflow".to_owned())?;
            let result = loop {
                match fs::read_to_string(&publication_path) {
                    Ok(source) => {
                        break parse_test_process_publication(&source).and_then(|publication| {
                            pin_published_test_descendant(publication, expected_session)
                        });
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        if Instant::now() >= deadline {
                            break Err(format!(
                                "test process publication {} was not created",
                                publication_path.display()
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => {
                        break Err(format!(
                            "failed to read test process publication {}: {error}",
                            publication_path.display()
                        ));
                    }
                }
            };
            match result {
                Ok(process) => {
                    fs::write(&release_path, b"release").map_err(|error| {
                        format!(
                            "failed to release test collector through {}: {error}",
                            release_path.display()
                        )
                    })?;
                    Ok(process)
                }
                Err(error) => Err(error),
            }
        })
        .unwrap()
}

fn join_test_process_monitor(
    monitor: std::thread::JoinHandle<Result<PinnedTestProcess, String>>,
) -> PinnedTestProcess {
    monitor
        .join()
        .expect("test process monitor panicked")
        .expect("test process publication failed validation")
}

fn wait_for_test_process_terminal(
    process: &PinnedTestProcess,
    timeout: Duration,
) -> Result<bool, String> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or("test process wait deadline overflow")?;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Ok(false);
        }
        let remaining = deadline.duration_since(now);
        let milliseconds = remaining
            .as_millis()
            .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
            .min(i32::MAX as u128) as i32;
        let mut poll = libc::pollfd {
            fd: process.descriptor.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `poll` points to one initialized pollfd and the timeout is bounded above.
        let result = unsafe { libc::poll(&mut poll, 1, milliseconds) };
        if result > 0 {
            let terminal = libc::POLLIN | libc::POLLHUP;
            let rejected = libc::POLLERR | libc::POLLNVAL;
            if poll.revents & rejected == 0 && poll.revents != 0 && poll.revents & !terminal == 0 {
                return Ok(true);
            }
            return Err(format!(
                "test process {} pidfd returned unexpected poll events {:#x}",
                process.identity.pid, poll.revents
            ));
        }
        if result == 0 {
            return Ok(false);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!(
                "failed to poll test process {} pidfd: {error}",
                process.identity.pid
            ));
        }
    }
}

fn assert_test_descendant_is_terminated(process: &PinnedTestProcess) {
    if wait_for_test_process_terminal(process, Duration::from_secs(1)).unwrap() {
        return;
    }
    let state = read_test_process_stat(process.identity.pid).unwrap();
    let cleanup =
        rustix::process::pidfd_send_signal(&process.descriptor, rustix::process::Signal::KILL);
    panic!(
        "live collector descendant survived process-group revoke: identity={:?} state={state:?} cleanup={cleanup:?}",
        process.identity
    );
}

struct Fixture {
    root: PathBuf,
    tool: PathBuf,
}

impl Fixture {
    fn new(exit_failure: bool) -> Self {
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let root = env::temp_dir().join(format!("cargo-fe2o3-profile-{}-{id}", process::id()));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
                    let tool = root.join("rocprofv3");
                    let behavior = if exit_failure {
                        "raise SystemExit(7)"
                    } else {
                        r#"
out = args[args.index("--output-directory") + 1]
os.makedirs(out, exist_ok=True)
with open(os.path.join(out, "capture_results.json"), "wb") as stream:
    stream.write(b'{"collector":"fixture"}')
target = args[args.index("--") + 1:]
raise SystemExit(subprocess.run(target, check=False).returncode)
"#
                    };
                    write_tool(&tool, behavior);
                    return Self { root, tool };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create fixture: {error}"),
            }
        }
    }

    fn output(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn plan(&self, output: &Path, target_args: &[&str]) -> Output {
        self.plan_with_options(output, &[], target_args)
    }

    fn plan_with_options(
        &self,
        output: &Path,
        profile_options: &[&str],
        target_args: &[&str],
    ) -> Output {
        self.plan_program_with_options(output, profile_options, Path::new("/bin/true"), target_args)
    }

    fn plan_program_with_options(
        &self,
        output: &Path,
        profile_options: &[&str],
        program: &Path,
        target_args: &[&str],
    ) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command.args(["profile", "--tool", self.tool.to_str().unwrap()]);
        command.args(profile_options);
        command.args([
            "--output-dir",
            output.to_str().unwrap(),
            "--",
            program.to_str().unwrap(),
        ]);
        command.args(target_args).output().unwrap()
    }

    fn replace_behavior(&self, behavior: &str) {
        write_tool(&self.tool, behavior);
    }
}

fn write_tool(path: &Path, behavior: &str) {
    fs::write(
        path,
        format!(
            r#"#!/usr/bin/env python3
# reviewed fixture surfaces: --kernel-trace --advanced-thread-trace
import os
import subprocess
import sys
import time
args = sys.argv[1:]
{behavior}
"#
        ),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn authorization(output: &Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("collection-authorization: "))
        .expect("plan authorization")
        .to_owned()
}

fn field(output: &Output, name: &str) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}: ")))
        .unwrap_or_else(|| panic!("missing {name}"))
        .to_owned()
}

fn discover_gfx942_kfd_agent() -> Option<serde_json::Value> {
    let root = Path::new("/sys/class/kfd/kfd/topology/nodes");
    let mut entries = fs::read_dir(root)
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let node = entry.file_name().to_str()?.parse::<u32>().ok()?;
        let gpu_id = fs::read_to_string(entry.path().join("gpu_id"))
            .ok()?
            .trim_end()
            .parse::<u64>()
            .ok()?;
        if gpu_id == 0 {
            continue;
        }
        let properties = fs::read_to_string(entry.path().join("properties")).ok()?;
        let mut values = BTreeMap::new();
        for line in properties.lines() {
            let (name, value) = line.split_once(' ')?;
            if values.insert(name, value.parse::<u64>().ok()?).is_some() {
                return None;
            }
        }
        if values.get("gfx_target_version") != Some(&90_402)
            || values.get("vendor_id") != Some(&4_098)
            || values.get("wave_front_size") != Some(&64)
        {
            continue;
        }
        return Some(serde_json::json!({
            "id": {"handle": 7001},
            "type": 2,
            "gpu_index": 0,
            "size": 312,
            "node_id": node,
            "simd_count": *values.get("simd_count")?,
            "gpu_id": gpu_id,
            "vendor_id": *values.get("vendor_id")?,
            "device_id": *values.get("device_id")?,
            "location_id": *values.get("location_id")?,
            "domain": *values.get("domain")?,
            "gfx_target_version": *values.get("gfx_target_version")?,
            "wave_front_size": *values.get("wave_front_size")?,
            "num_xcc": *values.get("num_xcc")?,
            "logical_node_id": node, "logical_node_type_id": 2,
            "cpu_cores_count": 0, "cpu_core_id_base": 0, "simd_id_base": 0,
            "max_waves_per_simd": 8, "lds_size_in_kb": 64, "gds_size_in_kb": 0,
            "num_gws": 64, "cu_count": *values.get("simd_count")?, "array_count": 8,
            "num_shader_banks": 4, "simd_arrays_per_engine": 2,
            "cu_per_simd_array": 19, "simd_per_cu": 4, "max_slots_scratch_cu": 32,
            "drm_render_minor": 128, "num_sdma_engines": 4,
            "num_sdma_xgmi_engines": 0, "num_sdma_queues_per_engine": 8,
            "num_cp_queues": 8, "max_engine_clk_ccompute": 2100,
            "max_engine_clk_fcompute": 2100,
            "sdma_fw_version": {"uCodeSDMA":1,"uCodeRes":0},
            "fw_version": {"uCode":1,"Major":0,"Minor":0,"Stepping":0},
            "capability": {"HotPluggable":0,"HSAMMUPresent":0,"SharedWithGraphics":0,"QueueSizePowerOfTwo":0,"QueueSize32bit":0,"QueueIdleEvent":0,"VALimit":0,"WatchPointsSupported":1,"WatchPointsTotalBits":2,"DoorbellType":2,"AQLQueueDoubleMap":0,"DebugTrapSupported":1,"WaveLaunchTrapOverrideSupported":1,"WaveLaunchModeSupported":1,"PreciseMemoryOperationsSupported":1,"DEPRECATED_SRAM_EDCSupport":0,"Mem_EDCSupport":1,"RASEventNotify":1,"ASICRevision":1,"SRAM_EDCSupport":1,"SVMAPISupported":1,"CoherentHostAccess":0,"DebugSupportedFirmware":1},
            "cu_per_engine": 38, "max_waves_per_cu": 32,
            "family_id": 145, "workgroup_max_size": 1024,
            "grid_max_size": 4294967295_u64, "local_mem_size": 65536, "hive_id": 1,
            "workgroup_max_dim": {"x":1024,"y":1024,"z":1024},
            "grid_max_dim": {"x":2147483647_u64,"y":65535,"z":65535},
            "name": "gfx942", "vendor_name": "AMD", "product_name": "MI300X",
            "model_name": "MI300X", "uuid": {"bytes":{"value0":1,"value1":2,"value2":3,"value3":4,"value4":5,"value5":6,"value6":7,"value7":8,"value8":0,"value9":0,"value10":0,"value11":0,"value12":0,"value13":0,"value14":0,"value15":0}}, "mem_banks": [],
            "mem_banks_count": 0, "caches": [], "caches_count": 0,
            "io_links": [], "io_links_count": 0,
            "runtime_visibility": {"hsa":1,"hip":1,"rccl":1,"rocdecode":1}
        }));
    }
    None
}

fn gfx942_kfd_agent() -> Option<serde_json::Value> {
    let agent = discover_gfx942_kfd_agent();
    let required = env::var("FE2O3_REQUIRE_GFX942_PROFILE_TEST")
        .ok()
        .as_deref()
        == Some("1");
    assert!(
        !required || agent.is_some(),
        "FE2O3_REQUIRE_GFX942_PROFILE_TEST=1 requires a directly observed gfx942 Wave64 KFD node"
    );
    agent
}

fn require_gfx942_profile_test() -> bool {
    env::var("FE2O3_REQUIRE_GFX942_PROFILE_TEST").as_deref() == Ok("1")
}

fn exact_gfx942_kir(fixture: &Fixture) -> PathBuf {
    let mut module = Module::new("profile-cli-gfx942-v1");
    module
        .required_capabilities
        .insert(TargetCapability::Extension {
            namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.to_owned(),
        });
    module
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    let owner = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
    let path = fixture.output("profile.kir");
    fs::write(&path, owner.canonical_bytes()).unwrap();
    path
}

fn current_dispatch_json(agent: serde_json::Value) -> Vec<u8> {
    let value = serde_json::json!({
        "rocprofiler-sdk-tool": [{
            "metadata": {"node": {"id":0,"hash":0,"machine_id":"fixture","system_name":"Linux","hostname":"fixture","release":"fixture","version":"fixture","hardware_name":"x86_64","domain_name":"(none)"}, "pid": 100, "init_time": 1, "fini_time": 2, "command": [], "config": {}},
            "agents": [agent],
            "buffer_records": {"kernel_dispatch": [{
                "size": 184,
                "kind": 11,
                "operation": 2,
                "thread_id": 100,
                "correlation_id": {"internal": 1, "external": 0},
                "start_timestamp": 100,
                "end_timestamp": 180,
                "dispatch_info": {
                    "size": 72,
                    "agent_id": {"handle": 7001},
                    "queue_id": {"handle": 1},
                    "kernel_id": 10,
                    "dispatch_id": 1,
                    "private_segment_size": 0,
                    "group_segment_size": 0,
                    "workgroup_size": {"x": 64, "y": 1, "z": 1},
                    "grid_size": {"x": 256, "y": 1, "z": 1}
                },
                "stream_id": {"handle": 0}
            }],
            "hip_api": [], "hsa_api": [], "rccl_api": [],
            "rocdecode_api": [], "rocjpeg_api": [], "marker_api": [],
            "memory_copy": [], "memory_allocation": [], "scratch_memory": [],
            "pc_sample_host_trap": [], "pc_sample_stochastic": []},
            "callback_records": {"counter_collection": []},
            "counters": [], "code_objects": [], "kernel_symbols": [],
            "strings": {"callback_records": [], "buffer_records": [], "marker_api": [],
                "correlation_id": {"external": []},
                "counters": {"dimension_ids": []}, "pc_sample_instructions": [],
                "pc_sample_comments": [], "att_filenames": [],
                "code_object_snapshot_filenames": []},
            "summary": [], "host_functions": []
        }]
    });
    serde_json::to_vec(&value).unwrap()
}

fn current_dispatch_csv(node: u64) -> Vec<u8> {
    format!(
        "\"Kind\",\"Agent_Id\",\"Queue_Id\",\"Stream_Id\",\"Thread_Id\",\"Dispatch_Id\",\"Kernel_Id\",\"Kernel_Name\",\"Correlation_Id\",\"Start_Timestamp\",\"End_Timestamp\",\"LDS_Block_Size\",\"Scratch_Size\",\"VGPR_Count\",\"Accum_VGPR_Count\",\"SGPR_Count\",\"Workgroup_Size_X\",\"Workgroup_Size_Y\",\"Workgroup_Size_Z\",\"Grid_Size_X\",\"Grid_Size_Y\",\"Grid_Size_Z\"\n\"KERNEL_DISPATCH\",\"Agent {node}\",1,0,100,1,10,\"fixture\",1,100,180,0,0,12,4,48,64,1,1,256,1,1\n"
    )
    .into_bytes()
}

fn install_dispatch_behavior(fixture: &Fixture, name: &str, source: &[u8], copies: usize) {
    let source = String::from_utf8(source.to_vec()).unwrap();
    let literal = serde_json::to_string(&source).unwrap();
    let mut writes = String::new();
    for ordinal in 0..copies {
        writes.push_str(&format!(
            "with open(os.path.join(out, {name:?} + str({ordinal})), \"w\", encoding=\"utf-8\") as stream:\n    stream.write({literal})\n"
        ));
    }
    fixture.replace_behavior(&format!(
        "out = args[args.index(\"--output-directory\") + 1]\nos.makedirs(out, exist_ok=True)\n{writes}target = args[args.index(\"--\") + 1:]\nraise SystemExit(subprocess.run(target, check=False).returncode)\n"
    ));
}

fn collect(fixture: &Fixture, output: &Path, auth: &str, target_args: &[&str]) -> Output {
    collect_with_options(fixture, output, auth, &[], target_args)
}

fn collect_with_options(
    fixture: &Fixture,
    output: &Path,
    auth: &str,
    profile_options: &[&str],
    target_args: &[&str],
) -> Output {
    collect_program_with_options(
        fixture,
        output,
        auth,
        profile_options,
        Path::new("/bin/true"),
        target_args,
    )
}

fn collect_program_with_options(
    fixture: &Fixture,
    output: &Path,
    auth: &str,
    profile_options: &[&str],
    program: &Path,
    target_args: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command.args([
        "profile",
        "--collect",
        "--authorize-collection",
        auth,
        "--tool",
        fixture.tool.to_str().unwrap(),
    ]);
    command.args(profile_options);
    command.args([
        "--output-dir",
        output.to_str().unwrap(),
        "--",
        program.to_str().unwrap(),
    ]);
    command.args(target_args).output().unwrap()
}

#[test]
fn dry_run_is_inert_and_reports_capabilities_without_claiming_observation() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let output = fixture.plan(&output_directory, &["argument with space"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("authority: plan-only"));
    assert!(stdout.contains("stateful-action: not-executed"));
    assert!(stdout.contains("dispatch-observability-origin: unavailable"));
    assert!(stdout.contains("collector-runtime-limitation:"));
    assert!(stdout.contains("next-import-status: unavailable-missing-exact-canonical-kir-v7"));
    assert!(!stdout.contains("next-import-program:"));
    assert!(!output_directory.exists());
}

#[test]
fn exact_authorization_collects_without_a_shell_and_writes_a_bounded_manifest() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let marker = fixture.output("injected");
    let payload = format!(";touch {}", marker.display());
    let plan = fixture.plan(&output_directory, &[&payload]);
    assert!(plan.status.success());
    let output = collect(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &[&payload],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence = String::from_utf8(output.stdout).unwrap();
    assert!(evidence.contains("outcome: collector-completed-artifacts-unvalidated"));
    assert!(evidence.contains("dispatch-observation-origin: unavailable"));
    assert!(!marker.exists());
    let manifest =
        fs::read_to_string(output_directory.join("fe2o3-profile-manifest-v1.txt")).unwrap();
    assert!(manifest.contains("schema: fe2o3-profile-artifact-manifest-v1"));
    assert!(manifest.contains("capture_results.json"));
    assert!(manifest.contains("status=content-schema-eligible-requires-admission"));
    assert_eq!(
        fs::metadata(&output_directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[test]
fn live_qualification_archives_exact_runtime_bytes_and_rejects_stale_input() {
    let runtime_source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/evidence/mi300x-direct-kfd-runtime-profile-v1.json")
        .canonicalize()
        .unwrap();
    let fixture = Fixture::new(false);
    fixture.replace_behavior(
        "os.umask(0o077)\ntarget = args[args.index(\"--\") + 1:]\nraise SystemExit(subprocess.run(target, check=False).returncode)",
    );
    let output_directory = fixture.output("live-capture");
    let runtime = fixture.output("runtime.json");
    let options = ["--direct-kfd-runtime-capture", runtime.to_str().unwrap()];
    let arguments = [runtime_source.to_str().unwrap(), runtime.to_str().unwrap()];
    let plan = fixture.plan_program_with_options(
        &output_directory,
        &options,
        Path::new("/bin/cp"),
        &arguments,
    );
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    assert!(!runtime.exists());
    assert!(!output_directory.exists());

    let output = collect_program_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        Path::new("/bin/cp"),
        &arguments,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let durable_runtime =
        fs::read(output_directory.join("fe2o3-direct-kfd-runtime-profile-v1.json")).unwrap();
    assert_eq!(durable_runtime, fs::read(&runtime_source).unwrap());
    assert_eq!(durable_runtime, fs::read(&runtime).unwrap());
    let qualification: serde_json::Value = serde_json::from_slice(
        &fs::read(output_directory.join("fe2o3-direct-kfd-rocprof-qualification-v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        qualification["outcome"],
        "runtime_dispatch_observed_collector_completed_no_artifacts"
    );
    assert_eq!(qualification["runtime"]["dispatches_published"], 3);
    assert_eq!(qualification["runtime"]["dropped_events"], 0);
    assert_eq!(qualification["collector_inventory_complete"], true);
    assert_eq!(qualification["collector_artifacts"], serde_json::json!([]));
    assert_eq!(qualification["grants_collection_authority"], false);
    assert_eq!(qualification["grants_dispatch_authority"], false);
    assert_eq!(qualification["proves_universal_collector_inability"], false);
    let manifest =
        fs::read_to_string(output_directory.join("fe2o3-profile-manifest-v1.txt")).unwrap();
    assert!(manifest.contains("direct-kfd-rocprof-qualification: published"));
    assert!(
        manifest
            .contains("direct-kfd-runtime-capture-copy: fe2o3-direct-kfd-runtime-profile-v1.json")
    );

    let stale_fixture = Fixture::new(false);
    let stale_output = stale_fixture.output("stale-output");
    let stale_runtime = stale_fixture.output("stale-runtime.json");
    let stale_options = [
        "--direct-kfd-runtime-capture",
        stale_runtime.to_str().unwrap(),
    ];
    let stale_arguments = [
        runtime_source.to_str().unwrap(),
        stale_runtime.to_str().unwrap(),
    ];
    let stale_plan = stale_fixture.plan_program_with_options(
        &stale_output,
        &stale_options,
        Path::new("/bin/cp"),
        &stale_arguments,
    );
    assert!(stale_plan.status.success());
    fs::write(&stale_runtime, b"stale").unwrap();
    let stale_collection = collect_program_with_options(
        &stale_fixture,
        &stale_output,
        &authorization(&stale_plan),
        &stale_options,
        Path::new("/bin/cp"),
        &stale_arguments,
    );
    assert!(!stale_collection.status.success());
    assert!(!stale_output.exists());
}

#[test]
fn in_process_json_and_csv_import_publish_bundle_receipt_then_manifest() {
    let Some(agent) = gfx942_kfd_agent() else {
        return;
    };
    let node = agent["node_id"].as_u64().unwrap();
    for (kind, suffix, source) in [
        (
            "dispatch-json",
            "json",
            current_dispatch_json(agent.clone()),
        ),
        ("dispatch-csv", "csv", current_dispatch_csv(node)),
    ] {
        let fixture = Fixture::new(false);
        install_dispatch_behavior(&fixture, "dispatch.", &source, 1);
        let kir = exact_gfx942_kir(&fixture);
        let output_directory = fixture.output(&format!("capture-{suffix}"));
        let options = ["--kind", kind, "--kir-v7", kir.to_str().unwrap()];
        let plan = fixture.plan_with_options(&output_directory, &options, &[]);
        assert!(
            plan.status.success(),
            "{}",
            String::from_utf8_lossy(&plan.stderr)
        );
        let output = collect_with_options(
            &fixture,
            &output_directory,
            &authorization(&plan),
            &options,
            &[],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bundle_path = output_directory.join("fe2o3-semantic-profiler-bundle-v4.json");
        let receipt_path = output_directory.join("fe2o3-profile-dispatch-import-receipt-v1.json");
        let manifest_path = output_directory.join("fe2o3-profile-manifest-v1.txt");
        let bundle = decode_profiler_bundle_v4(&fs::read(&bundle_path).unwrap()).unwrap();
        assert_eq!(
            bundle.source_kind,
            if kind == "dispatch-json" {
                ProfilerSourceKindV4::Rocprofv3KernelDispatchJson
            } else {
                ProfilerSourceKindV4::Rocprofv3KernelDispatchCsv
            }
        );
        assert_eq!(bundle.source.origin, TruthOriginV1::Observed);
        assert_eq!(bundle.normalized_projection.origin, TruthOriginV1::Observed);
        assert_eq!(bundle.environment.origin, TruthOriginV1::Declared);
        assert_eq!(bundle.collector_tool.origin, TruthOriginV1::Declared);
        assert_eq!(
            bundle.collector_configuration.origin,
            TruthOriginV1::Declared
        );
        assert_eq!(bundle.devices.len(), 1);
        assert_eq!(
            bundle.devices[0].stable_identity.origin,
            TruthOriginV1::Declared
        );
        assert_eq!(
            bundle.devices[0].source_bound_origin,
            TruthOriginV1::Observed
        );
        assert!(
            bundle
                .unavailable
                .contains(&ProfilerUnavailableFactV4::SourceIrIsaCorrelation)
        );
        let capture = bundle.dispatch_capture.as_ref().unwrap();
        assert_eq!(capture.runs.len(), 1);
        assert_eq!(capture.devices.len(), 1);
        assert_eq!(capture.dispatches.len(), 1);
        let dispatch = &capture.dispatches[0];
        assert_eq!(dispatch.kernel_ir.origin, TruthOriginV1::Declared);
        assert_eq!(dispatch.artifact.origin, TruthOriginV1::Unavailable);
        assert_eq!(
            dispatch.artifact.unavailable_reason,
            Some(CaptureUnavailableReasonV1::NotProvided)
        );
        assert_eq!(dispatch.source_map.origin, TruthOriginV1::Unavailable);
        assert_eq!(
            dispatch.source_map.unavailable_reason,
            Some(CaptureUnavailableReasonV1::NotProvided)
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
        assert!(
            receipt["authority"]
                .as_object()
                .unwrap()
                .values()
                .all(|value| value == false)
        );
        assert_eq!(
            receipt["source_kind"],
            if kind == "dispatch-json" {
                "rocprofv3_kernel_dispatch_json"
            } else {
                "rocprofv3_kernel_dispatch_csv"
            }
        );
        assert_eq!(
            receipt["source_schema_dialect"],
            if kind == "dispatch-json" {
                "rocprofv3_json_installed1_1_97f5574"
            } else {
                "rocprofv3_csv_current22_column_stream_id"
            }
        );
        assert_eq!(receipt["run_count"], 1);
        assert_eq!(receipt["device_count"], 1);
        assert_eq!(receipt["dispatch_count"], 1);
        assert_eq!(receipt["devices"][0]["kfd_node"], node);
        assert_eq!(receipt["devices"][0]["family"], "gfx942");
        assert_eq!(receipt["devices"][0]["gfx_target_version"], 90_402);
        assert_eq!(receipt["devices"][0]["wave_width"], 64);
        assert_eq!(receipt["devices"][0]["exact_xnack_origin"], "unavailable");
        assert_eq!(
            receipt["devices"][0]["exact_xnack_unavailable_reason"],
            "not_represented"
        );
        assert_eq!(
            receipt["source_agent_mappings"].as_array().unwrap().len(),
            1
        );
        let mapping = &receipt["source_agent_mappings"][0];
        assert_eq!(mapping["process_index"], 0);
        assert_eq!(mapping["kfd_node"], node);
        if kind == "dispatch-json" {
            assert_eq!(mapping["source_process_id"], 100);
            assert_eq!(mapping["opaque_agent_handle"], 7001);
        } else {
            assert!(mapping["source_process_id"].is_null());
            assert_eq!(mapping["opaque_agent_handle"], node);
        }
        for identity in ["artifact", "source_map"] {
            assert_eq!(receipt[identity]["origin"], "unavailable");
            assert_eq!(receipt[identity]["reason"], "not_provided");
        }
        let manifest = fs::read_to_string(&manifest_path).unwrap();
        assert!(manifest.contains("dispatch-observation-origin: observed-rocprof-source"));
        assert!(manifest.contains("fe2o3-semantic-profiler-bundle-v4.json"));
        assert!(manifest.contains("fe2o3-profile-dispatch-import-receipt-v1.json"));
    }
}

#[test]
fn schema_source_ambiguity_is_not_resolved_by_kfd_compatibility() {
    let Some(agent) = gfx942_kfd_agent() else {
        return;
    };
    let fixture = Fixture::new(false);
    let mut source =
        serde_json::from_slice::<serde_json::Value>(&current_dispatch_json(agent)).unwrap();
    let valid_but_foreign = source.clone();
    source["rocprofiler-sdk-tool"][0]["agents"][0]["node_id"] = serde_json::json!(u32::MAX);
    source["rocprofiler-sdk-tool"][0]["agents"][0]["gpu_id"] = serde_json::json!(u64::MAX);
    let first = serde_json::to_vec(&source).unwrap();
    let second = serde_json::to_vec(&valid_but_foreign).unwrap();
    let first_literal = serde_json::to_string(&String::from_utf8(first).unwrap()).unwrap();
    let second_literal = serde_json::to_string(&String::from_utf8(second).unwrap()).unwrap();
    fixture.replace_behavior(&format!(
        "out = args[args.index(\"--output-directory\") + 1]\nos.makedirs(out, exist_ok=True)\nwith open(os.path.join(out, \"a.json\"), \"w\", encoding=\"utf-8\") as stream:\n    stream.write({first_literal})\nwith open(os.path.join(out, \"b.json\"), \"w\", encoding=\"utf-8\") as stream:\n    stream.write({second_literal})\ntarget = args[args.index(\"--\") + 1:]\nraise SystemExit(subprocess.run(target, check=False).returncode)\n"
    ));
    let kir = exact_gfx942_kir(&fixture);
    let output_directory = fixture.output("ambiguous");
    let options = ["--kind", "dispatch-json", "--kir-v7", kir.to_str().unwrap()];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    assert!(plan.status.success());
    let output = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(output.status.success());
    assert!(
        !output_directory
            .join("fe2o3-semantic-profiler-bundle-v4.json")
            .exists()
    );
    let manifest =
        fs::read_to_string(output_directory.join("fe2o3-profile-manifest-v1.txt")).unwrap();
    assert!(manifest.contains("multiple-schema-valid-dispatch-sources"));
}

#[test]
fn malformed_source_is_unavailable_and_generated_budget_failure_cleans() {
    let Some(agent) = gfx942_kfd_agent() else {
        return;
    };

    let malformed = Fixture::new(false);
    install_dispatch_behavior(&malformed, "malformed.", b"{}", 1);
    let kir = exact_gfx942_kir(&malformed);
    let output_directory = malformed.output("malformed");
    let options = ["--kind", "dispatch-json", "--kir-v7", kir.to_str().unwrap()];
    let plan = malformed.plan_with_options(&output_directory, &options, &[]);
    assert!(plan.status.success());
    let output = collect_with_options(
        &malformed,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(output.status.success());
    let manifest =
        fs::read_to_string(output_directory.join("fe2o3-profile-manifest-v1.txt")).unwrap();
    assert!(manifest.contains("dispatch-observation-reason: no-schema-valid-dispatch-source"));
    assert!(
        !output_directory
            .join("fe2o3-semantic-profiler-bundle-v4.json")
            .exists()
    );

    let budget = Fixture::new(false);
    let source = current_dispatch_json(agent);
    install_dispatch_behavior(&budget, "dispatch.", &source, 1);
    let kir = exact_gfx942_kir(&budget);
    let output_directory = budget.output("budget");
    let options = [
        "--kind",
        "dispatch-json",
        "--kir-v7",
        kir.to_str().unwrap(),
        "--storage-limit",
        "1024",
    ];
    let plan = budget.plan_with_options(&output_directory, &options, &[]);
    assert!(plan.status.success());
    let output = collect_with_options(
        &budget,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("storage limit"));
    assert!(!output_directory.exists());
}

#[test]
fn reserved_generated_names_are_rejected_and_cleaned() {
    for name in [
        "fe2o3-semantic-profiler-bundle-v4.json",
        ".fe2o3-semantic-profiler-bundle-v4.redo",
        "fe2o3-profile-dispatch-import-receipt-v1.json",
        ".fe2o3-profile-dispatch-import-receipt-v1.redo",
        "fe2o3-profile-manifest-v1.txt",
        ".fe2o3-profile-manifest-v1.redo",
        "fe2o3-direct-kfd-rocprof-qualification-v1.json",
        ".fe2o3-direct-kfd-rocprof-qualification-v1.redo",
        "fe2o3-direct-kfd-runtime-profile-v1.json",
        ".fe2o3-direct-kfd-runtime-profile-v1.redo",
        "fe2o3-rocprof-wrapper-host-wall-comparison-v1.json",
        ".fe2o3-rocprof-wrapper-host-wall-comparison-v1.redo",
    ] {
        let fixture = Fixture::new(false);
        fixture.replace_behavior(&format!(
            "out = args[args.index(\"--output-directory\") + 1]\nwith open(os.path.join(out, {name:?}), \"w\", encoding=\"utf-8\") as stream:\n    stream.write(\"reserved\")\n"
        ));
        let output_directory = fixture.output("reserved");
        let plan = fixture.plan(&output_directory, &[]);
        assert!(plan.status.success());
        let output = collect(&fixture, &output_directory, &authorization(&plan), &[]);
        assert!(!output.status.success());
        assert!(!output_directory.exists());
    }
}

#[test]
fn wrapper_host_wall_comparison_requires_separate_acknowledgement_and_is_non_authoritative() {
    let fixture = Fixture::new(false);
    fixture.replace_behavior(
        r#"
target = args[args.index("--") + 1:]
raise SystemExit(subprocess.run(target, check=False).returncode)
"#,
    );
    let output_directory = fixture.output("wrapper-overhead");
    let options = [
        "--measure-direct-kfd-wrapper-overhead",
        "--overhead-warmup-pairs",
        "1",
        "--overhead-measured-pairs",
        "3",
        "--overhead-candidate-budget-bps",
        "1000000",
    ];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let acknowledgement = field(&plan, "repeated-target-execution-acknowledgement");

    let without_ack = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(!without_ack.status.success());
    assert!(!output_directory.exists());

    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command.args([
        "profile",
        "--collect",
        "--authorize-collection",
        &authorization(&plan),
        "--acknowledge-repeated-target-execution",
        &acknowledgement,
        "--tool",
        fixture.tool.to_str().unwrap(),
    ]);
    command.args(options).args([
        "--output-dir",
        output_directory.to_str().unwrap(),
        "--",
        "/bin/true",
    ]);
    let collected = command.output().unwrap();
    assert!(
        collected.status.success(),
        "{}",
        String::from_utf8_lossy(&collected.stderr)
    );
    let evidence: serde_json::Value = serde_json::from_slice(
        &fs::read(output_directory.join("fe2o3-rocprof-wrapper-host-wall-comparison-v1.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        evidence["schema"],
        "fe2o3-rocprof-wrapper-host-wall-comparison-v1"
    );
    assert_eq!(evidence["invocations"].as_array().unwrap().len(), 8);
    assert!(!evidence["summary"].is_null());
    assert_eq!(
        evidence["kernel_trace_capture_overhead"],
        "unavailable_no_admitted_capture"
    );
    assert_eq!(evidence["grants_collection_authority"], false);
    assert_eq!(evidence["grants_production_qualification"], false);
    assert!(
        output_directory
            .join("fe2o3-profile-manifest-v1.txt")
            .is_file()
    );
}

#[test]
fn legacy_kir_declaration_never_emits_an_import_recipe() {
    let fixture = Fixture::new(false);
    let output = fixture.plan_with_options(
        &fixture.output("capture"),
        &[
            "--kir-sha256",
            &"11".repeat(32),
            "--kir-len",
            "1",
            "--wave-width",
            "64",
        ],
        &[],
    );
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(
        "next-import-status: unavailable-legacy-kir-declaration-is-not-admitted-canonical-kir"
    ));
    assert!(!stdout.contains("next-import-program:"));
    assert!(!stdout.contains("next-import-arg["));
}

#[test]
fn legacy_wave_declaration_cannot_bypass_canonical_kir_admission() {
    let fixture = Fixture::new(false);
    let output = fixture.plan_with_options(
        &fixture.output("capture"),
        &[
            "--kir-sha256",
            &"11".repeat(32),
            "--kir-len",
            "1",
            "--wave-width",
            "32",
        ],
        &[],
    );
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("next-import-program:"));
    assert!(!stdout.contains("next-import-arg["));
    assert!(!stdout.contains("ready-after-collector-artifact"));
    assert!(stdout.contains(
        "next-import-status: unavailable-legacy-kir-declaration-is-not-admitted-canonical-kir"
    ));
}

#[test]
fn att_plan_is_unavailable_without_a_mutation_proof_sealed_decoder_route() {
    let fixture = Fixture::new(false);
    let output = fixture.plan_with_options(&fixture.output("capture"), &["--kind", "att"], &[]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("collection-readiness: unavailable"));
    assert!(stdout.contains(
        "next-import-status: unavailable-att-decoder-has-no-mutation-proof-sealed-directory-route"
    ));
    assert!(!stdout.contains("next-import-program:"));
    assert!(!stdout.contains("next-import-deferred-flag:"));
}

#[test]
fn authorization_is_bound_to_exact_target_argv_and_output_path() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let plan = fixture.plan(&output_directory, &["first"]);
    assert!(plan.status.success());
    let output = collect(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &["second"],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("does not match this exact plan")
    );
    assert!(!output_directory.exists());
}

#[test]
fn semantic_configuration_excludes_output_routing_but_authorization_binds_it() {
    let fixture = Fixture::new(false);
    let first = fixture.plan(&fixture.output("first"), &[]);
    let second = fixture.plan(&fixture.output("second"), &[]);
    assert!(first.status.success() && second.status.success());
    assert_eq!(
        field(&first, "configuration-identity"),
        field(&second, "configuration-identity")
    );
    assert_ne!(authorization(&first), authorization(&second));
}

#[test]
fn collector_failure_cleans_only_the_owned_new_directory() {
    let fixture = Fixture::new(true);
    let output_directory = fixture.output("capture");
    let plan = fixture.plan(&output_directory, &[]);
    let output = collect(&fixture, &output_directory, &authorization(&plan), &[]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("output-cleanup: complete")
    );
    assert!(!output_directory.exists());
    assert!(fixture.root.exists());
}

#[test]
fn timeout_kills_the_collector_process_group_and_cleans_output() {
    let fixture = Fixture::new(false);
    let pid_file = fixture.output("descendant-pid");
    let release_file = fixture.output("descendant-release");
    fixture.replace_behavior(&format!(
        r#"
def identity(pid):
    stat = open(f"/proc/{{pid}}/stat", encoding="utf-8").read().rsplit(") ", 1)[1].split()
    return f"{{pid}} {{stat[19]}} {{stat[2]}} {{stat[3]}}"
child = subprocess.Popen(["/bin/sleep", "30"])
temporary = {:?} + ".tmp"
with open(temporary, "w", encoding="utf-8") as stream:
    stream.write(identity(os.getpid()) + " " + identity(child.pid))
os.replace(temporary, {:?})
deadline = time.monotonic() + 3
while not os.path.exists({:?}) and time.monotonic() < deadline:
    time.sleep(0.005)
if not os.path.exists({:?}):
    raise SystemExit(98)
time.sleep(30)
"#,
        pid_file, pid_file, release_file, release_file
    ));
    let output_directory = fixture.output("capture");
    let options = ["--timeout-ms", "5000"];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    assert!(plan.status.success());
    let monitor =
        monitor_test_process_publication(pid_file.clone(), release_file, current_test_session());
    let output = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    let descendant = join_test_process_monitor(monitor);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("outcome: timeout")
    );
    assert!(!output_directory.exists());
    assert_test_descendant_is_terminated(&descendant);
}

#[test]
fn successful_collector_exit_kills_descendants_and_completes_publication() {
    let fixture = Fixture::new(false);
    let pid_file = fixture.output("normal-exit-descendant-pid");
    let release_file = fixture.output("normal-exit-descendant-release");
    fixture.replace_behavior(&format!(
        r#"
def identity(pid):
    stat = open(f"/proc/{{pid}}/stat", encoding="utf-8").read().rsplit(") ", 1)[1].split()
    return f"{{pid}} {{stat[19]}} {{stat[2]}} {{stat[3]}}"
child = subprocess.Popen(["/bin/sleep", "30"])
temporary = {:?} + ".tmp"
with open(temporary, "w", encoding="utf-8") as stream:
    stream.write(identity(os.getpid()) + " " + identity(child.pid))
os.replace(temporary, {:?})
deadline = time.monotonic() + 3
while not os.path.exists({:?}) and time.monotonic() < deadline:
    time.sleep(0.005)
if not os.path.exists({:?}):
    raise SystemExit(98)
out = args[args.index("--output-directory") + 1]
with open(os.path.join(out, "capture_results.json"), "w", encoding="utf-8") as stream:
    stream.write("{{}}")
raise SystemExit(0)
"#,
        pid_file, pid_file, release_file, release_file
    ));
    let output_directory = fixture.output("normal-exit-capture");
    let plan = fixture.plan(&output_directory, &[]);
    assert!(plan.status.success());
    let monitor =
        monitor_test_process_publication(pid_file.clone(), release_file, current_test_session());
    let output = collect(&fixture, &output_directory, &authorization(&plan), &[]);
    let descendant = join_test_process_monitor(monitor);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_directory
            .join("fe2o3-profile-manifest-v1.txt")
            .is_file()
    );
    assert_test_descendant_is_terminated(&descendant);
}

#[test]
fn installed_collector_executes_sealed_entry_images_without_role_env_leakage() {
    let tool = Path::new("/opt/rocm-7.2.4/bin/rocprofv3");
    let python = Path::new("/usr/bin/python3.12");
    if !tool.is_file() || !python.is_file() {
        assert!(
            !require_gfx942_profile_test(),
            "required MI300X profile test lacks the reviewed ROCm 7.2.4 collector or Python 3.12"
        );
        return;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "cargo-fe2o3-profile-sealed-installed-{}-{id}",
        process::id()
    ));
    fs::create_dir(&root).unwrap();
    let output = root.join("capture");
    let evidence = root.join("target-evidence.txt");
    let code = format!(
        "import os; open({:?},'w').write('role='+str(any(k.startswith(\"FE2O3_ROCPROF_\") for k in os.environ))+'\\n'+open('/proc/self/maps').read())",
        evidence
    );
    let invoke = |authorization: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command.args([
            "profile",
            "--kind",
            "dispatch-json",
            "--tool",
            tool.to_str().unwrap(),
            "--output-dir",
            output.to_str().unwrap(),
        ]);
        if let Some(authorization) = authorization {
            command.args(["--collect", "--authorize-collection", authorization]);
        }
        command
            .args(["--", python.to_str().unwrap(), "-c", &code])
            .output()
            .unwrap()
    };
    let plan = invoke(None);
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    assert_eq!(
        field(&plan, "collector-execution-mode"),
        "sealed-installed-adapter-v1"
    );
    let authorization = authorization(&plan);
    let collected = invoke(Some(&authorization));
    assert!(
        collected.status.success(),
        "{}",
        String::from_utf8_lossy(&collected.stderr)
    );
    let evidence = fs::read_to_string(&evidence).unwrap();
    assert!(evidence.starts_with("role=False\n"));
    assert!(!evidence.contains("/opt/rocm-7.2.4/lib/librocprofiler-sdk.so"));
    assert!(!evidence.contains("/opt/rocm-7.2.4/lib/rocprofiler-sdk/librocprofiler-sdk-tool.so"));
    let image_inodes = evidence
        .lines()
        .filter(|line| line.contains("/memfd:fe2o3-profile-execution-v1 (deleted)"))
        .filter_map(|line| line.split_whitespace().nth(4))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        image_inodes.len() >= 3,
        "expected sealed target, SDK core, and SDK tool mappings"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn output_and_storage_overflow_are_bounded_and_cleaned() {
    let fixture = Fixture::new(false);
    fixture.replace_behavior(
        r#"
sys.stdout.write("x" * 8192)
sys.stdout.flush()
time.sleep(30)
"#,
    );
    let output_directory = fixture.output("stdout-overflow");
    let options = ["--stdout-limit", "64", "--timeout-ms", "2000"];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    let output = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("outcome: output-overflow")
    );
    assert!(!output_directory.exists());

    fixture.replace_behavior(
        r#"
out = args[args.index("--output-directory") + 1]
with open(os.path.join(out, "too-large.json"), "wb") as stream:
    stream.write(b"x" * 1024)
"#,
    );
    let output_directory = fixture.output("storage-overflow");
    let options = ["--storage-limit", "128"];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    let output = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("storage limit")
    );
    assert!(!output_directory.exists());
}

#[test]
fn existing_symlink_output_and_symlink_tool_are_rejected() {
    let fixture = Fixture::new(false);
    let destination = fixture.output("destination");
    fs::create_dir(&destination).unwrap();
    let output_link = fixture.output("output-link");
    symlink(&destination, &output_link).unwrap();
    assert!(!fixture.plan(&output_link, &[]).status.success());

    let linked_directory = fixture.root.join("linked");
    fs::create_dir(&linked_directory).unwrap();
    let tool_link = linked_directory.join("rocprofv3");
    symlink(&fixture.tool, &tool_link).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "profile",
            "--tool",
            tool_link.to_str().unwrap(),
            "--output-dir",
            fixture.output("capture").to_str().unwrap(),
            "--",
            "/bin/true",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn canonical_output_through_non_utf8_parent_is_rejected() {
    let fixture = Fixture::new(false);
    let non_utf8_parent = fixture
        .root
        .join(OsString::from_vec(b"non-utf8-\xff".to_vec()));
    fs::create_dir(&non_utf8_parent).unwrap();
    let utf8_alias = fixture.output("utf8-alias");
    symlink(&non_utf8_parent, &utf8_alias).unwrap();
    let requested = utf8_alias.join("capture");

    let output = fixture.plan(&requested, &[]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("canonical --output-dir must be valid UTF-8")
    );
    assert!(!non_utf8_parent.join("capture").exists());
}

#[test]
fn duplicates_and_bounds_fail_before_any_output_creation() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let prefixes = [
        vec!["--kind", "att", "--kind", "dispatch-json"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        vec!["--timeout-ms", "0"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        vec!["--storage-limit", "4294967297"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        vec!["--kir-sha256".to_owned(), "0".repeat(64)],
        vec![
            "--timeout-ms".to_owned(),
            "1".to_owned(),
            "--timeout-ms=2".to_owned(),
        ],
        vec![
            "--stdout-limit=1".to_owned(),
            "--stdout-limit".to_owned(),
            "2".to_owned(),
        ],
        vec![
            "--stderr-limit".to_owned(),
            "1".to_owned(),
            "--stderr-limit=2".to_owned(),
        ],
        vec![
            "--storage-limit=1".to_owned(),
            "--storage-limit=2".to_owned(),
        ],
        vec![
            format!("--tool={}", fixture.tool.display()),
            "--tool".to_owned(),
            fixture.tool.to_str().unwrap().to_owned(),
        ],
    ];
    for prefix in prefixes {
        let mut arguments = vec!["profile".to_owned()];
        arguments.extend(prefix);
        arguments.extend(
            [
                "--tool",
                fixture.tool.to_str().unwrap(),
                "--output-dir",
                output_directory.to_str().unwrap(),
                "--",
                "/bin/true",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output_directory.exists());
    }
}
