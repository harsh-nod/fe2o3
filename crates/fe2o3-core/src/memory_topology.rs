use crate::{Error, GpuContext, check};
use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_ALLOCATION_ID: AtomicU64 = AtomicU64::new(1);

fn next_identity(counter: &AtomicU64, kind: &'static str) -> u64 {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .unwrap_or_else(|_| panic!("process-local {kind} identity space exhausted"))
}

/// Exact identity of one [`GpuContext`] wrapper in this process.
///
/// The HIP runtime's primary-context model means two wrappers may refer to the
/// same physical device while having distinct Rust lifetimes. This identity
/// captures that distinction. It is not stable across process restarts and is
/// not native context, peer, memory, or launch authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextIdentity {
    device_ordinal: i32,
    process_sequence: u64,
}

impl ContextIdentity {
    pub(crate) fn fresh(device_ordinal: i32) -> Self {
        Self {
            device_ordinal,
            process_sequence: next_identity(&NEXT_CONTEXT_ID, "context"),
        }
    }

    pub const fn device_ordinal(self) -> i32 {
        self.device_ordinal
    }
}

/// Exact HIP-reported identity of one physical device.
///
/// UUID and canonical PCI address must agree in the same successful query.
/// The ordinal is retained because all HIP operations in this process address
/// the device by that ordinal. This copyable record is descriptive only.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalDeviceIdentity {
    ordinal: i32,
    uuid: [u8; 16],
    pci_domain: u16,
    pci_bus: u8,
    pci_device: u8,
    pci_function: u8,
}

impl PhysicalDeviceIdentity {
    pub const fn ordinal(self) -> i32 {
        self.ordinal
    }

    pub const fn uuid(self) -> [u8; 16] {
        self.uuid
    }

    pub const fn pci_address(self) -> (u16, u8, u8, u8) {
        (
            self.pci_domain,
            self.pci_bus,
            self.pci_device,
            self.pci_function,
        )
    }
}

/// Descriptive HIP memory capabilities for one exact physical device query.
///
/// Capability bits do not enable peer access, create an allocation, authorize
/// a mapping, establish residency, or grant launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryCapabilities {
    managed_memory: bool,
    concurrent_managed_access: bool,
    pageable_memory_access: bool,
    virtual_memory_management: bool,
}

impl MemoryCapabilities {
    pub const fn managed_memory(self) -> bool {
        self.managed_memory
    }

    pub const fn concurrent_managed_access(self) -> bool {
        self.concurrent_managed_access
    }

    pub const fn pageable_memory_access(self) -> bool {
        self.pageable_memory_access
    }

    pub const fn virtual_memory_management(self) -> bool {
        self.virtual_memory_management
    }
}

/// One internally consistent observation of physical identity, context
/// identity, and memory capability bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryTopologyObservation {
    physical_device: PhysicalDeviceIdentity,
    context: ContextIdentity,
    capabilities: MemoryCapabilities,
}

impl MemoryTopologyObservation {
    pub const fn physical_device(self) -> PhysicalDeviceIdentity {
        self.physical_device
    }

    pub const fn context(self) -> ContextIdentity {
        self.context
    }

    pub const fn capabilities(self) -> MemoryCapabilities {
        self.capabilities
    }

    pub fn is_for_context(self, context: &Arc<GpuContext>) -> bool {
        self.context == context.identity()
    }

