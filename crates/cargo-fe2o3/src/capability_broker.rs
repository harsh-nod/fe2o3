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
        BROKERED_INVOCATION_REQUEST_BYTES_V1, BrokeredInvocationCapabilityRequestV1, BuildSession,
    };
    use fe2o3_process_identity::LinuxObjectIdentityV3;
    use rustix::net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
    };
    use sha2::{Digest, Sha256};

    use crate::cargo_invocation_boundary::{InvocationAuthorizationRegistryV1, ProcessIdentityV1};
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
    use crate::project::PinnedDirectory;
    use fe2o3_compiler_closure_capability::CompilerClosureCapabilityV1;

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
            self.profile.descriptor_count() + self.protected_compiler_closure_v2 as usize
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
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
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
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Self::start_with_compiler_closure(
                session,
                binding,
                Some(compiler_closure),
                backend,
                artifact,
                pinned_cargo_image,
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
            Self::start_with_compiler_closure(
                session,
                binding,
                None,
                backend,
                artifact,
                pinned_cargo_image,
                limits,
            )
        }

        fn start_with_compiler_closure(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler_closure: Option<fe2o3_build_authority::CompilerClosureV2>,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            limits: BrokerLimits,
        ) -> Result<Self, String> {
            if limits.max_active_connections == 0
                || limits.authentication_timeout.is_zero()
                || limits.invocation_frame_timeout.is_zero()
                || limits.invocation_lifetime.is_zero()
            {
                return Err("capability broker limits must be nonzero".to_owned());
            }
            if binding.requires_compiler_closure_v2() != compiler_closure.is_some() {
                return Err(
                    "capability binding and compiler-closure descriptor presence differ".to_owned(),
                );
            }
            let compiler_closure = compiler_closure
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
        pub(crate) invocation_authority: Option<BrokeredInvocationAuthorityV1>,
    }

    pub(crate) struct BrokeredInvocationAuthorityV1 {
        stream: UnixStream,
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
            let mut encoded = [0_u8; BROKERED_INVOCATION_REQUEST_BYTES_V1];
            liveness.read_frame(stream, &mut encoded)?;
            let request = BrokeredInvocationCapabilityRequestV1::decode(&encoded)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
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
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) use platform::*;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod unsupported {

    use fe2o3_artifact_transaction::BuildSession;

    use crate::cargo_invocation_boundary::InvocationAuthorizationRegistryV1;
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::PinnedExecutable;
    use crate::project::PinnedDirectory;
    use fe2o3_compiler_closure_capability::CompilerClosureCapabilityV1;

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
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn route(&self) -> &str {
            ""
        }

        pub(crate) fn invocation_authorization(&self) -> InvocationAuthorizationRegistryV1 {
            InvocationAuthorizationRegistryV1::new()
        }
    }

    pub(crate) struct BrokeredInvocationAuthorityV1;

    impl BrokeredInvocationAuthorityV1 {
        pub(crate) fn release(self) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) fn prepare(
            &self,
            _claim: BrokeredInvocationCapabilityClaimV1,
        ) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }
    }

    pub(crate) struct BrokeredCapabilities {
        pub(crate) backend: PinnedCodegenBackend,
        pub(crate) artifact: PinnedDirectory,
        pub(crate) compiler_closure: Option<CompilerClosureCapabilityV1>,
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
