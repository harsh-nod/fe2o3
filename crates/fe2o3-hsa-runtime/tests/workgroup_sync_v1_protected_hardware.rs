#![cfg(target_os = "linux")]

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffV2;
use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    GeneratedWorkgroupLdsReductionV1HostAdapterV1, GeneratedWorkgroupScopedAtomicV1HostAdapterV1,
    ObservedContext, join_workgroup_lds_reduction_v1, join_workgroup_scoped_atomic_v1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, PreparedFinalizedWorkgroupSyncHsacoV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkgroupSyncCompilerPinsV1, WorkgroupSyncDirectWorkerExpectationV1,
    WorkgroupSyncDirectWorkerPinsV1, WorkgroupSyncProfileKindV1,
    construct_inert_workgroup_sync_v1_compiler_handoff_v1,
    execute_reproducible_first_build_worker_v2, finalize_workgroup_sync_v1_worker_v2_hsaco_v1,
    inspect_workgroup_sync_v1_worker_v2_hsaco_v1,
};

const WORKER_ENV: &str = "FE2O3_WORKGROUP_SYNC_V1_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_WORKGROUP_SYNC_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_WORKGROUP_SYNC_V1_LLVM_BUILD_ID";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(profile: WorkgroupSyncProfileKindV1) -> Result<Self, Box<dyn std::error::Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-protected-workgroup-sync-{profile:?}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn required_authority_pin(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(name).map_err(|_| {
        format!(
            "missing production workgroup-sync authority pin {name}; refusing HSA load/dispatch"
        )
        .into()
    })
}

fn profile_byte(profile: WorkgroupSyncProfileKindV1) -> u8 {
    match profile {
        WorkgroupSyncProfileKindV1::LdsReduction => 0x1d,
        WorkgroupSyncProfileKindV1::ScopedAtomic => 0xa7,
    }
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
    profile: WorkgroupSyncProfileKindV1,
) -> Result<ConsumedCompilerModuleHandoffV1, Box<dyn std::error::Error>> {
    let producer = ProducerIdentity::from_codegen(
        &format!("protected_workgroup_sync_v1_{profile:?}"),
        Some(Path::new("tests/workgroup_sync_v1_protected_hardware.rs")),
    )?;
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([profile_byte(profile); 32]),
        BuildSession::from_bytes([0x94; 16]),
    )?;
    publish_compiler_module_handoff_v1(
        &directory.0,
        &producer,
        attempt,
        handoff.canonical_bytes(),
    )?;
    Ok(consume_compiler_module_handoff_v1(
        &directory.0,
        &producer,
        attempt,
    )?)
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed direct-link option"))
    .collect()
}

fn produce_receipt(
    profile: WorkgroupSyncProfileKindV1,
) -> Result<PreparedFinalizedWorkgroupSyncHsacoV1, Box<dyn std::error::Error>> {
    let worker_path = PathBuf::from(required_authority_pin(WORKER_ENV)?);
    let worker_bytes = fs::read(&worker_path)?;
    let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
    let worker_build_identity = required_authority_pin(WORKER_BUILD_ID_ENV)?;
    let llvm_build_identity = required_authority_pin(LLVM_BUILD_ID_ENV)?;
    let measurement = WorkerMeasurementV1::new(
        worker_identity,
        worker_build_identity.clone(),
        llvm_build_identity.clone(),
    )?;
    let worker = PinnedWorkerV1::open(&worker_path, measurement)?;
    let compiler_pins = match profile {
        WorkgroupSyncProfileKindV1::LdsReduction => {
            WorkgroupSyncCompilerPinsV1::exact_lds_reduction_v1()
        }
        WorkgroupSyncProfileKindV1::ScopedAtomic => {
            WorkgroupSyncCompilerPinsV1::exact_scoped_atomic_v1()
        }
    };
    let handoff = construct_inert_workgroup_sync_v1_compiler_handoff_v1(compiler_pins)?;
    let direct_worker_pins = WorkgroupSyncDirectWorkerPinsV1::new(
        worker_identity,
        &worker_build_identity,
        &llvm_build_identity,
    )?;
    let expectation = WorkgroupSyncDirectWorkerExpectationV1::from_pinned_handoff(
        &handoff,
        *handoff.identity().sha256(),
        compiler_pins,
        direct_worker_pins,
    )?;
    let directory = TestDirectory::new(profile)?;
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed_handoff(&directory, &handoff, profile)?,
        &worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64)?,
        WorkerExecutionLimitsV1::default(),
    )?;
    let inspected = inspect_workgroup_sync_v1_worker_v2_hsaco_v1(evidence, expectation)?;
    let finalized = finalize_workgroup_sync_v1_worker_v2_hsaco_v1(inspected)?;
    if finalized.profile() != profile
        || finalized.target().to_string() != "gfx942:xnack-"
        || finalized.grants_publication_authority()
        || finalized.grants_load_authority()
        || finalized.grants_launch_authority()
    {
        return Err("production workgroup-sync finalizer receipt identity drifted".into());
    }
    Ok(finalized)
}

