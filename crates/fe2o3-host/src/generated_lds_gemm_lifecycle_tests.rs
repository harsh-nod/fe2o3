use crate::{
    HsaAgentIdentityV1, HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1,
    HsaEnvironmentObservationV1, HsaExecutableObjectIdentityV1,
    HsaImplicitKernargInitializationObservationV1, HsaKernelObjectIdentityV1,
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, HsaPhysicalDeviceIdentityV1,
    HsaRuntimeIdentityV1, HsaUnloadObservationV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
    generated_lds_gemm_lifecycle::{
        CompletedExactLdsGemmSlice1V1, ExactLdsGemmKernelResourceObservationV1,
        ExactLdsGemmSlice1DispatchErrorV1, ExactLdsGemmSlice1LoadErrorV1,
        ReviewedExactLdsGemmRuntimeAdapterV1,
        test_support::{
            TestJoinMutationV1, exact_test_complete_bytes_v1, exact_test_explicit_bytes_v1,
            exact_test_hsa_alignment_v1, exact_test_implicit_bytes_v1, exact_test_static_lds_v1,
            load_test_lifecycle_v1, validate_join_mutation_v1,
        },
    },
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_core::ContextIdentity;
use std::{
    error::Error,
    fmt,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

#[allow(dead_code)]
fn completion_type_has_no_buffer_lifetimes<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    completed: CompletedExactLdsGemmSlice1V1<A>,
) -> CompletedExactLdsGemmSlice1V1<A> {
    completed
}

const RUNTIME_INSTANCE: [u8; 16] = [0x31; 16];
const DEVICE_UUID: [u8; 16] = [0x42; 16];
const AGENT_HANDLE: u64 = 0x5354;
const EXECUTABLE_OBJECT: [u8; 32] = [0x65; 32];
const KERNEL_OBJECT: [u8; 32] = [0x76; 32];
const DISPATCH_IDENTITY: [u8; 16] = [0x87; 16];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    None,
    EnvironmentAdapter,
    EnvironmentTarget,
    EnvironmentOrdinal,
    LoadAdapter,
    LoadDigest,
    LoadLength,
    LoadRuntime,
    LoadAgent,
    ResolveAdapter,
    ResolutionExecutable,
    ResolutionSymbol,
    ResolutionSize,
    ResolutionAlignmentEight,
    ResourceAdapter,
    ResourceExecutable,
    ResourceKernel,
    ResourceLds,
    ResourcePrivate,
    ImplicitAdapter,
    ImplicitExecutable,
    ImplicitKernel,
    ImplicitGeometry,
    ImplicitSpan,
    ImplicitIncomplete,
    ExplicitMutation,
    DispatchAdapter,
    DispatchExecutable,
    DispatchKernel,
    DispatchGeometry,
    DispatchIncomplete,
    DispatchPanic,
    UnloadAdapter,
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
    aligned: Arc<AtomicBool>,
    explicit_preserved: Arc<AtomicBool>,
    hidden_initialized: Arc<AtomicBool>,
}

struct FakeState {
    adapter: FakeAdapter,
    events: Arc<Mutex<Vec<&'static str>>>,
    unloads: Arc<AtomicUsize>,
    aligned: Arc<AtomicBool>,
    explicit_preserved: Arc<AtomicBool>,
    hidden_initialized: Arc<AtomicBool>,
}