    pub(crate) fn new_allocation_identity(
        self,
        kind: AllocationKind,
        byte_len: usize,
    ) -> AllocationIdentity {
        AllocationIdentity {
            physical_device: self.physical_device,
            context: self.context,
            process_sequence: next_identity(&NEXT_ALLOCATION_ID, "allocation"),
            kind,
            byte_len,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(context: &Arc<GpuContext>, ordinal: i32) -> Self {
        assert_eq!(context.device_id(), ordinal);
        Self {
            physical_device: PhysicalDeviceIdentity {
                ordinal,
                uuid: [ordinal as u8 + 1; 16],
                pci_domain: 0,
                pci_bus: ordinal as u8,
                pci_device: 0,
                pci_function: 0,
            },
            context: context.identity(),
            capabilities: MemoryCapabilities {
                managed_memory: true,
                concurrent_managed_access: true,
                pageable_memory_access: false,
                virtual_memory_management: true,
            },
        }
    }
}

/// Native allocation category committed by an [`AllocationIdentity`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AllocationKind {
    Managed,
    VmmPhysical,
    VmmVirtualRange,
}

/// Exact process-local identity of one allocation or virtual reservation.
///
/// This record is copyable for comparison and logging. Ownership and cleanup
/// authority live only in the non-`Clone` allocation witnesses.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AllocationIdentity {
    physical_device: PhysicalDeviceIdentity,
    context: ContextIdentity,
    process_sequence: u64,
    kind: AllocationKind,
    byte_len: usize,
}

impl AllocationIdentity {
    pub const fn physical_device(self) -> PhysicalDeviceIdentity {
        self.physical_device
    }

    pub const fn context(self) -> ContextIdentity {
        self.context
    }

    pub const fn kind(self) -> AllocationKind {
        self.kind
    }

    pub const fn byte_len(self) -> usize {
        self.byte_len
    }
}

trait TopologyBackend {
    fn bind(&self, context: &GpuContext) -> Result<(), Error>;
    fn physical_identity(
        &self,
        device_id: i32,
    ) -> Result<fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity, Error>;
    fn capabilities(
        &self,
        device_id: i32,
    ) -> Result<fe2o3_hip_sys::Fe2o3HipMemoryCapabilities, Error>;
}

struct HipTopologyBackend;

impl TopologyBackend for HipTopologyBackend {
    fn bind(&self, context: &GpuContext) -> Result<(), Error> {
        context.bind_to_thread()
    }

    fn physical_identity(
        &self,
        device_id: i32,
    ) -> Result<fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity, Error> {
        let mut identity = fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity::default();
        check(unsafe {
            fe2o3_hip_sys::fe2o3HipGetPhysicalDeviceIdentity(device_id, &mut identity)
        })?;
        Ok(identity)
    }

    fn capabilities(
        &self,
        device_id: i32,
    ) -> Result<fe2o3_hip_sys::Fe2o3HipMemoryCapabilities, Error> {
        let mut capabilities = fe2o3_hip_sys::Fe2o3HipMemoryCapabilities::default();
        check(unsafe {
            fe2o3_hip_sys::fe2o3HipGetMemoryCapabilities(device_id, &mut capabilities)
        })?;
        Ok(capabilities)
    }
}

impl GpuContext {
    /// Queries exact physical identity and managed/VMM support for this context.
    ///
    /// Every native field is validated before an observation is returned. The
    /// observation is descriptive and grants no allocation, peer, mapping, or
    /// launch authority.
    pub fn observe_memory_topology(
        self: &Arc<Self>,
    ) -> Result<MemoryTopologyObservation, MemoryTopologyObservationError> {
        observe_memory_topology(self, &HipTopologyBackend)
    }
}

fn observe_memory_topology<B: TopologyBackend>(
    context: &Arc<GpuContext>,
    backend: &B,
) -> Result<MemoryTopologyObservation, MemoryTopologyObservationError> {
    backend
        .bind(context)
        .map_err(MemoryTopologyObservationError::Hip)?;
    let raw_identity = backend
        .physical_identity(context.device_id())
        .map_err(MemoryTopologyObservationError::Hip)?;
    let physical_device = parse_physical_identity(context.device_id(), raw_identity)?;
    let raw_capabilities = backend
        .capabilities(context.device_id())
        .map_err(MemoryTopologyObservationError::Hip)?;
    let capabilities = parse_capabilities(raw_capabilities)?;
    Ok(MemoryTopologyObservation {
        physical_device,
        context: context.identity(),
        capabilities,
    })
}

