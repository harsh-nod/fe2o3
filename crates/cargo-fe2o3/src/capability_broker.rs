//! Descriptor transport from the `cargo-fe2o3` parent to managed rustc wrappers.
//!
//! Cargo receives a strict per-instance route and build-session binding. Both peers check Linux
//! credentials, exact executable identity, the prepared profile/config identity, and a
//! challenge-response bound to a separate 256-bit broker secret before transferring a
//! sealed backend image and a read-only artifact-directory descriptor with `SCM_RIGHTS`. A
//! protected release also receives one sealed descriptor carrying the complete admitted
//! compiler-closure preimage.
//! Receivers validate the exact profile-specific descriptor count and positional types before
//! installing capabilities in the caller-selected compiler process for a compile-shaped wrapper
//! invocation.
//!
//! An independent seccomp exec boundary additionally grants a one-use broker permit only to a
//! direct Cargo child stopped while requesting the pinned wrapper image. Inherited route material
//! therefore cannot authorize build-script or procedural-macro replay. A procedural macro still
//! executes inside an already-authorized rustc process and can observe that compilation's
//! descriptors. The directory is opened `O_RDONLY`, but still grants descriptor-relative namespace
//! mutation. The
//! receiver treats that route as untrusted: before connecting, it independently observes its own
//! running `cargo-fe2o3` image and requires the advertised broker to have the same uid, executable
//! object, and bytes. This closes a self-consistent route redirected to an arbitrary mock
//! executable. A substitute running the same executable object and bytes remains inside the
//! executable-authentication boundary, but it has no public broker-server entry point and must
//! still possess a kernel-observed one-use invocation permit. This is not a sandbox against hostile
//! same-user code that can ptrace or inject into another process; untrusted build dependencies
//! require a separate process sandbox.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod platform {
    use std::collections::BTreeMap;
    use std::fs::{self, File};
    use std::io::{self, IoSlice, IoSliceMut, Read, Write};
    use std::mem::MaybeUninit;
    use std::net::Shutdown;
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use fe2o3_artifact_transaction::{
        BROKERED_INVOCATION_ADMITTED_V1, BROKERED_INVOCATION_PREPARED_V1,
        BROKERED_INVOCATION_REQUEST_BYTES_V1, BROKERED_INVOCATION_REQUEST_BYTES_V2,
        BrokeredInvocationCapabilityRequestV1, BrokeredInvocationCapabilityRequestV2, BuildAttempt,
        BuildSession,
    };
    use fe2o3_process_identity::LinuxObjectIdentityV3;
    use rustix::net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
    };
    use sha2::{Digest, Sha256};

    use crate::build_config::ProductionSourceIsaObserverPolicyV1;
    use crate::cargo_invocation_boundary::{InvocationAuthorizationRegistryV1, ProcessIdentityV1};
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
    use crate::project::PinnedDirectory;
    use fe2o3_compiler_closure_capability::{
        CompilerClosureCapabilityV1, CompilerExecutionClientProfileCapabilityV1,
    };
    use fe2o3_source_isa_observation::wire_v1::{
        SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1, SourceIsaObservationCollectionV1,
        SourceIsaObservationFrameV1, SourceIsaObservationTransportFailureV1,
    };

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";
    const REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-V3\0";
    const ROUTE_PREFIX: &str = "fe2o3-capability-route-v3";
    const ENDPOINT_BYTES: usize = 32;
    const ENDPOINT_HEX_BYTES: usize = ENDPOINT_BYTES * 2;
    const SECRET_BYTES: usize = 32;
    const CHALLENGE_BYTES: usize = 32;
    const CONFIG_ID_BYTES: usize = 32;
    const COMPILER_CLOSURE_ID_BYTES: usize = 32;
    const RUSTC_EXECUTABLE_ID_BYTES: usize = 32;
    const RETAINED_OBJECT_BINDING_BYTES: usize = 32;
    const REQUEST_AUTH_BYTES: usize = 32;
    const REQUEST_BYTES: usize = REQUEST_MAGIC.len()
        + 16
        + 1
        + CONFIG_ID_BYTES
        + 1
        + COMPILER_CLOSURE_ID_BYTES
        + RUSTC_EXECUTABLE_ID_BYTES
        + RETAINED_OBJECT_BINDING_BYTES
        + CHALLENGE_BYTES
        + REQUEST_AUTH_BYTES;
    const RESPONSE_BYTES: usize = 1 + REQUEST_AUTH_BYTES;
    const REQUEST_AUTH_DOMAIN: &[u8] = b"FE2O3/CAPABILITY-BROKER/REQUEST-AUTH/V3\0";
    const RESPONSE_AUTH_DOMAIN: &[u8] = b"FE2O3/CAPABILITY-BROKER/RESPONSE-AUTH/V3\0";
    const MAX_PROC_STAT_BYTES: usize = 4096;
    const EXECUTABLE_PIN_ATTEMPTS: usize = 8;
    const RECEIVED_DESCRIPTOR_FLOOR: i32 = 210;
    const BROKER_AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(30);
    const BROKER_CLIENT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
    const _: () = assert!(
        BROKER_CLIENT_RESPONSE_TIMEOUT.as_secs()
            >= BROKER_AUTHENTICATION_TIMEOUT.as_secs().saturating_mul(2)
    );
    const BROKER_INVOCATION_FRAME_TIMEOUT: Duration = Duration::from_secs(30);
    const BROKER_INVOCATION_LIFETIME: Duration = Duration::from_secs(6 * 60 * 60);
    const MAX_ACTIVE_CONNECTIONS: usize = 64;
    const MAX_CONCURRENT_AUTHENTICATIONS: usize = 8;
    const MAX_SOURCE_ISA_OBSERVATION_UNITS_V1: usize = 1024;
    const MAX_SOURCE_ISA_OBSERVATION_AGGREGATE_BYTES_V1: usize = 4 * 1024 * 1024;
    const _: () = assert!(
        fe2o3_source_isa_observation::wire_v1::MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1
            <= MAX_SOURCE_ISA_OBSERVATION_AGGREGATE_BYTES_V1
    );
    const BROKERED_INVOCATION_REQUEST_MAGIC_V1: &[u8; 8] = b"F2BRKIV1";
    const BROKERED_INVOCATION_REQUEST_MAGIC_V2: &[u8; 8] = b"F2BRKIV2";
    const BROKERED_SOURCE_ISA_PREPARED_V1: &[u8; 16] = b"F2SI-PREPARED-V1";

    #[derive(Clone, Copy)]
    struct BrokerLimits {
        max_active_connections: usize,
        authentication_timeout: Duration,
        invocation_frame_timeout: Duration,
        invocation_lifetime: Duration,
    }

    const PRODUCTION_BROKER_LIMITS: BrokerLimits = BrokerLimits {
        max_active_connections: MAX_ACTIVE_CONNECTIONS,
        authentication_timeout: BROKER_AUTHENTICATION_TIMEOUT,
        invocation_frame_timeout: BROKER_INVOCATION_FRAME_TIMEOUT,
        invocation_lifetime: BROKER_INVOCATION_LIFETIME,
    };

    #[derive(Clone, Copy)]
    struct BrokerCompilerCapabilities<'profile> {
        closure: Option<fe2o3_build_authority::CompilerClosureV2>,
        execution_profile: Option<&'profile CompilerExecutionClientProfileCapabilityV1>,
    }

    #[derive(Clone)]
    struct BrokerSourceIsaObserverV1 {
        config_identity: [u8; 32],
        session: BuildSession,
        selected_units: Vec<[u8; 32]>,
        collector: Arc<Mutex<SourceIsaObservationCollectorStateV1>>,
    }

    impl BrokerSourceIsaObserverV1 {
        fn from_policy(
            policy: &ProductionSourceIsaObserverPolicyV1,
            session: BuildSession,
        ) -> Result<Self, String> {
            if session == BuildSession::DIRECT {
                return Err("source/ISA observer requires a managed build session".to_owned());
            }
            let unit_count = policy.selected_units().len();
            if unit_count == 0 || unit_count > MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 {
                return Err(format!(
                    "source/ISA observer requires 1..={MAX_SOURCE_ISA_OBSERVATION_UNITS_V1} exact units"
                ));
            }
            let mut selected_units = Vec::new();
            selected_units.try_reserve_exact(unit_count).map_err(|_| {
                "cannot allocate the bounded source/ISA observer broker policy".to_owned()
            })?;
            selected_units.extend(
                policy
                    .selected_units()
                    .iter()
                    .map(|identity| *identity.as_bytes()),
            );
            if selected_units.contains(&[0; 32])
                || selected_units.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err("source/ISA observer broker policy is not canonical".to_owned());
            }
            let config_identity = *policy.config_identity().as_bytes();
            let collector = SourceIsaObservationCollectorStateV1::with_expected_context(
                config_identity,
                session,
                &selected_units,
            )?;
            Ok(Self {
                config_identity,
                session,
                selected_units,
                collector: Arc::new(Mutex::new(collector)),
            })
        }

        fn accepts(&self, request: BrokeredInvocationCapabilityRequestV2) -> bool {
            request.config_identity() == self.config_identity
                && self
                    .selected_units
                    .binary_search(&request.unit_identity())
                    .is_ok()
        }

        fn collect(&self, frame: SourceIsaObservationFrameV1) -> io::Result<()> {
            let context = frame.context();
            if context.config() != self.config_identity
                || context.attempt().session()
                    != crate::source_isa_observation::inert_source_isa_session_v1(self.session)
                || self.selected_units.binary_search(&context.unit()).is_err()
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "source/ISA observation frame is not bound to the configured unit",
                ));
            }
            self.collector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(frame)
                .map_err(io::Error::other)
        }

        fn fail(&self, reason: SourceIsaObservationTransportFailureV1) {
            self.collector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail(reason);
        }
    }

    struct SourceIsaObservationCollectorStateV1 {
        config_identity: [u8; 32],
        session: BuildSession,
        frames: Vec<([u8; 32], SourceIsaObservationFrameV1)>,
        expected_units: Vec<[u8; 32]>,
        aggregate_bytes: usize,
        failure: Option<SourceIsaObservationTransportFailureV1>,
    }

    impl SourceIsaObservationCollectorStateV1 {
        fn with_expected_context(
            config_identity: [u8; 32],
            session: BuildSession,
            expected: &[[u8; 32]],
        ) -> Result<Self, String> {
            if config_identity == [0; 32] || session == BuildSession::DIRECT {
                return Err("source/ISA collector requires exact nonzero context".to_owned());
            }
            let mut frames = Vec::new();
            frames.try_reserve_exact(expected.len()).map_err(|_| {
                "cannot allocate the bounded source/ISA observation collector".to_owned()
            })?;
            let mut expected_units = Vec::new();
            expected_units
                .try_reserve_exact(expected.len())
                .map_err(|_| {
                    "cannot allocate the bounded source/ISA expected-unit set".to_owned()
                })?;
            expected_units.extend_from_slice(expected);
            Ok(Self {
                config_identity,
                session,
                frames,
                expected_units,
                aggregate_bytes: 0,
                failure: None,
            })
        }

        fn insert(
            &mut self,
            frame: SourceIsaObservationFrameV1,
        ) -> Result<(), SourceIsaObservationTransportFailureV1> {
            if frame.context().config() != self.config_identity
                || frame.context().attempt().session()
                    != crate::source_isa_observation::inert_source_isa_session_v1(self.session)
            {
                self.fail(SourceIsaObservationTransportFailureV1::RejectedFrame);
                return Err(SourceIsaObservationTransportFailureV1::RejectedFrame);
            }
            let unit = frame.context().unit();
            let insertion = match self.frames.binary_search_by_key(&unit, |(unit, _)| *unit) {
                Ok(index) if self.frames[index].1 == frame => return Ok(()),
                Ok(_) => {
                    self.fail(SourceIsaObservationTransportFailureV1::ConflictingDuplicate);
                    return Err(SourceIsaObservationTransportFailureV1::ConflictingDuplicate);
                }
                Err(insertion) => insertion,
            };
            if self.frames.len() >= MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 {
                self.fail(SourceIsaObservationTransportFailureV1::UnitBound);
                return Err(SourceIsaObservationTransportFailureV1::UnitBound);
            }
            let Some(aggregate_bytes) = self
                .aggregate_bytes
                .checked_add(SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1)
                .filter(|bytes| *bytes <= MAX_SOURCE_ISA_OBSERVATION_AGGREGATE_BYTES_V1)
            else {
                self.fail(SourceIsaObservationTransportFailureV1::AggregateByteBound);
                return Err(SourceIsaObservationTransportFailureV1::AggregateByteBound);
            };
            self.frames.insert(insertion, (unit, frame));
            self.aggregate_bytes = aggregate_bytes;
            Ok(())
        }

        fn fail(&mut self, reason: SourceIsaObservationTransportFailureV1) {
            self.failure.get_or_insert(reason);
        }

        fn finish(mut self) -> SourceIsaObservationCollectionV1 {
            self.expected_units.retain(|unit| {
                self.frames
                    .binary_search_by_key(unit, |(observed, _)| *observed)
                    .is_err()
            });
            if !self.expected_units.is_empty() && self.failure.is_none() {
                self.failure = Some(SourceIsaObservationTransportFailureV1::MissingSelectedUnits);
            }
            SourceIsaObservationCollectionV1::from_collected(
                self.config_identity,
                crate::source_isa_observation::inert_source_isa_session_v1(self.session),
                self.frames,
                self.expected_units,
                self.failure,
            )
        }
    }

    impl<'profile> BrokerCompilerCapabilities<'profile> {
        const fn ordinary() -> Self {
            Self {
                closure: None,
                execution_profile: None,
            }
        }

        const fn protected(
            closure: fe2o3_build_authority::CompilerClosureV2,
            execution_profile: &'profile CompilerExecutionClientProfileCapabilityV1,
        ) -> Self {
            Self {
                closure: Some(closure),
                execution_profile: Some(execution_profile),
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CapabilityProfileV1 {
        Ordinary,
    }

    impl CapabilityProfileV1 {
        const fn request_magic(self) -> &'static [u8] {
            match self {
                Self::Ordinary => REQUEST_MAGIC,
            }
        }

        const fn descriptor_count(self) -> usize {
            match self {
                Self::Ordinary => 2,
            }
        }

        const fn name(self) -> &'static str {
            match self {
                Self::Ordinary => "ordinary",
            }
        }

        const fn route_name(self) -> &'static str {
            match self {
                Self::Ordinary => "ordinary",
            }
        }

        fn parse_route_name(value: &str) -> Option<Self> {
            match value {
                "ordinary" => Some(Self::Ordinary),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct CapabilityBindingV3 {
        profile: CapabilityProfileV1,
        config_identity: Option<[u8; CONFIG_ID_BYTES]>,
        protected_compiler_closure_v2: bool,
        compiler_closure_sha256: [u8; COMPILER_CLOSURE_ID_BYTES],
        rustc_executable_sha256: [u8; RUSTC_EXECUTABLE_ID_BYTES],
        retained_object_binding_sha256: [u8; RETAINED_OBJECT_BINDING_BYTES],
    }

    impl CapabilityBindingV3 {
        pub(crate) fn new(
            profile: CapabilityProfileV1,
            config_identity: Option<[u8; CONFIG_ID_BYTES]>,
            compiler_closure_sha256: [u8; COMPILER_CLOSURE_ID_BYTES],
            rustc_executable_sha256: [u8; RUSTC_EXECUTABLE_ID_BYTES],
            retained_object_binding_sha256: [u8; RETAINED_OBJECT_BINDING_BYTES],
        ) -> Result<Self, String> {
            if compiler_closure_sha256 == [0; COMPILER_CLOSURE_ID_BYTES]
                || rustc_executable_sha256 == [0; RUSTC_EXECUTABLE_ID_BYTES]
                || retained_object_binding_sha256 == [0; RETAINED_OBJECT_BINDING_BYTES]
            {
                return Err("capability binding identities must be nonzero".into());
            }
            Ok(Self {
                profile,
                config_identity,
                protected_compiler_closure_v2: false,
                compiler_closure_sha256,
                rustc_executable_sha256,
                retained_object_binding_sha256,
            })
        }

        pub(crate) fn new_protected(
            profile: CapabilityProfileV1,
            config_identity: Option<[u8; CONFIG_ID_BYTES]>,
            compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            retained_object_binding_sha256: [u8; RETAINED_OBJECT_BINDING_BYTES],
        ) -> Result<Self, String> {
            let mut binding = Self::new(
                profile,
                config_identity,
                compiler_closure.identity_sha256(),
                compiler_closure.rustc_executable_sha256(),
                retained_object_binding_sha256,
            )?;
            binding.protected_compiler_closure_v2 = true;
            Ok(binding)
        }

        pub(crate) fn from_environment_for_client(
            profile: CapabilityProfileV1,
            config_identity: Option<[u8; CONFIG_ID_BYTES]>,
        ) -> Result<Self, String> {
            let encoded_route = std::env::var(CAPABILITY_BROKER_ENV).map_err(|_| {
                format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}")
            })?;
            let route = BrokerRouteV3::parse(&encoded_route)?;
            if route.binding.profile != profile || route.binding.config_identity != config_identity
            {
                return Err("capability broker route has the wrong profile/config identity".into());
            }
            Ok(route.binding)
        }

        pub(crate) const fn compiler_closure_sha256(self) -> [u8; 32] {
            self.compiler_closure_sha256
        }

        pub(crate) const fn requires_compiler_closure_v2(self) -> bool {
            self.protected_compiler_closure_v2
        }

        const fn descriptor_count(self) -> usize {
            self.profile.descriptor_count()
                + if self.protected_compiler_closure_v2 {
                    2
                } else {
                    0
                }
        }

        pub(crate) const fn rustc_executable_sha256(self) -> [u8; 32] {
            self.rustc_executable_sha256
        }

        pub(crate) const fn retained_object_binding_sha256(self) -> [u8; 32] {
            self.retained_object_binding_sha256
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct BrokerPeerIdentityV2 {
        uid: u32,
        pid: u32,
        start_time_ticks: u64,
        device: u64,
        inode: u64,
        mode: u32,
        executable_sha256: [u8; 32],
    }

    #[derive(Clone, Copy)]
    struct CurrentExecutableObservation {
        device: u64,
        inode: u64,
        mode: u32,
        executable_sha256: [u8; 32],
    }

    static CURRENT_EXECUTABLE_OBSERVATION: OnceLock<Result<CurrentExecutableObservation, String>> =
        OnceLock::new();
    fn current_executable_observation() -> Result<CurrentExecutableObservation, String> {
        CURRENT_EXECUTABLE_OBSERVATION
            .get_or_init(|| {
                let pid = std::process::id();
                let initial_start = process_start_time_ticks(pid)?;
                let (pinned, metadata) = pin_process_executable(pid)?;
                if process_start_time_ticks(pid)? != initial_start {
                    return Err("current broker process identity changed while pinning".to_owned());
                }
                Ok(CurrentExecutableObservation {
                    device: metadata.dev(),
                    inode: metadata.ino(),
                    mode: metadata.mode(),
                    executable_sha256: *pinned.sha256(),
                })
            })
            .clone()
    }

    impl BrokerPeerIdentityV2 {
        fn current() -> Result<Self, String> {
            let pid = std::process::id();
            let start_time_ticks = process_start_time_ticks(pid)?;
            let executable = current_executable_observation()?;
            let identity = Self {
                uid: unsafe { libc::geteuid() },
                pid,
                start_time_ticks,
                device: executable.device,
                inode: executable.inode,
                mode: executable.mode,
                executable_sha256: executable.executable_sha256,
            };
            if process_start_time_ticks(pid)? != start_time_ticks {
                return Err("current broker process identity changed while pinning".into());
            }
            Ok(identity)
        }

        fn require_current_executable(self) -> Result<(), String> {
            let current = current_executable_observation()?;
            if self.uid != unsafe { libc::geteuid() }
                || self.object_identity()
                    != LinuxObjectIdentityV3::from_linux_stat(
                        current.device,
                        current.inode,
                        current.mode,
                    )
                || self.executable_sha256 != current.executable_sha256
            {
                return Err(
                    "capability broker route does not name the current cargo-fe2o3 executable"
                        .into(),
                );
            }
            Ok(())
        }

        const fn object_identity(self) -> LinuxObjectIdentityV3 {
            LinuxObjectIdentityV3::from_linux_stat(self.device, self.inode, self.mode)
        }

        fn authenticate(self, stream: &UnixStream) -> Result<(), String> {
            let credentials = rustix::net::sockopt::socket_peercred(stream)
                .map_err(|error| format!("cannot inspect capability broker peer: {error}"))?;
            let peer_pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
                .map_err(|_| "capability broker peer PID is negative".to_owned())?;
            let current_uid = unsafe { libc::geteuid() };
            if credentials.uid.as_raw() != current_uid || credentials.uid.as_raw() != self.uid {
                return Err("capability broker peer uid does not match the current user".into());
            }
            if peer_pid != self.pid {
                return Err("capability broker peer PID does not match the prepared route".into());
            }
            let initial_start = process_start_time_ticks(peer_pid)?;
            if initial_start != self.start_time_ticks {
                return Err(
                    "capability broker peer start time does not match the prepared route".into(),
                );
            }
            let path = PathBuf::from(format!("/proc/{peer_pid}/exe"));
            let executable = File::open(&path).map_err(|error| {
                format!(
                    "cannot open capability broker peer executable {}: {error}",
                    path.display()
                )
            })?;
            let metadata = executable.metadata().map_err(|error| {
                format!(
                    "cannot inspect capability broker peer executable {}: {error}",
                    path.display()
                )
            })?;
            let final_start = process_start_time_ticks(peer_pid)?;
            if final_start != initial_start {
                return Err("capability broker peer PID was reused while authenticating".into());
            }
            if LinuxObjectIdentityV3::from_linux_stat(
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
            ) != self.object_identity()
            {
                return Err("capability broker peer executable does not match the prepared object and bytes".into());
            }
            Ok(())
        }

        fn authenticate_client(self, stream: &UnixStream) -> Result<ProcessIdentityV1, String> {
            let credentials = rustix::net::sockopt::socket_peercred(stream)
                .map_err(|error| format!("cannot inspect capability broker client: {error}"))?;
            let client_pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
                .map_err(|_| "capability broker client PID is negative".to_owned())?;
            if credentials.uid.as_raw() != self.uid {
                return Err("capability broker client uid does not match the broker".into());
            }
            let initial_start = process_start_time_ticks(client_pid)?;
            let (executable, metadata) = pin_process_executable(client_pid)?;
            if process_start_time_ticks(client_pid)? != initial_start {
                return Err("capability broker client PID was reused while authenticating".into());
            }
            if LinuxObjectIdentityV3::from_linux_stat(
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
            ) != self.object_identity()
                || executable.sha256() != &self.executable_sha256
            {
                return Err(
                    "capability broker client is not the exact pinned cargo-fe2o3 object and bytes"
                        .into(),
                );
            }
            ProcessIdentityV1::observe(client_pid)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct BrokerRouteV3 {
        endpoint: String,
        secret: [u8; SECRET_BYTES],
        binding: CapabilityBindingV3,
        peer: BrokerPeerIdentityV2,
    }

    impl BrokerRouteV3 {
        fn encode(&self) -> String {
            format!(
                "{ROUTE_PREFIX}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{:x}:{:x}:{:x}:{}",
                self.endpoint,
                hex(&self.secret),
                self.binding.profile.route_name(),
                self.binding
                    .config_identity
                    .map(|identity| hex(&identity))
                    .unwrap_or_else(|| "-".to_owned()),
                if self.binding.protected_compiler_closure_v2 {
                    "v2"
                } else {
                    "-"
                },
                hex(&self.binding.compiler_closure_sha256),
                hex(&self.binding.rustc_executable_sha256),
                hex(&self.binding.retained_object_binding_sha256),
                self.peer.uid,
                self.peer.pid,
                self.peer.start_time_ticks,
                self.peer.device,
                self.peer.inode,
                self.peer.mode,
                hex(&self.peer.executable_sha256),
            )
        }

        fn parse(value: &str) -> Result<Self, String> {
            let fields = value.split(':').collect::<Vec<_>>();
            if fields.len() != 16 || fields[0] != ROUTE_PREFIX {
                return Err("capability broker route is not canonical V3".into());
            }
            let endpoint = fields[1].to_owned();
            endpoint_address(&endpoint)?;
            let secret = decode_fixed_hex(fields[2], "broker secret")?;
            let profile = CapabilityProfileV1::parse_route_name(fields[3])
                .ok_or_else(|| "capability broker route has an unknown profile".to_owned())?;
            let config_identity = if fields[4] == "-" {
                None
            } else {
                Some(decode_fixed_hex(fields[4], "config identity")?)
            };
            let protected_compiler_closure_v2 = match fields[5] {
                "-" => false,
                "v2" => true,
                _ => return Err("capability broker route has an unknown closure schema".into()),
            };
            let compiler_closure_sha256 = decode_fixed_hex(fields[6], "compiler closure digest")?;
            let rustc_executable_sha256 = decode_fixed_hex(fields[7], "rustc executable digest")?;
            let retained_object_binding_sha256 =
                decode_fixed_hex(fields[8], "retained object binding digest")?;
            let mut binding = CapabilityBindingV3::new(
                profile,
                config_identity,
                compiler_closure_sha256,
                rustc_executable_sha256,
                retained_object_binding_sha256,
            )?;
            binding.protected_compiler_closure_v2 = protected_compiler_closure_v2;
            let peer = BrokerPeerIdentityV2 {
                uid: u32::try_from(parse_canonical_decimal(fields[9], "peer uid", true)?)
                    .map_err(|_| "capability broker peer uid exceeds u32".to_owned())?,
                pid: u32::try_from(parse_canonical_decimal(fields[10], "peer pid", false)?)
                    .map_err(|_| "capability broker peer pid exceeds u32".to_owned())?,
                start_time_ticks: parse_canonical_decimal(fields[11], "peer start time", false)?,
                device: parse_canonical_hex(fields[12], "peer device")?,
                inode: parse_canonical_hex(fields[13], "peer inode")?,
                mode: u32::try_from(parse_canonical_hex(fields[14], "peer mode")?)
                    .map_err(|_| "capability broker peer mode exceeds u32".to_owned())?,
                executable_sha256: decode_fixed_hex(fields[15], "peer executable digest")?,
            };
            let route = Self {
                endpoint,
                secret,
                binding,
                peer,
            };
            if route.encode() != value {
                return Err("capability broker route is not canonically encoded".into());
            }
            Ok(route)
        }
    }

    pub(crate) struct CapabilityBroker {
        route: String,
        invocation_authorization: InvocationAuthorizationRegistryV1,
        source_isa_observer: Option<Arc<Mutex<SourceIsaObservationCollectorStateV1>>>,
        shutdown: Arc<BrokerShutdown>,
        worker: Option<JoinHandle<()>>,
    }

    #[derive(Default)]
    struct BrokerShutdownState {
        stopping: bool,
        next_connection_id: u64,
        active: BTreeMap<u64, Arc<UnixStream>>,
        active_authentications: usize,
    }

    struct BrokerShutdown {
        // This mutex is the shutdown/SCM_RIGHTS linearization point and owns the wakeup socket.
        state: Mutex<BrokerShutdownState>,
        authentication_available: Condvar,
        max_concurrent_authentications: usize,
        max_active_connections: usize,
    }

    impl BrokerShutdown {
        fn new(max_active_connections: usize) -> Self {
            assert!(max_active_connections != 0);
            Self {
                state: Mutex::new(BrokerShutdownState::default()),
                authentication_available: Condvar::new(),
                max_concurrent_authentications: max_active_connections
                    .min(MAX_CONCURRENT_AUTHENTICATIONS),
                max_active_connections,
            }
        }

        fn state(&self) -> MutexGuard<'_, BrokerShutdownState> {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        fn is_stopping(&self) -> bool {
            self.state().stopping
        }

        fn register(
            self: &Arc<Self>,
            stream: &Arc<UnixStream>,
            deadline: BrokerDeadline,
        ) -> io::Result<Option<ConnectionRegistryGuard>> {
            let mut state = self.state();
            if state.stopping {
                let _ = stream.shutdown(Shutdown::Both);
                return Ok(None);
            }
            if state.active.len() >= self.max_active_connections {
                let _ = stream.shutdown(Shutdown::Both);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "capability broker is at active connection capacity",
                ));
            }
            if let Err(error) = deadline.require_remaining() {
                let _ = stream.shutdown(Shutdown::Both);
                return Err(error);
            }
            let connection_id = state.next_connection_id;
            state.next_connection_id =
                state.next_connection_id.checked_add(1).ok_or_else(|| {
                    io::Error::other("capability broker exhausted connection identifiers")
                })?;
            state.active.insert(connection_id, Arc::clone(stream));
            Ok(Some(ConnectionRegistryGuard {
                shutdown: Arc::clone(self),
                connection_id,
            }))
        }

        fn finish(&self, connection_id: u64) {
            self.state().active.remove(&connection_id);
        }

        fn begin_authentication(
            self: &Arc<Self>,
            deadline: BrokerDeadline,
        ) -> io::Result<AuthenticationRegistryGuard> {
            let mut state = self.state();
            while !state.stopping
                && state.active_authentications >= self.max_concurrent_authentications
            {
                let remaining = deadline.remaining()?;
                let (next_state, _) = self
                    .authentication_available
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state = next_state;
            }
            if state.stopping {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "capability broker is shutting down",
                ));
            }
            deadline.require_remaining()?;
            state.active_authentications += 1;
            Ok(AuthenticationRegistryGuard {
                shutdown: Arc::clone(self),
            })
        }

        fn finish_authentication(&self) {
            let mut state = self.state();
            state.active_authentications = state
                .active_authentications
                .checked_sub(1)
                .expect("authentication registry guard must own an active slot");
            drop(state);
            self.authentication_available.notify_one();
        }

        fn begin(&self) {
            let mut state = self.state();
            state.stopping = true;
            for active in state.active.values() {
                let _ = active.shutdown(Shutdown::Both);
            }
            state.active.clear();
            drop(state);
            self.authentication_available.notify_all();
        }

        fn send_response(
            &self,
            stream: &UnixStream,
            response: &[u8],
            descriptors: &[BorrowedFd<'_>],
            deadline: BrokerDeadline,
        ) -> io::Result<()> {
            let state = self.state();
            if state.stopping {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "capability broker is shutting down",
                ));
            }
            deadline.require_remaining()?;
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !ancillary.push(SendAncillaryMessage::ScmRights(descriptors)) {
                return Err(io::Error::other("capability control buffer is too small"));
            }
            let sent = sendmsg(
                stream,
                &[IoSlice::new(response)],
                &mut ancillary,
                SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
            )
            .map_err(io::Error::from)?;
            if sent != response.len() {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "capability broker response was truncated",
                ));
            }
            drop(state);
            Ok(())
        }
    }

    struct ConnectionRegistryGuard {
        shutdown: Arc<BrokerShutdown>,
        connection_id: u64,
    }

    impl Drop for ConnectionRegistryGuard {
        fn drop(&mut self) {
            self.shutdown.finish(self.connection_id);
        }
    }

    struct AuthenticationRegistryGuard {
        shutdown: Arc<BrokerShutdown>,
    }

    impl Drop for AuthenticationRegistryGuard {
        fn drop(&mut self) {
            self.shutdown.finish_authentication();
        }
    }

    impl CapabilityBroker {
        pub(crate) fn start(
            session: BuildSession,
            binding: CapabilityBindingV3,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Self::start_with_limits(
                session,
                binding,
                backend,
                artifact,
                pinned_cargo_image,
                PRODUCTION_BROKER_LIMITS,
            )
        }

        pub(crate) fn start_protected(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Self::start_with_compiler_capabilities(
                session,
                binding,
                BrokerCompilerCapabilities::protected(compiler_closure, compiler_execution_profile),
                backend,
                artifact,
                pinned_cargo_image,
                None,
                PRODUCTION_BROKER_LIMITS,
            )
        }

        // Keep each retained authority and observer policy explicit at this security boundary.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn start_protected_with_source_isa_observer(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            observer_policy: &ProductionSourceIsaObserverPolicyV1,
        ) -> Result<Self, String> {
            Self::start_with_compiler_capabilities(
                session,
                binding,
                BrokerCompilerCapabilities::protected(compiler_closure, compiler_execution_profile),
                backend,
                artifact,
                pinned_cargo_image,
                Some(observer_policy),
                PRODUCTION_BROKER_LIMITS,
            )
        }

        fn start_with_limits(
            session: BuildSession,
            binding: CapabilityBindingV3,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            limits: BrokerLimits,
        ) -> Result<Self, String> {
            Self::start_with_compiler_capabilities(
                session,
                binding,
                BrokerCompilerCapabilities::ordinary(),
                backend,
                artifact,
                pinned_cargo_image,
                None,
                limits,
            )
        }

        // Keep each retained authority, policy, and test-injected limit explicit.
        #[allow(clippy::too_many_arguments)]
        fn start_with_compiler_capabilities(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler: BrokerCompilerCapabilities<'_>,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            observer_policy: Option<&ProductionSourceIsaObserverPolicyV1>,
            limits: BrokerLimits,
        ) -> Result<Self, String> {
            if limits.max_active_connections == 0
                || limits.authentication_timeout.is_zero()
                || limits.invocation_frame_timeout.is_zero()
                || limits.invocation_lifetime.is_zero()
            {
                return Err("capability broker limits must be nonzero".to_owned());
            }
            if binding.requires_compiler_closure_v2() != compiler.closure.is_some()
                || compiler.closure.is_some() != compiler.execution_profile.is_some()
            {
                return Err(
                    "capability binding and protected compiler capability presence differ"
                        .to_owned(),
                );
            }
            let source_isa_observer = observer_policy
                .map(|policy| BrokerSourceIsaObserverV1::from_policy(policy, session))
                .transpose()?;
            if let Some(observer) = &source_isa_observer
                && (!binding.requires_compiler_closure_v2()
                    || binding.config_identity != Some(observer.config_identity))
            {
                return Err(
                    "source/ISA observer policy requires the exact protected V2 config binding"
                        .to_owned(),
                );
            }
            let compiler_closure = compiler
                .closure
                .map(|closure| {
                    if closure.identity_sha256() != binding.compiler_closure_sha256()
                        || closure.rustc_executable_sha256() != binding.rustc_executable_sha256()
                        || closure.codegen_backend_sha256() != *backend.sha256()
                        || closure.cargo_executable_sha256() != *pinned_cargo_image.sha256()
                    {
                        return Err(
                            "compiler-closure descriptor differs from broker-retained images"
                                .to_owned(),
                        );
                    }
                    CompilerClosureCapabilityV1::create(closure)
                })
                .transpose()?;
            let compiler_execution_profile = compiler
                .execution_profile
                .map(|profile| {
                    profile.revalidate()?;
                    CompilerExecutionClientProfileCapabilityV1::from_file(
                        profile.try_clone_for_transfer()?,
                    )
                })
                .transpose()?;
            let endpoint = random_endpoint().map_err(|error| {
                format!("failed to allocate capability broker endpoint: {error}")
            })?;
            let address = endpoint_address(&endpoint)?;
            let listener = UnixListener::bind_addr(&address)
                .map_err(|error| format!("failed to bind capability broker: {error}"))?;
            listener
                .set_nonblocking(true)
                .map_err(|error| format!("failed to configure capability broker: {error}"))?;
            let backend = backend
                .try_clone_for_transfer()
                .map_err(|error| format!("failed to retain broker backend: {error}"))?;
            let artifact = artifact
                .try_clone_for_transfer()
                .map_err(|error| format!("failed to retain broker artifact directory: {error}"))?;
            let executable = BrokerPeerIdentityV2::current().map_err(|error| {
                format!("failed to identify capability broker executable: {error}")
            })?;
            let secret = random_bytes()
                .map_err(|error| format!("failed to allocate capability broker secret: {error}"))?;
            let route = BrokerRouteV3 {
                endpoint,
                secret,
                binding,
                peer: executable,
            }
            .encode();
            let shutdown = Arc::new(BrokerShutdown::new(limits.max_active_connections));
            let invocation_authorization = InvocationAuthorizationRegistryV1::new();
            let worker_invocation_authorization = invocation_authorization.clone();
            let worker_shutdown = Arc::clone(&shutdown);
            let returned_source_isa_observer = source_isa_observer
                .as_ref()
                .map(|observer| Arc::clone(&observer.collector));
            let worker = thread::Builder::new()
                .name("fe2o3-capability-broker".to_string())
                .spawn(move || {
                    BrokerServer {
                        listener,
                        session,
                        binding,
                        secret,
                        executable,
                        backend,
                        artifact,
                        compiler_closure,
                        compiler_execution_profile,
                        source_isa_observer,
                        authentication_timeout: limits.authentication_timeout,
                        invocation_frame_timeout: limits.invocation_frame_timeout,
                        invocation_lifetime: limits.invocation_lifetime,
                        invocation_authorization: worker_invocation_authorization,
                        shutdown: worker_shutdown,
                    }
                    .serve();
                })
                .map_err(|error| format!("failed to start capability broker: {error}"))?;
            Ok(Self {
                route,
                invocation_authorization,
                source_isa_observer: returned_source_isa_observer,
                shutdown,
                worker: Some(worker),
            })
        }

        pub(crate) fn route(&self) -> &str {
            &self.route
        }

        pub(crate) fn invocation_authorization(&self) -> InvocationAuthorizationRegistryV1 {
            self.invocation_authorization.clone()
        }

        pub(crate) fn finish_source_isa_observations(
            mut self,
        ) -> Result<SourceIsaObservationCollectionV1, String> {
            let collector = self.source_isa_observer.take().ok_or_else(|| {
                "capability broker has no source/ISA observer collector".to_owned()
            })?;
            self.shutdown.begin();
            let worker_panicked = self
                .worker
                .take()
                .is_some_and(|worker| worker.join().is_err());
            let collector = Arc::try_unwrap(collector).map_err(|_| {
                "source/ISA observer collector still has a live broker owner".to_owned()
            })?;
            let mut collector = collector
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if worker_panicked {
                collector.fail(SourceIsaObservationTransportFailureV1::BrokerWorkerPanic);
            }
            Ok(collector.finish())
        }
    }

    impl Drop for CapabilityBroker {
        fn drop(&mut self) {
            self.shutdown.begin();
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    pub(crate) struct BrokeredCapabilities {
        pub(crate) backend: PinnedCodegenBackend,
        pub(crate) artifact: PinnedDirectory,
        pub(crate) compiler_closure: Option<CompilerClosureCapabilityV1>,
        pub(crate) compiler_execution_profile: Option<CompilerExecutionClientProfileCapabilityV1>,
        pub(crate) invocation_authority: Option<BrokeredInvocationAuthorityV1>,
    }

    pub(crate) struct BrokeredInvocationAuthorityV1 {
        stream: UnixStream,
    }

    pub(crate) struct SourceIsaObservationSinkV1 {
        stream: UnixStream,
        config_identity: [u8; 32],
        unit_identity: [u8; 32],
        attempt: BuildAttempt,
    }

    impl BrokeredInvocationAuthorityV1 {
        fn from_authenticated_stream(stream: UnixStream) -> Result<Self, String> {
            let normalized = rustix::io::fcntl_dupfd_cloexec(&stream, RECEIVED_DESCRIPTOR_FLOOR)
                .map_err(|error| {
                    format!("failed to retain authenticated invocation capability: {error}")
                })?;
            Ok(Self {
                stream: UnixStream::from(normalized),
            })
        }

        pub(crate) fn release(self) -> Result<(), String> {
            self.exchange(
                BrokeredInvocationCapabilityRequestV1::Release,
                BROKERED_INVOCATION_PREPARED_V1,
            )
        }

        pub(crate) fn release_with_source_isa_observer(
            self,
            config_identity: [u8; 32],
            unit_identity: [u8; 32],
            attempt: BuildAttempt,
        ) -> Result<SourceIsaObservationSinkV1, String> {
            let request = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                config_identity,
                unit_identity,
                attempt,
            )
            .map_err(|error| error.to_string())?;
            let mut stream = self.stream;
            stream.write_all(&request.encode()).map_err(|error| {
                format!("failed to write observer invocation capability: {error}")
            })?;
            let mut response = [0; BROKERED_SOURCE_ISA_PREPARED_V1.len()];
            stream.read_exact(&mut response).map_err(|error| {
                format!("failed to read observer invocation preparation: {error}")
            })?;
            if &response != BROKERED_SOURCE_ISA_PREPARED_V1 {
                return Err(
                    "observer invocation capability returned a malformed response".to_owned(),
                );
            }
            Ok(SourceIsaObservationSinkV1 {
                stream,
                config_identity,
                unit_identity,
                attempt,
            })
        }

        fn exchange(
            &self,
            request: BrokeredInvocationCapabilityRequestV1,
            expected: &[u8; 16],
        ) -> Result<(), String> {
            let mut stream = &self.stream;
            stream
                .write_all(&request.encode())
                .map_err(|error| format!("failed to write invocation capability: {error}"))?;
            let mut response = [0_u8; 16];
            stream
                .read_exact(&mut response)
                .map_err(|error| format!("failed to read invocation capability: {error}"))?;
            if &response != expected {
                return Err("invocation capability returned a malformed response".to_owned());
            }
            Ok(())
        }
    }

    impl SourceIsaObservationSinkV1 {
        pub(crate) fn submit(mut self, frame: &SourceIsaObservationFrameV1) -> Result<(), String> {
            let context = frame.context();
            let observation_attempt =
                crate::source_isa_observation::inert_source_isa_attempt_v1(self.attempt)
                    .map_err(|error| format!("invalid source/ISA observation attempt: {error}"))?;
            if context.config() != self.config_identity
                || context.unit() != self.unit_identity
                || context.attempt() != observation_attempt
            {
                return Err(
                    "source/ISA observation frame differs from its authenticated sink".to_owned(),
                );
            }
            self.stream.write_all(&frame.encode()).map_err(|error| {
                format!("failed to write source/ISA observation frame: {error}")
            })?;
            self.stream
                .shutdown(Shutdown::Write)
                .map_err(|error| format!("failed to close source/ISA observation frame: {error}"))
        }
    }

    pub(crate) fn receive(
        session: BuildSession,
        binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        let encoded_route = std::env::var(CAPABILITY_BROKER_ENV)
            .map_err(|_| format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}"))?;
        let route = BrokerRouteV3::parse(&encoded_route)?;
        receive_from(&route, session, binding)
    }

    fn receive_from(
        route: &BrokerRouteV3,
        session: BuildSession,
        binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        if route.binding != binding {
            return Err(
                "capability broker route does not match the prepared profile/config/rustc identity"
                    .into(),
            );
        }
        route.peer.require_current_executable()?;
        let address = endpoint_address(&route.endpoint)?;
        let mut stream = UnixStream::connect_addr(&address)
            .map_err(|error| format!("failed to connect to capability broker: {error}"))?;
        stream
            .set_read_timeout(Some(BROKER_CLIENT_RESPONSE_TIMEOUT))
            .map_err(|error| format!("failed to bound capability broker read: {error}"))?;
        route.peer.authenticate(&stream)?;
        let challenge = random_bytes()
            .map_err(|error| format!("failed to allocate broker client challenge: {error}"))?;
        let request = request_bytes(session, binding, challenge, &route.secret);
        let request_auth: [u8; REQUEST_AUTH_BYTES] = request[REQUEST_BYTES - REQUEST_AUTH_BYTES..]
            .try_into()
            .expect("request authentication field has a fixed size");
        stream
            .write_all(&request)
            .map_err(|error| format!("failed to authenticate to capability broker: {error}"))?;

        let mut response = [0_u8; RESPONSE_BYTES];
        let mut iov = [IoSliceMut::new(&mut response)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let message = recvmsg(&stream, &mut iov, &mut ancillary, RecvFlags::CMSG_CLOEXEC)
            .map_err(|error| format!("failed to receive brokered capabilities: {error}"))?;
        if message.flags.contains(ReturnFlags::CTRUNC) {
            return Err("capability broker descriptor response was truncated".to_string());
        }
        let expected_response = response_bytes(&route.secret, challenge, request_auth);
        if message.bytes != RESPONSE_BYTES || response != expected_response {
            return Err("capability broker returned a malformed response".to_string());
        }
        let mut descriptors = Vec::new();
        for message in ancillary.drain() {
            if let RecvAncillaryMessage::ScmRights(received) = message {
                descriptors.extend(received);
            }
        }
        let mut capabilities = decode_received_descriptors(descriptors, binding)?;
        capabilities.invocation_authority = Some(
            BrokeredInvocationAuthorityV1::from_authenticated_stream(stream)?,
        );
        Ok(capabilities)
    }

    fn decode_received_descriptors(
        mut descriptors: Vec<OwnedFd>,
        binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        if descriptors.len() != binding.descriptor_count() {
            return Err(format!(
                "capability broker returned {} descriptors instead of {} for the {} profile",
                descriptors.len(),
                binding.descriptor_count(),
                binding.profile.name(),
            ));
        }
        let compiler_execution_profile = if binding.requires_compiler_closure_v2() {
            let image = normalize_received_descriptor(
                descriptors
                    .pop()
                    .expect("compiler-execution profile descriptor count checked"),
                "compiler-execution client profile",
            )?;
            Some(CompilerExecutionClientProfileCapabilityV1::from_file(
                image,
            )?)
        } else {
            None
        };
        let compiler_closure = if binding.requires_compiler_closure_v2() {
            let image = normalize_received_descriptor(
                descriptors
                    .pop()
                    .expect("compiler-closure descriptor count checked"),
                "compiler closure",
            )?;
            let capability = CompilerClosureCapabilityV1::from_file(image)?;
            if capability.closure().identity_sha256() != binding.compiler_closure_sha256()
                || capability.closure().rustc_executable_sha256()
                    != binding.rustc_executable_sha256()
            {
                return Err(
                    "brokered compiler closure differs from the authenticated binding".to_owned(),
                );
            }
            Some(capability)
        } else {
            None
        };
        let artifact = normalize_received_descriptor(
            descriptors.pop().expect("descriptor count checked"),
            "artifact directory",
        )?;
        let artifact =
            PinnedDirectory::from_transferred_file(artifact, "artifact output directory")
                .map_err(|error| format!("invalid brokered artifact directory: {error}"))?;
        let backend = normalize_received_descriptor(
            descriptors.pop().expect("descriptor count checked"),
            "codegen backend",
        )?;
        let backend = PinnedCodegenBackend::from_transferred_file(backend)
            .map_err(|error| format!("invalid brokered codegen backend: {error}"))?;
        Ok(BrokeredCapabilities {
            backend,
            artifact,
            compiler_closure,
            compiler_execution_profile,
            invocation_authority: None,
        })
    }

    fn normalize_received_descriptor(descriptor: OwnedFd, kind: &str) -> Result<File, String> {
        let normalized = rustix::io::fcntl_dupfd_cloexec(&descriptor, RECEIVED_DESCRIPTOR_FLOOR)
            .map_err(|error| format!("failed to normalize brokered {kind} descriptor: {error}"))?;
        let file = File::from(normalized);
        if file.as_raw_fd() < RECEIVED_DESCRIPTOR_FLOOR {
            return Err(format!(
                "brokered {kind} descriptor overlaps reserved child descriptors"
            ));
        }
        Ok(file)
    }

    struct BrokerServer {
        listener: UnixListener,
        session: BuildSession,
        binding: CapabilityBindingV3,
        secret: [u8; SECRET_BYTES],
        executable: BrokerPeerIdentityV2,
        backend: File,
        artifact: File,
        compiler_closure: Option<CompilerClosureCapabilityV1>,
        compiler_execution_profile: Option<CompilerExecutionClientProfileCapabilityV1>,
        source_isa_observer: Option<BrokerSourceIsaObserverV1>,
        authentication_timeout: Duration,
        invocation_frame_timeout: Duration,
        invocation_lifetime: Duration,
        invocation_authorization: InvocationAuthorizationRegistryV1,
        shutdown: Arc<BrokerShutdown>,
    }

    impl BrokerServer {
        fn serve(self) {
            thread::scope(|scope| {
                while !self.shutdown.is_stopping() {
                    match self.listener.accept() {
                        Ok((stream, _)) => {
                            let stream = Arc::new(stream);
                            let accepted_at = Instant::now();
                            let deadline =
                                BrokerDeadline::new(accepted_at, self.authentication_timeout);
                            match self.shutdown.register(&stream, deadline) {
                                Ok(Some(registry_guard)) => {
                                    let server = &self;
                                    let worker = move || {
                                        let _registry_guard = registry_guard;
                                        let _ =
                                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                                || server.serve_one(&stream, deadline),
                                            ));
                                    };
                                    let spawned =
                                        thread::Builder::new().spawn_scoped(scope, worker);
                                    if spawned.is_err() {
                                        continue;
                                    }
                                }
                                Ok(None) => break,
                                Err(_) => continue,
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => {
                            // Descriptor exhaustion and transient listener failures must reject
                            // work without permanently disabling the broker.
                            thread::sleep(Duration::from_millis(2));
                        }
                    }
                }
            });
        }

        fn serve_one(&self, stream: &UnixStream, deadline: BrokerDeadline) -> io::Result<()> {
            deadline.require_remaining()?;
            let authentication = self.shutdown.begin_authentication(deadline)?;
            let deadline = BrokerDeadline::new(Instant::now(), self.authentication_timeout);
            let client = self
                .executable
                .authenticate_client(stream)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            drop(authentication);
            self.invocation_authorization
                .consume(client)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            deadline.require_remaining()?;
            let mut request = vec![0_u8; REQUEST_BYTES];
            deadline.read_exact(stream, &mut request)?;
            let challenge_start = REQUEST_MAGIC.len()
                + 16
                + 1
                + CONFIG_ID_BYTES
                + 1
                + COMPILER_CLOSURE_ID_BYTES
                + RUSTC_EXECUTABLE_ID_BYTES
                + RETAINED_OBJECT_BINDING_BYTES;
            let challenge: [u8; CHALLENGE_BYTES] = request
                [challenge_start..challenge_start + CHALLENGE_BYTES]
                .try_into()
                .expect("request challenge has a fixed size");
            let expected_request =
                request_bytes(self.session, self.binding, challenge, &self.secret);
            if request != expected_request {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "capability broker request is not bound to this broker, session, profile, and config",
                ));
            }
            deadline.require_remaining()?;
            let request_auth: [u8; REQUEST_AUTH_BYTES] = request
                [REQUEST_BYTES - REQUEST_AUTH_BYTES..]
                .try_into()
                .expect("request authentication field has a fixed size");
            deadline.require_remaining()?;
            let mut descriptors = vec![self.backend.as_fd(), self.artifact.as_fd()];
            let compiler_closure = self
                .compiler_closure
                .as_ref()
                .map(CompilerClosureCapabilityV1::try_clone_for_transfer)
                .transpose()
                .map_err(io::Error::other)?;
            if let Some(compiler_closure) = &compiler_closure {
                descriptors.push(compiler_closure.as_fd());
            }
            let compiler_execution_profile = self
                .compiler_execution_profile
                .as_ref()
                .map(CompilerExecutionClientProfileCapabilityV1::try_clone_for_transfer)
                .transpose()
                .map_err(io::Error::other)?;
            if let Some(compiler_execution_profile) = &compiler_execution_profile {
                descriptors.push(compiler_execution_profile.as_fd());
            }
            let response = response_bytes(&self.secret, challenge, request_auth);
            self.shutdown
                .send_response(stream, &response, &descriptors, deadline)?;
            self.serve_invocation_authority(stream, client)
        }

        fn serve_invocation_authority(
            &self,
            stream: &UnixStream,
            client: ProcessIdentityV1,
        ) -> io::Result<()> {
            let liveness = InvocationLiveness {
                client,
                started_at: Instant::now(),
                frame_timeout: self.invocation_frame_timeout,
                lifetime: self.invocation_lifetime,
            };
            let request = read_invocation_request(liveness, stream)?;
            if let BrokerInvocationRequest::V2(request) = request {
                return self.receive_source_isa_observation(stream, liveness, request);
            }
            let BrokerInvocationRequest::V1(request) = request else {
                unreachable!("V2 observer request returned above")
            };
            let claim = match request {
                BrokeredInvocationCapabilityRequestV1::Release => {
                    let mut stream = stream;
                    stream.write_all(BROKERED_INVOCATION_PREPARED_V1)?;
                    return Ok(());
                }
                BrokeredInvocationCapabilityRequestV1::Prepare(claim)
                    if claim.attempt().session() == self.session =>
                {
                    claim
                }
                BrokeredInvocationCapabilityRequestV1::Prepare(_)
                | BrokeredInvocationCapabilityRequestV1::Consume(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "invocation capability preparation is not bound to this build session",
                    ));
                }
            };
            let mut stream = stream;
            stream.write_all(BROKERED_INVOCATION_PREPARED_V1)?;

            let mut encoded = [0_u8; BROKERED_INVOCATION_REQUEST_BYTES_V1];
            liveness.read_frame(stream, &mut encoded)?;
            if BrokeredInvocationCapabilityRequestV1::decode(&encoded)
                != Ok(BrokeredInvocationCapabilityRequestV1::Consume(claim))
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "rustc did not consume the exact wrapper-prepared invocation claim",
                ));
            }
            stream.write_all(BROKERED_INVOCATION_ADMITTED_V1)
        }

        fn receive_source_isa_observation(
            &self,
            stream: &UnixStream,
            liveness: InvocationLiveness,
            request: BrokeredInvocationCapabilityRequestV2,
        ) -> io::Result<()> {
            let observer = self.source_isa_observer.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "source/ISA observer request is unavailable for this broker",
                )
            })?;
            let result = (|| {
                prepare_source_isa_observer_request(observer, self.session, stream, request)?;
                let mut encoded = [0; SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1];
                liveness.read_frame(stream, &mut encoded)?;
                liveness.require_eof(stream)?;
                let frame = SourceIsaObservationFrameV1::decode(&encoded)
                    .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
                validate_source_isa_observer_frame_binding(self.session, request, &frame)?;
                observer.collect(frame)
            })();
            if result.is_err() {
                observer.fail(SourceIsaObservationTransportFailureV1::RejectedFrame);
            }
            result
        }
    }

    fn prepare_source_isa_observer_request(
        observer: &BrokerSourceIsaObserverV1,
        broker_session: BuildSession,
        stream: &UnixStream,
        request: BrokeredInvocationCapabilityRequestV2,
    ) -> io::Result<()> {
        if !observer.accepts(request) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "source/ISA observer request is not bound to the exact configured unit",
            ));
        }
        if request.attempt().session() == BuildSession::DIRECT
            || request.attempt().session() != broker_session
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "source/ISA observer request is not bound to this build session",
            ));
        }
        let mut stream = stream;
        stream.write_all(BROKERED_SOURCE_ISA_PREPARED_V1)
    }

    fn validate_source_isa_observer_frame_binding(
        broker_session: BuildSession,
        request: BrokeredInvocationCapabilityRequestV2,
        frame: &SourceIsaObservationFrameV1,
    ) -> io::Result<()> {
        let context = frame.context();
        let observation_attempt =
            crate::source_isa_observation::inert_source_isa_attempt_v1(request.attempt())
                .map_err(|error| io::Error::other(error.to_string()))?;
        if request.attempt().session() != broker_session
            || context.config() != request.config_identity()
            || context.unit() != request.unit_identity()
            || context.attempt() != observation_attempt
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "source/ISA observation frame differs from its exact authenticated request",
            ));
        }
        Ok(())
    }

    enum BrokerInvocationRequest {
        V1(BrokeredInvocationCapabilityRequestV1),
        V2(BrokeredInvocationCapabilityRequestV2),
    }

    fn read_invocation_request(
        liveness: InvocationLiveness,
        stream: &UnixStream,
    ) -> io::Result<BrokerInvocationRequest> {
        let mut magic = [0; 8];
        liveness.read_frame(stream, &mut magic)?;
        if &magic == BROKERED_INVOCATION_REQUEST_MAGIC_V1 {
            let mut encoded = [0; BROKERED_INVOCATION_REQUEST_BYTES_V1];
            encoded[..magic.len()].copy_from_slice(&magic);
            liveness.read_frame(stream, &mut encoded[magic.len()..])?;
            return BrokeredInvocationCapabilityRequestV1::decode(&encoded)
                .map(BrokerInvocationRequest::V1)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error));
        }
        if &magic == BROKERED_INVOCATION_REQUEST_MAGIC_V2 {
            let mut encoded = [0; BROKERED_INVOCATION_REQUEST_BYTES_V2];
            encoded[..magic.len()].copy_from_slice(&magic);
            liveness.read_frame(stream, &mut encoded[magic.len()..])?;
            return BrokeredInvocationCapabilityRequestV2::decode(&encoded)
                .map(BrokerInvocationRequest::V2)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error));
        }
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "brokered invocation request has an unknown version",
        ))
    }

    #[derive(Clone, Copy)]
    struct InvocationLiveness {
        client: ProcessIdentityV1,
        started_at: Instant,
        frame_timeout: Duration,
        lifetime: Duration,
    }

    impl InvocationLiveness {
        fn read_frame(self, stream: &UnixStream, buffer: &mut [u8]) -> io::Result<()> {
            let mut stream = stream;
            let mut offset = 0;
            let mut frame_deadline = None;
            while offset < buffer.len() {
                let now = Instant::now();
                if now.duration_since(self.started_at) >= self.lifetime {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "invocation capability exceeded its total lifetime",
                    ));
                }
                let deadline = frame_deadline
                    .get_or_insert_with(|| now + self.frame_timeout)
                    .to_owned();
                stream.set_read_timeout(Some(
                    deadline
                        .checked_duration_since(now)
                        .filter(|remaining| !remaining.is_zero())
                        .unwrap_or(Duration::from_millis(1)),
                ))?;
                match stream.read(&mut buffer[offset..]) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "invocation capability frame ended early",
                        ));
                    }
                    Ok(read) => offset += read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) && offset == 0 =>
                    {
                        self.client.require_current().map_err(|error| {
                            io::Error::new(io::ErrorKind::PermissionDenied, error)
                        })?;
                        frame_deadline = None;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "invocation capability frame deadline expired",
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        }

        fn require_eof(self, stream: &UnixStream) -> io::Result<()> {
            let mut trailing = [0; 1];
            let mut stream = stream;
            loop {
                let remaining = self
                    .lifetime
                    .checked_sub(self.started_at.elapsed())
                    .filter(|remaining| !remaining.is_zero())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            "invocation capability exceeded its total lifetime",
                        )
                    })?;
                stream.set_read_timeout(Some(self.frame_timeout.min(remaining)))?;
                match stream.read(&mut trailing) {
                    Ok(0) => return Ok(()),
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "source/ISA observation contains trailing bytes",
                        ));
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
        }
    }

    #[derive(Clone, Copy)]
    struct BrokerDeadline {
        expires_at: Instant,
    }

    impl BrokerDeadline {
        fn new(accepted_at: Instant, timeout: Duration) -> Self {
            Self {
                expires_at: accepted_at + timeout,
            }
        }

        fn remaining(self) -> io::Result<Duration> {
            self.expires_at
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "capability broker connection deadline expired",
                    )
                })
        }

        fn require_remaining(self) -> io::Result<()> {
            self.remaining().map(|_| ())
        }

        fn read_exact(self, stream: &UnixStream, mut buffer: &mut [u8]) -> io::Result<()> {
            let mut stream = stream;
            while !buffer.is_empty() {
                stream.set_read_timeout(Some(self.remaining()?))?;
                match stream.read(buffer) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "capability broker request ended early",
                        ));
                    }
                    Ok(read) => buffer = &mut buffer[read..],
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        self.require_remaining()?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        }
    }

    fn request_bytes(
        session: BuildSession,
        binding: CapabilityBindingV3,
        challenge: [u8; CHALLENGE_BYTES],
        secret: &[u8; SECRET_BYTES],
    ) -> Vec<u8> {
        let mut request = Vec::with_capacity(REQUEST_BYTES);
        request.extend_from_slice(binding.profile.request_magic());
        request.extend_from_slice(session.as_bytes());
        match binding.config_identity {
            Some(identity) => {
                request.push(1);
                request.extend_from_slice(&identity);
            }
            None => {
                request.push(0);
                request.extend_from_slice(&[0; CONFIG_ID_BYTES]);
            }
        }
        request.push(u8::from(binding.protected_compiler_closure_v2));
        request.extend_from_slice(&binding.compiler_closure_sha256);
        request.extend_from_slice(&binding.rustc_executable_sha256);
        request.extend_from_slice(&binding.retained_object_binding_sha256);
        request.extend_from_slice(&challenge);
        let authentication = keyed_digest(REQUEST_AUTH_DOMAIN, secret, &[&request]);
        request.extend_from_slice(&authentication);
        debug_assert_eq!(request.len(), REQUEST_BYTES);
        request
    }

    fn response_bytes(
        secret: &[u8; SECRET_BYTES],
        challenge: [u8; CHALLENGE_BYTES],
        request_auth: [u8; REQUEST_AUTH_BYTES],
    ) -> [u8; RESPONSE_BYTES] {
        let authentication =
            keyed_digest(RESPONSE_AUTH_DOMAIN, secret, &[&challenge, &request_auth]);
        let mut response = [0_u8; RESPONSE_BYTES];
        response[0] = 1;
        response[1..].copy_from_slice(&authentication);
        response
    }

    fn endpoint_address(endpoint: &str) -> Result<SocketAddr, String> {
        if endpoint.len() != ENDPOINT_HEX_BYTES
            || endpoint
                .bytes()
                .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        {
            return Err("capability broker endpoint is not canonical lowercase hexadecimal".into());
        }
        SocketAddr::from_abstract_name(format!("fe2o3-cap-v2-{endpoint}").as_bytes())
            .map_err(|error| format!("invalid capability broker endpoint: {error}"))
    }

    fn random_endpoint() -> io::Result<String> {
        let bytes = random_bytes()?;
        Ok(hex(&bytes))
    }

    fn random_bytes() -> io::Result<[u8; ENDPOINT_BYTES]> {
        let mut bytes = [0_u8; ENDPOINT_BYTES];
        File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn keyed_digest(domain: &[u8], secret: &[u8; SECRET_BYTES], fields: &[&[u8]]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update((domain.len() as u64).to_le_bytes());
        digest.update(domain);
        digest.update((secret.len() as u64).to_le_bytes());
        digest.update(secret);
        for field in fields {
            digest.update((field.len() as u64).to_le_bytes());
            digest.update(field);
        }
        digest.finalize().into()
    }

    fn process_start_time_ticks(pid: u32) -> Result<u64, String> {
        let path = PathBuf::from(format!("/proc/{pid}/stat"));
        let bytes = fs::read(&path)
            .map_err(|error| format!("cannot read broker process {}: {error}", path.display()))?;
        if bytes.is_empty() || bytes.len() > MAX_PROC_STAT_BYTES {
            return Err(format!(
                "broker process {} must contain 1 through {MAX_PROC_STAT_BYTES} bytes",
                path.display()
            ));
        }
        let close = bytes
            .iter()
            .rposition(|byte| *byte == b')')
            .ok_or_else(|| "broker process stat has no command terminator".to_owned())?;
        let recorded_pid = bytes[..close]
            .split(|byte| *byte == b' ')
            .next()
            .and_then(|value| std::str::from_utf8(value).ok())
            .and_then(|value| value.parse::<u32>().ok());
        if recorded_pid != Some(pid) {
            return Err("broker process stat PID does not match its proc entry".into());
        }
        bytes[close + 1..]
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty())
            .nth(19)
            .and_then(|value| std::str::from_utf8(value).ok())
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value != 0)
            .ok_or_else(|| "broker process stat has no valid start-time field".to_owned())
    }

    fn pin_process_executable(pid: u32) -> Result<(PinnedExecutable, fs::Metadata), String> {
        let path = PathBuf::from(format!("/proc/{pid}/exe"));
        for attempt in 0..EXECUTABLE_PIN_ATTEMPTS {
            let file = File::open(&path).map_err(|error| {
                format!(
                    "cannot open broker process executable {}: {error}",
                    path.display()
                )
            })?;
            let metadata = file.metadata().map_err(|error| {
                format!(
                    "cannot inspect broker process executable {}: {error}",
                    path.display()
                )
            })?;
            match PinnedExecutable::from_transferred_file(file, path.clone()) {
                Ok(pinned) => {
                    if pinned.object_identity()
                        != LinuxObjectIdentityV3::from_linux_stat(
                            metadata.dev(),
                            metadata.ino(),
                            metadata.mode(),
                        )
                    {
                        return Err("broker process executable object changed while pinning".into());
                    }
                    return Ok((pinned, metadata));
                }
                Err(PinExecutableError::ChangedDuringRead { .. })
                    if attempt + 1 < EXECUTABLE_PIN_ATTEMPTS =>
                {
                    thread::yield_now();
                }
                Err(error) => {
                    return Err(format!(
                        "cannot pin broker process executable {}: {error}",
                        path.display()
                    ));
                }
            }
        }
        unreachable!("executable pin retries either return or report their final error")
    }

    fn decode_fixed_hex<const N: usize>(value: &str, label: &str) -> Result<[u8; N], String> {
        if value.len() != N * 2
            || value
                .bytes()
                .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        {
            return Err(format!(
                "capability broker {label} is not canonical lowercase hex"
            ));
        }
        let mut decoded = [0_u8; N];
        for (index, output) in decoded.iter_mut().enumerate() {
            let offset = index * 2;
            *output = u8::from_str_radix(&value[offset..offset + 2], 16)
                .map_err(|_| format!("capability broker {label} is invalid"))?;
        }
        Ok(decoded)
    }

    fn parse_canonical_decimal(value: &str, label: &str, allow_zero: bool) -> Result<u64, String> {
        let parsed = value
            .parse::<u64>()
            .map_err(|_| format!("capability broker {label} is not decimal"))?;
        if (!allow_zero && parsed == 0) || parsed.to_string() != value {
            return Err(format!("capability broker {label} is not canonical"));
        }
        Ok(parsed)
    }

    fn parse_canonical_hex(value: &str, label: &str) -> Result<u64, String> {
        let parsed = u64::from_str_radix(value, 16)
            .map_err(|_| format!("capability broker {label} is not hexadecimal"))?;
        if parsed == 0 || format!("{parsed:x}") != value {
            return Err(format!("capability broker {label} is not canonical"));
        }
        Ok(parsed)
    }

    fn hex(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut endpoint = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            endpoint.push(char::from(HEX[(byte >> 4) as usize]));
            endpoint.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
        endpoint
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use fe2o3_artifact_transaction::{BuildAttempt, BuildInvocation};
        use fe2o3_source_isa_observation::wire_v1::{
            MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1,
            MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1,
            SOURCE_ISA_COLLECTION_HEADER_BYTES_V1, SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1,
            SOURCE_ISA_COLLECTION_IDENTITY_DOMAIN_V1, SOURCE_ISA_COLLECTION_MAGIC_V1,
            SourceIsaObservationContextV1, SourceIsaObservationErrorCodeV1,
            SourceIsaObservationFrameV1, SourceIsaObservationOutcomeV1,
            SourceIsaObservationUnavailableReasonV1, source_isa_collection_encoded_length,
        };

        fn liveness() -> InvocationLiveness {
            InvocationLiveness {
                client: ProcessIdentityV1::observe(std::process::id()).unwrap(),
                started_at: Instant::now(),
                frame_timeout: Duration::from_millis(100),
                lifetime: Duration::from_secs(1),
            }
        }

        fn read_request(encoded: &[u8]) -> io::Result<BrokerInvocationRequest> {
            let (mut writer, reader) = UnixStream::pair().unwrap();
            writer.write_all(encoded).unwrap();
            writer.shutdown(Shutdown::Write).unwrap();
            read_invocation_request(liveness(), &reader)
        }

        fn attempt(generation: u64, session: [u8; 16], invocation: [u8; 32]) -> BuildAttempt {
            BuildAttempt::from_env_value(&format!(
                "{generation}:{}:{}",
                BuildSession::from_bytes(session),
                BuildInvocation::from_bytes(invocation)
            ))
            .unwrap()
        }

        fn frame_with_context(
            config: [u8; 32],
            unit: [u8; 32],
            attempt: BuildAttempt,
            outcome: SourceIsaObservationOutcomeV1,
        ) -> SourceIsaObservationFrameV1 {
            SourceIsaObservationFrameV1::new(
                SourceIsaObservationContextV1::new(
                    config,
                    unit,
                    crate::source_isa_observation::inert_source_isa_attempt_v1(attempt).unwrap(),
                    [0x33; 32],
                )
                .unwrap(),
                outcome,
            )
        }

        fn frame(
            unit: [u8; 32],
            outcome: SourceIsaObservationOutcomeV1,
        ) -> SourceIsaObservationFrameV1 {
            frame_with_context(
                [0x30; 32],
                unit,
                attempt(3, [0x31; 16], [0x32; 32]),
                outcome,
            )
        }

        fn collector(units: &[[u8; 32]]) -> SourceIsaObservationCollectorStateV1 {
            SourceIsaObservationCollectorStateV1::with_expected_context(
                [0x30; 32],
                BuildSession::from_bytes([0x31; 16]),
                units,
            )
            .unwrap()
        }

        fn observer(units: &[[u8; 32]]) -> BrokerSourceIsaObserverV1 {
            BrokerSourceIsaObserverV1 {
                config_identity: [0x30; 32],
                session: BuildSession::from_bytes([0x31; 16]),
                selected_units: units.to_vec(),
                collector: Arc::new(Mutex::new(collector(units))),
            }
        }

        #[test]
        fn invocation_request_dispatch_preserves_v1_and_accepts_exact_v2_width() {
            let v1 = BrokeredInvocationCapabilityRequestV1::Release.encode();
            assert!(matches!(
                read_request(&v1).unwrap(),
                BrokerInvocationRequest::V1(BrokeredInvocationCapabilityRequestV1::Release)
            ));

            let expected = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                [0x30; 32],
                [0x40; 32],
                attempt(3, [0x31; 16], [0x32; 32]),
            )
            .unwrap();
            assert!(matches!(
                read_request(&expected.encode()).unwrap(),
                BrokerInvocationRequest::V2(actual) if actual == expected
            ));
        }

        #[test]
        fn invocation_request_dispatch_rejects_unknown_and_every_truncated_width() {
            assert!(read_request(b"UNKNOWN!").is_err());
            let requests = [
                BrokeredInvocationCapabilityRequestV1::Release
                    .encode()
                    .to_vec(),
                BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                    [0x30; 32],
                    [0x40; 32],
                    attempt(3, [0x31; 16], [0x32; 32]),
                )
                .unwrap()
                .encode()
                .to_vec(),
            ];
            for request in requests {
                for length in 0..request.len() {
                    assert!(
                        read_request(&request[..length]).is_err(),
                        "truncated request length {length} was accepted"
                    );
                }
            }
        }

        #[test]
        fn v1_release_waits_for_the_frozen_prepared_ack_and_rejects_substitution() {
            let (client, mut server) = UnixStream::pair().unwrap();
            let (result_sender, result_receiver) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                result_sender
                    .send(BrokeredInvocationAuthorityV1 { stream: client }.release())
                    .unwrap();
            });
            let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V1];
            server.read_exact(&mut request).unwrap();
            assert_eq!(
                BrokeredInvocationCapabilityRequestV1::decode(&request),
                Ok(BrokeredInvocationCapabilityRequestV1::Release)
            );
            assert!(matches!(
                result_receiver.recv_timeout(Duration::from_millis(25)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            server.write_all(BROKERED_INVOCATION_PREPARED_V1).unwrap();
            assert!(
                result_receiver
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .is_ok()
            );
            worker.join().unwrap();

            for response in [Some([0xa5; 16]), None] {
                let (client, mut server) = UnixStream::pair().unwrap();
                let worker = std::thread::spawn(move || {
                    let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V1];
                    server.read_exact(&mut request).unwrap();
                    if let Some(response) = response {
                        server.write_all(&response).unwrap();
                    }
                });
                assert!(
                    BrokeredInvocationAuthorityV1 { stream: client }
                        .release()
                        .is_err()
                );
                worker.join().unwrap();
            }
        }

        #[test]
        fn observer_frame_requires_exact_request_config_unit_and_attempt() {
            let broker_session = BuildSession::from_bytes([0x31; 16]);
            let exact_attempt = attempt(3, [0x31; 16], [0x32; 32]);
            let request = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                [0x30; 32],
                [0x40; 32],
                exact_attempt,
            )
            .unwrap();
            let selected_units = vec![[0x40; 32], [0x41; 32]];
            let observer = observer(&selected_units);
            assert!(observer.accepts(request));
            assert!(
                observer.accepts(
                    BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                        [0x30; 32],
                        [0x41; 32],
                        exact_attempt,
                    )
                    .unwrap()
                )
            );
            let outcome = SourceIsaObservationOutcomeV1::Unavailable(
                SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
            );
            let exact_frame = frame_with_context([0x30; 32], [0x40; 32], exact_attempt, outcome);
            assert!(
                validate_source_isa_observer_frame_binding(broker_session, request, &exact_frame,)
                    .is_ok()
            );

            for substituted in [
                frame_with_context([0x35; 32], [0x40; 32], exact_attempt, outcome),
                // A different configured unit must not substitute for the request's exact unit.
                frame_with_context([0x30; 32], [0x41; 32], exact_attempt, outcome),
                frame_with_context(
                    [0x30; 32],
                    [0x40; 32],
                    attempt(4, [0x31; 16], [0x32; 32]),
                    outcome,
                ),
                frame_with_context(
                    [0x30; 32],
                    [0x40; 32],
                    attempt(3, [0x31; 16], [0x36; 32]),
                    outcome,
                ),
            ] {
                assert!(
                    validate_source_isa_observer_frame_binding(
                        broker_session,
                        request,
                        &substituted,
                    )
                    .is_err()
                );
            }

            for substituted_attempt in [
                attempt(4, [0x31; 16], [0x32; 32]),
                attempt(3, [0x31; 16], [0x36; 32]),
            ] {
                let substituted_request =
                    BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                        [0x30; 32],
                        [0x40; 32],
                        substituted_attempt,
                    )
                    .unwrap();
                assert!(
                    validate_source_isa_observer_frame_binding(
                        broker_session,
                        substituted_request,
                        &exact_frame,
                    )
                    .is_err()
                );
            }

            let wrong_session_attempt = attempt(3, [0x37; 16], [0x32; 32]);
            let wrong_session_request =
                BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                    [0x30; 32],
                    [0x40; 32],
                    wrong_session_attempt,
                )
                .unwrap();
            assert!(
                validate_source_isa_observer_frame_binding(
                    broker_session,
                    wrong_session_request,
                    &frame_with_context([0x30; 32], [0x40; 32], wrong_session_attempt, outcome,),
                )
                .is_err()
            );
        }

        #[test]
        fn observer_sink_is_returned_only_after_exact_server_preparation() {
            fn release(
                observer: BrokerSourceIsaObserverV1,
                config: [u8; 32],
                unit: [u8; 32],
                request_attempt: BuildAttempt,
            ) -> Result<SourceIsaObservationSinkV1, String> {
                let (client, server) = UnixStream::pair().unwrap();
                let worker = std::thread::spawn(move || {
                    let BrokerInvocationRequest::V2(request) =
                        read_invocation_request(liveness(), &server).unwrap()
                    else {
                        panic!("expected V2 observer request");
                    };
                    prepare_source_isa_observer_request(
                        &observer,
                        BuildSession::from_bytes([0x31; 16]),
                        &server,
                        request,
                    )
                });
                let result = BrokeredInvocationAuthorityV1 { stream: client }
                    .release_with_source_isa_observer(config, unit, request_attempt);
                let server_result = worker.join().unwrap();
                assert_eq!(result.is_ok(), server_result.is_ok());
                result
            }

            let units = [[0x40; 32], [0x41; 32]];
            let exact_attempt = attempt(3, [0x31; 16], [0x32; 32]);

            let (client, mut server) = UnixStream::pair().unwrap();
            let (result_sender, result_receiver) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let result = BrokeredInvocationAuthorityV1 { stream: client }
                    .release_with_source_isa_observer([0x30; 32], [0x40; 32], exact_attempt)
                    .map(drop);
                result_sender.send(result).unwrap();
            });
            let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V2];
            server.read_exact(&mut request).unwrap();
            assert!(BrokeredInvocationCapabilityRequestV2::decode(&request).is_ok());
            assert!(matches!(
                result_receiver.recv_timeout(Duration::from_millis(25)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            server.write_all(BROKERED_SOURCE_ISA_PREPARED_V1).unwrap();
            assert!(
                result_receiver
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .is_ok()
            );
            worker.join().unwrap();

            assert!(release(observer(&units), [0x30; 32], units[0], exact_attempt).is_ok());
            assert!(release(observer(&units), [0x35; 32], units[0], exact_attempt).is_err());
            assert!(release(observer(&units), [0x30; 32], [0x45; 32], exact_attempt).is_err());
            assert!(
                release(
                    observer(&units),
                    [0x30; 32],
                    units[0],
                    attempt(3, [0x37; 16], [0x32; 32]),
                )
                .is_err()
            );

            let (client, mut server) = UnixStream::pair().unwrap();
            let worker = std::thread::spawn(move || {
                let mut request = [0; BROKERED_INVOCATION_REQUEST_BYTES_V2];
                server.read_exact(&mut request).unwrap();
                server.write_all(&[0xa5; 16]).unwrap();
            });
            assert!(
                BrokeredInvocationAuthorityV1 { stream: client }
                    .release_with_source_isa_observer([0x30; 32], units[0], exact_attempt,)
                    .is_err()
            );
            worker.join().unwrap();

            let mut direct =
                BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                    [0x30; 32],
                    units[0],
                    exact_attempt,
                )
                .unwrap()
                .encode();
            direct[88..136].fill(0);
            assert!(read_request(&direct).is_err());
            assert!(
                BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                    [0x30; 32],
                    units[0],
                    attempt(3, [0; 16], [0; 32]),
                )
                .is_err()
            );
        }

        #[test]
        fn collector_deduplicates_exact_recovery_and_preserves_partial_failure() {
            let units = [[0x40; 32], [0x41; 32]];
            let mut collector = collector(&units);
            let accepted = frame(
                units[0],
                SourceIsaObservationOutcomeV1::Unavailable(
                    SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
                ),
            );
            assert!(collector.insert(accepted.clone()).is_ok());
            assert!(collector.insert(accepted).is_ok());
            collector.fail(SourceIsaObservationTransportFailureV1::RejectedFrame);
            let conflicting = frame(
                units[0],
                SourceIsaObservationOutcomeV1::Error(
                    SourceIsaObservationErrorCodeV1::ResourceLimit,
                ),
            );
            assert_eq!(
                collector.insert(conflicting),
                Err(SourceIsaObservationTransportFailureV1::ConflictingDuplicate)
            );
            collector
                .insert(frame(
                    units[1],
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                    ),
                ))
                .unwrap();
            collector.fail(SourceIsaObservationTransportFailureV1::AggregateByteBound);
            let collection = collector.finish();
            assert_eq!(collection.frames().len(), 2);
            assert!(collection.missing_units().is_empty());
            assert_eq!(
                collection.failure(),
                Some(SourceIsaObservationTransportFailureV1::RejectedFrame)
            );
            let encoded = collection.encode_canonical().unwrap();
            assert_eq!(&encoded[..8], SOURCE_ISA_COLLECTION_MAGIC_V1);
            assert_eq!(&encoded[32..64], &[0x30; 32]);
            assert_eq!(&encoded[64..80], &[0x31; 16]);
            assert_eq!(u32::from_le_bytes(encoded[16..20].try_into().unwrap()), 2);
            assert_eq!(
                u16::from_le_bytes(encoded[24..26].try_into().unwrap()),
                SourceIsaObservationTransportFailureV1::RejectedFrame.code()
            );
            assert!(!collection.grants_compiler_authority());
            assert!(!collection.grants_publication_authority());
            assert!(!collection.grants_runtime_authority());
            assert_eq!(
                SourceIsaObservationCollectionV1::decode_canonical(&encoded),
                Ok(collection)
            );
        }

        #[test]
        fn collector_deduplicates_exact_recovery_at_the_unit_bound() {
            let units = (0..MAX_SOURCE_ISA_OBSERVATION_UNITS_V1)
                .map(|index| {
                    let mut unit = [0x40; 32];
                    unit[..8].copy_from_slice(&(index as u64).to_le_bytes());
                    unit
                })
                .collect::<Vec<_>>();
            let mut collector = collector(&units);
            for &unit in &units {
                collector
                    .insert(frame(
                        unit,
                        SourceIsaObservationOutcomeV1::Unavailable(
                            SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                        ),
                    ))
                    .unwrap();
            }

            assert!(
                collector
                    .insert(frame(
                        units[MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 - 1],
                        SourceIsaObservationOutcomeV1::Unavailable(
                            SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                        ),
                    ))
                    .is_ok()
            );
            let collection = collector.finish();
            assert_eq!(
                collection.frames().len(),
                MAX_SOURCE_ISA_OBSERVATION_UNITS_V1
            );
            assert!(collection.missing_units().is_empty());
            assert_eq!(collection.failure(), None);
        }

        #[test]
        fn collector_reports_missing_selected_units_without_discarding_frames() {
            let units = [[0x40; 32], [0x41; 32]];
            let mut collector = collector(&units);
            collector
                .insert(frame(
                    units[1],
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                    ),
                ))
                .unwrap();
            let collection = collector.finish();
            assert_eq!(collection.frames().len(), 1);
            assert_eq!(collection.missing_units(), &[units[0]]);
            assert_eq!(
                collection.failure(),
                Some(SourceIsaObservationTransportFailureV1::MissingSelectedUnits)
            );
        }

        #[test]
        fn all_missing_collection_retains_exact_config_and_session() {
            let collection = collector(&[[0x40; 32], [0x41; 32]]).finish();
            assert_eq!(collection.config_identity(), [0x30; 32]);
            assert_eq!(
                collection.session(),
                crate::source_isa_observation::inert_source_isa_session_v1(
                    BuildSession::from_bytes([0x31; 16])
                )
            );
            assert_eq!(collection.frames().len(), 0);
            assert_eq!(collection.missing_units(), &[[0x40; 32], [0x41; 32]]);
            assert_eq!(
                SourceIsaObservationCollectionV1::decode_canonical(
                    &collection.encode_canonical().unwrap()
                ),
                Ok(collection)
            );
        }

        #[test]
        fn canonical_collection_length_is_bounded_and_fallible() {
            assert_eq!(
                source_isa_collection_encoded_length(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1, 0,),
                Ok(MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1)
            );
            assert_eq!(MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1, 696_432);
            assert!(
                source_isa_collection_encoded_length(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1, 1,)
                    .is_err()
            );
            assert!(
                source_isa_collection_encoded_length(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 + 1, 0,)
                    .is_err()
            );
            assert!(source_isa_collection_encoded_length(usize::MAX, usize::MAX).is_err());
            assert_eq!(
                MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1,
                MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 * 2
            );
            assert_eq!(
                MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1,
                1_392_864
            );
        }

        #[test]
        fn canonical_collection_decoder_rejects_hostile_framing_and_payloads() {
            fn rehash(encoded: &mut [u8]) {
                let identity_start = encoded.len() - SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1;
                let mut digest = Sha256::new();
                digest.update(SOURCE_ISA_COLLECTION_IDENTITY_DOMAIN_V1);
                digest.update(&encoded[..identity_start]);
                encoded[identity_start..].copy_from_slice(&digest.finalize());
            }

            let units = [[0x40; 32], [0x41; 32]];
            let mut collector = collector(&units);
            collector
                .insert(frame(
                    units[0],
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                    ),
                ))
                .unwrap();
            let encoded = collector.finish().encode_canonical().unwrap();

            let mut truncated = encoded.clone();
            truncated.pop();
            assert!(SourceIsaObservationCollectionV1::decode_canonical(&truncated).is_err());
            let mut trailing = encoded.clone();
            trailing.push(0);
            assert!(SourceIsaObservationCollectionV1::decode_canonical(&trailing).is_err());
            let oversized = vec![0; MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 + 1];
            assert!(SourceIsaObservationCollectionV1::decode_canonical(&oversized).is_err());

            let mut over_combined_count = encoded.clone();
            over_combined_count[16..20]
                .copy_from_slice(&(MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 as u32).to_le_bytes());
            over_combined_count[20..24].copy_from_slice(&1_u32.to_le_bytes());
            rehash(&mut over_combined_count);
            assert!(
                SourceIsaObservationCollectionV1::decode_canonical(&over_combined_count).is_err()
            );

            for substituted in [
                frame_with_context(
                    [0x35; 32],
                    units[0],
                    attempt(3, [0x31; 16], [0x32; 32]),
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                    ),
                ),
                frame_with_context(
                    [0x30; 32],
                    units[0],
                    attempt(3, [0x36; 16], [0x32; 32]),
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
                    ),
                ),
            ] {
                let mut mixed_context = encoded.clone();
                mixed_context[SOURCE_ISA_COLLECTION_HEADER_BYTES_V1
                    ..SOURCE_ISA_COLLECTION_HEADER_BYTES_V1
                        + SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1]
                    .copy_from_slice(&substituted.encode());
                rehash(&mut mixed_context);
                assert!(
                    SourceIsaObservationCollectionV1::decode_canonical(&mixed_context).is_err()
                );
            }

            for offset in [0, 8, 12, 16, 20, 24, 28, 32, 64, 80, encoded.len() - 1] {
                let mut changed = encoded.clone();
                changed[offset] ^= 1;
                assert!(
                    SourceIsaObservationCollectionV1::decode_canonical(&changed).is_err(),
                    "hostile byte {offset} was accepted"
                );
            }

            let mut unknown_failure = encoded.clone();
            unknown_failure[24..26].copy_from_slice(&99_u16.to_le_bytes());
            rehash(&mut unknown_failure);
            assert!(SourceIsaObservationCollectionV1::decode_canonical(&unknown_failure).is_err());

            let mut nonzero_truth = encoded;
            nonzero_truth[28] = 1;
            rehash(&mut nonzero_truth);
            assert!(SourceIsaObservationCollectionV1::decode_canonical(&nonzero_truth).is_err());
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) use platform::*;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod unsupported {
    use fe2o3_artifact_transaction::{
        BrokeredInvocationCapabilityClaimV1, BuildAttempt, BuildSession,
    };

    use crate::build_config::ProductionSourceIsaObserverPolicyV1;
    use crate::cargo_invocation_boundary::InvocationAuthorizationRegistryV1;
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::PinnedExecutable;
    use crate::project::PinnedDirectory;
    use fe2o3_compiler_closure_capability::{
        CompilerClosureCapabilityV1, CompilerExecutionClientProfileCapabilityV1,
    };
    use fe2o3_source_isa_observation::wire_v1::{
        SourceIsaObservationCollectionV1, SourceIsaObservationFrameV1,
    };

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CapabilityProfileV1 {
        Ordinary,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct CapabilityBindingV3;

    impl CapabilityBindingV3 {
        pub(crate) fn new(
            _profile: CapabilityProfileV1,
            _config_identity: Option<[u8; 32]>,
            _compiler_closure_sha256: [u8; 32],
            _rustc_executable_sha256: [u8; 32],
            _retained_object_binding_sha256: [u8; 32],
        ) -> Result<Self, String> {
            Ok(Self)
        }

        pub(crate) fn from_environment_for_client(
            _profile: CapabilityProfileV1,
            _config_identity: Option<[u8; 32]>,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) fn new_protected(
            _profile: CapabilityProfileV1,
            _config_identity: Option<[u8; 32]>,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _retained_object_binding_sha256: [u8; 32],
        ) -> Result<Self, String> {
            Ok(Self)
        }

        pub(crate) const fn compiler_closure_sha256(self) -> [u8; 32] {
            [0; 32]
        }

        pub(crate) const fn requires_compiler_closure_v2(self) -> bool {
            false
        }

        pub(crate) const fn rustc_executable_sha256(self) -> [u8; 32] {
            [0; 32]
        }

        pub(crate) const fn retained_object_binding_sha256(self) -> [u8; 32] {
            [0; 32]
        }
    }

    pub(crate) struct CapabilityBroker;

    impl CapabilityBroker {
        pub(crate) fn start(
            _session: BuildSession,
            _binding: CapabilityBindingV3,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn start_protected(
            _session: BuildSession,
            _binding: CapabilityBindingV3,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn start_protected_with_source_isa_observer(
            _session: BuildSession,
            _binding: CapabilityBindingV3,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
            _observer_policy: &ProductionSourceIsaObserverPolicyV1,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn route(&self) -> &str {
            ""
        }

        pub(crate) fn invocation_authorization(&self) -> InvocationAuthorizationRegistryV1 {
            InvocationAuthorizationRegistryV1::new()
        }

        pub(crate) fn finish_source_isa_observations(
            self,
        ) -> Result<SourceIsaObservationCollectionV1, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }
    }

    pub(crate) struct BrokeredInvocationAuthorityV1;

    impl BrokeredInvocationAuthorityV1 {
        pub(crate) fn release(self) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) fn release_with_source_isa_observer(
            self,
            _config_identity: [u8; 32],
            _unit_identity: [u8; 32],
            _attempt: BuildAttempt,
        ) -> Result<SourceIsaObservationSinkV1, String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) fn prepare(
            &self,
            _claim: BrokeredInvocationCapabilityClaimV1,
        ) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }
    }

    pub(crate) struct SourceIsaObservationSinkV1;

    impl SourceIsaObservationSinkV1 {
        pub(crate) fn submit(self, _frame: &SourceIsaObservationFrameV1) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }
    }

    pub(crate) struct BrokeredCapabilities {
        pub(crate) backend: PinnedCodegenBackend,
        pub(crate) artifact: PinnedDirectory,
        pub(crate) compiler_closure: Option<CompilerClosureCapabilityV1>,
        pub(crate) compiler_execution_profile: Option<CompilerExecutionClientProfileCapabilityV1>,
        pub(crate) invocation_authority: Option<BrokeredInvocationAuthorityV1>,
    }

    pub(crate) fn receive(
        _session: BuildSession,
        _binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        Err("Cargo capability transport requires Linux".to_string())
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(crate) use unsupported::*;