fn fake(fault: Fault) -> FakeState {
    let events = Arc::new(Mutex::new(Vec::new()));
    let unloads = Arc::new(AtomicUsize::new(0));
    let aligned = Arc::new(AtomicBool::new(false));
    let explicit_preserved = Arc::new(AtomicBool::new(false));
    let hidden_initialized = Arc::new(AtomicBool::new(false));
    FakeState {
        adapter: FakeAdapter {
            fault,
            events: events.clone(),
            unloads: unloads.clone(),
            aligned: aligned.clone(),
            explicit_preserved: explicit_preserved.clone(),
            hidden_initialized: hidden_initialized.clone(),
        },
        events,
        unloads,
        aligned,
        explicit_preserved,
        hidden_initialized,
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

unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for FakeAdapter {
    type Executable = FakeExecutable;
    type Kernel = FakeKernel;
    type Error = FakeError;

    unsafe fn observe_environment(&mut self) -> Result<HsaEnvironmentObservationV1, Self::Error> {
        self.events.lock().unwrap().push("environment");
        match self.fault {
            Fault::EnvironmentAdapter => Err(FakeError("environment")),
            Fault::EnvironmentTarget => Ok(environment("gfx1100", 0)),
            Fault::EnvironmentOrdinal => Ok(environment("gfx942:xnack-", 1)),
            _ => Ok(environment("gfx942:xnack-", 0)),
        }
    }

    unsafe fn load_executable(
        &mut self,
        bytes: &[u8],
        finalized_digest: fe2o3_artifacts::PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
        self.events.lock().unwrap().push("load");
        if self.fault == Fault::LoadAdapter {
            return Err(FakeError("load"));
        }
        let digest = if self.fault == Fault::LoadDigest {
            DigestAlgorithm::Sha256.calculate(b"substituted")
        } else {
            finalized_digest
        };
        let byte_len = if self.fault == Fault::LoadLength {
            bytes.len() as u64 + 1
        } else {
            bytes.len() as u64
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
            HsaCodeObjectLoadObservationV1::new(digest, byte_len, runtime, agent, object),
        ))
    }

    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        _export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
        self.events.lock().unwrap().push("resolve");
        if self.fault == Fault::ResolveAdapter {
            return Err(FakeError("resolve"));
        }
        let executable_observation = if self.fault == Fault::ResolutionExecutable {
            HsaExecutableObjectIdentityV1::new([0xa1; 32]).unwrap()
        } else {
            executable.object
        };
        let symbol = if self.fault == Fault::ResolutionSymbol {
            "substituted"
        } else {
            "tiled_gemm_lds_v1"
        };
        let size = if self.fault == Fault::ResolutionSize {
            exact_test_complete_bytes_v1() as u64 - 8
        } else {
            exact_test_complete_bytes_v1() as u64
        };
        let alignment = if self.fault == Fault::ResolutionAlignmentEight {
            8
        } else {
            exact_test_hsa_alignment_v1()
        };
        let object = kernel_object();
        let observation = HsaKernelResolutionObservationV1::new(
            executable_observation,
            object,
            symbol,
            size,
            alignment,
        )
        .unwrap();
        Ok((
            FakeKernel {
                object,
                events: self.events.clone(),
            },
            observation,
        ))
    }

    unsafe fn launch_and_wait(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        self.events.lock().unwrap().push("dispatch");
        if self.fault == Fault::DispatchPanic {
            panic!("hostile adapter panic");
        }
        if self.fault == Fault::DispatchAdapter {
            return Err(FakeError("dispatch"));
        }
        self.aligned.store(
            (kernarg.as_ptr() as usize).is_multiple_of(exact_test_hsa_alignment_v1() as usize),
            Ordering::SeqCst,
        );
        self.explicit_preserved.store(
            kernarg[..exact_test_explicit_bytes_v1()]
                .iter()
                .copied()
                .eq((0..exact_test_explicit_bytes_v1()).map(|index| index as u8)),
            Ordering::SeqCst,
        );
        self.hidden_initialized.store(
            kernarg[exact_test_explicit_bytes_v1()..]
                .iter()
                .all(|byte| *byte == 0xa5),
            Ordering::SeqCst,
        );
        let executable_observation = if self.fault == Fault::DispatchExecutable {
            HsaExecutableObjectIdentityV1::new([0xa2; 32]).unwrap()
        } else {
            executable.object
        };
        let kernel_observation = if self.fault == Fault::DispatchKernel {
            HsaKernelObjectIdentityV1::new([0xa3; 32]).unwrap()
        } else {
            kernel.object
        };
        let geometry_observation = if self.fault == Fault::DispatchGeometry {
            HsaLaunchGeometryV1::new([2, 1, 1], [64, 1, 1], 0)
        } else {
            geometry
        };
        HsaDispatchObservationV1::new(
            DISPATCH_IDENTITY,
            executable_observation,
            kernel_observation,
            geometry_observation,
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
        if self.fault == Fault::UnloadAdapter {
            return Err(FakeError("unload"));
        }
        Ok(HsaUnloadObservationV1::new(
            executable.object,
            RUNTIME_INSTANCE,
            AGENT_HANDLE,
            true,
        ))
    }
}

unsafe impl ReviewedHsaImplicitKernargAdapterV1 for FakeAdapter {
    unsafe fn initialize_implicit_kernarg(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error> {
        self.events.lock().unwrap().push("implicit");
        if self.fault == Fault::ImplicitAdapter {
            return Err(FakeError("implicit"));
        }
        let end = implicit_byte_offset + implicit_byte_len;
        kernarg[implicit_byte_offset..end].fill(0xa5);
        if self.fault == Fault::ExplicitMutation {
            kernarg[0] ^= 1;
        }
        let executable_observation = if self.fault == Fault::ImplicitExecutable {
            HsaExecutableObjectIdentityV1::new([0xb1; 32]).unwrap()
        } else {
            executable.object
        };
        let kernel_observation = if self.fault == Fault::ImplicitKernel {
            HsaKernelObjectIdentityV1::new([0xb2; 32]).unwrap()
        } else {
            kernel.object
        };
        let geometry_observation = if self.fault == Fault::ImplicitGeometry {
            HsaLaunchGeometryV1::new([2, 1, 1], [64, 1, 1], 0)
        } else {
            geometry
        };
        let observed_explicit = if self.fault == Fault::ImplicitSpan {
            explicit_byte_len - 8
        } else {
            explicit_byte_len
        };
        Ok(HsaImplicitKernargInitializationObservationV1::new(
            executable_observation,
            kernel_observation,
            geometry_observation,
            observed_explicit as u64,
            implicit_byte_offset as u64,
            implicit_byte_len as u64,
            self.fault != Fault::ImplicitIncomplete,
        ))
    }
}

unsafe impl ReviewedExactLdsGemmRuntimeAdapterV1 for FakeAdapter {
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity {
        unreachable!("the test harness supplies the already-validated context result")
    }

    unsafe fn observe_exact_lds_gemm_kernel_resources_v1(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<ExactLdsGemmKernelResourceObservationV1, Self::Error> {
        self.events.lock().unwrap().push("resources");
        if self.fault == Fault::ResourceAdapter {
            return Err(FakeError("resources"));
        }
        let executable_observation = if self.fault == Fault::ResourceExecutable {
            HsaExecutableObjectIdentityV1::new([0xc1; 32]).unwrap()
        } else {
            executable.object
        };
        let kernel_observation = if self.fault == Fault::ResourceKernel {
            HsaKernelObjectIdentityV1::new([0xc2; 32]).unwrap()
        } else {
            kernel.object
        };
        Ok(ExactLdsGemmKernelResourceObservationV1::new(
            executable_observation,
            kernel_observation,
            if self.fault == Fault::ResourceLds {
                exact_test_static_lds_v1() - 1
            } else {
                exact_test_static_lds_v1()
            },
            u32::from(self.fault == Fault::ResourcePrivate),
        ))
    }
}

fn events(state: &FakeState) -> Vec<&'static str> {
    state.events.lock().unwrap().clone()
}

#[test]
fn join_rejects_every_identity_contract_and_host_substitution() {
    assert_eq!(validate_join_mutation_v1(TestJoinMutationV1::None), Ok(()));
    for mutation in [
        TestJoinMutationV1::ImportIdentity,
        TestJoinMutationV1::ProfileIdentity,
        TestJoinMutationV1::ContractBinding,
        TestJoinMutationV1::FinalizedOutput,
        TestJoinMutationV1::Profile,
        TestJoinMutationV1::Target,
        TestJoinMutationV1::CodeObjectVersion,
        TestJoinMutationV1::Grid,
        TestJoinMutationV1::Workgroup,
        TestJoinMutationV1::Wavefront,
        TestJoinMutationV1::ExplicitKernarg,
        TestJoinMutationV1::CompleteKernarg,
        TestJoinMutationV1::ContractKernargAlignment,
        TestJoinMutationV1::StaticLds,
        TestJoinMutationV1::LdsAllocations,
        TestJoinMutationV1::LdsBytesPerAllocation,
        TestJoinMutationV1::LdsAlignment,
        TestJoinMutationV1::BufferRole,
        TestJoinMutationV1::BufferElement,
        TestJoinMutationV1::BufferElements,
        TestJoinMutationV1::BufferBytes,
        TestJoinMutationV1::BufferLengthIdentity,
        TestJoinMutationV1::BufferOwnership,
        TestJoinMutationV1::BufferAccess,
        TestJoinMutationV1::BufferAlias,
        TestJoinMutationV1::HostTarget,
        TestJoinMutationV1::HostProfile,
        TestJoinMutationV1::HostProfileIdentity,
        TestJoinMutationV1::HostGrid,
        TestJoinMutationV1::HostWorkgroup,
        TestJoinMutationV1::HostStaticLds,
        TestJoinMutationV1::HostDynamicLds,
        TestJoinMutationV1::HostExplicitKernarg,
        TestJoinMutationV1::HostCompleteKernarg,
        TestJoinMutationV1::HostContractKernargAlignment,
        TestJoinMutationV1::HostLengthIdentity,
    ] {
        assert!(
            validate_join_mutation_v1(mutation).is_err(),
            "accepted {mutation:?}"
        );
    }
}

#[test]
fn successful_lifecycle_has_exact_order_alignment_spans_and_retention() {
    let state = fake(Fault::None);
    let drops = Arc::new(AtomicUsize::new(0));
    let loaded = load_test_lifecycle_v1(state.adapter.clone(), true, drops.clone()).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        events(&state),
        ["environment", "load", "resolve", "resources"]
    );

    let completed = loaded.dispatch_and_wait().unwrap();
    // Quiescent completion releases the joined artifact and all A/B/C leases;
    // executable unload authority remains live independently.
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(state.unloads.load(Ordering::SeqCst), 0);
    assert!(state.aligned.load(Ordering::SeqCst));
    assert!(state.explicit_preserved.load(Ordering::SeqCst));
    assert!(state.hidden_initialized.load(Ordering::SeqCst));
    assert_eq!(exact_test_explicit_bytes_v1(), 48);
    assert_eq!(exact_test_implicit_bytes_v1(), 256);
    assert_eq!(exact_test_complete_bytes_v1(), 304);
    assert_eq!(exact_test_hsa_alignment_v1(), 16);

    let receipt = completed.unload();
    assert_eq!(receipt.executable_object, executable_object());
    assert_eq!(receipt.kernel_object, kernel_object());
    assert_eq!(receipt.dispatch_identity, DISPATCH_IDENTITY);
    assert!(
        receipt
            .unload_identity
            .as_bytes()
            .iter()
            .any(|byte| *byte != 0)
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
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
            "unload",
        ]
    );
}