fn parse_physical_identity(
    ordinal: i32,
    raw: fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity,
) -> Result<PhysicalDeviceIdentity, MemoryTopologyObservationError> {
    if raw.uuid == [0; 16] {
        return Err(MemoryTopologyObservationError::InvalidPhysicalIdentity(
            "device UUID is all zero",
        ));
    }
    let nul = raw.pci_bus_id.iter().position(|byte| *byte == 0).ok_or(
        MemoryTopologyObservationError::InvalidPhysicalIdentity("PCI bus ID is not NUL terminated"),
    )?;
    let bytes: Vec<u8> = raw.pci_bus_id[..nul]
        .iter()
        .map(|byte| *byte as u8)
        .collect();
    let text = core::str::from_utf8(&bytes).map_err(|_| {
        MemoryTopologyObservationError::InvalidPhysicalIdentity("PCI bus ID is not UTF-8")
    })?;
    let (domain, bus, device, function) = parse_pci_address(text)?;
    Ok(PhysicalDeviceIdentity {
        ordinal,
        uuid: raw.uuid,
        pci_domain: domain,
        pci_bus: bus,
        pci_device: device,
        pci_function: function,
    })
}

fn parse_pci_address(value: &str) -> Result<(u16, u8, u8, u8), MemoryTopologyObservationError> {
    if value.len() != 12
        || value.as_bytes()[4] != b':'
        || value.as_bytes()[7] != b':'
        || value.as_bytes()[10] != b'.'
    {
        return Err(MemoryTopologyObservationError::InvalidPhysicalIdentity(
            "PCI bus ID is not canonical dddd:bb:dd.f",
        ));
    }
    let parse = |text: &str| {
        u16::from_str_radix(text, 16).map_err(|_| {
            MemoryTopologyObservationError::InvalidPhysicalIdentity(
                "PCI bus ID contains a non-hex component",
            )
        })
    };
    let domain = parse(&value[0..4])?;
    let bus = u8::try_from(parse(&value[5..7])?).expect("two hex digits fit u8");
    let device = u8::try_from(parse(&value[8..10])?).expect("two hex digits fit u8");
    let function = u8::try_from(parse(&value[11..12])?).expect("one hex digit fits u8");
    if device > 31 || function > 7 {
        return Err(MemoryTopologyObservationError::InvalidPhysicalIdentity(
            "PCI device or function exceeds its architectural range",
        ));
    }
    Ok((domain, bus, device, function))
}

fn parse_capabilities(
    raw: fe2o3_hip_sys::Fe2o3HipMemoryCapabilities,
) -> Result<MemoryCapabilities, MemoryTopologyObservationError> {
    let decode = |name, value| match value {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(MemoryTopologyObservationError::InvalidCapabilityValue { name, value }),
    };
    let capabilities = MemoryCapabilities {
        managed_memory: decode("managed memory", raw.managed_memory)?,
        concurrent_managed_access: decode(
            "concurrent managed access",
            raw.concurrent_managed_access,
        )?,
        pageable_memory_access: decode("pageable memory access", raw.pageable_memory_access)?,
        virtual_memory_management: decode(
            "virtual memory management",
            raw.virtual_memory_management,
        )?,
    };
    if capabilities.concurrent_managed_access && !capabilities.managed_memory {
        return Err(MemoryTopologyObservationError::InvalidCapabilityClosure(
            "concurrent managed access requires managed memory",
        ));
    }
    Ok(capabilities)
}

