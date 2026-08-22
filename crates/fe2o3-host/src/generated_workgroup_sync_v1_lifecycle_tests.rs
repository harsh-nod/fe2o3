use crate::{
    HsaAgentIdentityV1, HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1,
    HsaEnvironmentObservationV1, HsaExecutableObjectIdentityV1, HsaKernelObjectIdentityV1,
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, HsaPhysicalDeviceIdentityV1,
    HsaRuntimeIdentityV1, HsaUnloadObservationV1, ReviewedHsaExecutableLifecycleAdapterV1,
    generated_workgroup_sync_v1_lifecycle::{
        ExactWorkgroupSyncHostProfileV1, ReviewedWorkgroupSyncRuntimeAdapterV1,
        WorkgroupLdsReductionProfileV1, WorkgroupScopedAtomicProfileV1,
        WorkgroupSyncDispatchErrorV1, WorkgroupSyncImplicitKernargObservationV1,
        WorkgroupSyncKernelResourceObservationV1, WorkgroupSyncLoadErrorV1,
        test_support::{
            load_test_lifecycle_v1, test_complete_bytes_v1, test_explicit_bytes_v1,
            test_hidden_lds_offset_v1, test_implicit_bytes_v1, test_runtime_alignment_v1,
        },
    },
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_core::ContextIdentity;
use fe2o3_hsaco_finalize::WorkgroupSyncProfileKindV1;
use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

const RUNTIME_INSTANCE: [u8; 16] = [0x31; 16];
const DEVICE_UUID: [u8; 16] = [0x42; 16];
const AGENT_HANDLE: u64 = 0x5354;
const EXECUTABLE_OBJECT: [u8; 32] = [0x65; 32];
const KERNEL_OBJECT: [u8; 32] = [0x76; 32];
const DISPATCH_IDENTITY: [u8; 16] = [0x87; 16];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    None,
    EnvironmentTarget,
    EnvironmentOrdinal,
    LoadDigest,
    LoadRuntime,
    LoadAgent,
    ResolutionSymbol,
    ResolutionSize,
    ResolutionAlignment,
    ResourceProfile,
    ResourceExecutable,
    ResourceKernel,
    ResourceGroup,
    ResourcePrivate,
    ImplicitProfile,
    ImplicitGeometry,
    ImplicitSpan,
    ImplicitHiddenOffset,
    ImplicitHiddenValue,
    ImplicitAqlGroup,
    ImplicitIncomplete,
    ExplicitMutation,
    HiddenMutation,
    DispatchGeometry,
    DispatchIncomplete,
}

#[derive(Debug)]
struct FakeError(&'static str);
impl fmt::Display for FakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}
impl Error for FakeError {}

#[derive(Debug)]
struct FakeExecutable {
    object: HsaExecutableObjectIdentityV1,
}

#[derive(Debug)]
struct FakeKernel {
    object: HsaKernelObjectIdentityV1,
    events: Arc<Mutex<Vec<&'static str>>>,
}
impl Drop for FakeKernel {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("kernel_drop");
    }
}

#[derive(Clone)]
struct FakeAdapter {
    fault: Fault,
    events: Arc<Mutex<Vec<&'static str>>>,
    unloads: Arc<AtomicUsize>,
    explicit_preserved: Arc<AtomicBool>,
    observed_dynamic_lds: Arc<AtomicUsize>,
}

struct FakeState {
    adapter: FakeAdapter,
    events: Arc<Mutex<Vec<&'static str>>>,
    unloads: Arc<AtomicUsize>,
    explicit_preserved: Arc<AtomicBool>,
    observed_dynamic_lds: Arc<AtomicUsize>,
}

fn fake(fault: Fault) -> FakeState {
    let events = Arc::new(Mutex::new(Vec::new()));
    let unloads = Arc::new(AtomicUsize::new(0));
    let explicit_preserved = Arc::new(AtomicBool::new(false));
    let observed_dynamic_lds = Arc::new(AtomicUsize::new(usize::MAX));
    FakeState {
        adapter: FakeAdapter {
            fault,
            events: events.clone(),
            unloads: unloads.clone(),
            explicit_preserved: explicit_preserved.clone(),
            observed_dynamic_lds: observed_dynamic_lds.clone(),
        },
        events,
        unloads,
        explicit_preserved,
        observed_dynamic_lds,
    }
}