#[test]
#[ignore = "requires measured direct LLVM/LLD worker pins and gfx942:xnack-"]
fn protected_gfx942_lds_reduction_exact_nominal_vector() -> Result<(), Box<dyn std::error::Error>> {
    let receipt = produce_receipt(WorkgroupSyncProfileKindV1::LdsReduction)?;
    let context = GpuContext::new(0)?;
    let observed = ObservedContext::observe(&context)?;
    let stream = context.default_stream();
    let values_host = std::array::from_fn::<_, 66, _>(|index| {
        if index == 0 || index == 65 {
            0x1234_5678
        } else {
            index as i32 - 17
        }
    });
    let output_host = [0x1357_2468_i32, -1, 0x2468_1357];
    let values = DeviceBuffer::from_host(&stream, &values_host)?;
    let mut output = DeviceBuffer::from_host(&stream, &output_host)?;
    let host = GeneratedWorkgroupLdsReductionV1HostAdapterV1::prepare(
        &observed,
        values.view(1..65)?,
        output.view_mut(1..2)?,
    )?;
    let joined = join_workgroup_lds_reduction_v1(receipt, host)?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    if adapter.completion_timeout_v1() != std::time::Duration::from_secs(5) {
        return Err("protected workgroup-sync completion timeout drifted".into());
    }
    let completed = joined.load(adapter)?.dispatch_and_wait()?;
    let actual_values = values.to_host_vec(&stream)?;
    let actual_output = output.to_host_vec(&stream)?;
    let expected = values_host[1..65].iter().copied().sum::<i32>();
    if actual_values != values_host || actual_output != [output_host[0], expected, output_host[2]] {
        return Err("protected LDS reduction oracle or canary mismatch".into());
    }
    let unloaded = completed.unload();
    if unloaded
        .unload_identity()
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err("protected LDS reduction terminal unload marker is absent".into());
    }
    Ok(())
}

#[test]
#[ignore = "requires measured direct LLVM/LLD worker pins and gfx942:xnack-"]
fn protected_gfx942_scoped_atomic_exact_nominal_vector() -> Result<(), Box<dyn std::error::Error>> {
    let receipt = produce_receipt(WorkgroupSyncProfileKindV1::ScopedAtomic)?;
    let context = GpuContext::new(0)?;
    let observed = ObservedContext::observe(&context)?;
    let stream = context.default_stream();
    let values_host = std::array::from_fn::<_, 66, _>(|index| {
        if index == 0 || index == 65 {
            0xfeed_beef
        } else {
            index as u32
        }
    });
    let eligible_host = std::array::from_fn::<_, 66, _>(|index| {
        if index == 0 || index == 65 {
            0xabcd_1234
        } else {
            u32::from(index % 3 != 0)
        }
    });
    let target_host = [0x1357_2468_u32, 11, 0x2468_1357];
    let values = DeviceBuffer::from_host(&stream, &values_host)?;
    let eligible = DeviceBuffer::from_host(&stream, &eligible_host)?;
    let mut target = DeviceBuffer::from_host(&stream, &target_host)?;
    let host = GeneratedWorkgroupScopedAtomicV1HostAdapterV1::prepare(
        &observed,
        values.view(1..65)?,
        eligible.view(1..65)?,
        target.view_mut(1..2)?,
    )?;
    let joined = join_workgroup_scoped_atomic_v1(receipt, host)?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    let completed = joined.load(adapter)?.dispatch_and_wait()?;
    let actual_values = values.to_host_vec(&stream)?;
    let actual_eligible = eligible.to_host_vec(&stream)?;
    let actual_target = target.to_host_vec(&stream)?;
    let added = values_host[1..65]
        .iter()
        .zip(&eligible_host[1..65])
        .filter_map(|(value, eligible)| (*eligible != 0).then_some(*value))
        .sum::<u32>();
    if actual_values != values_host
        || actual_eligible != eligible_host
        || actual_target != [target_host[0], target_host[1] + added, target_host[2]]
    {
        return Err("protected scoped-atomic oracle, input, or canary mismatch".into());
    }
    let unloaded = completed.unload();
    if unloaded
        .unload_identity()
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err("protected scoped-atomic terminal unload marker is absent".into());
    }
    Ok(())
}