#[test]
fn context_and_environment_rejections_create_no_native_authority() {
    let state = fake(Fault::None);
    let drops = Arc::new(AtomicUsize::new(0));
    assert!(matches!(
        load_test_lifecycle_v1(state.adapter.clone(), false, drops.clone()),
        Err(ExactLdsGemmSlice1LoadErrorV1::ContextIdentity)
    ));
    assert!(events(&state).is_empty());
    assert_eq!(state.unloads.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);

    for fault in [
        Fault::EnvironmentAdapter,
        Fault::EnvironmentTarget,
        Fault::EnvironmentOrdinal,
        Fault::LoadAdapter,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        assert!(load_test_lifecycle_v1(state.adapter.clone(), true, drops.clone()).is_err());
        assert_eq!(state.unloads.load(Ordering::SeqCst), 0, "{fault:?}");
        assert_eq!(drops.load(Ordering::SeqCst), 1, "{fault:?}");
    }
}

#[test]
fn load_and_resolution_substitutions_cleanup_exactly_once() {
    for fault in [
        Fault::LoadDigest,
        Fault::LoadLength,
        Fault::LoadRuntime,
        Fault::LoadAgent,
        Fault::ResolveAdapter,
        Fault::ResolutionExecutable,
        Fault::ResolutionSymbol,
        Fault::ResolutionSize,
        Fault::ResolutionAlignmentEight,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        assert!(load_test_lifecycle_v1(state.adapter.clone(), true, drops.clone()).is_err());
        assert_eq!(state.unloads.load(Ordering::SeqCst), 1, "{fault:?}");
        assert_eq!(drops.load(Ordering::SeqCst), 1, "{fault:?}");
        let observed = events(&state);
        assert_eq!(observed.last(), Some(&"unload"), "{fault:?}: {observed:?}");
        if observed.contains(&"kernel_drop") {
            assert!(
                observed.iter().position(|event| *event == "kernel_drop")
                    < observed.iter().position(|event| *event == "unload"),
                "{fault:?}: {observed:?}"
            );
        }
    }
}