fn executable_object() -> HsaExecutableObjectIdentityV1 {
    HsaExecutableObjectIdentityV1::new(EXECUTABLE_OBJECT).unwrap()
}
fn kernel_object() -> HsaKernelObjectIdentityV1 {
    HsaKernelObjectIdentityV1::new(KERNEL_OBJECT).unwrap()
}

fn environment(target: &str, ordinal: i32) -> HsaEnvironmentObservationV1 {
    let target = AmdTargetId::parse(target).unwrap();
    let runtime = HsaRuntimeIdentityV1::new(
        "fake-hsa",
        "1.0",
        DigestAlgorithm::Sha256.calculate(b"fake-runtime"),
        RUNTIME_INSTANCE,
    )
    .unwrap();
    let physical = HsaPhysicalDeviceIdentityV1::new(DEVICE_UUID, 7, ordinal, target).unwrap();
    let agent =
        HsaAgentIdentityV1::new(RUNTIME_INSTANCE, AGENT_HANDLE, DEVICE_UUID, target).unwrap();
    HsaEnvironmentObservationV1::new(runtime, physical, agent).unwrap()
}

fn other_profile(profile: WorkgroupSyncProfileKindV1) -> WorkgroupSyncProfileKindV1 {
    match profile {
        WorkgroupSyncProfileKindV1::LdsReduction => WorkgroupSyncProfileKindV1::ScopedAtomic,
        WorkgroupSyncProfileKindV1::ScopedAtomic => WorkgroupSyncProfileKindV1::LdsReduction,
    }
}

unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for FakeAdapter {
    type Executable = FakeExecutable;
    type Kernel = FakeKernel;
    type Error = FakeError;

    unsafe fn observe_environment(&mut self) -> Result<HsaEnvironmentObservationV1, Self::Error> {
        self.events.lock().unwrap().push("environment");
        Ok(match self.fault {
            Fault::EnvironmentTarget => environment("gfx1100", 0),
            Fault::EnvironmentOrdinal => environment("gfx942:xnack-", 1),
            _ => environment("gfx942:xnack-", 0),
        })
    }

    unsafe fn load_executable(
        &mut self,
        bytes: &[u8],
        finalized_digest: fe2o3_artifacts::PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
        self.events.lock().unwrap().push("load");
        let digest = if self.fault == Fault::LoadDigest {
            DigestAlgorithm::Sha256.calculate(b"substituted")
        } else {
            finalized_digest
        };
        let runtime = if self.fault == Fault::LoadRuntime {
            [0x99; 16]
        } else {
            RUNTIME_INSTANCE
        };
        let agent = if self.fault == Fault::LoadAgent {
            AGENT_HANDLE + 1
        } else {
            AGENT_HANDLE
        };
        let object = executable_object();
        Ok((
            FakeExecutable { object },
            HsaCodeObjectLoadObservationV1::new(digest, bytes.len() as u64, runtime, agent, object),
        ))
    }

    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
        self.events.lock().unwrap().push("resolve");
        let symbol = if self.fault == Fault::ResolutionSymbol {
            "substituted"
        } else {
            export_symbol
        };
        let profile = if export_symbol == WorkgroupLdsReductionProfileV1::EXPORT_SYMBOL {
            WorkgroupSyncProfileKindV1::LdsReduction
        } else {
            WorkgroupSyncProfileKindV1::ScopedAtomic
        };
        let complete_bytes = test_complete_bytes_v1(profile) as u64;
        let size = if self.fault == Fault::ResolutionSize {
            complete_bytes - 8
        } else {
            complete_bytes
        };
        let alignment = if self.fault == Fault::ResolutionAlignment {
            8
        } else {
            test_runtime_alignment_v1()
        };
        let object = kernel_object();
        Ok((
            FakeKernel {
                object,
                events: self.events.clone(),
            },
            HsaKernelResolutionObservationV1::new(
                executable.object,
                object,
                symbol,
                size,
                alignment,
            )
            .unwrap(),
        ))
    }

    unsafe fn launch_and_wait(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        _kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        self.events.lock().unwrap().push("dispatch");
        self.observed_dynamic_lds.store(
            geometry.dynamic_shared_memory_bytes() as usize,
            Ordering::SeqCst,
        );
        let geometry = if self.fault == Fault::DispatchGeometry {
            HsaLaunchGeometryV1::new(
                [2, 1, 1],
                [64, 1, 1],
                geometry.dynamic_shared_memory_bytes(),
            )
        } else {
            geometry
        };
        HsaDispatchObservationV1::new(
            DISPATCH_IDENTITY,
            executable.object,
            kernel.object,
            geometry,
            self.fault != Fault::DispatchIncomplete,
        )
        .map_err(|_| FakeError("dispatch observation"))
    }

    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error> {
        self.events.lock().unwrap().push("unload");
        self.unloads.fetch_add(1, Ordering::SeqCst);
        Ok(HsaUnloadObservationV1::new(
            executable.object,
            RUNTIME_INSTANCE,
            AGENT_HANDLE,
            true,
        ))
    }
}

