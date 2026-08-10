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
    use std::fs::{self, File};
    use std::io::{self, IoSlice, IoSliceMut, Read, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::{AsFd, AsRawFd, OwnedFd};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

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

    impl BrokerPeerIdentityV2 {
        fn current() -> Result<Self, String> {
            let pid = std::process::id();
            let start_time_ticks = process_start_time_ticks(pid)?;
            let (pinned, metadata) = pin_process_executable(pid)?;
            let identity = Self {
                uid: unsafe { libc::geteuid() },
                pid,
                start_time_ticks,
                device: metadata.dev(),
                inode: metadata.ino(),
                mode: metadata.mode(),
                executable_sha256: *pinned.sha256(),
            };
            if process_start_time_ticks(pid)? != start_time_ticks {
                return Err("current broker process identity changed while pinning".into());
            }
            Ok(identity)
        }

        const fn object_identity(self) -> LinuxObjectIdentityV3 {
            LinuxObjectIdentityV3::from_linux_stat(self.device, self.inode, self.mode)
        }

        fn require_same_executable(self, current: Self) -> Result<(), String> {
            if self.uid != current.uid
                || self.object_identity() != current.object_identity()
                || self.executable_sha256 != current.executable_sha256
            {
                return Err(
                    "capability broker route does not name the current cargo-fe2o3 executable"
                        .into(),
                );
            }
            Ok(())
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
            let (pinned, _) = pin_process_executable(peer_pid)?;
            let final_start = process_start_time_ticks(peer_pid)?;
            if final_start != initial_start {
                return Err("capability broker peer PID was reused while authenticating".into());
            }
            if pinned.object_identity() != self.object_identity()
                || pinned.sha256() != &self.executable_sha256
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
            let (pinned, _) = pin_process_executable(client_pid)?;
            if process_start_time_ticks(client_pid)? != initial_start {
                return Err("capability broker client PID was reused while authenticating".into());
            }
            if pinned.object_identity() != self.object_identity()
                || pinned.sha256() != &self.executable_sha256
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
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }

    impl CapabilityBroker {
        pub(crate) fn start(
            session: BuildSession,
            binding: CapabilityBindingV2,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
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
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
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
                        stop: worker_stop,
                    }
                    .serve();
                })
                .map_err(|error| format!("failed to start capability broker: {error}"))?;
            Ok(Self {
                route,
                stop,
                worker: Some(worker),
            })
        }

        pub(crate) fn route(&self) -> &str {
            &self.route
        }
    }

    impl Drop for CapabilityBroker {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
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
        let current = BrokerPeerIdentityV2::current()?;
        route.peer.require_same_executable(current)?;
        let address = endpoint_address(&route.endpoint)?;
        let mut stream = UnixStream::connect_addr(&address)
            .map_err(|error| format!("failed to connect to capability broker: {error}"))?;
        stream
            .set_read_timeout(Some(BROKER_IO_TIMEOUT))
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
        stop: Arc<AtomicBool>,
    }

    impl BrokerServer {
        fn serve(self) {
            while !self.stop.load(Ordering::Acquire) {
                match self.listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = self.serve_one(&mut stream);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        }

        fn serve_one(&self, stream: &mut UnixStream) -> io::Result<()> {
            stream.set_read_timeout(Some(BROKER_IO_TIMEOUT))?;
            self.executable
                .authenticate_client(stream)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            let mut request = vec![0_u8; REQUEST_BYTES];
            stream.read_exact(&mut request)?;
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
            let mut descriptors = vec![self.backend.as_fd(), self.artifact.as_fd()];
            if let Some(pinned_cargo_image) = &pinned_cargo_image {
                descriptors.push(pinned_cargo_image.as_fd());
            }
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
                return Err(io::Error::other("capability control buffer is too small"));
            }
            let response = response_bytes(&self.secret, challenge, request_auth);
            let sent = sendmsg(
                &*stream,
                &[IoSlice::new(&response)],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .map_err(io::Error::from)?;
            if sent != response.len() {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "capability broker response was truncated",
                ));
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
        Ok(hex(&random_bytes()?))
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
        use std::path::PathBuf;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        use fe2o3_artifact_transaction::{BuildInvocation, ProducerIdentity, begin_build_attempt};

        use super::*;

        static NEXT: AtomicU64 = AtomicU64::new(1);

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
            let mut mock = std::process::Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .unwrap();
            let mock_pid = mock.id();
            let mock_start_time = process_start_time_ticks(mock_pid).unwrap();
            let (mock_image, mock_metadata) = pin_process_executable(mock_pid).unwrap();
            let route = BrokerRouteV2 {
                endpoint: random_endpoint().unwrap(),
                secret: random_bytes().unwrap(),
                binding: ordinary_binding(),
                peer: BrokerPeerIdentityV2 {
                    uid: unsafe { libc::geteuid() },
                    pid: mock_pid,
                    start_time_ticks: mock_start_time,
                    device: mock_metadata.dev(),
                    inode: mock_metadata.ino(),
                    mode: mock_metadata.mode(),
                    executable_sha256: *mock_image.sha256(),
                },
            };

            let result = receive_from(&route, session, ordinary_binding());
            mock.kill().unwrap();
            mock.wait().unwrap();
            let error = result
                .err()
                .expect("an arbitrary executable route must fail closed");
            assert!(error.contains("does not name the current cargo-fe2o3 executable"));
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
