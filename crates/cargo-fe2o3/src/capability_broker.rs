//! Descriptor transport from the `cargo-fe2o3` parent to managed rustc wrappers.
//!
//! Cargo receives a strict per-instance route and build-session binding. Both peers check Linux
//! credentials, exact executable identity, the prepared profile/config identity, and a
//! challenge-response bound to a separate 256-bit broker secret before transferring a
//! sealed backend image and a read-only artifact-directory descriptor with `SCM_RIGHTS`. The
//! explicit S09 profile additionally receives an observed pinned Cargo image. A protected release
//! also receives one sealed descriptor carrying the complete admitted compiler-closure preimage.
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
    #[cfg(test)]
    use std::fs::OpenOptions;
    use std::fs::{self, File};
    use std::io::{self, IoSlice, IoSliceMut, Read, Write};
    use std::mem::MaybeUninit;
    use std::net::Shutdown;
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd, IntoRawFd, OwnedFd};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    #[cfg(any(test, feature = "qualification-oracles-test-only"))]
    use fe2o3_artifact_transaction::BrokeredInvocationCapabilityClaimV1;
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
    const S09_REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-10\0";
    const _: () = assert!(REQUEST_MAGIC.len() == S09_REQUEST_MAGIC.len());
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
    pub(crate) const INVOCATION_AUTHORITY_CHILD_FD_V1: i32 =
        fe2o3_artifact_transaction::BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1;
    const BROKER_AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(30);
    const BROKER_CLIENT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
    const _: () = assert!(
        BROKER_CLIENT_RESPONSE_TIMEOUT.as_secs()
            >= BROKER_AUTHENTICATION_TIMEOUT.as_secs().saturating_mul(2)
    );
    const BROKER_INVOCATION_FRAME_TIMEOUT: Duration = Duration::from_secs(30);
    const BROKER_INVOCATION_LIFETIME: Duration = Duration::from_secs(6 * 60 * 60);
    #[cfg(test)]
    const BROKER_IO_TIMEOUT: Duration = BROKER_AUTHENTICATION_TIMEOUT;
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
        S09,
    }

    impl CapabilityProfileV1 {
        const fn request_magic(self) -> &'static [u8] {
            match self {
                Self::Ordinary => REQUEST_MAGIC,
                Self::S09 => S09_REQUEST_MAGIC,
            }
        }

        const fn descriptor_count(self) -> usize {
            match self {
                Self::Ordinary => 2,
                Self::S09 => 3,
            }
        }

        const fn name(self) -> &'static str {
            match self {
                Self::Ordinary => "ordinary",
                Self::S09 => "S09",
            }
        }

        const fn route_name(self) -> &'static str {
            match self {
                Self::Ordinary => "ordinary",
                Self::S09 => "s09",
            }
        }

        fn parse_route_name(value: &str) -> Option<Self> {
            match value {
                "ordinary" => Some(Self::Ordinary),
                "s09" => Some(Self::S09),
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
            if profile == CapabilityProfileV1::S09 && config_identity.is_none() {
                return Err("S09 capability binding requires a Worker V2 config identity".into());
            }
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
        #[cfg(test)]
        _test_permit: TestBrokerPermit,
    }

    #[cfg(test)]
    static TEST_BROKER_ACTIVE: Mutex<bool> = Mutex::new(false);
    #[cfg(test)]
    static TEST_BROKER_AVAILABLE: Condvar = Condvar::new();

    #[cfg(test)]
    struct TestBrokerPermit {
        process_lock: File,
    }

    #[cfg(test)]
    impl TestBrokerPermit {
        fn acquire() -> Self {
            let mut active = TEST_BROKER_ACTIVE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while *active {
                active = TEST_BROKER_AVAILABLE
                    .wait(active)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            *active = true;
            drop(active);

            let process_lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open("/tmp/fe2o3-capability-broker-tests.lock")
                .unwrap_or_else(|error| panic!("cannot open broker test process lock: {error}"));
            if unsafe { libc::flock(process_lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
                let error = io::Error::last_os_error();
                let mut active = TEST_BROKER_ACTIVE
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *active = false;
                drop(active);
                TEST_BROKER_AVAILABLE.notify_one();
                panic!("cannot acquire broker test process lock: {error}");
            }
            Self { process_lock }
        }
    }

    #[cfg(test)]
    impl Drop for TestBrokerPermit {
        fn drop(&mut self) {
            if unsafe { libc::flock(self.process_lock.as_raw_fd(), libc::LOCK_UN) } != 0 {
                std::process::abort();
            }
            let mut active = TEST_BROKER_ACTIVE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *active = false;
            drop(active);
            TEST_BROKER_AVAILABLE.notify_one();
        }
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
        #[cfg(test)]
        accept_pause: Mutex<Option<Arc<TestPause>>>,
        #[cfg(test)]
        worker_pause: Mutex<Option<Arc<TestPause>>>,
        #[cfg(test)]
        dispatch_pause: Mutex<Option<Arc<TestPause>>>,
        #[cfg(test)]
        locked_dispatch_pause: Mutex<Option<Arc<TestPause>>>,
        #[cfg(test)]
        begin_started: std::sync::atomic::AtomicBool,
        #[cfg(test)]
        request_read_started: std::sync::atomic::AtomicBool,
        #[cfg(test)]
        panic_next_worker: std::sync::atomic::AtomicBool,
        #[cfg(test)]
        fail_next_worker_spawn: std::sync::atomic::AtomicBool,
        #[cfg(test)]
        caught_worker_panics: std::sync::atomic::AtomicUsize,
        #[cfg(test)]
        admission_rejections: std::sync::atomic::AtomicUsize,
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
                #[cfg(test)]
                accept_pause: Mutex::new(None),
                #[cfg(test)]
                worker_pause: Mutex::new(None),
                #[cfg(test)]
                dispatch_pause: Mutex::new(None),
                #[cfg(test)]
                locked_dispatch_pause: Mutex::new(None),
                #[cfg(test)]
                begin_started: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                request_read_started: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                panic_next_worker: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                fail_next_worker_spawn: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                caught_worker_panics: std::sync::atomic::AtomicUsize::new(0),
                #[cfg(test)]
                admission_rejections: std::sync::atomic::AtomicUsize::new(0),
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
                #[cfg(test)]
                self.admission_rejections
                    .fetch_add(1, std::sync::atomic::Ordering::Release);
                let _ = stream.shutdown(Shutdown::Both);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "capability broker is at active connection capacity",
                ));
            }
            if let Err(error) = deadline.require_remaining() {
                #[cfg(test)]
                self.admission_rejections
                    .fetch_add(1, std::sync::atomic::Ordering::Release);
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
            #[cfg(test)]
            self.begin_started
                .store(true, std::sync::atomic::Ordering::Release);
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
            #[cfg(test)]
            self.pause(&self.dispatch_pause, None);

            let state = self.state();
            if state.stopping {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "capability broker is shutting down",
                ));
            }
            #[cfg(test)]
            self.pause(&self.locked_dispatch_pause, None);
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

        #[cfg(test)]
        fn install_pause(slot: &Mutex<Option<Arc<TestPause>>>) -> TestPauseControl {
            let (pause, control) = TestPause::new();
            *slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pause);
            control
        }

        #[cfg(test)]
        fn pause(&self, slot: &Mutex<Option<Arc<TestPause>>>, socket_identity: Option<(u64, u64)>) {
            let pause = {
                slot.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
            };
            if let Some(pause) = pause {
                pause.server_wait(socket_identity);
            }
        }

        #[cfg(test)]
        fn install_accept_pause(&self) -> TestPauseControl {
            Self::install_pause(&self.accept_pause)
        }

        #[cfg(test)]
        fn pause_after_accept(&self, stream: &UnixStream) {
            let socket_identity = fs::metadata(format!("/proc/self/fd/{}", stream.as_raw_fd()))
                .ok()
                .map(|metadata| (metadata.dev(), metadata.ino()));
            let pause = self
                .accept_pause
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(pause) = pause {
                pause.server_wait(socket_identity);
            }
        }

        #[cfg(test)]
        fn install_dispatch_pause(&self) -> TestPauseControl {
            Self::install_pause(&self.dispatch_pause)
        }

        #[cfg(test)]
        fn install_worker_pause(&self) -> TestPauseControl {
            Self::install_pause(&self.worker_pause)
        }

        #[cfg(test)]
        fn remove_worker_pause(&self) {
            self.worker_pause
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }

        #[cfg(test)]
        fn install_locked_dispatch_pause(&self) -> TestPauseControl {
            Self::install_pause(&self.locked_dispatch_pause)
        }

        #[cfg(test)]
        fn wait_for_begin(&self) {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !self
                .begin_started
                .load(std::sync::atomic::Ordering::Acquire)
            {
                assert!(
                    std::time::Instant::now() < deadline,
                    "capability broker shutdown did not start"
                );
                thread::yield_now();
            }
        }

        #[cfg(test)]
        fn wait_for_request_read(&self) {
            let deadline = std::time::Instant::now() + BROKER_AUTHENTICATION_TIMEOUT;
            while !self
                .request_read_started
                .load(std::sync::atomic::Ordering::Acquire)
            {
                assert!(
                    std::time::Instant::now() < deadline,
                    "capability broker did not reach the request read"
                );
                thread::yield_now();
            }
        }

        #[cfg(test)]
        fn active_socket_identity(&self) -> Option<(u64, u64)> {
            let state = self.state();
            let active = state.active.values().next()?;
            fs::metadata(format!("/proc/self/fd/{}", active.as_raw_fd()))
                .ok()
                .map(|metadata| (metadata.dev(), metadata.ino()))
        }

        #[cfg(test)]
        fn active_connection_count(&self) -> usize {
            self.state().active.len()
        }

        #[cfg(test)]
        fn inject_worker_panic(&self) {
            self.panic_next_worker
                .store(true, std::sync::atomic::Ordering::Release);
        }

        #[cfg(test)]
        fn inject_worker_spawn_failure(&self) {
            self.fail_next_worker_spawn
                .store(true, std::sync::atomic::Ordering::Release);
        }

        #[cfg(test)]
        fn take_worker_spawn_failure(&self) -> bool {
            self.fail_next_worker_spawn
                .swap(false, std::sync::atomic::Ordering::AcqRel)
        }

        #[cfg(test)]
        fn maybe_inject_worker_panic(&self) {
            if self
                .panic_next_worker
                .swap(false, std::sync::atomic::Ordering::AcqRel)
            {
                panic!("injected capability broker worker panic");
            }
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

    #[cfg(test)]
    struct TestPause {
        reached: std::sync::mpsc::SyncSender<Option<(u64, u64)>>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
    }

    #[cfg(test)]
    struct TestPauseControl {
        reached: std::sync::mpsc::Receiver<Option<(u64, u64)>>,
        release: std::sync::mpsc::SyncSender<()>,
    }

    #[cfg(test)]
    impl TestPause {
        fn new() -> (Arc<Self>, TestPauseControl) {
            let (reached_tx, reached_rx) = std::sync::mpsc::sync_channel(0);
            let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
            (
                Arc::new(Self {
                    reached: reached_tx,
                    release: Mutex::new(release_rx),
                }),
                TestPauseControl {
                    reached: reached_rx,
                    release: release_tx,
                },
            )
        }

        fn server_wait(&self, socket_identity: Option<(u64, u64)>) {
            if self.reached.send(socket_identity).is_ok() {
                let _ = self
                    .release
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .recv();
            }
        }
    }

    #[cfg(test)]
    impl TestPauseControl {
        fn wait_until_reached(&self) -> Option<(u64, u64)> {
            self.reached
                .recv_timeout(BROKER_AUTHENTICATION_TIMEOUT)
                .expect("capability broker did not reach the test pause")
        }

        fn release(&self) {
            self.release
                .send(())
                .expect("capability broker did not wait for dispatch release");
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
            #[cfg(test)]
            let test_permit = TestBrokerPermit::acquire();
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
            let pinned_cargo_image =
                pinned_cargo_image
                    .try_clone_for_transfer()
                    .map_err(|error| {
                        format!("failed to retain pinned Cargo image observation: {error}")
                    })?;
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
                        pinned_cargo_image,
                        compiler_closure,
                        authentication_timeout: limits.authentication_timeout,
                        invocation_frame_timeout: limits.invocation_frame_timeout,
                        invocation_lifetime: limits.invocation_lifetime,
                        invocation_authorization: worker_invocation_authorization,
                        #[cfg(test)]
                        test_invocation_authorization: Mutex::new(()),
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
                #[cfg(test)]
                _test_permit: test_permit,
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
        pub(crate) pinned_cargo_image: Option<PinnedExecutable>,
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

        #[cfg(any(test, feature = "qualification-oracles-test-only"))]
        pub(crate) fn prepare(
            &self,
            claim: BrokeredInvocationCapabilityClaimV1,
        ) -> Result<(), String> {
            self.exchange(
                BrokeredInvocationCapabilityRequestV1::Prepare(claim),
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

        pub(crate) fn inherit_for_child(&self, command: &mut Command) -> Result<(), String> {
            // SAFETY: this only probes the process-local reserved descriptor.
            let target = unsafe { BorrowedFd::borrow_raw(INVOCATION_AUTHORITY_CHILD_FD_V1) };
            match rustix::io::fcntl_getfd(target) {
                Err(rustix::io::Errno::BADF) => {}
                Err(error) => {
                    return Err(format!(
                        "cannot inspect reserved invocation-capability descriptor: {error}"
                    ));
                }
                Ok(_) => {
                    return Err(
                        "reserved invocation-capability descriptor is already occupied".to_owned(),
                    );
                }
            }
            let source = self.stream.as_raw_fd();
            // SAFETY: `self.stream` remains alive through the synchronous spawn. The callback
            // duplicates only that authenticated connected socket onto the reserved child FD.
            unsafe {
                use std::os::unix::process::CommandExt as _;
                command.pre_exec(move || {
                    let installed = rustix::io::fcntl_dupfd_cloexec(
                        BorrowedFd::borrow_raw(source),
                        INVOCATION_AUTHORITY_CHILD_FD_V1,
                    )
                    .map_err(std::io::Error::from)?;
                    if installed.as_raw_fd() != INVOCATION_AUTHORITY_CHILD_FD_V1 {
                        return Err(std::io::Error::from_raw_os_error(
                            rustix::io::Errno::BUSY.raw_os_error(),
                        ));
                    }
                    rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty())
                        .map_err(std::io::Error::from)?;
                    let _ = installed.into_raw_fd();
                    Ok(())
                });
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
        let pinned_cargo_image = if binding.profile == CapabilityProfileV1::S09 {
            let image = normalize_received_descriptor(
                descriptors.pop().expect("S09 descriptor count checked"),
                "pinned Cargo image observation",
            )?;
            Some(
                PinnedExecutable::from_transferred_file(
                    image,
                    PathBuf::from("<brokered pinned Cargo image observation>"),
                )
                .map_err(|error| format!("invalid brokered pinned Cargo image: {error}"))?,
            )
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
            pinned_cargo_image,
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
        pinned_cargo_image: File,
        compiler_closure: Option<CompilerClosureCapabilityV1>,
        authentication_timeout: Duration,
        invocation_frame_timeout: Duration,
        invocation_lifetime: Duration,
        invocation_authorization: InvocationAuthorizationRegistryV1,
        #[cfg(test)]
        test_invocation_authorization: Mutex<()>,
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
                            #[cfg(test)]
                            self.shutdown.pause_after_accept(&stream);
                            match self.shutdown.register(&stream, deadline) {
                                Ok(Some(registry_guard)) => {
                                    let server = &self;
                                    let worker = move || {
                                        let _registry_guard = registry_guard;
                                        let outcome = std::panic::catch_unwind(
                                            std::panic::AssertUnwindSafe(|| {
                                                #[cfg(test)]
                                                server
                                                    .shutdown
                                                    .pause(&server.shutdown.worker_pause, None);
                                                #[cfg(test)]
                                                server.shutdown.maybe_inject_worker_panic();
                                                server.serve_one(&stream, deadline)
                                            }),
                                        );
                                        if outcome.is_err() {
                                            #[cfg(test)]
                                            server
                                                .shutdown
                                                .caught_worker_panics
                                                .fetch_add(1, std::sync::atomic::Ordering::Release);
                                        }
                                    };
                                    #[cfg(test)]
                                    let injected_spawn_failure =
                                        self.shutdown.take_worker_spawn_failure();
                                    #[cfg(not(test))]
                                    let injected_spawn_failure = false;
                                    let spawned = if injected_spawn_failure {
                                        Err(io::Error::other(
                                            "injected capability broker worker spawn failure",
                                        ))
                                    } else {
                                        thread::Builder::new().spawn_scoped(scope, worker)
                                    };
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
            #[cfg(test)]
            let test_authorization = self
                .test_invocation_authorization
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            #[cfg(test)]
            self.invocation_authorization
                .authorize_test_process(client)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            self.invocation_authorization
                .consume(client)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            #[cfg(test)]
            drop(test_authorization);
            deadline.require_remaining()?;
            let mut request = vec![0_u8; REQUEST_BYTES];
            #[cfg(test)]
            self.shutdown
                .request_read_started
                .store(true, std::sync::atomic::Ordering::Release);
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
            let profile = self.binding.profile;

            // SCM_RIGHTS preserves the open-file-description offset. Give each S09 wrapper an
            // independently opened description of the retained pinned image. Ordinary clients
            // retain their historical two-descriptor response.
            let pinned_cargo_image = (profile == CapabilityProfileV1::S09)
                .then(|| {
                    File::open(format!(
                        "/proc/self/fd/{}",
                        self.pinned_cargo_image.as_raw_fd()
                    ))
                })
                .transpose()?;
            deadline.require_remaining()?;
            let mut descriptors = vec![self.backend.as_fd(), self.artifact.as_fd()];
            if let Some(pinned_cargo_image) = &pinned_cargo_image {
                descriptors.push(pinned_cargo_image.as_fd());
            }
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
        #[cfg(test)]
        let bytes = {
            let mut bytes = bytes;
            static NEXT_TEST_ENDPOINT: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(1);
            bytes[..4].copy_from_slice(&std::process::id().to_le_bytes());
            bytes[4..12].copy_from_slice(
                &NEXT_TEST_ENDPOINT
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    .to_le_bytes(),
            );
            bytes
        };
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
        use std::collections::BTreeSet;
        use std::path::PathBuf;
        use std::process::{Child, Command, Stdio};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::{Arc, Barrier};
        use std::time::{Duration, Instant};

        use fe2o3_artifact_transaction::{
            BuildAttempt, BuildInvocation, ProducerIdentity, begin_build_attempt,
        };

        use super::*;

        static NEXT: AtomicU64 = AtomicU64::new(1);
        const MOCK_EXEC_READY_BOUND: Duration = Duration::from_secs(30);
        const PROMPT_SHUTDOWN_BOUND: Duration = Duration::from_secs(5);
        const _: () = assert!(PROMPT_SHUTDOWN_BOUND.as_secs() < BROKER_IO_TIMEOUT.as_secs());

        struct TestDirectory(PathBuf);

        impl TestDirectory {
            fn new() -> Self {
                fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
                let path = std::env::temp_dir().join(format!(
                    "cargo-fe2o3-capability-broker-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&path).unwrap();
                Self(path)
            }
        }

        impl Drop for TestDirectory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        struct ReapedChild(Child);

        impl Drop for ReapedChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        struct SpawnedMockExecutable {
            child: ReapedChild,
            start_time_ticks: u64,
            image: PinnedExecutable,
            metadata: fs::Metadata,
        }

        impl SpawnedMockExecutable {
            fn ready_shell() -> Self {
                const READY: &[u8] = b"fe2o3-ready";

                let shell = fs::canonicalize("/bin/sh").unwrap();
                let expected = PinnedExecutable::open(&shell).unwrap();
                let expected_object = expected.object_identity();
                let expected_sha256 = *expected.sha256();
                let (mut readiness, child_readiness) = UnixStream::pair().unwrap();
                readiness
                    .set_read_timeout(Some(MOCK_EXEC_READY_BOUND))
                    .unwrap();
                let mut command = Command::new(shell);
                command
                    .arg("-c")
                    .arg("printf fe2o3-ready; read _")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::from(OwnedFd::from(child_readiness)));
                let child = ReapedChild(crate::process_execution::spawn(&mut command).unwrap());
                let mut ready = [0_u8; READY.len()];
                readiness.read_exact(&mut ready).unwrap();
                assert_eq!(&ready, READY);

                let pid = child.0.id();
                let start_time_ticks = process_start_time_ticks(pid).unwrap();
                let (image, metadata) = pin_process_executable(pid).unwrap();
                assert_eq!(image.object_identity(), expected_object);
                assert_eq!(image.sha256(), &expected_sha256);
                assert_eq!(process_start_time_ticks(pid).unwrap(), start_time_ticks);
                Self {
                    child,
                    start_time_ticks,
                    image,
                    metadata,
                }
            }

            fn peer_identity(&self) -> BrokerPeerIdentityV2 {
                BrokerPeerIdentityV2 {
                    uid: unsafe { libc::geteuid() },
                    pid: self.child.0.id(),
                    start_time_ticks: self.start_time_ticks,
                    device: self.metadata.dev(),
                    inode: self.metadata.ino(),
                    mode: self.metadata.mode(),
                    executable_sha256: *self.image.sha256(),
                }
            }
        }

        struct ArbitraryRouteProbe {
            endpoint: String,
            listener: UnixListener,
            route: BrokerRouteV3,
            _mock: SpawnedMockExecutable,
        }

        impl ArbitraryRouteProbe {
            fn new() -> Self {
                let endpoint = random_endpoint().unwrap();
                let address = endpoint_address(&endpoint).unwrap();
                let listener = UnixListener::bind_addr(&address).unwrap();
                listener.set_nonblocking(true).unwrap();
                let mock = SpawnedMockExecutable::ready_shell();
                let route = BrokerRouteV3 {
                    endpoint: endpoint.clone(),
                    secret: random_bytes().unwrap(),
                    binding: ordinary_binding(),
                    peer: mock.peer_identity(),
                };
                Self {
                    endpoint,
                    listener,
                    route,
                    _mock: mock,
                }
            }

            fn assert_pre_connect_rejection(&self, session: BuildSession) {
                let error = receive_from(&self.route, session, ordinary_binding())
                    .err()
                    .expect("an arbitrary executable route must fail closed");
                assert_eq!(
                    error,
                    "capability broker route does not name the current cargo-fe2o3 executable"
                );
                match self.listener.accept() {
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) => panic!(
                        "unexpected mock broker accept error for endpoint {}: {error}",
                        self.endpoint
                    ),
                    Ok(_) => panic!(
                        "arbitrary executable route reached uniquely marked endpoint {}",
                        self.endpoint
                    ),
                }
            }
        }

        fn fixture() -> (
            TestDirectory,
            PinnedCodegenBackend,
            PinnedDirectory,
            PinnedExecutable,
            BuildSession,
        ) {
            let temp = TestDirectory::new();
            let backend_path = temp.0.join("backend.so");
            let artifact_path = temp.0.join("artifact");
            fs::write(&backend_path, b"exact broker backend bytes").unwrap();
            fs::create_dir(&artifact_path).unwrap();
            let backend = PinnedCodegenBackend::open(&backend_path).unwrap();
            let artifact =
                PinnedDirectory::open_existing(artifact_path, "test artifact directory").unwrap();
            let pinned_cargo_image =
                PinnedExecutable::open(&std::env::current_exe().unwrap()).unwrap();
            let session = BuildSession::from_bytes([0x42; 16]);
            (temp, backend, artifact, pinned_cargo_image, session)
        }

        fn ordinary_binding() -> CapabilityBindingV3 {
            CapabilityBindingV3::new(
                CapabilityProfileV1::Ordinary,
                None,
                [0x70; 32],
                [0x71; 32],
                [0x72; 32],
            )
            .unwrap()
        }

        fn s09_binding() -> CapabilityBindingV3 {
            CapabilityBindingV3::new(
                CapabilityProfileV1::S09,
                Some([0x91; 32]),
                [0x70; 32],
                [0x71; 32],
                [0x72; 32],
            )
            .unwrap()
        }

        fn protected_closure(
            backend: &PinnedCodegenBackend,
            pinned_cargo_image: &PinnedExecutable,
        ) -> fe2o3_build_authority::CompilerClosureV2 {
            fe2o3_build_authority::CompilerClosureV2::new(
                *pinned_cargo_image.sha256(),
                [0x31; 32],
                [0x32; 32],
                [0x33; 32],
                [0x34; 32],
                *backend.sha256(),
            )
            .unwrap()
        }

        fn protected_binding(
            profile: CapabilityProfileV1,
            closure: fe2o3_build_authority::CompilerClosureV2,
        ) -> CapabilityBindingV3 {
            CapabilityBindingV3::new_protected(
                profile,
                (profile == CapabilityProfileV1::S09).then_some([0x91; 32]),
                closure,
                [0x72; 32],
            )
            .unwrap()
        }

        fn wait_for_active_socket(broker: &CapabilityBroker) -> (u64, u64) {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(identity) = broker.shutdown.active_socket_identity() {
                    return identity;
                }
                assert!(
                    Instant::now() < deadline,
                    "capability broker did not accept the test connection"
                );
                thread::sleep(Duration::from_millis(1));
            }
        }

        fn wait_until(mut predicate: impl FnMut() -> bool, failure: &str) {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !predicate() {
                assert!(Instant::now() < deadline, "{failure}");
                thread::sleep(Duration::from_millis(1));
            }
        }

        fn start_test_broker(
            session: BuildSession,
            binding: CapabilityBindingV3,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            max_active_connections: usize,
            io_timeout: Duration,
        ) -> CapabilityBroker {
            CapabilityBroker::start_with_limits(
                session,
                binding,
                backend,
                artifact,
                pinned_cargo_image,
                BrokerLimits {
                    max_active_connections,
                    authentication_timeout: io_timeout,
                    invocation_frame_timeout: io_timeout,
                    invocation_lifetime: io_timeout,
                },
            )
            .unwrap()
        }

        fn object_is_open(identity: (u64, u64)) -> bool {
            fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(Result::ok)
                .filter_map(|entry| fs::metadata(entry.path()).ok())
                .any(|metadata| (metadata.dev(), metadata.ino()) == identity)
        }

        fn receive_raw_response(
            stream: &UnixStream,
        ) -> io::Result<(usize, [u8; RESPONSE_BYTES], Vec<OwnedFd>)> {
            let mut response = [0_u8; RESPONSE_BYTES];
            let mut iov = [IoSliceMut::new(&mut response)];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
            let mut ancillary = RecvAncillaryBuffer::new(&mut space);
            let message = recvmsg(stream, &mut iov, &mut ancillary, RecvFlags::CMSG_CLOEXEC)
                .map_err(io::Error::from)?;
            if message.flags.contains(ReturnFlags::CTRUNC) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "test capability response control data was truncated",
                ));
            }
            let bytes = message.bytes;
            let mut descriptors = Vec::new();
            for message in ancillary.drain() {
                if let RecvAncillaryMessage::ScmRights(received) = message {
                    descriptors.extend(received);
                }
            }
            Ok((bytes, response, descriptors))
        }

        fn received_descriptor_count(stream: &UnixStream) -> usize {
            receive_raw_response(stream)
                .map(|(_, _, descriptors)| descriptors.len())
                .unwrap_or(0)
        }

        #[test]
        fn transfers_exact_capabilities_after_path_substitution() {
            let (temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let backend_sha = *backend.sha256();
            let cargo_sha = *pinned_cargo_image.sha256();
            let original_artifact = temp.0.join("artifact");
            let moved_artifact = temp.0.join("moved-artifact");
            let binding = s09_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();

            fs::write(temp.0.join("backend.so"), b"replacement backend bytes").unwrap();
            fs::rename(&original_artifact, &moved_artifact).unwrap();
            fs::create_dir(&original_artifact).unwrap();

            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let transferred = receive_from(&route, session, binding).unwrap();
            let transferred_cargo = transferred
                .pinned_cargo_image
                .expect("S09 transfer has a pinned Cargo image");
            assert_eq!(transferred_cargo.sha256(), &cargo_sha);
            assert_eq!(transferred.backend.sha256(), &backend_sha);
            let transferred_artifact = transferred.artifact;
            let source = PathBuf::from("/src/broker_probe.rs");
            let producer = ProducerIdentity::from_codegen("broker_probe", Some(&source)).unwrap();
            begin_build_attempt(
                &transferred_artifact.child_path(),
                &producer,
                BuildInvocation::from_bytes([0x41; 32]),
                session,
            )
            .unwrap();
            fs::write(transferred_artifact.child_path().join("proof"), b"retained").unwrap();
            assert_eq!(fs::read(moved_artifact.join("proof")).unwrap(), b"retained");
            assert!(!original_artifact.join("proof").exists());
        }

        #[test]
        fn ordinary_profile_preserves_the_two_descriptor_contract() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let backend_sha = *backend.sha256();
            let binding = ordinary_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();

            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let transferred = receive_from(&route, session, binding).unwrap();
            assert_eq!(transferred.backend.sha256(), &backend_sha);
            assert!(transferred.pinned_cargo_image.is_none());
            assert!(transferred.compiler_closure.is_none());
        }

        #[test]
        fn protected_profile_transfers_the_exact_full_compiler_closure() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let closure = protected_closure(&backend, &pinned_cargo_image);
            let binding = protected_binding(CapabilityProfileV1::Ordinary, closure);
            let broker = CapabilityBroker::start_protected(
                session,
                binding,
                closure,
                &backend,
                &artifact,
                &pinned_cargo_image,
            )
            .unwrap();

            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            assert!(route.binding.requires_compiler_closure_v2());
            let mut transferred = receive_from(&route, session, binding).unwrap();
            assert!(transferred.pinned_cargo_image.is_none());
            let capability = transferred
                .compiler_closure
                .take()
                .expect("protected response carries a compiler closure");
            capability.revalidate().unwrap();
            assert_eq!(capability.closure(), closure);

            assert!(transferred.invocation_authority.is_some());
            const TEST_COMPILER_CLOSURE_CHILD_FD: i32 = 511;
            let mut command = Command::new("/bin/sh");
            command.arg("-c").arg(format!(
                "test -s /proc/self/fd/{TEST_COMPILER_CLOSURE_CHILD_FD}"
            ));
            capability
                .inherit_for_child_at(&mut command, TEST_COMPILER_CLOSURE_CHILD_FD)
                .unwrap();
            assert!(
                crate::process_execution::status(&mut command)
                    .unwrap()
                    .success()
            );
        }

        #[test]
        fn protected_broker_rejects_downgrades_and_retained_image_mismatches() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let closure = protected_closure(&backend, &pinned_cargo_image);
            let protected = protected_binding(CapabilityProfileV1::Ordinary, closure);
            assert!(
                CapabilityBroker::start(
                    session,
                    protected,
                    &backend,
                    &artifact,
                    &pinned_cargo_image,
                )
                .is_err()
            );
            assert!(
                CapabilityBroker::start_protected(
                    session,
                    ordinary_binding(),
                    closure,
                    &backend,
                    &artifact,
                    &pinned_cargo_image,
                )
                .is_err()
            );

            let wrong_backend = fe2o3_build_authority::CompilerClosureV2::new(
                closure.cargo_executable_sha256(),
                closure.cargo_binding_trampoline_sha256(),
                closure.cargo_fe2o3_binding_wrapper_sha256(),
                closure.rustc_executable_sha256(),
                closure.rustc_runtime_tree_sha256(),
                [0x35; 32],
            )
            .unwrap();
            assert!(
                CapabilityBroker::start_protected(
                    session,
                    protected_binding(CapabilityProfileV1::Ordinary, wrong_backend),
                    wrong_backend,
                    &backend,
                    &artifact,
                    &pinned_cargo_image,
                )
                .is_err()
            );

            let wrong_cargo = fe2o3_build_authority::CompilerClosureV2::new(
                [0x36; 32],
                closure.cargo_binding_trampoline_sha256(),
                closure.cargo_fe2o3_binding_wrapper_sha256(),
                closure.rustc_executable_sha256(),
                closure.rustc_runtime_tree_sha256(),
                closure.codegen_backend_sha256(),
            )
            .unwrap();
            assert!(
                CapabilityBroker::start_protected(
                    session,
                    protected_binding(CapabilityProfileV1::Ordinary, wrong_cargo),
                    wrong_cargo,
                    &backend,
                    &artifact,
                    &pinned_cargo_image,
                )
                .is_err()
            );
        }

        #[test]
        fn invocation_authority_admits_only_the_exact_prepared_claim() {
            fn attempt(invocation: u8) -> BuildAttempt {
                BuildAttempt::from_env_value(&format!(
                    "1:{}:{}",
                    "42".repeat(16),
                    format!("{invocation:02x}").repeat(32)
                ))
                .unwrap()
            }

            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let mut transferred = receive_from(&route, session, binding).unwrap();
            let authority = transferred
                .invocation_authority
                .take()
                .expect("authenticated invocation authority");
            let exact =
                BrokeredInvocationCapabilityClaimV1::new(attempt(0x22), [0x22; 32]).unwrap();
            authority.prepare(exact).unwrap();
            authority
                .exchange(
                    BrokeredInvocationCapabilityRequestV1::Consume(exact),
                    BROKERED_INVOCATION_ADMITTED_V1,
                )
                .unwrap();

            let mut transferred = receive_from(&route, session, binding).unwrap();
            let authority = transferred
                .invocation_authority
                .take()
                .expect("authenticated invocation authority");
            authority.prepare(exact).unwrap();
            let substituted =
                BrokeredInvocationCapabilityClaimV1::new(attempt(0x23), [0x23; 32]).unwrap();
            assert!(
                authority
                    .exchange(
                        BrokeredInvocationCapabilityRequestV1::Consume(substituted),
                        BROKERED_INVOCATION_ADMITTED_V1,
                    )
                    .is_err(),
                "broker admitted an invocation other than its exact prepared claim"
            );
        }

        #[test]
        fn invocation_authority_refreshes_bounded_phases_for_slow_frontends() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = CapabilityBroker::start_with_limits(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                BrokerLimits {
                    max_active_connections: 1,
                    authentication_timeout: Duration::from_secs(5),
                    invocation_frame_timeout: Duration::from_millis(50),
                    invocation_lifetime: Duration::from_secs(1),
                },
            )
            .unwrap();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let mut transferred = receive_from(&route, session, binding).unwrap();
            let authority = transferred.invocation_authority.take().unwrap();
            let attempt =
                BuildAttempt::from_env_value(&format!("1:{}:{}", "42".repeat(16), "24".repeat(32)))
                    .unwrap();
            let claim = BrokeredInvocationCapabilityClaimV1::new(attempt, [0x24; 32]).unwrap();

            thread::sleep(Duration::from_millis(150));
            authority.prepare(claim).unwrap();
            thread::sleep(Duration::from_millis(150));
            authority
                .exchange(
                    BrokeredInvocationCapabilityRequestV1::Consume(claim),
                    BROKERED_INVOCATION_ADMITTED_V1,
                )
                .unwrap();
        }

        fn raw_descriptor_set(
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
        ) -> Vec<OwnedFd> {
            vec![
                backend.try_clone_for_transfer().unwrap().into(),
                artifact.try_clone_for_transfer().unwrap().into(),
                pinned_cargo_image.try_clone_for_transfer().unwrap().into(),
            ]
        }

        #[test]
        fn descriptor_profiles_are_exact_and_fail_closed() {
            let (_temp, backend, artifact, pinned_cargo_image, _session) = fixture();

            let mut ordinary = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            ordinary.truncate(2);
            let ordinary = decode_received_descriptors(ordinary, ordinary_binding()).unwrap();
            assert!(ordinary.pinned_cargo_image.is_none());

            let s09 = decode_received_descriptors(
                raw_descriptor_set(&backend, &artifact, &pinned_cargo_image),
                s09_binding(),
            )
            .unwrap();
            assert!(s09.pinned_cargo_image.is_some());

            let mut missing_s09 = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            missing_s09.truncate(2);
            assert!(decode_received_descriptors(missing_s09, s09_binding()).is_err());

            let ordinary_extra = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            assert!(decode_received_descriptors(ordinary_extra, ordinary_binding()).is_err());

            let mut s09_extra = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            s09_extra.push(pinned_cargo_image.try_clone_for_transfer().unwrap().into());
            assert!(decode_received_descriptors(s09_extra, s09_binding()).is_err());

            let closure = protected_closure(&backend, &pinned_cargo_image);
            let protected = protected_binding(CapabilityProfileV1::Ordinary, closure);
            let closure_capability = CompilerClosureCapabilityV1::create(closure).unwrap();
            let mut exact = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            exact.truncate(2);
            exact.push(closure_capability.try_clone_for_transfer().unwrap().into());
            assert_eq!(
                decode_received_descriptors(exact, protected)
                    .unwrap()
                    .compiler_closure
                    .unwrap()
                    .closure(),
                closure
            );

            let mut missing = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            missing.truncate(2);
            assert!(decode_received_descriptors(missing, protected).is_err());

            let mut extra = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            extra.truncate(2);
            extra.push(closure_capability.try_clone_for_transfer().unwrap().into());
            extra.push(pinned_cargo_image.try_clone_for_transfer().unwrap().into());
            assert!(decode_received_descriptors(extra, protected).is_err());
        }

        #[test]
        fn descriptor_order_is_part_of_each_profile() {
            let (_temp, backend, artifact, pinned_cargo_image, _session) = fixture();

            let mut ordinary = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            ordinary.truncate(2);
            ordinary.swap(0, 1);
            assert!(decode_received_descriptors(ordinary, ordinary_binding()).is_err());

            let mut s09 = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            s09.swap(1, 2);
            assert!(decode_received_descriptors(s09, s09_binding()).is_err());
        }

        #[test]
        fn prepared_profile_config_rustc_and_peer_identity_reject_substitution() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = s09_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();

            let ordinary = ordinary_binding();
            assert!(receive_from(&route, session, ordinary).is_err());
            let wrong_config = CapabilityBindingV3::new(
                CapabilityProfileV1::S09,
                Some([0x92; CONFIG_ID_BYTES]),
                [0x70; COMPILER_CLOSURE_ID_BYTES],
                [0x71; RUSTC_EXECUTABLE_ID_BYTES],
                [0x72; RETAINED_OBJECT_BINDING_BYTES],
            )
            .unwrap();
            assert!(receive_from(&route, session, wrong_config).is_err());
            let wrong_rustc = CapabilityBindingV3::new(
                CapabilityProfileV1::S09,
                Some([0x91; CONFIG_ID_BYTES]),
                [0x71; COMPILER_CLOSURE_ID_BYTES],
                [0x71; RUSTC_EXECUTABLE_ID_BYTES],
                [0x72; RETAINED_OBJECT_BINDING_BYTES],
            )
            .unwrap();
            assert!(receive_from(&route, session, wrong_rustc).is_err());

            let mut wrong_uid = route.clone();
            wrong_uid.peer.uid ^= 1;
            assert!(receive_from(&wrong_uid, session, binding).is_err());

            let mut wrong_pid = route.clone();
            wrong_pid.peer.pid = wrong_pid.peer.pid.saturating_add(1);
            assert!(receive_from(&wrong_pid, session, binding).is_err());

            let mut stale_process = route.clone();
            stale_process.peer.start_time_ticks += 1;
            assert!(receive_from(&stale_process, session, binding).is_err());

            let mut wrong_object = route.clone();
            wrong_object.peer.inode ^= 1;
            assert!(receive_from(&wrong_object, session, binding).is_err());

            let mut substituted_executable = route.clone();
            substituted_executable.peer.executable_sha256[0] ^= 1;
            assert!(receive_from(&substituted_executable, session, binding).is_err());

            let mut wrong_secret = route;
            wrong_secret.secret[0] ^= 1;
            assert!(receive_from(&wrong_secret, session, binding).is_err());
        }

        #[test]
        fn endpoint_only_mock_cannot_forge_the_broker_response() {
            let (_temp, backend, artifact, _pinned_cargo_image, session) = fixture();
            let endpoint = random_endpoint().unwrap();
            let address = endpoint_address(&endpoint).unwrap();
            let listener = UnixListener::bind_addr(&address).unwrap();
            let backend = backend.try_clone_for_transfer().unwrap();
            let artifact = artifact.try_clone_for_transfer().unwrap();
            let mock = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; REQUEST_BYTES];
                stream.read_exact(&mut request).unwrap();
                let descriptors = [backend.as_fd(), artifact.as_fd()];
                let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
                let mut ancillary = SendAncillaryBuffer::new(&mut space);
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
                let mut forged = [0_u8; RESPONSE_BYTES];
                forged[0] = 1;
                sendmsg(
                    &stream,
                    &[IoSlice::new(&forged)],
                    &mut ancillary,
                    SendFlags::NOSIGNAL,
                )
                .unwrap();
            });
            let route = BrokerRouteV3 {
                endpoint,
                secret: random_bytes().unwrap(),
                binding: ordinary_binding(),
                peer: BrokerPeerIdentityV2::current().unwrap(),
            };

            assert!(receive_from(&route, session, ordinary_binding()).is_err());
            mock.join().unwrap();
        }

        #[test]
        fn self_consistent_arbitrary_executable_route_is_rejected_before_connect() {
            let (_temp, _backend, _artifact, _pinned_cargo_image, session) = fixture();
            ArbitraryRouteProbe::new().assert_pre_connect_rejection(session);
        }

        #[test]
        fn arbitrary_executable_routes_reject_before_connect_under_concurrent_exec() {
            const ROUNDS: usize = 8;
            const PROBES_PER_ROUND: usize = 4;
            let (_temp, _backend, _artifact, _pinned_cargo_image, session) = fixture();
            let mut observed_endpoints = BTreeSet::new();
            for _ in 0..ROUNDS {
                let probes = (0..PROBES_PER_ROUND)
                    .map(|_| ArbitraryRouteProbe::new())
                    .collect::<Vec<_>>();
                for probe in &probes {
                    assert!(
                        observed_endpoints.insert(probe.endpoint.clone()),
                        "test endpoint marker was reused"
                    );
                }
                thread::scope(|scope| {
                    for probe in &probes {
                        scope.spawn(move || probe.assert_pre_connect_rejection(session));
                    }
                });
            }
        }

        #[test]
        fn rejects_wrong_session_and_serves_concurrent_exact_clients() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let backend_sha = *backend.sha256();
            let cargo_sha = *pinned_cargo_image.sha256();
            let binding = s09_binding();
            let broker = Arc::new(
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap(),
            );
            assert!(
                receive_from(
                    &BrokerRouteV3::parse(broker.route()).unwrap(),
                    BuildSession::from_bytes([0x43; 16]),
                    binding,
                )
                .is_err()
            );

            let clients = (0..8)
                .map(|_| {
                    let broker = Arc::clone(&broker);
                    std::thread::spawn(move || {
                        let route = BrokerRouteV3::parse(broker.route()).unwrap();
                        let transferred = receive_from(&route, session, binding).unwrap();
                        assert_eq!(transferred.backend.sha256(), &backend_sha);
                        let cargo = transferred
                            .pinned_cargo_image
                            .expect("S09 transfer has a pinned Cargo image");
                        assert_eq!(cargo.sha256(), &cargo_sha);
                        drop(transferred.artifact);
                    })
                })
                .collect::<Vec<_>>();
            for client in clients {
                client.join().unwrap();
            }
        }

        #[test]
        fn active_connection_limit_rejects_max_plus_one_and_recovers() {
            const TEST_LIMIT: usize = 4;
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                TEST_LIMIT,
                BROKER_IO_TIMEOUT,
            );
            let pause = broker.shutdown.install_worker_pause();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let stalled = (0..TEST_LIMIT)
                .map(|_| UnixStream::connect_addr(&address).unwrap())
                .collect::<Vec<_>>();
            for reached in 0..TEST_LIMIT {
                assert!(
                    pause
                        .reached
                        .recv_timeout(BROKER_IO_TIMEOUT)
                        .unwrap_or_else(|_| {
                            panic!(
                                "only {reached} capacity workers paused; {} registered",
                                broker.shutdown.active_connection_count()
                            )
                        })
                        .is_none(),
                    "capacity worker pause unexpectedly reported a socket"
                );
            }
            assert_eq!(broker.shutdown.active_connection_count(), TEST_LIMIT);

            let rejection_started = Instant::now();
            let mut excess = UnixStream::connect_addr(&address).unwrap();
            wait_until(
                || broker.shutdown.admission_rejections.load(Ordering::Acquire) == 1,
                "capability broker did not reject overload admission",
            );
            assert!(
                rejection_started.elapsed() < PROMPT_SHUTDOWN_BOUND,
                "limit-plus-one connection waited for an active slot"
            );
            excess
                .set_read_timeout(Some(PROMPT_SHUTDOWN_BOUND))
                .unwrap();
            let mut byte = [0_u8; 1];
            assert!(
                !matches!(
                    excess.read(&mut byte),
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        )
                ),
                "limit-plus-one connection remained live"
            );
            assert!(
                rejection_started.elapsed() < PROMPT_SHUTDOWN_BOUND,
                "limit-plus-one connection was not rejected promptly"
            );
            assert!(broker.shutdown.active_connection_count() <= TEST_LIMIT);
            assert_eq!(received_descriptor_count(&excess), 0);

            drop(excess);
            for _ in 0..TEST_LIMIT {
                pause.release();
            }
            broker.shutdown.remove_worker_pause();
            drop(stalled);
            wait_until(
                || broker.shutdown.active_connection_count() == 0,
                "stalled connection slots were not released",
            );
            receive_from(&route, session, binding).unwrap();
        }

        #[test]
        fn accepted_connection_deadline_rejects_slow_drip() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                4,
                Duration::from_millis(500),
            );
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let mut client = UnixStream::connect_addr(&address).unwrap();
            let request = request_bytes(session, binding, [0x19; CHALLENGE_BYTES], &route.secret);
            let mut sent = 0;
            while sent < request.len() {
                match client.write(&request[sent..sent + 1]) {
                    Ok(1) => sent += 1,
                    Ok(_) => break,
                    Err(_) => break,
                }
                thread::sleep(Duration::from_millis(25));
            }
            assert!(
                sent < request.len(),
                "a slow-drip request outlived the accepted-connection deadline"
            );
            wait_until(
                || broker.shutdown.active_connection_count() == 0,
                "expired slow-drip connection remained registered",
            );
        }

        #[test]
        fn caught_worker_panic_always_releases_registry_slot() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                2,
                Duration::from_secs(5),
            );
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            broker.shutdown.inject_worker_panic();
            let failed = UnixStream::connect_addr(&address).unwrap();
            wait_until(
                || broker.shutdown.caught_worker_panics.load(Ordering::Acquire) == 1,
                "injected worker panic was not caught",
            );
            wait_until(
                || broker.shutdown.active_connection_count() == 0,
                "panicking worker retained its registry slot",
            );
            drop(failed);
            receive_from(&route, session, binding).unwrap();
        }

        #[test]
        fn scoped_worker_spawn_failure_releases_registry_slot() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                1,
                Duration::from_secs(5),
            );
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            broker.shutdown.inject_worker_spawn_failure();
            let failed = UnixStream::connect_addr(&address).unwrap();
            wait_until(
                || broker.shutdown.active_connection_count() == 0,
                "failed worker spawn retained its registry slot",
            );
            assert_eq!(received_descriptor_count(&failed), 0);
            drop(failed);
            receive_from(&route, session, binding).unwrap();
        }

        #[test]
        fn exactly_max_parallel_connections_admit_and_shutdown_cleanly() {
            const CLIENTS: usize = MAX_ACTIVE_CONNECTIONS;
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                CLIENTS,
                BROKER_IO_TIMEOUT,
            );
            let pause = broker.shutdown.install_worker_pause();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let clients = (0..CLIENTS)
                .map(|_| UnixStream::connect_addr(&address).unwrap())
                .collect::<Vec<_>>();
            for reached in 0..CLIENTS {
                assert!(
                    pause
                        .reached
                        .recv_timeout(BROKER_IO_TIMEOUT)
                        .unwrap_or_else(|_| {
                            panic!(
                                "only {reached} capacity workers paused; {} registered",
                                broker.shutdown.active_connection_count()
                            )
                        })
                        .is_none(),
                    "capacity worker pause unexpectedly reported a socket"
                );
                assert!(broker.shutdown.active_connection_count() > reached);
            }
            assert_eq!(broker.shutdown.active_connection_count(), CLIENTS);
            assert_eq!(
                broker.shutdown.admission_rejections.load(Ordering::Acquire),
                0
            );
            let shutdown = Arc::clone(&broker.shutdown);
            drop(clients);
            drop(pause);
            drop(broker);
            assert_eq!(shutdown.active_connection_count(), 0);
        }

        #[test]
        fn dispatch_lock_contention_past_deadline_sends_no_descriptors() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                2,
                Duration::from_millis(250),
            );
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let mut client = UnixStream::connect_addr(&address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            wait_for_active_socket(&broker);

            let shutdown = Arc::clone(&broker.shutdown);
            let acquired = Arc::new(Barrier::new(2));
            let holder_acquired = Arc::clone(&acquired);
            let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
            let holder = thread::spawn(move || {
                let _state = shutdown.state();
                holder_acquired.wait();
                release_rx.recv().unwrap();
            });
            acquired.wait();

            let request = request_bytes(session, binding, [0x7a; CHALLENGE_BYTES], &route.secret);
            client.write_all(&request).unwrap();
            thread::sleep(Duration::from_millis(400));
            release_tx.send(()).unwrap();
            holder.join().unwrap();

            assert_eq!(received_descriptor_count(&client), 0);
            wait_until(
                || broker.shutdown.active_connection_count() == 0,
                "expired dispatch retained its registry slot",
            );
        }

        #[test]
        fn concurrent_requests_are_registered_before_dispatch_completes() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = s09_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let pause = broker.shutdown.install_dispatch_pause();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let clients = (0..2)
                .map(|_| {
                    let route = route.clone();
                    thread::spawn(move || receive_from(&route, session, binding))
                })
                .collect::<Vec<_>>();

            assert!(pause.wait_until_reached().is_none());
            let deadline = Instant::now() + BROKER_IO_TIMEOUT;
            while broker.shutdown.active_connection_count() != clients.len() {
                assert!(
                    Instant::now() < deadline,
                    "capability broker serialized accepted requests before dispatch"
                );
                thread::sleep(Duration::from_millis(1));
            }
            pause.release();
            assert!(pause.wait_until_reached().is_none());
            pause.release();

            for client in clients {
                client.join().unwrap().unwrap();
            }
        }

        #[test]
        fn shutdown_closes_every_connection_at_the_admission_limit() {
            const TEST_LIMIT: usize = 6;
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                TEST_LIMIT,
                Duration::from_secs(5),
            );
            let shutdown = Arc::clone(&broker.shutdown);
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let clients = (0..TEST_LIMIT)
                .map(|_| {
                    let client = UnixStream::connect_addr(&address).unwrap();
                    client
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    client
                })
                .collect::<Vec<_>>();
            wait_until(
                || shutdown.active_connection_count() == TEST_LIMIT,
                "capability broker did not register all shutdown test clients",
            );

            drop(broker);
            assert_eq!(shutdown.active_connection_count(), 0);
            for client in clients {
                assert_eq!(received_descriptor_count(&client), 0);
            }
        }

        #[test]
        fn descriptor_pressure_does_not_disable_the_accept_loop() {
            if std::env::var_os("FE2O3_CAPABILITY_BROKER_FD_PRESSURE_CHILD").is_some() {
                return;
            }
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "capability_broker::platform::tests::descriptor_pressure_child",
                    "--nocapture",
                ])
                .env("FE2O3_CAPABILITY_BROKER_FD_PRESSURE_CHILD", "1");
            let output = crate::process_execution::capture_output(&mut command).unwrap();
            assert!(
                output.status.success(),
                "descriptor-pressure child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }

        #[test]
        fn descriptor_pressure_child() {
            if std::env::var_os("FE2O3_CAPABILITY_BROKER_FD_PRESSURE_CHILD").is_none() {
                return;
            }
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                4,
                Duration::from_secs(5),
            );
            let pause = broker.shutdown.install_accept_pause();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let client = UnixStream::connect_addr(&address).unwrap();
            assert!(pause.wait_until_reached().is_some());

            let mut original = MaybeUninit::<libc::rlimit>::uninit();
            assert_eq!(
                unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, original.as_mut_ptr()) },
                0
            );
            let original = unsafe { original.assume_init() };
            let open_descriptors = fs::read_dir("/proc/self/fd").unwrap().count() as libc::rlim_t;
            let reduced = libc::rlimit {
                rlim_cur: (open_descriptors + 24).min(original.rlim_max),
                rlim_max: original.rlim_max,
            };
            assert!(reduced.rlim_cur > open_descriptors + 4);
            assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &reduced) }, 0);
            let mut fillers = Vec::new();
            loop {
                match File::open("/dev/null") {
                    Ok(file) => fillers.push(file),
                    Err(error) if error.raw_os_error() == Some(libc::EMFILE) => break,
                    Err(error) => panic!("unexpected descriptor-pressure error: {error}"),
                }
            }

            pause.release();
            assert_eq!(received_descriptor_count(&client), 0);
            drop(fillers);
            assert_eq!(
                unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &original) },
                0
            );
            drop(client);

            receive_from(&route, session, binding).unwrap();
        }

        #[test]
        fn shutdown_wakes_stalled_and_partial_accepted_requests() {
            for request_prefix_len in [0, REQUEST_BYTES / 2] {
                let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
                let binding = ordinary_binding();
                let broker = CapabilityBroker::start(
                    session,
                    binding,
                    &backend,
                    &artifact,
                    &pinned_cargo_image,
                )
                .unwrap();
                let route = BrokerRouteV3::parse(broker.route()).unwrap();
                let address = endpoint_address(&route.endpoint).unwrap();
                let mut client = UnixStream::connect_addr(&address).unwrap();
                client
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                client.write_all(&vec![0_u8; request_prefix_len]).unwrap();
                let active_identity = wait_for_active_socket(&broker);
                assert!(object_is_open(active_identity));
                broker.shutdown.wait_for_request_read();

                let started = Instant::now();
                drop(broker);
                let elapsed = started.elapsed();
                assert!(
                    elapsed < PROMPT_SHUTDOWN_BOUND,
                    "broker shutdown took {elapsed:?}, exceeding the {PROMPT_SHUTDOWN_BOUND:?} prompt bound"
                );
                assert_eq!(received_descriptor_count(&client), 0);
                drop(client);
                assert!(!object_is_open(active_identity));
            }
        }

        #[test]
        fn request_racing_shutdown_receives_no_descriptors() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = s09_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let pause = broker.shutdown.install_dispatch_pause();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let mut client = UnixStream::connect_addr(&address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let active_identity = wait_for_active_socket(&broker);
            let request = request_bytes(session, binding, [0x73; CHALLENGE_BYTES], &route.secret);
            client.write_all(&request).unwrap();
            let _ = pause.wait_until_reached();

            let started = Instant::now();
            broker.shutdown.begin();
            pause.release();
            drop(broker);
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "broker shutdown did not promptly cancel descriptor dispatch"
            );
            assert_eq!(received_descriptor_count(&client), 0);
            drop(client);
            assert!(!object_is_open(active_identity));
        }

        #[test]
        fn shutdown_race_before_register_is_fail_closed() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let pause = broker.shutdown.install_accept_pause();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let client = UnixStream::connect_addr(&address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let accepted_identity = pause
                .wait_until_reached()
                .expect("accept pause must report the accepted socket identity");
            assert!(broker.shutdown.active_socket_identity().is_none());
            assert!(object_is_open(accepted_identity));

            let started = Instant::now();
            broker.shutdown.begin();
            pause.release();
            drop(broker);
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "broker did not promptly reject an unregistered accepted connection"
            );
            assert_eq!(received_descriptor_count(&client), 0);
            drop(client);
            assert!(!object_is_open(accepted_identity));
        }

        #[test]
        fn shutdown_race_after_send_lock_preserves_exact_response() {
            let (temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let backend_sha = *backend.sha256();
            let cargo_sha = *pinned_cargo_image.sha256();
            let binding = s09_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let pause = broker.shutdown.install_locked_dispatch_pause();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let mut client = UnixStream::connect_addr(&address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let active_identity = wait_for_active_socket(&broker);
            let challenge = [0x74; CHALLENGE_BYTES];
            let request = request_bytes(session, binding, challenge, &route.secret);
            let request_auth = request[REQUEST_BYTES - REQUEST_AUTH_BYTES..]
                .try_into()
                .unwrap();
            let expected_response = response_bytes(&route.secret, challenge, request_auth);
            client.write_all(&request).unwrap();
            assert!(pause.wait_until_reached().is_none());

            let shutdown = Arc::clone(&broker.shutdown);
            let shutdown_thread = thread::spawn(move || {
                let started = Instant::now();
                shutdown.begin();
                started.elapsed()
            });
            broker.shutdown.wait_for_begin();
            pause.release();

            let (bytes, response, descriptors) = receive_raw_response(&client).unwrap();
            assert_eq!(bytes, RESPONSE_BYTES);
            assert_eq!(response, expected_response);
            let transferred = decode_received_descriptors(descriptors, s09_binding()).unwrap();
            assert_eq!(transferred.backend.sha256(), &backend_sha);
            assert_eq!(
                transferred
                    .pinned_cargo_image
                    .as_ref()
                    .expect("S09 response must contain the pinned Cargo image")
                    .sha256(),
                &cargo_sha
            );
            fs::write(
                transferred.artifact.child_path().join("send-wins-proof"),
                b"exact retained artifact",
            )
            .unwrap();
            assert_eq!(
                fs::read(temp.0.join("artifact/send-wins-proof")).unwrap(),
                b"exact retained artifact"
            );
            drop(transferred);

            assert!(
                shutdown_thread.join().unwrap() < Duration::from_secs(2),
                "broker shutdown did not promptly follow the winning descriptor send"
            );
            drop(broker);
            drop(client);
            assert!(!object_is_open(active_identity));
        }

        #[test]
        fn dropping_broker_closes_the_endpoint_before_post_build_work() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let route = BrokerRouteV3::parse(broker.route()).unwrap();
            drop(broker);

            assert!(receive_from(&route, session, binding).is_err());
        }

        #[test]
        fn endpoint_grammar_is_strict() {
            for endpoint in ["", "01", &"A".repeat(ENDPOINT_HEX_BYTES), &"0".repeat(65)] {
                assert!(endpoint_address(endpoint).is_err());
            }
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) use platform::*;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod unsupported {
    use std::process::Command;

    use fe2o3_artifact_transaction::{BrokeredInvocationCapabilityClaimV1, BuildSession};

    use crate::cargo_invocation_boundary::InvocationAuthorizationRegistryV1;
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::PinnedExecutable;
    use crate::project::PinnedDirectory;
    use fe2o3_compiler_closure_capability::CompilerClosureCapabilityV1;

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";
    pub(crate) const INVOCATION_AUTHORITY_CHILD_FD_V1: i32 =
        fe2o3_artifact_transaction::BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CapabilityProfileV1 {
        Ordinary,
        S09,
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

        pub(crate) fn inherit_for_child(&self, _command: &mut Command) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }
    }

    pub(crate) struct BrokeredCapabilities {
        pub(crate) backend: PinnedCodegenBackend,
        pub(crate) artifact: PinnedDirectory,
        pub(crate) pinned_cargo_image: Option<PinnedExecutable>,
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
