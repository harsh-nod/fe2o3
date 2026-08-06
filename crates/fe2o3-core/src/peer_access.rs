use crate::{Error, GpuContext, Result, check};
use core::fmt;
use std::sync::Arc;

trait PeerBackend {
    fn bind(&self, context: &GpuContext) -> Result<()>;
    fn can_access(&self, source: i32, destination: i32) -> Result<i32>;
    fn enable(&self, destination: i32) -> Result<EnableDisposition>;
    fn disable(&self, destination: i32) -> Result<DisableDisposition>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnableDisposition {
    Enabled,
    AlreadyEnabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisableDisposition {
    Disabled,
    AlreadyDisabled,
}

struct HipPeerBackend;

impl PeerBackend for HipPeerBackend {
    fn bind(&self, context: &GpuContext) -> Result<()> {
        context.bind_to_thread()
    }

    fn can_access(&self, source: i32, destination: i32) -> Result<i32> {
        let mut accessible = 0;
        check(unsafe {
            fe2o3_hip_sys::hipDeviceCanAccessPeer(&mut accessible, source, destination)
        })?;
        Ok(accessible)
    }

    fn enable(&self, destination: i32) -> Result<EnableDisposition> {
        let status = unsafe {
            fe2o3_hip_sys::hipDeviceEnablePeerAccess(
                destination,
                fe2o3_hip_sys::HIP_PEER_ACCESS_DEFAULT,
            )
        };
        match status {
            fe2o3_hip_sys::HIP_SUCCESS => Ok(EnableDisposition::Enabled),
            fe2o3_hip_sys::HIP_ERROR_PEER_ACCESS_ALREADY_ENABLED => {
                Ok(EnableDisposition::AlreadyEnabled)
            }
            status => {
                check(status)?;
                unreachable!("non-success HIP status passed check")
            }
        }
    }

    fn disable(&self, destination: i32) -> Result<DisableDisposition> {
        let status = unsafe { fe2o3_hip_sys::hipDeviceDisablePeerAccess(destination) };
        match status {
            fe2o3_hip_sys::HIP_SUCCESS => Ok(DisableDisposition::Disabled),
            fe2o3_hip_sys::HIP_ERROR_PEER_ACCESS_NOT_ENABLED => {
                Ok(DisableDisposition::AlreadyDisabled)
            }
            status => {
                check(status)?;
                unreachable!("non-success HIP status passed check")
            }
        }
    }
}

/// Directional identity of one observed peer-access relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerAccessDirection {
    source_device: i32,
    destination_device: i32,
}

impl PeerAccessDirection {
    pub const fn source_device(self) -> i32 {
        self.source_device
    }

    pub const fn destination_device(self) -> i32 {
        self.destination_device
    }
}

/// Live, directional evidence that HIP reported peer reachability between two
/// exact retained context wrappers.
///
/// Reachability is not enablement and this type grants no pointer, copy,
/// coherence, aliasing, synchronization, or kernel authority.
#[derive(Debug)]
pub struct PeerAccessCapability {
    source: Arc<GpuContext>,
    destination: Arc<GpuContext>,
}

impl PeerAccessCapability {
    pub fn direction(&self) -> PeerAccessDirection {
        direction(&self.source, &self.destination)
    }

    pub fn is_for(&self, source: &Arc<GpuContext>, destination: &Arc<GpuContext>) -> bool {
        Arc::ptr_eq(&self.source, source) && Arc::ptr_eq(&self.destination, destination)
    }

    /// Enables the exact observed direction.
    ///
    /// A duplicate HIP enable is reported as an error and does not mint a
    /// second cleanup owner. This prevents one safe token from disabling a
    /// mapping still represented by another safe token.
    pub fn enable(self) -> std::result::Result<PeerAccess, PeerAccessEnableError> {
        enable_peer_access(self, &HipPeerBackend)
    }