#[test]
fn runtime_resource_substitutions_cleanup_exactly_once() {
    for fault in [
        Fault::ResourceAdapter,
        Fault::ResourceExecutable,
        Fault::ResourceKernel,
        Fault::ResourceLds,
        Fault::ResourcePrivate,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        assert!(load_test_lifecycle_v1(state.adapter.clone(), true, drops.clone()).is_err());
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
fn implicit_and_dispatch_failures_are_recoverable_only_with_terminal_cleanup() {
    for fault in [
        Fault::ImplicitAdapter,
        Fault::ImplicitExecutable,
        Fault::ImplicitKernel,
        Fault::ImplicitGeometry,
        Fault::ImplicitSpan,
        Fault::ImplicitIncomplete,
        Fault::ExplicitMutation,
        Fault::DispatchAdapter,
        Fault::DispatchExecutable,
        Fault::DispatchKernel,
        Fault::DispatchGeometry,
        Fault::DispatchIncomplete,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        let loaded = load_test_lifecycle_v1(state.adapter.clone(), true, drops.clone()).unwrap();
        let error = match loaded.dispatch_and_wait() {
            Ok(_) => panic!("accepted {fault:?}"),
            Err(error) => error,
        };
        match fault {
            Fault::ImplicitAdapter => assert!(matches!(
                error,
                ExactLdsGemmSlice1DispatchErrorV1::ImplicitAdapter(_)
            )),
            Fault::ExplicitMutation => assert!(matches!(
                error,
                ExactLdsGemmSlice1DispatchErrorV1::ExplicitKernargMutation
            )),
            Fault::DispatchAdapter => assert!(matches!(
                error,
                ExactLdsGemmSlice1DispatchErrorV1::DispatchAdapter(_)
            )),
            _ => {}
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
fn dropping_loaded_or_completed_state_unloads_once() {
    let loaded_state = fake(Fault::None);
    let loaded_drops = Arc::new(AtomicUsize::new(0));
    let loaded =
        load_test_lifecycle_v1(loaded_state.adapter.clone(), true, loaded_drops.clone()).unwrap();
    drop(loaded);
    assert_eq!(loaded_state.unloads.load(Ordering::SeqCst), 1);
    assert_eq!(loaded_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        events(&loaded_state).as_slice(),
        [
            "environment",
            "load",
            "resolve",
            "resources",
            "kernel_drop",
            "unload"
        ]
    );

    let completed_state = fake(Fault::None);
    let completed_drops = Arc::new(AtomicUsize::new(0));
    let completed = load_test_lifecycle_v1(
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

#[test]
fn adapter_panic_and_unload_ambiguity_are_process_terminal() {
    const CHILD: &str = "FE2O3_EXACT_LDS_GEMM_TERMINAL_CHILD";
    if let Ok(mode) = std::env::var(CHILD) {
        let fault = match mode.as_str() {
            "panic" => Fault::DispatchPanic,
            "unload" => Fault::UnloadAdapter,
            _ => unreachable!(),
        };
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        let loaded = load_test_lifecycle_v1(state.adapter.clone(), true, drops).unwrap();
        if fault == Fault::DispatchPanic {
            let _ = loaded.dispatch_and_wait();
        } else {
            drop(loaded);
        }
        panic!("terminal adapter condition returned to Rust");
    }

    for mode in ["panic", "unload"] {
        let status = fe2o3_artifact_transaction::with_test_artifact_fork_exec_barrier_v1(|| {
            Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg(
                    "generated_lds_gemm_lifecycle_tests::\
                     adapter_panic_and_unload_ambiguity_are_process_terminal",
                )
                .arg("--nocapture")
                .env(CHILD, mode)
                .status()
        })
        .unwrap();
        assert!(
            !status.success(),
            "{mode} unexpectedly returned successfully"
        );
    }
}