#[derive(Debug)]
#[non_exhaustive]
pub enum MemoryTopologyObservationError {
    Hip(Error),
    InvalidPhysicalIdentity(&'static str),
    InvalidCapabilityValue { name: &'static str, value: i32 },
    InvalidCapabilityClosure(&'static str),
}

impl fmt::Display for MemoryTopologyObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hip(error) => error.fmt(formatter),
            Self::InvalidPhysicalIdentity(reason) => {
                write!(
                    formatter,
                    "HIP returned an invalid physical device identity: {reason}"
                )
            }
            Self::InvalidCapabilityValue { name, value } => {
                write!(
                    formatter,
                    "HIP returned invalid {name} capability value {value}"
                )
            }
            Self::InvalidCapabilityClosure(reason) => {
                write!(
                    formatter,
                    "HIP returned inconsistent memory capabilities: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for MemoryTopologyObservationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockBackend {
        identity: fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity,
        capabilities: fe2o3_hip_sys::Fe2o3HipMemoryCapabilities,
        calls: Mutex<Vec<(&'static str, i32)>>,
    }

    impl TopologyBackend for MockBackend {
        fn bind(&self, context: &GpuContext) -> Result<(), Error> {
            self.calls
                .lock()
                .unwrap()
                .push(("bind", context.device_id()));
            Ok(())
        }

        fn physical_identity(
            &self,
            device_id: i32,
        ) -> Result<fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity, Error> {
            self.calls.lock().unwrap().push(("identity", device_id));
            Ok(self.identity)
        }

        fn capabilities(
            &self,
            device_id: i32,
        ) -> Result<fe2o3_hip_sys::Fe2o3HipMemoryCapabilities, Error> {
            self.calls.lock().unwrap().push(("capabilities", device_id));
            Ok(self.capabilities)
        }
    }

    fn raw_identity(pci: &str) -> fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity {
        let mut identity = fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity {
            uuid: [0x5a; 16],
            ..fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity::default()
        };
        for (destination, source) in identity.pci_bus_id.iter_mut().zip(pci.bytes()) {
            *destination = source as i8;
        }
        identity
    }

    fn backend() -> MockBackend {
        MockBackend {
            identity: raw_identity("0000:41:00.0"),
            capabilities: fe2o3_hip_sys::Fe2o3HipMemoryCapabilities {
                managed_memory: 1,
                concurrent_managed_access: 1,
                pageable_memory_access: 0,
                virtual_memory_management: 1,
            },
            calls: Mutex::new(Vec::new()),
        }
    }

    #[test]
    fn exact_observation_binds_physical_and_context_identity() {
        let context = Arc::new(GpuContext::for_test(3));
        let other = Arc::new(GpuContext::for_test(3));
        let backend = backend();

        let observed = observe_memory_topology(&context, &backend).unwrap();

        assert_eq!(observed.physical_device().ordinal(), 3);
        assert_eq!(observed.physical_device().uuid(), [0x5a; 16]);
        assert_eq!(observed.physical_device().pci_address(), (0, 0x41, 0, 0));
        assert!(observed.is_for_context(&context));
        assert!(!observed.is_for_context(&other));
        assert_eq!(
            *backend.calls.lock().unwrap(),
            [("bind", 3), ("identity", 3), ("capabilities", 3)]
        );
    }

    #[test]
    fn malformed_physical_identities_fail_before_capability_query() {
        for identity in [
            fe2o3_hip_sys::Fe2o3HipPhysicalDeviceIdentity::default(),
            raw_identity("0000:41:20.0"),
            raw_identity("0000:41:00.8"),
            raw_identity("not-a-pci-id"),
        ] {
            let mut backend = backend();
            backend.identity = identity;
            let error =
                observe_memory_topology(&Arc::new(GpuContext::for_test(0)), &backend).unwrap_err();
            assert!(matches!(
                error,
                MemoryTopologyObservationError::InvalidPhysicalIdentity(_)
            ));
            assert_eq!(backend.calls.lock().unwrap().len(), 2);
        }
    }

    #[test]
    fn non_boolean_and_inconsistent_capabilities_fail_closed() {
        let context = Arc::new(GpuContext::for_test(0));
        let mut malformed = backend();
        malformed.capabilities.managed_memory = 7;
        assert!(matches!(
            observe_memory_topology(&context, &malformed),
            Err(MemoryTopologyObservationError::InvalidCapabilityValue {
                name: "managed memory",
                value: 7
            })
        ));

        let mut inconsistent = backend();
        inconsistent.capabilities.managed_memory = 0;
        assert!(matches!(
            observe_memory_topology(&context, &inconsistent),
            Err(MemoryTopologyObservationError::InvalidCapabilityClosure(_))
        ));
    }

    #[test]
    fn canonical_pci_parser_rejects_trailing_or_non_hex_input() {
        for value in [
            "0000:41:00.0x",
            "0000:4g:00.0",
            "000:41:00.0",
            "0000-41-00-0",
        ] {
            assert!(parse_pci_address(value).is_err(), "accepted {value}");
        }
    }
}