    fn leak_contexts(&self) {
        core::mem::forget(self.source.clone());
        core::mem::forget(self.destination.clone());
    }
}

impl GpuContext {
    /// Observes directional peer reachability between two exact live context
    /// wrappers. Equal device ordinals are rejected before consulting HIP.
    pub fn observe_peer_access(
        self: &Arc<Self>,
        destination: &Arc<Self>,
    ) -> std::result::Result<PeerAccessCapability, PeerAccessObservationError> {
        observe_peer_access(self, destination, &HipPeerBackend)
    }
}

fn observe_peer_access<B: PeerBackend>(
    source: &Arc<GpuContext>,
    destination: &Arc<GpuContext>,
    backend: &B,
) -> std::result::Result<PeerAccessCapability, PeerAccessObservationError> {
    let direction = direction(source, destination);
    if direction.source_device == direction.destination_device {
        return Err(PeerAccessObservationError::SameDevice { direction });
    }
    backend
        .bind(source)
        .map_err(PeerAccessObservationError::Hip)?;
    match backend
        .can_access(direction.source_device, direction.destination_device)
        .map_err(PeerAccessObservationError::Hip)?
    {
        1 => Ok(PeerAccessCapability {
            source: source.clone(),
            destination: destination.clone(),
        }),
        0 => Err(PeerAccessObservationError::Unavailable { direction }),
        value => Err(PeerAccessObservationError::InvalidCapabilityValue { direction, value }),
    }
}

fn enable_peer_access<B: PeerBackend>(
    capability: PeerAccessCapability,
    backend: &B,
) -> std::result::Result<PeerAccess, PeerAccessEnableError> {
    let direction = capability.direction();
    backend
        .bind(&capability.source)
        .map_err(PeerAccessEnableError::Hip)?;
    match backend.enable(direction.destination_device) {
        Ok(EnableDisposition::Enabled) => Ok(PeerAccess {
            source: Some(capability.source),
            destination: Some(capability.destination),
            active: true,
        }),
        Ok(EnableDisposition::AlreadyEnabled) => {
            Err(PeerAccessEnableError::AlreadyEnabled { direction })
        }
        Err(error) => {
            // HIP did not establish whether native state changed. Keep both
            // exact contexts alive and issue no safe authority.
            capability.leak_contexts();
            Err(PeerAccessEnableError::Hip(error))
        }
    }
}

/// Exclusive cleanup ownership for one successfully enabled directional HIP
/// peer mapping.
///
/// This token is neither `Clone` nor `Copy`. It does not authorize memory
/// dereferences or cross-device operations. Dropping it attempts disablement.
/// If HIP cannot establish cleanup, the retained context ownership is leaked so
/// destruction cannot race an ambiguously live native mapping.
#[derive(Debug)]
#[must_use = "dropping peer access attempts to disable the native mapping"]
pub struct PeerAccess {
    source: Option<Arc<GpuContext>>,
    destination: Option<Arc<GpuContext>>,
    active: bool,
}

impl PeerAccess {
    pub fn direction(&self) -> PeerAccessDirection {
        direction(
            self.source.as_ref().expect("active peer access has source"),
            self.destination
                .as_ref()
                .expect("active peer access has destination"),
        )
    }

    /// Explicitly disables this mapping and reports whether HIP had already
    /// observed it disabled.
    pub fn disable(
        mut self,
    ) -> std::result::Result<PeerAccessCleanupOutcome, PeerAccessCleanupError> {
        self.cleanup(&HipPeerBackend)
    }

    fn cleanup<B: PeerBackend>(
        &mut self,
        backend: &B,
    ) -> std::result::Result<PeerAccessCleanupOutcome, PeerAccessCleanupError> {
        debug_assert!(self.active);
        let direction = self.direction();
        self.active = false;

        if let Err(error) = backend.bind(self.source.as_ref().expect("peer access has source")) {
            self.leak_contexts();
            return Err(PeerAccessCleanupError { direction, error });
        }

        match backend.disable(direction.destination_device) {
            Ok(DisableDisposition::Disabled) => Ok(PeerAccessCleanupOutcome::Disabled),
            Ok(DisableDisposition::AlreadyDisabled) => {
                Ok(PeerAccessCleanupOutcome::AlreadyDisabled)
            }
            Err(error) => {
                self.leak_contexts();
                Err(PeerAccessCleanupError { direction, error })
            }
        }
    }

