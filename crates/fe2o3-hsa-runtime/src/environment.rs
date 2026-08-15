use crate::api::{
    AgentFacts, ApiError, DirectRuntimeApi, EnvironmentApi, HipFacts, PoolFacts, RuntimeFacts,
};
use crate::dispatch::PendingDispatch;
use fe2o3_amd_target::AmdTargetId;
use fe2o3_core::GpuContext;
use fe2o3_host::{
    HsaAgentIdentityV1, HsaEnvironmentObservationV1, HsaPhysicalDeviceIdentityV1,
    HsaRuntimeIdentityV1,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

const REQUIRED_PROCESSOR: &str = "gfx942";
const HSA_DEVICE_TYPE_CPU: u32 = 0;
const HSA_DEVICE_TYPE_GPU: u32 = 1;
const HSA_AGENT_FEATURE_KERNEL_DISPATCH: u32 = 1;
const HSA_AMD_SEGMENT_GLOBAL: u32 = 0;
const HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_KERNARG_INIT: u32 = 1;

/// Production direct-HSA implementation of the Worker V2 reviewed adapter.
///
/// Construction retains the exact HIP context wrapper whose ordinal, target,
/// UUID, and PCI identity are correlated with one HSA agent. Raw HSA handles
/// remain private and cannot be supplied by callers.
pub struct ReviewedHsaRuntimeAdapterV1 {
    pub(crate) core: AdapterCore<DirectRuntimeApi>,
    pub(crate) pending_dispatch: Option<PendingDispatch>,
    _not_sync: PhantomData<Cell<()>>,
}

impl ReviewedHsaRuntimeAdapterV1 {
    pub fn new(context: Arc<GpuContext>) -> Result<Self, HsaRuntimeAdapterError> {
        Self::with_api(context, DirectRuntimeApi::new()).map(|core| Self {
            core,
            pending_dispatch: None,
            _not_sync: PhantomData,
        })
    }

    fn with_api(
        context: Arc<GpuContext>,
        mut api: DirectRuntimeApi,
    ) -> Result<AdapterCore<DirectRuntimeApi>, HsaRuntimeAdapterError> {
        let target = context
            .observe_target()
            .map_err(|error| HsaRuntimeAdapterError::HipContext(error.to_string()))?;
        let ordinal = target.device_id();
        if target.target_id().processor() != REQUIRED_PROCESSOR {
            return Err(HsaRuntimeAdapterError::UnsupportedTarget(
                target.target_id().to_string(),
            ));
        }
        let runtime = api.initialize().map_err(HsaRuntimeAdapterError::api)?;
        let result = (|| {
            let hip = api
                .observe_hip_device(ordinal)
                .map_err(HsaRuntimeAdapterError::api)?;
            let agents = api.collect_agents().map_err(HsaRuntimeAdapterError::api)?;
            let pools = api
                .collect_kernarg_pools()
                .map_err(HsaRuntimeAdapterError::api)?;
            select_environment(ordinal, &runtime, &hip, &agents, &pools)
        })();
        match result {
            Ok(selected) => Ok(AdapterCore {
                api,
                environment: selected.environment,
                agent: selected.agent,
                profile: selected.profile,
                queue_min_size: selected.queue_min_size,
                queue_max_size: selected.queue_max_size,
                kernarg_pool: selected.kernarg_pool,
                completion_timeout: crate::dispatch::COMPLETION_TIMEOUT,
                next_identity: 1,
                runtime_live: true,
                _context: Some(context),
            }),
            Err(error) => {
                if api.shut_down().is_err() {
                    std::process::abort();
                }
                Err(error)
            }
        }
    }

    pub const fn environment(&self) -> &HsaEnvironmentObservationV1 {
        &self.core.environment
    }

    /// Production synchronous-completion deadline used by protected dispatch.
    pub const fn completion_timeout_v1(&self) -> std::time::Duration {
        self.core.completion_timeout
    }
}

impl Drop for ReviewedHsaRuntimeAdapterV1 {
    fn drop(&mut self) {
        crate::dispatch::destroy_pending_dispatch(&mut self.core.api, &mut self.pending_dispatch);
    }
}

#[allow(dead_code)]
pub(crate) struct AdapterCore<A: EnvironmentApi> {
    pub api: A,
    pub environment: HsaEnvironmentObservationV1,
    pub agent: u64,
    pub profile: u32,
    pub queue_min_size: u32,
    pub queue_max_size: u32,
    pub kernarg_pool: u64,
    pub completion_timeout: std::time::Duration,
    pub next_identity: u64,
    pub(crate) runtime_live: bool,
    pub(crate) _context: Option<Arc<GpuContext>>,
}

impl<A: EnvironmentApi> Drop for AdapterCore<A> {
    fn drop(&mut self) {
        if self.runtime_live && self.api.shut_down().is_err() {
            std::process::abort();
        }
        self.runtime_live = false;
    }
}

struct SelectedEnvironment {
    environment: HsaEnvironmentObservationV1,
    agent: u64,
    profile: u32,
    queue_min_size: u32,
    queue_max_size: u32,
    kernarg_pool: u64,
}

fn select_environment(
    ordinal: i32,
    runtime: &RuntimeFacts,
    hip: &HipFacts,
    agents: &[AgentFacts],
    pools: &[PoolFacts],
) -> Result<SelectedEnvironment, HsaRuntimeAdapterError> {
    if hip.round_trip_ordinal != ordinal {
        return Err(HsaRuntimeAdapterError::HipOrdinalRoundTrip {
            expected: ordinal,
            actual: hip.round_trip_ordinal,
        });
    }
    if hip.uuid.iter().all(|byte| *byte == 0) {
        return Err(HsaRuntimeAdapterError::InvalidHipUuid);
    }
    let pci = parse_pci_bus_id(&hip.pci_bus_id)?;
    let mut matching = agents
        .iter()
        .filter(|agent| {
            agent.device_type == HSA_DEVICE_TYPE_GPU
                && agent.feature & HSA_AGENT_FEATURE_KERNEL_DISPATCH != 0
                && agent.name == REQUIRED_PROCESSOR
                && agent.domain == u32::from(pci.domain)
                && agent.bdf_id == pci.bdf_id()
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Err(HsaRuntimeAdapterError::HsaAgentNotFound);
    }
    if matching.len() != 1 {
        return Err(HsaRuntimeAdapterError::HsaAgentAmbiguous(matching.len()));
    }
    let agent = matching.pop().expect("one matching HSA agent");
    let target = parse_hsa_isa_target(&agent.isa)?;
    if agent.handle == 0 || agent.matching_isa_count != 1 {
        return Err(HsaRuntimeAdapterError::InvalidHsaAgentIdentity);
    }
    let hsa_unique_id = parse_hsa_uuid(&agent.uuid)?;
    if !hip_uuid_contains_unique_id(hip.uuid, hsa_unique_id) {
        return Err(HsaRuntimeAdapterError::PhysicalUuidMismatch);
    }

    let mut kernarg_pools = pools
        .iter()
        .filter(|pool| {
            pool.handle != 0
                && pool.segment == HSA_AMD_SEGMENT_GLOBAL
                && pool.global_flags & HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_KERNARG_INIT != 0
                && pool.runtime_alloc_allowed
                && pool.runtime_alloc_alignment != 0
                && pool.runtime_alloc_alignment.is_power_of_two()
                && agents.iter().any(|owner| {
                    owner.handle == pool.owner_agent && owner.device_type == HSA_DEVICE_TYPE_CPU
                })
        })
        .collect::<Vec<_>>();
    kernarg_pools.sort_by_key(|pool| (pool.owner_node, pool.owner_agent, pool.handle));
    let kernarg_pool = kernarg_pools
        .first()
        .ok_or(HsaRuntimeAdapterError::KernargPoolNotFound)?;

    let physical_uuid = derive_physical_identity(hip.uuid, hsa_unique_id, &hip.pci_bus_id);
    let runtime_identity = HsaRuntimeIdentityV1::new(
        "ROCr HSA",
        format!("{}.{}", runtime.version_major, runtime.version_minor),
        runtime.image_digest,
        runtime.instance,
    )
    .map_err(|_| HsaRuntimeAdapterError::InvalidEnvironmentObservation)?;
    let physical = HsaPhysicalDeviceIdentityV1::new(physical_uuid, agent.node, ordinal, target)
        .map_err(|_| HsaRuntimeAdapterError::InvalidEnvironmentObservation)?;
    let agent_identity =
        HsaAgentIdentityV1::new(runtime.instance, agent.handle, physical_uuid, target)
            .map_err(|_| HsaRuntimeAdapterError::InvalidEnvironmentObservation)?;
    let environment = HsaEnvironmentObservationV1::new(runtime_identity, physical, agent_identity)
        .map_err(|_| HsaRuntimeAdapterError::InvalidEnvironmentObservation)?;
    Ok(SelectedEnvironment {
        environment,
        agent: agent.handle,
        profile: agent.profile,
        queue_min_size: agent.queue_min_size,
        queue_max_size: agent.queue_max_size,
        kernarg_pool: kernarg_pool.handle,
    })
}

fn parse_hsa_isa_target(text: &str) -> Result<AmdTargetId, HsaRuntimeAdapterError> {
    let target = text
        .strip_prefix("amdgcn-amd-amdhsa--")
        .and_then(|target| AmdTargetId::parse(target).ok())
        .filter(|target| target.processor() == REQUIRED_PROCESSOR)
        .ok_or(HsaRuntimeAdapterError::InvalidHsaAgentIdentity)?;
    Ok(target)
}

#[derive(Clone, Copy)]
struct PciAddress {
    domain: u16,
    bus: u8,
    device: u8,
    function: u8,
}

impl PciAddress {
    const fn bdf_id(self) -> u32 {
        (self.bus as u32) << 8 | (self.device as u32) << 3 | self.function as u32
    }
}

fn parse_pci_bus_id(text: &str) -> Result<PciAddress, HsaRuntimeAdapterError> {
    let (domain, rest) = text
        .split_once(':')
        .ok_or(HsaRuntimeAdapterError::InvalidHipPciBusId)?;
    let (bus, rest) = rest
        .split_once(':')
        .ok_or(HsaRuntimeAdapterError::InvalidHipPciBusId)?;
    let (device, function) = rest
        .split_once('.')
        .ok_or(HsaRuntimeAdapterError::InvalidHipPciBusId)?;
    if domain.len() != 4 || bus.len() != 2 || device.len() != 2 || function.len() != 1 {
        return Err(HsaRuntimeAdapterError::InvalidHipPciBusId);
    }
    let address = PciAddress {
        domain: u16::from_str_radix(domain, 16)
            .map_err(|_| HsaRuntimeAdapterError::InvalidHipPciBusId)?,
        bus: u8::from_str_radix(bus, 16).map_err(|_| HsaRuntimeAdapterError::InvalidHipPciBusId)?,
        device: u8::from_str_radix(device, 16)
            .map_err(|_| HsaRuntimeAdapterError::InvalidHipPciBusId)?,
        function: u8::from_str_radix(function, 16)
            .map_err(|_| HsaRuntimeAdapterError::InvalidHipPciBusId)?,
    };
    if address.device > 31 || address.function > 7 {
        return Err(HsaRuntimeAdapterError::InvalidHipPciBusId);
    }
    Ok(address)
}

fn parse_hsa_uuid(text: &str) -> Result<[u8; 8], HsaRuntimeAdapterError> {
    let body = text
        .strip_prefix("GPU-")
        .ok_or(HsaRuntimeAdapterError::InvalidHsaUuid)?;
    if body.len() != 16 {
        return Err(HsaRuntimeAdapterError::InvalidHsaUuid);
    }
    let mut result = [0; 8];
    for (index, output) in result.iter_mut().enumerate() {
        *output = u8::from_str_radix(&body[index * 2..index * 2 + 2], 16)
            .map_err(|_| HsaRuntimeAdapterError::InvalidHsaUuid)?;
    }
    if result.iter().all(|byte| *byte == 0) {
        return Err(HsaRuntimeAdapterError::InvalidHsaUuid);
    }
    Ok(result)
}

fn hip_uuid_contains_unique_id(hip: [u8; 16], hsa: [u8; 8]) -> bool {
    let mut ascii = [0; 16];
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in hsa.into_iter().enumerate() {
        ascii[index * 2] = HEX[usize::from(byte >> 4)];
        ascii[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
    let reversed = hsa.map(|byte| byte).into_iter().rev().collect::<Vec<_>>();
    hip == ascii
        || hip
            .windows(8)
            .any(|window| window == hsa || window == reversed)
}

fn derive_physical_identity(hip: [u8; 16], hsa: [u8; 8], pci: &str) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3-hsa-hip-physical-device-v1\0");
    hasher.update(hip);
    hasher.update(hsa);
    hasher.update(pci.as_bytes());
    let digest = hasher.finalize();
    let mut result = [0; 16];
    result.copy_from_slice(&digest[..16]);
    result
}

/// Failure to establish or operate the reviewed direct-HSA boundary.
#[derive(Debug)]
#[non_exhaustive]
pub enum HsaRuntimeAdapterError {
    HipContext(String),
    UnsupportedTarget(String),
    RuntimeCall {
        operation: &'static str,
        status: i32,
    },
    CleanupAmbiguous {
        operation: &'static str,
        status: i32,
        cleanup_operation: &'static str,
        cleanup_status: i32,
    },
    InvalidExecutableObservation(&'static str),
    InvalidImplicitKernarg(&'static str),
    DispatchAmbiguous {
        operation: &'static str,
        status: i64,
    },
    HipOrdinalRoundTrip {
        expected: i32,
        actual: i32,
    },
    InvalidHipUuid,
    InvalidHipPciBusId,
    HsaAgentNotFound,
    HsaAgentAmbiguous(usize),
    InvalidHsaAgentIdentity,
    InvalidHsaUuid,
    PhysicalUuidMismatch,
    KernargPoolNotFound,
    InvalidEnvironmentObservation,
}

impl fmt::Display for HsaRuntimeAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HipContext(error) => write!(formatter, "HIP context observation: {error}"),
            Self::UnsupportedTarget(target) => {
                write!(
                    formatter,
                    "target {target} is not the reviewed gfx942 profile"
                )
            }
            Self::RuntimeCall { operation, status } => write!(
                formatter,
                "{} failed with runtime status {}",
                operation, status
            ),
            Self::CleanupAmbiguous {
                operation,
                status,
                cleanup_operation,
                cleanup_status,
            } => write!(
                formatter,
                "{operation} failed with status {status}; cleanup {cleanup_operation} failed with status {cleanup_status}"
            ),
            Self::InvalidExecutableObservation(field) => {
                write!(formatter, "invalid HSA executable observation: {field}")
            }
            Self::InvalidImplicitKernarg(field) => {
                write!(formatter, "invalid AMDHSA COV6 implicit kernarg: {field}")
            }
            Self::DispatchAmbiguous { operation, status } => write!(
                formatter,
                "HSA dispatch became ambiguous during {operation} with status {status}"
            ),
            Self::HipOrdinalRoundTrip { expected, actual } => write!(
                formatter,
                "HIP PCI identity round trip returned ordinal {actual}, expected {expected}"
            ),
            Self::InvalidHipUuid => formatter.write_str("HIP returned an invalid device UUID"),
            Self::InvalidHipPciBusId => {
                formatter.write_str("HIP returned a malformed PCI bus identity")
            }
            Self::HsaAgentNotFound => formatter
                .write_str("no HSA gfx942 kernel agent matches the HIP PCI device identity"),
            Self::HsaAgentAmbiguous(count) => write!(
                formatter,
                "{count} HSA agents match one HIP PCI device identity"
            ),
            Self::InvalidHsaAgentIdentity => {
                formatter.write_str("the selected HSA agent has an invalid exact ISA identity")
            }
            Self::InvalidHsaUuid => {
                formatter.write_str("the selected HSA agent has no reviewed GPU unique ID")
            }
            Self::PhysicalUuidMismatch => {
                formatter.write_str("HIP and HSA device UUID observations do not correlate")
            }
            Self::KernargPoolNotFound => {
                formatter.write_str("no allocatable CPU kernarg pool is available")
            }
            Self::InvalidEnvironmentObservation => {
                formatter.write_str("reviewed HSA environment observation is internally invalid")
            }
        }
    }
}

impl std::error::Error for HsaRuntimeAdapterError {}

impl HsaRuntimeAdapterError {
    pub(crate) fn api(error: ApiError) -> Self {
        Self::RuntimeCall {
            operation: error.operation,
            status: error.status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::DigestAlgorithm;

    fn runtime() -> RuntimeFacts {
        RuntimeFacts {
            version_major: 1,
            version_minor: 18,
            image_digest: DigestAlgorithm::Sha256.calculate(b"reviewed runtime"),
            instance: [7; 16],
        }
    }

    fn hip() -> HipFacts {
        HipFacts {
            uuid: [
                0x6c, 0xed, 0x16, 0x47, 0xa2, 0x96, 0x54, 0x5c, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            pci_bus_id: "0000:05:00.0".into(),
            round_trip_ordinal: 0,
        }
    }

    fn agents() -> Vec<AgentFacts> {
        vec![
            AgentFacts {
                handle: 10,
                node: 0,
                device_type: HSA_DEVICE_TYPE_CPU,
                feature: 0,
                profile: 1,
                queue_min_size: 0,
                queue_max_size: 0,
                queue_type: 1,
                domain: 0,
                bdf_id: 0,
                name: "CPU".into(),
                uuid: "CPU-XX".into(),
                isa: String::new(),
                matching_isa_count: 0,
            },
            AgentFacts {
                handle: 20,
                node: 2,
                device_type: HSA_DEVICE_TYPE_GPU,
                feature: HSA_AGENT_FEATURE_KERNEL_DISPATCH,
                profile: 0,
                queue_min_size: 64,
                queue_max_size: 131_072,
                queue_type: 1,
                domain: 0,
                bdf_id: 0x500,
                name: "gfx942".into(),
                uuid: "GPU-6ced1647a296545c".into(),
                isa: "amdgcn-amd-amdhsa--gfx942:sramecc+:xnack-".into(),
                matching_isa_count: 1,
            },
        ]
    }

    fn pools() -> Vec<PoolFacts> {
        vec![PoolFacts {
            handle: 30,
            owner_agent: 10,
            owner_node: 0,
            segment: HSA_AMD_SEGMENT_GLOBAL,
            global_flags: HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_KERNARG_INIT,
            runtime_alloc_allowed: true,
            runtime_alloc_alignment: 4096,
        }]
    }

    #[test]
    fn exact_hip_hsa_physical_identity_selects_one_gfx942_agent() {
        let selected = select_environment(0, &runtime(), &hip(), &agents(), &pools()).unwrap();
        assert_eq!(selected.agent, 20);
        assert_eq!(selected.kernarg_pool, 30);
        assert_eq!(selected.environment.physical_device().hip_ordinal(), 0);
        assert_eq!(
            selected.environment.physical_device().target().to_string(),
            "gfx942:sramecc+:xnack-"
        );
    }

    #[test]
    fn rocm_ascii_hip_uuid_correlates_with_the_hsa_gpu_unique_id() {
        let mut hip = hip();
        hip.uuid = *b"6ced1647a296545c";
        let selected = select_environment(0, &runtime(), &hip, &agents(), &pools()).unwrap();
        assert_eq!(selected.agent, 20);
    }

    #[test]
    fn identity_selection_rejects_every_ambiguous_or_unbound_edge() {
        let mut wrong_round_trip = hip();
        wrong_round_trip.round_trip_ordinal = 1;
        assert!(matches!(
            select_environment(0, &runtime(), &wrong_round_trip, &agents(), &pools()),
            Err(HsaRuntimeAdapterError::HipOrdinalRoundTrip { .. })
        ));

        let mut zero_uuid = hip();
        zero_uuid.uuid = [0; 16];
        assert!(matches!(
            select_environment(0, &runtime(), &zero_uuid, &agents(), &pools()),
            Err(HsaRuntimeAdapterError::InvalidHipUuid)
        ));

        let mut wrong_pci = hip();
        wrong_pci.pci_bus_id = "not-pci".into();
        assert!(matches!(
            select_environment(0, &runtime(), &wrong_pci, &agents(), &pools()),
            Err(HsaRuntimeAdapterError::InvalidHipPciBusId)
        ));

        let mut absent = agents();
        absent[1].bdf_id = 0x600;
        assert!(matches!(
            select_environment(0, &runtime(), &hip(), &absent, &pools()),
            Err(HsaRuntimeAdapterError::HsaAgentNotFound)
        ));

        let mut duplicate = agents();
        duplicate.push(duplicate[1].clone());
        assert!(matches!(
            select_environment(0, &runtime(), &hip(), &duplicate, &pools()),
            Err(HsaRuntimeAdapterError::HsaAgentAmbiguous(2))
        ));

        let mut duplicate_isa = agents();
        duplicate_isa[1].matching_isa_count = 2;
        assert!(matches!(
            select_environment(0, &runtime(), &hip(), &duplicate_isa, &pools()),
            Err(HsaRuntimeAdapterError::InvalidHsaAgentIdentity)
        ));

        for invalid_isa in [
            "amdgcn-amd-amdhsa--gfx942evil",
            "amdgcn-amd-amdhsa--gfx942:xnack-:xnack+",
            "amdgcn-amd-amdhsa--gfx950:sramecc+:xnack-",
        ] {
            let mut malformed = agents();
            malformed[1].isa = invalid_isa.into();
            assert!(matches!(
                select_environment(0, &runtime(), &hip(), &malformed, &pools()),
                Err(HsaRuntimeAdapterError::InvalidHsaAgentIdentity)
            ));
        }

        let mut mismatched_uuid = hip();
        mismatched_uuid.uuid[0] ^= 0xff;
        assert!(matches!(
            select_environment(0, &runtime(), &mismatched_uuid, &agents(), &pools()),
            Err(HsaRuntimeAdapterError::PhysicalUuidMismatch)
        ));

        assert!(matches!(
            select_environment(0, &runtime(), &hip(), &agents(), &[]),
            Err(HsaRuntimeAdapterError::KernargPoolNotFound)
        ));
    }

    #[test]
    fn kernarg_pool_selection_is_deterministic_and_requires_cpu_ownership() {
        let mut candidates = pools();
        candidates.push(PoolFacts {
            handle: 29,
            owner_agent: 10,
            owner_node: 1,
            ..candidates[0]
        });
        assert_eq!(
            select_environment(0, &runtime(), &hip(), &agents(), &candidates)
                .unwrap()
                .kernarg_pool,
            30
        );
        candidates[0].owner_agent = 20;
        candidates[1].owner_agent = 20;
        assert!(matches!(
            select_environment(0, &runtime(), &hip(), &agents(), &candidates),
            Err(HsaRuntimeAdapterError::KernargPoolNotFound)
        ));
    }

    #[test]
    fn adapter_may_move_between_threads_but_requires_exclusive_access() {
        fn require_send<T: Send>() {}
        require_send::<ReviewedHsaRuntimeAdapterV1>();
    }
}
