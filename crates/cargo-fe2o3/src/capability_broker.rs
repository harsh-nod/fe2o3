//! Descriptor transport from the `cargo-fe2o3` parent to managed rustc wrappers.
//!
//! Cargo receives a strict per-instance route and build-session binding. Both peers check Linux
//! credentials, exact executable identity, the prepared profile/config identity, and a
//! challenge-response bound to a separate 256-bit broker secret before transferring a
//! sealed backend image and a read-only artifact-directory descriptor with `SCM_RIGHTS`. The
//! explicit S09 profile additionally receives an observed pinned Cargo image. Receivers validate
//! the exact profile-specific descriptor count and positional types before installing capabilities
//! in the caller-selected compiler process for a compile-shaped wrapper invocation.
//!
//! This boundary prevents accidental descriptor inheritance through Cargo and pathname
//! substitution between orchestration and rustc. It is not an OS sandbox against project code or
//! hostile code already running as the same user. Cargo children inherit the routing values, so a
//! hostile build script can deliberately replay the wrapper, and a procedural macro executes
//! inside rustc after the descriptors are installed. Both are trusted by this design. The
//! directory is opened `O_RDONLY`, but still grants descriptor-relative namespace mutation. The
//! receiver treats that route as untrusted: before connecting, it independently observes its own
//! running `cargo-fe2o3` image and requires the advertised broker to have the same uid, executable
//! object, and bytes. This closes a self-consistent route redirected to an arbitrary mock
//! executable. A substitute running the same executable object and bytes remains inside the
//! executable-authentication boundary, but it has no public broker-server entry point and must
//! still possess the fresh build session and per-broker secret. The route is inherited by trusted
//! Cargo children and is therefore not a sandbox boundary against hostile same-user project code
//! that can read or rewrite another child's environment or ptrace the broker. Untrusted build
//! dependencies require a separate process sandbox.