    fn leak_contexts(&self) {
        if let Some(source) = &self.source {
            core::mem::forget(source.clone());
        }
        if let Some(destination) = &self.destination {
            core::mem::forget(destination.clone());
        }
    }
}

impl Drop for PeerAccess {
    fn drop(&mut self) {
        if self.active {
            let _ = self.cleanup(&HipPeerBackend);
        }
    }
}

fn direction(source: &GpuContext, destination: &GpuContext) -> PeerAccessDirection {
    PeerAccessDirection {
        source_device: source.device_id(),
        destination_device: destination.device_id(),
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PeerAccessObservationError {
    SameDevice {
        direction: PeerAccessDirection,
    },
    Unavailable {
        direction: PeerAccessDirection,
    },
    InvalidCapabilityValue {
        direction: PeerAccessDirection,
        value: i32,
    },
    Hip(Error),
}

impl fmt::Display for PeerAccessObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameDevice { direction } => write!(
                formatter,
                "peer access requires distinct devices, but both endpoints name HIP device {}",
                direction.source_device
            ),
            Self::Unavailable { direction } => write!(
                formatter,
                "HIP device {} cannot access peer device {}",
                direction.source_device, direction.destination_device
            ),
            Self::InvalidCapabilityValue { direction, value } => write!(
                formatter,
                "HIP returned invalid peer capability {value} for device {} -> {}",
                direction.source_device, direction.destination_device
            ),
            Self::Hip(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PeerAccessObservationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PeerAccessEnableError {
    AlreadyEnabled { direction: PeerAccessDirection },
    Hip(Error),
}

impl fmt::Display for PeerAccessEnableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyEnabled { direction } => write!(
                formatter,
                "peer access {} -> {} was already enabled; no cleanup authority was issued",
                direction.source_device, direction.destination_device
            ),
            Self::Hip(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PeerAccessEnableError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip(error) => Some(error),
            Self::AlreadyEnabled { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerAccessCleanupOutcome {
    Disabled,
    AlreadyDisabled,
}

#[derive(Debug)]
pub struct PeerAccessCleanupError {
    direction: PeerAccessDirection,
    error: Error,
}

impl PeerAccessCleanupError {
    pub const fn direction(&self) -> PeerAccessDirection {
        self.direction
    }
}

impl fmt::Display for PeerAccessCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "peer access cleanup for device {} -> {} is ambiguous: {}",
            self.direction.source_device, self.direction.destination_device, self.error
        )
    }
}

impl std::error::Error for PeerAccessCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[derive(Debug, Eq, PartialEq)]
    enum Call {
        Bind(i32),
        Query(i32, i32),
        Enable(i32),
        Disable(i32),
    }

    struct MockBackend {
        bind: Result<()>,
        query: Result<i32>,
        enable: Result<EnableDisposition>,
        disables: Mutex<VecDeque<Result<DisableDisposition>>>,
        calls: Mutex<Vec<Call>>,
    }

    impl PeerBackend for MockBackend {
        fn bind(&self, context: &GpuContext) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Bind(context.device_id()));
            match &self.bind {
                Ok(()) => Ok(()),
                Err(_) => Err(Error::SizeOverflow),
            }
        }

        fn can_access(&self, source: i32, destination: i32) -> Result<i32> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Query(source, destination));
            match &self.query {
                Ok(value) => Ok(*value),
                Err(_) => Err(Error::SizeOverflow),
            }
        }

        fn enable(&self, destination: i32) -> Result<EnableDisposition> {
            self.calls.lock().unwrap().push(Call::Enable(destination));
            match &self.enable {
                Ok(value) => Ok(*value),
                Err(_) => Err(Error::SizeOverflow),
            }
        }

        fn disable(&self, destination: i32) -> Result<DisableDisposition> {
            self.calls.lock().unwrap().push(Call::Disable(destination));
            self.disables
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(DisableDisposition::Disabled))
        }
    }

    fn backend(query: i32) -> MockBackend {
        MockBackend {
            bind: Ok(()),
            query: Ok(query),
            enable: Ok(EnableDisposition::Enabled),
            disables: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn context(device: i32) -> Arc<GpuContext> {
        Arc::new(GpuContext::for_test(device))
    }

    #[test]
    fn observation_is_directional_and_binds_exact_wrappers() {
        let source = context(1);
        let destination = context(3);
        let other_source = context(1);
        let backend = backend(1);

        let capability = observe_peer_access(&source, &destination, &backend).unwrap();

        assert_eq!(
            capability.direction(),
            PeerAccessDirection {
                source_device: 1,
                destination_device: 3,
            }
        );
        assert!(capability.is_for(&source, &destination));
        assert!(!capability.is_for(&other_source, &destination));
        assert_eq!(
            *backend.calls.lock().unwrap(),
            [Call::Bind(1), Call::Query(1, 3)]
        );
    }

    #[test]
    fn reverse_direction_requires_its_own_observation() {
        let left = context(1);
        let right = context(3);
        let backend = backend(1);

        let capability = observe_peer_access(&right, &left, &backend).unwrap();

        assert_eq!(capability.direction().source_device(), 3);
        assert_eq!(capability.direction().destination_device(), 1);
    }

    #[test]
    fn same_device_never_reaches_backend() {
        let backend = backend(1);
        let error = observe_peer_access(&context(2), &context(2), &backend).unwrap_err();

        assert!(matches!(
            error,
            PeerAccessObservationError::SameDevice { .. }
        ));
        assert!(backend.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn unavailable_malformed_and_query_failure_fail_closed() {
        let source = context(0);
        let destination = context(1);
        assert!(matches!(
            observe_peer_access(&source, &destination, &backend(0)),
            Err(PeerAccessObservationError::Unavailable { .. })
        ));
        assert!(matches!(
            observe_peer_access(&source, &destination, &backend(7)),
            Err(PeerAccessObservationError::InvalidCapabilityValue { value: 7, .. })
        ));
        let mut failed = backend(1);
        failed.query = Err(Error::SizeOverflow);
        assert!(matches!(
            observe_peer_access(&source, &destination, &failed),
            Err(PeerAccessObservationError::Hip(Error::SizeOverflow))
        ));
    }

    #[test]
    fn successful_enable_owns_exactly_one_disable() {
        let source = context(0);
        let destination = context(1);
        let backend = backend(1);
        let capability = observe_peer_access(&source, &destination, &backend).unwrap();

        let mut access = enable_peer_access(capability, &backend).unwrap();
        let outcome = access.cleanup(&backend).unwrap();

        assert_eq!(outcome, PeerAccessCleanupOutcome::Disabled);
        assert_eq!(
            *backend.calls.lock().unwrap(),
            [
                Call::Bind(0),
                Call::Query(0, 1),
                Call::Bind(0),
                Call::Enable(1),
                Call::Bind(0),
                Call::Disable(1),
            ]
        );
        drop(access);
        assert_eq!(backend.calls.lock().unwrap().len(), 6);
    }

    #[test]
    fn duplicate_enable_never_mints_cleanup_owner() {
        let source = context(0);
        let destination = context(1);
        let mut backend = backend(1);
        backend.enable = Ok(EnableDisposition::AlreadyEnabled);
        let capability = observe_peer_access(&source, &destination, &backend).unwrap();

        let error = enable_peer_access(capability, &backend).unwrap_err();

        assert!(matches!(
            error,
            PeerAccessEnableError::AlreadyEnabled { .. }
        ));
        assert_eq!(
            *backend.calls.lock().unwrap(),
            [
                Call::Bind(0),
                Call::Query(0, 1),
                Call::Bind(0),
                Call::Enable(1),
            ]
        );
    }

    #[test]
    fn context_binding_failures_stop_before_each_native_operation() {
        let source = context(0);
        let destination = context(1);
        let failed = MockBackend {
            bind: Err(Error::SizeOverflow),
            query: Ok(1),
            enable: Ok(EnableDisposition::Enabled),
            disables: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        };

        assert!(matches!(
            observe_peer_access(&source, &destination, &failed),
            Err(PeerAccessObservationError::Hip(Error::SizeOverflow))
        ));
        assert_eq!(*failed.calls.lock().unwrap(), [Call::Bind(0)]);

        let observed = backend(1);
        let capability = observe_peer_access(&source, &destination, &observed).unwrap();
        assert!(matches!(
            enable_peer_access(capability, &failed),
            Err(PeerAccessEnableError::Hip(Error::SizeOverflow))
        ));
        assert_eq!(
            *failed.calls.lock().unwrap(),
            [Call::Bind(0), Call::Bind(0)]
        );

        let capability = observe_peer_access(&source, &destination, &observed).unwrap();
        let mut access = enable_peer_access(capability, &observed).unwrap();
        let error = access.cleanup(&failed).unwrap_err();
        assert_eq!(error.direction().destination_device(), 1);
        assert_eq!(
            *failed.calls.lock().unwrap(),
            [Call::Bind(0), Call::Bind(0), Call::Bind(0)]
        );
    }

    #[test]
    fn enable_failure_issues_no_cleanup_authority() {
        let source = context(0);
        let destination = context(1);
        let mut backend = backend(1);
        backend.enable = Err(Error::SizeOverflow);
        let capability = observe_peer_access(&source, &destination, &backend).unwrap();

        let error = enable_peer_access(capability, &backend).unwrap_err();

        assert!(matches!(
            error,
            PeerAccessEnableError::Hip(Error::SizeOverflow)
        ));
        assert!(
            backend
                .calls
                .lock()
                .unwrap()
                .iter()
                .all(|call| !matches!(call, Call::Disable(_)))
        );
    }

    #[test]
    fn already_disabled_is_a_defined_terminal_outcome() {
        let source = context(0);
        let destination = context(1);
        let backend = MockBackend {
            bind: Ok(()),
            query: Ok(1),
            enable: Ok(EnableDisposition::Enabled),
            disables: Mutex::new(VecDeque::from([Ok(DisableDisposition::AlreadyDisabled)])),
            calls: Mutex::new(Vec::new()),
        };
        let capability = observe_peer_access(&source, &destination, &backend).unwrap();
        let mut access = enable_peer_access(capability, &backend).unwrap();

        assert_eq!(
            access.cleanup(&backend).unwrap(),
            PeerAccessCleanupOutcome::AlreadyDisabled
        );
    }

    #[test]
    fn cleanup_failure_is_ambiguous_and_not_retried() {
        let source = context(0);
        let destination = context(1);
        let backend = MockBackend {
            bind: Ok(()),
            query: Ok(1),
            enable: Ok(EnableDisposition::Enabled),
            disables: Mutex::new(VecDeque::from([Err(Error::SizeOverflow)])),
            calls: Mutex::new(Vec::new()),
        };
        let capability = observe_peer_access(&source, &destination, &backend).unwrap();
        let mut access = enable_peer_access(capability, &backend).unwrap();

        let error = access.cleanup(&backend).unwrap_err();
        assert_eq!(error.direction().source_device(), 0);
        assert!(error.to_string().contains("ambiguous"));
        drop(access);
        assert_eq!(
            backend
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|call| matches!(call, Call::Disable(_)))
                .count(),
            1
        );
    }
}