unsafe impl ReviewedWorkgroupSyncRuntimeAdapterV1 for FakeAdapter {
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity {
        unreachable!("test support performs the context result before native calls")
    }

    unsafe fn initialize_workgroup_sync_implicit_kernarg_v1(
        &mut self,
        profile: WorkgroupSyncProfileKindV1,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<WorkgroupSyncImplicitKernargObservationV1, Self::Error> {
        self.events.lock().unwrap().push("implicit");
        let explicit = kernarg[..explicit_byte_len].to_vec();
        kernarg[implicit_byte_offset..implicit_byte_offset + implicit_byte_len].fill(0xa5);
        if profile == WorkgroupSyncProfileKindV1::LdsReduction {
            kernarg[test_hidden_lds_offset_v1()..test_hidden_lds_offset_v1() + 4]
                .copy_from_slice(&256_u32.to_le_bytes());
        } else {
            kernarg[test_hidden_lds_offset_v1()..test_hidden_lds_offset_v1() + 4].fill(0);
        }
        if self.fault == Fault::ExplicitMutation {
            kernarg[0] ^= 1;
        }
        if self.fault == Fault::HiddenMutation {
            kernarg[test_hidden_lds_offset_v1()..test_hidden_lds_offset_v1() + 4]
                .copy_from_slice(&255_u32.to_le_bytes());
        }
        self.explicit_preserved
            .store(kernarg[..explicit_byte_len] == explicit, Ordering::SeqCst);
        let observed_profile = if self.fault == Fault::ImplicitProfile {
            other_profile(profile)
        } else {
            profile
        };
        let observed_geometry = if self.fault == Fault::ImplicitGeometry {
            HsaLaunchGeometryV1::new(
                [2, 1, 1],
                [64, 1, 1],
                geometry.dynamic_shared_memory_bytes(),
            )
        } else {
            geometry
        };
        let hidden_offset = if profile == WorkgroupSyncProfileKindV1::LdsReduction {
            Some(test_hidden_lds_offset_v1() as u64)
        } else {
            None
        };
        Ok(WorkgroupSyncImplicitKernargObservationV1::new(
            observed_profile,
            executable.object,
            kernel.object,
            observed_geometry,
            if self.fault == Fault::ImplicitSpan {
                explicit_byte_len as u64 - 1
            } else {
                explicit_byte_len as u64
            },
            implicit_byte_offset as u64,
            implicit_byte_len as u64,
            if self.fault == Fault::ImplicitHiddenOffset {
                hidden_offset.map(|offset| offset + 4).or(Some(160))
            } else {
                hidden_offset
            },
            if self.fault == Fault::ImplicitHiddenValue {
                geometry.dynamic_shared_memory_bytes() + 1
            } else {
                geometry.dynamic_shared_memory_bytes()
            },
            if self.fault == Fault::ImplicitAqlGroup {
                geometry.dynamic_shared_memory_bytes() + 1
            } else {
                geometry.dynamic_shared_memory_bytes()
            },
            self.fault != Fault::ImplicitIncomplete,
        ))
    }

    unsafe fn observe_workgroup_sync_resources_v1(
        &mut self,
        profile: WorkgroupSyncProfileKindV1,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<WorkgroupSyncKernelResourceObservationV1, Self::Error> {
        self.events.lock().unwrap().push("resources");
        Ok(WorkgroupSyncKernelResourceObservationV1::new(
            if self.fault == Fault::ResourceProfile {
                other_profile(profile)
            } else {
                profile
            },
            if self.fault == Fault::ResourceExecutable {
                HsaExecutableObjectIdentityV1::new([0xc1; 32]).unwrap()
            } else {
                executable.object
            },
            if self.fault == Fault::ResourceKernel {
                HsaKernelObjectIdentityV1::new([0xc2; 32]).unwrap()
            } else {
                kernel.object
            },
            u32::from(self.fault == Fault::ResourceGroup),
            u32::from(self.fault == Fault::ResourcePrivate),
        ))
    }
}

fn events(state: &FakeState) -> Vec<&'static str> {
    state.events.lock().unwrap().clone()
}

fn exercise_success<P: ExactWorkgroupSyncHostProfileV1>(expected_dynamic_lds: usize) {
    let state = fake(Fault::None);
    let drops = Arc::new(AtomicUsize::new(0));
    let loaded =
        load_test_lifecycle_v1::<P, _>(state.adapter.clone(), true, drops.clone()).unwrap();
    assert_eq!(
        events(&state),
        ["environment", "load", "resolve", "resources"]
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let completed = loaded.dispatch_and_wait().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(state.explicit_preserved.load(Ordering::SeqCst));
    assert_eq!(
        state.observed_dynamic_lds.load(Ordering::SeqCst),
        expected_dynamic_lds
    );
    let unloaded = completed.unload();
    assert_eq!(unloaded.executable_object, executable_object());
    assert_eq!(unloaded.kernel_object, kernel_object());
    assert_eq!(unloaded.dispatch_identity, DISPATCH_IDENTITY);
    assert!(
        unloaded
            .unload_identity
            .as_bytes()
            .iter()
            .any(|byte| *byte != 0)
    );
    assert_eq!(state.unloads.load(Ordering::SeqCst), 1);
    assert_eq!(
        events(&state),
        [
            "environment",
            "load",
            "resolve",
            "resources",
            "implicit",
            "dispatch",
            "kernel_drop",
            "unload"
        ]
    );
}

#[test]
fn both_exact_profiles_complete_and_terminally_unload_once() {
    exercise_success::<WorkgroupLdsReductionProfileV1>(256);
    exercise_success::<WorkgroupScopedAtomicProfileV1>(0);
    assert_eq!(
        (
            test_explicit_bytes_v1(WorkgroupSyncProfileKindV1::LdsReduction),
            test_implicit_bytes_v1(),
            test_complete_bytes_v1(WorkgroupSyncProfileKindV1::LdsReduction)
        ),
        (32, 256, 288)
    );
    assert_eq!(
        (
            test_explicit_bytes_v1(WorkgroupSyncProfileKindV1::ScopedAtomic),
            test_complete_bytes_v1(WorkgroupSyncProfileKindV1::ScopedAtomic)
        ),
        (40, 296)
    );
}

#[test]
fn context_environment_abi_resource_and_profile_substitution_fail_closed() {
    let state = fake(Fault::None);
    let drops = Arc::new(AtomicUsize::new(0));
    assert!(matches!(
        load_test_lifecycle_v1::<WorkgroupLdsReductionProfileV1, _>(
            state.adapter.clone(),
            false,
            drops.clone()
        ),
        Err(WorkgroupSyncLoadErrorV1::ContextIdentity)
    ));
    assert!(events(&state).is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 1);

    for fault in [
        Fault::EnvironmentTarget,
        Fault::EnvironmentOrdinal,
        Fault::LoadDigest,
        Fault::LoadRuntime,
        Fault::LoadAgent,
        Fault::ResolutionSymbol,
        Fault::ResolutionSize,
        Fault::ResolutionAlignment,
        Fault::ResourceProfile,
        Fault::ResourceExecutable,
        Fault::ResourceKernel,
        Fault::ResourceGroup,
        Fault::ResourcePrivate,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        assert!(
            load_test_lifecycle_v1::<WorkgroupLdsReductionProfileV1, _>(
                state.adapter.clone(),
                true,
                drops.clone()
            )
            .is_err(),
            "accepted {fault:?}"
        );
        let loaded = events(&state).contains(&"load");
        assert_eq!(
            state.unloads.load(Ordering::SeqCst),
            usize::from(loaded),
            "{fault:?}"
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1, "{fault:?}");
    }
}

#[test]
fn implicit_hidden_lds_aql_and_completion_substitutions_cleanup_once() {
    for fault in [
        Fault::ImplicitProfile,
        Fault::ImplicitGeometry,
        Fault::ImplicitSpan,
        Fault::ImplicitHiddenOffset,
        Fault::ImplicitHiddenValue,
        Fault::ImplicitAqlGroup,
        Fault::ImplicitIncomplete,
        Fault::ExplicitMutation,
        Fault::HiddenMutation,
        Fault::DispatchGeometry,
        Fault::DispatchIncomplete,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        let loaded = load_test_lifecycle_v1::<WorkgroupLdsReductionProfileV1, _>(
            state.adapter.clone(),
            true,
            drops.clone(),
        )
        .unwrap();
        let error = match loaded.dispatch_and_wait() {
            Ok(_) => panic!("accepted {fault:?}"),
            Err(error) => error,
        };
        if fault == Fault::ExplicitMutation {
            assert!(matches!(
                error,
                WorkgroupSyncDispatchErrorV1::ExplicitKernargMutation
            ));
        }
        if fault == Fault::HiddenMutation {
            assert!(matches!(
                error,
                WorkgroupSyncDispatchErrorV1::HiddenDynamicLdsMutation
            ));
        }
        assert_eq!(state.unloads.load(Ordering::SeqCst), 1, "{fault:?}");
        assert_eq!(drops.load(Ordering::SeqCst), 1, "{fault:?}");
        let observed = events(&state);
        assert!(
            observed.iter().position(|event| *event == "kernel_drop")
                < observed.iter().position(|event| *event == "unload"),
            "{fault:?}: {observed:?}"
        );
    }
}

#[test]
fn dropping_loaded_or_completed_state_unloads_exactly_once() {
    let loaded_state = fake(Fault::None);
    let loaded_drops = Arc::new(AtomicUsize::new(0));
    let loaded = load_test_lifecycle_v1::<WorkgroupScopedAtomicProfileV1, _>(
        loaded_state.adapter.clone(),
        true,
        loaded_drops.clone(),
    )
    .unwrap();
    drop(loaded);
    assert_eq!(loaded_state.unloads.load(Ordering::SeqCst), 1);
    assert_eq!(loaded_drops.load(Ordering::SeqCst), 1);

    let completed_state = fake(Fault::None);
    let completed_drops = Arc::new(AtomicUsize::new(0));
    let completed = load_test_lifecycle_v1::<WorkgroupScopedAtomicProfileV1, _>(
        completed_state.adapter.clone(),
        true,
        completed_drops.clone(),
    )
    .unwrap()
    .dispatch_and_wait()
    .unwrap();
    drop(completed);
    assert_eq!(completed_state.unloads.load(Ordering::SeqCst), 1);
    assert_eq!(completed_drops.load(Ordering::SeqCst), 1);
}