#[cfg(target_os = "linux")]
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

    use fe2o3_artifact_transaction::BuildSession;
    use fe2o3_process_identity::LinuxObjectIdentityV3;
    use rustix::net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
    };
    use sha2::{Digest, Sha256};

    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
    use crate::project::PinnedDirectory;

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";
    const REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-V2\0";
    const S09_REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-09\0";
    const _: () = assert!(REQUEST_MAGIC.len() == S09_REQUEST_MAGIC.len());
    const ROUTE_PREFIX: &str = "fe2o3-capability-route-v2";
    const ENDPOINT_BYTES: usize = 32;
    const ENDPOINT_HEX_BYTES: usize = ENDPOINT_BYTES * 2;
    const SECRET_BYTES: usize = 32;
    const CHALLENGE_BYTES: usize = 32;
    const CONFIG_ID_BYTES: usize = 32;
    const REQUEST_AUTH_BYTES: usize = 32;
    const REQUEST_BYTES: usize =
        REQUEST_MAGIC.len() + 16 + 1 + CONFIG_ID_BYTES + CHALLENGE_BYTES + REQUEST_AUTH_BYTES;
    const RESPONSE_BYTES: usize = 1 + REQUEST_AUTH_BYTES;
    const REQUEST_AUTH_DOMAIN: &[u8] = b"FE2O3/CAPABILITY-BROKER/REQUEST-AUTH/V2\0";
    const RESPONSE_AUTH_DOMAIN: &[u8] = b"FE2O3/CAPABILITY-BROKER/RESPONSE-AUTH/V2\0";
    const MAX_PROC_STAT_BYTES: usize = 4096;
    const EXECUTABLE_PIN_ATTEMPTS: usize = 8;
    const RECEIVED_DESCRIPTOR_FLOOR: i32 = 199;
    const BROKER_IO_TIMEOUT: Duration = Duration::from_secs(30);
    const MAX_ACTIVE_CONNECTIONS: usize = 64;

    #[derive(Clone, Copy)]
    struct BrokerLimits {
        max_active_connections: usize,
        io_timeout: Duration,
    }

    const PRODUCTION_BROKER_LIMITS: BrokerLimits = BrokerLimits {
        max_active_connections: MAX_ACTIVE_CONNECTIONS,
        io_timeout: BROKER_IO_TIMEOUT,
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
    pub(crate) struct CapabilityBindingV2 {
        profile: CapabilityProfileV1,
        config_identity: Option<[u8; CONFIG_ID_BYTES]>,
    }

    impl CapabilityBindingV2 {
        pub(crate) fn new(
            profile: CapabilityProfileV1,
            config_identity: Option<[u8; CONFIG_ID_BYTES]>,
        ) -> Result<Self, String> {
            if profile == CapabilityProfileV1::S09 && config_identity.is_none() {
                return Err("S09 capability binding requires a Worker V2 config identity".into());
            }
            Ok(Self {
                profile,
                config_identity,
            })
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
    static PEER_AUTHENTICATION: Mutex<()> = Mutex::new(());

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

        fn authenticate_client(self, stream: &UnixStream) -> Result<(), String> {
            let credentials = rustix::net::sockopt::socket_peercred(stream)
                .map_err(|error| format!("cannot inspect capability broker client: {error}"))?;
            let client_pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
                .map_err(|_| "capability broker client PID is negative".to_owned())?;
            if credentials.uid.as_raw() != self.uid {
                return Err("capability broker client uid does not match the broker".into());
            }
            let initial_start = process_start_time_ticks(client_pid)?;
            let path = PathBuf::from(format!("/proc/{client_pid}/exe"));
            let executable = File::open(&path).map_err(|error| {
                format!(
                    "cannot open capability broker client executable {}: {error}",
                    path.display()
                )
            })?;
            let metadata = executable.metadata().map_err(|error| {
                format!(
                    "cannot inspect capability broker client executable {}: {error}",
                    path.display()
                )
            })?;
            if process_start_time_ticks(client_pid)? != initial_start {
                return Err("capability broker client PID was reused while authenticating".into());
            }
            if LinuxObjectIdentityV3::from_linux_stat(
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
            ) != self.object_identity()
            {
                return Err(
                    "capability broker client is not the exact cargo-fe2o3 executable".into(),
                );
            }
            Ok(())
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct BrokerRouteV2 {
        endpoint: String,
        secret: [u8; SECRET_BYTES],
        binding: CapabilityBindingV2,
        peer: BrokerPeerIdentityV2,
    }

    impl BrokerRouteV2 {
        fn encode(&self) -> String {
            format!(
                "{ROUTE_PREFIX}:{}:{}:{}:{}:{}:{}:{}:{:x}:{:x}:{:x}:{}",
                self.endpoint,
                hex(&self.secret),
                self.binding.profile.route_name(),
                self.binding
                    .config_identity
                    .map(|identity| hex(&identity))
                    .unwrap_or_else(|| "-".to_owned()),
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
            if fields.len() != 12 || fields[0] != ROUTE_PREFIX {
                return Err("capability broker route is not canonical V2".into());
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
            let binding = CapabilityBindingV2::new(profile, config_identity)?;
            let peer = BrokerPeerIdentityV2 {
                uid: u32::try_from(parse_canonical_decimal(fields[5], "peer uid", true)?)
                    .map_err(|_| "capability broker peer uid exceeds u32".to_owned())?,
                pid: u32::try_from(parse_canonical_decimal(fields[6], "peer pid", false)?)
                    .map_err(|_| "capability broker peer pid exceeds u32".to_owned())?,
                start_time_ticks: parse_canonical_decimal(fields[7], "peer start time", false)?,
                device: parse_canonical_hex(fields[8], "peer device")?,
                inode: parse_canonical_hex(fields[9], "peer inode")?,
                mode: u32::try_from(parse_canonical_hex(fields[10], "peer mode")?)
                    .map_err(|_| "capability broker peer mode exceeds u32".to_owned())?,
                executable_sha256: decode_fixed_hex(fields[11], "peer executable digest")?,
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
        shutdown: Arc<BrokerShutdown>,
        worker: Option<JoinHandle<()>>,
    }

    #[derive(Default)]
    struct BrokerShutdownState {
        stopping: bool,
        next_connection_id: u64,
        active: BTreeMap<u64, Arc<UnixStream>>,
    }

    struct BrokerShutdown {
        // This mutex is the shutdown/SCM_RIGHTS linearization point and owns the wakeup socket.
        state: Mutex<BrokerShutdownState>,
        slot_available: Condvar,
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
        #[cfg(test)]
        admission_wait_started: std::sync::atomic::AtomicBool,
    }

    impl BrokerShutdown {
        fn new(max_active_connections: usize) -> Self {
            assert!(max_active_connections != 0);
            Self {
                state: Mutex::new(BrokerShutdownState::default()),
                slot_available: Condvar::new(),
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
                #[cfg(test)]
                admission_wait_started: std::sync::atomic::AtomicBool::new(false),
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
            while !state.stopping && state.active.len() >= self.max_active_connections {
                #[cfg(test)]
                self.admission_wait_started
                    .store(true, std::sync::atomic::Ordering::Release);
                let remaining = match deadline.remaining() {
                    Ok(remaining) => remaining,
                    Err(error) => {
                        #[cfg(test)]
                        self.admission_rejections
                            .fetch_add(1, std::sync::atomic::Ordering::Release);
                        let _ = stream.shutdown(Shutdown::Both);
                        return Err(error);
                    }
                };
                let (next_state, _) = self
                    .slot_available
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state = next_state;
            }
            if state.stopping {
                let _ = stream.shutdown(Shutdown::Both);
                return Ok(None);
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
            let removed = self.state().active.remove(&connection_id).is_some();
            if removed {
                self.slot_available.notify_one();
            }
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
            self.slot_available.notify_all();
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
            let deadline = std::time::Instant::now() + BROKER_IO_TIMEOUT;
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
                .recv_timeout(BROKER_IO_TIMEOUT)
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
            binding: CapabilityBindingV2,
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

        fn start_with_limits(
            session: BuildSession,
            binding: CapabilityBindingV2,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            limits: BrokerLimits,
        ) -> Result<Self, String> {
            if limits.max_active_connections == 0 || limits.io_timeout.is_zero() {
                return Err("capability broker limits must be nonzero".to_owned());
            }
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
            let route = BrokerRouteV2 {
                endpoint,
                secret,
                binding,
                peer: executable,
            }
            .encode();
            let shutdown = Arc::new(BrokerShutdown::new(limits.max_active_connections));
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
                        io_timeout: limits.io_timeout,
                        client_authentication: Mutex::new(()),
                        shutdown: worker_shutdown,
                    }
                    .serve();
                })
                .map_err(|error| format!("failed to start capability broker: {error}"))?;
            Ok(Self {
                route,
                shutdown,
                worker: Some(worker),
            })
        }

        pub(crate) fn route(&self) -> &str {
            &self.route
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
    }

    pub(crate) fn receive(
        session: BuildSession,
        binding: CapabilityBindingV2,
    ) -> Result<BrokeredCapabilities, String> {
        let encoded_route = std::env::var(CAPABILITY_BROKER_ENV)
            .map_err(|_| format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}"))?;
        let route = BrokerRouteV2::parse(&encoded_route)?;
        receive_from(&route, session, binding)
    }

    fn receive_from(
        route: &BrokerRouteV2,
        session: BuildSession,
        binding: CapabilityBindingV2,
    ) -> Result<BrokeredCapabilities, String> {
        if route.binding != binding {
            return Err(
                "capability broker route does not match the prepared profile/config identity"
                    .into(),
            );
        }
        route.peer.require_current_executable()?;
        let address = endpoint_address(&route.endpoint)?;
        let mut stream = UnixStream::connect_addr(&address)
            .map_err(|error| format!("failed to connect to capability broker: {error}"))?;
        stream
            .set_read_timeout(Some(BROKER_IO_TIMEOUT))
            .map_err(|error| format!("failed to bound capability broker read: {error}"))?;
        let authentication = PEER_AUTHENTICATION
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        route.peer.authenticate(&stream)?;
        drop(authentication);
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
        decode_received_descriptors(descriptors, binding.profile)
    }

    fn decode_received_descriptors(
        mut descriptors: Vec<OwnedFd>,
        profile: CapabilityProfileV1,
    ) -> Result<BrokeredCapabilities, String> {
        if descriptors.len() != profile.descriptor_count() {
            return Err(format!(
                "capability broker returned {} descriptors instead of {} for the {} profile",
                descriptors.len(),
                profile.descriptor_count(),
                profile.name(),
            ));
        }
        let pinned_cargo_image = if profile == CapabilityProfileV1::S09 {
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
        binding: CapabilityBindingV2,
        secret: [u8; SECRET_BYTES],
        executable: BrokerPeerIdentityV2,
        backend: File,
        artifact: File,
        pinned_cargo_image: File,
        io_timeout: Duration,
        client_authentication: Mutex<()>,
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
                            let deadline = BrokerDeadline::new(accepted_at, self.io_timeout);
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
            let authentication = self
                .client_authentication
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.executable
                .authenticate_client(stream)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            drop(authentication);
            deadline.require_remaining()?;
            let mut request = vec![0_u8; REQUEST_BYTES];
            #[cfg(test)]
            self.shutdown
                .request_read_started
                .store(true, std::sync::atomic::Ordering::Release);
            deadline.read_exact(stream, &mut request)?;
            let challenge_start = REQUEST_MAGIC.len() + 16 + 1 + CONFIG_ID_BYTES;
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
            let response = response_bytes(&self.secret, challenge, request_auth);
            self.shutdown
                .send_response(stream, &response, &descriptors, deadline)
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
        binding: CapabilityBindingV2,
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
        use std::process::{Child, Command};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::{Arc, Barrier};
        use std::time::{Duration, Instant};

        use fe2o3_artifact_transaction::{BuildInvocation, ProducerIdentity, begin_build_attempt};

        use super::*;

        static NEXT: AtomicU64 = AtomicU64::new(1);
        const PROMPT_SHUTDOWN_BOUND: Duration = Duration::from_secs(5);
        const _: () = assert!(PROMPT_SHUTDOWN_BOUND.as_secs() < BROKER_IO_TIMEOUT.as_secs());

        struct TestDirectory(PathBuf);

        impl TestDirectory {
            fn new() -> Self {
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
            fn sleep() -> Self {
                let expected = PinnedExecutable::open(&PathBuf::from("/bin/sleep")).unwrap();
                let expected_object = expected.object_identity();
                let expected_sha256 = *expected.sha256();
                let child = ReapedChild(Command::new("/bin/sleep").arg("30").spawn().unwrap());
                let pid = child.0.id();
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    let start_time_ticks = process_start_time_ticks(pid).unwrap();
                    if let Ok((image, metadata)) = pin_process_executable(pid)
                        && image.object_identity() == expected_object
                        && image.sha256() == &expected_sha256
                    {
                        assert_eq!(process_start_time_ticks(pid).unwrap(), start_time_ticks);
                        return Self {
                            child,
                            start_time_ticks,
                            image,
                            metadata,
                        };
                    }
                    assert!(
                        Instant::now() < deadline,
                        "mock executable did not complete exec before its identity was pinned"
                    );
                    thread::yield_now();
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
            route: BrokerRouteV2,
            _mock: SpawnedMockExecutable,
        }

        impl ArbitraryRouteProbe {
            fn new() -> Self {
                let endpoint = random_endpoint().unwrap();
                let address = endpoint_address(&endpoint).unwrap();
                let listener = UnixListener::bind_addr(&address).unwrap();
                listener.set_nonblocking(true).unwrap();
                let mock = SpawnedMockExecutable::sleep();
                let route = BrokerRouteV2 {
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

        fn ordinary_binding() -> CapabilityBindingV2 {
            CapabilityBindingV2::new(CapabilityProfileV1::Ordinary, None).unwrap()
        }

        fn s09_binding() -> CapabilityBindingV2 {
            CapabilityBindingV2::new(CapabilityProfileV1::S09, Some([0x91; 32])).unwrap()
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
            binding: CapabilityBindingV2,
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
                    io_timeout,
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

            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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

            let route = BrokerRouteV2::parse(broker.route()).unwrap();
            let transferred = receive_from(&route, session, binding).unwrap();
            assert_eq!(transferred.backend.sha256(), &backend_sha);
            assert!(transferred.pinned_cargo_image.is_none());
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
            let ordinary =
                decode_received_descriptors(ordinary, CapabilityProfileV1::Ordinary).unwrap();
            assert!(ordinary.pinned_cargo_image.is_none());

            let s09 = decode_received_descriptors(
                raw_descriptor_set(&backend, &artifact, &pinned_cargo_image),
                CapabilityProfileV1::S09,
            )
            .unwrap();
            assert!(s09.pinned_cargo_image.is_some());

            let mut missing_s09 = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            missing_s09.truncate(2);
            assert!(decode_received_descriptors(missing_s09, CapabilityProfileV1::S09).is_err());

            let ordinary_extra = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            assert!(
                decode_received_descriptors(ordinary_extra, CapabilityProfileV1::Ordinary).is_err()
            );

            let mut s09_extra = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            s09_extra.push(pinned_cargo_image.try_clone_for_transfer().unwrap().into());
            assert!(decode_received_descriptors(s09_extra, CapabilityProfileV1::S09).is_err());
        }

        #[test]
        fn descriptor_order_is_part_of_each_profile() {
            let (_temp, backend, artifact, pinned_cargo_image, _session) = fixture();

            let mut ordinary = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            ordinary.truncate(2);
            ordinary.swap(0, 1);
            assert!(decode_received_descriptors(ordinary, CapabilityProfileV1::Ordinary).is_err());

            let mut s09 = raw_descriptor_set(&backend, &artifact, &pinned_cargo_image);
            s09.swap(1, 2);
            assert!(decode_received_descriptors(s09, CapabilityProfileV1::S09).is_err());
        }

        #[test]
        fn prepared_profile_config_and_peer_identity_reject_substitution() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = s09_binding();
            let broker =
                CapabilityBroker::start(session, binding, &backend, &artifact, &pinned_cargo_image)
                    .unwrap();
            let route = BrokerRouteV2::parse(broker.route()).unwrap();

            let ordinary = ordinary_binding();
            assert!(receive_from(&route, session, ordinary).is_err());
            let wrong_config =
                CapabilityBindingV2::new(CapabilityProfileV1::S09, Some([0x92; CONFIG_ID_BYTES]))
                    .unwrap();
            assert!(receive_from(&route, session, wrong_config).is_err());

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
            let route = BrokerRouteV2 {
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
                    &BrokerRouteV2::parse(broker.route()).unwrap(),
                    BuildSession::from_bytes([0x43; 16]),
                    binding,
                )
                .is_err()
            );

            let clients = (0..8)
                .map(|_| {
                    let broker = Arc::clone(&broker);
                    std::thread::spawn(move || {
                        let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            const TEST_LIMIT: usize = MAX_ACTIVE_CONNECTIONS;
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
            let address = endpoint_address(&route.endpoint).unwrap();
            let stalled = (0..TEST_LIMIT)
                .map(|_| UnixStream::connect_addr(&address).unwrap())
                .collect::<Vec<_>>();
            wait_until(
                || broker.shutdown.active_connection_count() == TEST_LIMIT,
                "capability broker did not fill its configured admission limit",
            );

            let mut excess = UnixStream::connect_addr(&address).unwrap();
            excess
                .set_read_timeout(Some(Duration::from_secs(6)))
                .unwrap();
            wait_until(
                || {
                    broker
                        .shutdown
                        .admission_wait_started
                        .load(Ordering::Acquire)
                },
                "capability broker did not begin bounded overload admission",
            );
            let rejection_started = Instant::now();
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
                rejection_started.elapsed() < Duration::from_secs(6),
                "limit-plus-one connection was not rejected within its connection deadline"
            );
            assert!(broker.shutdown.active_connection_count() <= TEST_LIMIT);
            assert_eq!(received_descriptor_count(&excess), 0);

            drop(excess);
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
        fn exactly_max_parallel_requests_do_not_leak_slots() {
            const CLIENTS: usize = MAX_ACTIVE_CONNECTIONS;
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let binding = ordinary_binding();
            let broker = Arc::new(start_test_broker(
                session,
                binding,
                &backend,
                &artifact,
                &pinned_cargo_image,
                CLIENTS,
                BROKER_IO_TIMEOUT,
            ));
            let pause = broker.shutdown.install_worker_pause();
            let clients = (0..CLIENTS)
                .map(|_| {
                    let broker = Arc::clone(&broker);
                    thread::spawn(move || {
                        let route = BrokerRouteV2::parse(broker.route()).unwrap();
                        receive_from(&route, session, binding).unwrap();
                    })
                })
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
            for _ in 0..CLIENTS {
                pause.release();
            }
            for client in clients {
                client.join().unwrap();
            }
            wait_until(
                || broker.shutdown.active_connection_count() == 0,
                "parallel requests leaked active registry slots",
            );
            assert_eq!(
                broker.shutdown.admission_rejections.load(Ordering::Acquire),
                0
            );
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "capability_broker::platform::tests::descriptor_pressure_child",
                    "--nocapture",
                ])
                .env("FE2O3_CAPABILITY_BROKER_FD_PRESSURE_CHILD", "1")
                .output()
                .unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
                let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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
            let transferred =
                decode_received_descriptors(descriptors, CapabilityProfileV1::S09).unwrap();
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
            let route = BrokerRouteV2::parse(broker.route()).unwrap();
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

#[cfg(target_os = "linux")]
pub(crate) use platform::*;

#[cfg(not(target_os = "linux"))]
mod unsupported {
    use fe2o3_artifact_transaction::BuildSession;

    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::PinnedExecutable;
    use crate::project::PinnedDirectory;

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CapabilityProfileV1 {
        Ordinary,
        S09,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct CapabilityBindingV2;

    impl CapabilityBindingV2 {
        pub(crate) fn new(
            _profile: CapabilityProfileV1,
            _config_identity: Option<[u8; 32]>,
        ) -> Result<Self, String> {
            Ok(Self)
        }
    }

    pub(crate) struct CapabilityBroker;

    impl CapabilityBroker {
        pub(crate) fn start(
            _session: BuildSession,
            _binding: CapabilityBindingV2,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn route(&self) -> &str {
            ""
        }
    }

    pub(crate) struct BrokeredCapabilities {
        pub(crate) backend: PinnedCodegenBackend,
        pub(crate) artifact: PinnedDirectory,
        pub(crate) pinned_cargo_image: Option<PinnedExecutable>,
    }

    pub(crate) fn receive(
        _session: BuildSession,
        _binding: CapabilityBindingV2,
    ) -> Result<BrokeredCapabilities, String> {
        Err("Cargo capability transport requires Linux".to_string())
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::*;
