//! Descriptor transport from the `cargo-fe2o3` parent to managed rustc wrappers.
//!
//! Cargo receives only an abstract-socket endpoint and build-session binding. The broker checks
//! Linux peer credentials and the exact `cargo-fe2o3` executable identity before transferring a
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
//! client does not authenticate the broker server. Untrusted build dependencies require a
//! separate process sandbox; the endpoint and session are routing bindings, not bearer secrets.

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
    use rustix::net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
    };

    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::PinnedExecutable;
    use crate::project::PinnedDirectory;

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";
    const REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-V1\0";
    const S09_REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-09\0";
    const _: () = assert!(REQUEST_MAGIC.len() == S09_REQUEST_MAGIC.len());
    const ENDPOINT_BYTES: usize = 32;
    const ENDPOINT_HEX_BYTES: usize = ENDPOINT_BYTES * 2;
    const RECEIVED_DESCRIPTOR_FLOOR: i32 = 199;

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
    }

    #[derive(Clone, Copy)]
    struct ExecutableIdentity {
        device: u64,
        inode: u64,
    }

    impl ExecutableIdentity {
        fn current() -> io::Result<Self> {
            let executable = std::env::current_exe()?;
            let metadata = fs::metadata(executable)?;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }

        fn matches_peer(self, pid: i32) -> bool {
            fs::metadata(PathBuf::from(format!("/proc/{pid}/exe")))
                .map(|metadata| metadata.dev() == self.device && metadata.ino() == self.inode)
                .unwrap_or(false)
        }
    }

    pub(crate) struct CapabilityBroker {
        endpoint: String,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }

    impl CapabilityBroker {
        pub(crate) fn start(
            session: BuildSession,
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
            let executable = ExecutableIdentity::current().map_err(|error| {
                format!("failed to identify capability broker executable: {error}")
            })?;
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let worker = thread::Builder::new()
                .name("fe2o3-capability-broker".to_string())
                .spawn(move || {
                    serve(
                        listener,
                        session,
                        executable,
                        backend,
                        artifact,
                        pinned_cargo_image,
                        worker_stop,
                    );
                })
                .map_err(|error| format!("failed to start capability broker: {error}"))?;
            Ok(Self {
                endpoint,
                stop,
                worker: Some(worker),
            })
        }

        pub(crate) fn endpoint(&self) -> &str {
            &self.endpoint
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
        profile: CapabilityProfileV1,
    ) -> Result<BrokeredCapabilities, String> {
        let endpoint = std::env::var(CAPABILITY_BROKER_ENV)
            .map_err(|_| format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}"))?;
        receive_from(&endpoint, session, profile)
    }

    fn receive_from(
        endpoint: &str,
        session: BuildSession,
        profile: CapabilityProfileV1,
    ) -> Result<BrokeredCapabilities, String> {
        let address = endpoint_address(endpoint)?;
        let mut stream = UnixStream::connect_addr(&address)
            .map_err(|error| format!("failed to connect to capability broker: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| format!("failed to bound capability broker read: {error}"))?;
        let request = request_bytes(session, profile);
        stream
            .write_all(&request)
            .map_err(|error| format!("failed to authenticate to capability broker: {error}"))?;

        let mut response = [0_u8; 1];
        let mut iov = [IoSliceMut::new(&mut response)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let message = recvmsg(&stream, &mut iov, &mut ancillary, RecvFlags::CMSG_CLOEXEC)
            .map_err(|error| format!("failed to receive brokered capabilities: {error}"))?;
        if message.flags.contains(ReturnFlags::CTRUNC) {
            return Err("capability broker descriptor response was truncated".to_string());
        }
        if message.bytes != 1 || response != [1] {
            return Err("capability broker returned a malformed response".to_string());
        }
        let mut descriptors = Vec::new();
        for message in ancillary.drain() {
            if let RecvAncillaryMessage::ScmRights(received) = message {
                descriptors.extend(received);
            }
        }
        decode_received_descriptors(descriptors, profile)
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

    fn serve(
        listener: UnixListener,
        session: BuildSession,
        executable: ExecutableIdentity,
        backend: File,
        artifact: File,
        pinned_cargo_image: File,
        stop: Arc<AtomicBool>,
    ) {
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = serve_one(
                        &mut stream,
                        session,
                        executable,
                        &backend,
                        &artifact,
                        &pinned_cargo_image,
                    );
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                }
                Err(_) => break,
            }
        }
    }

    fn serve_one(
        stream: &mut UnixStream,
        session: BuildSession,
        executable: ExecutableIdentity,
        backend: &File,
        artifact: &File,
        pinned_cargo_image: &File,
    ) -> io::Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let credentials =
            rustix::net::sockopt::socket_peercred(&*stream).map_err(io::Error::from)?;
        let current_uid = unsafe { libc::geteuid() };
        if credentials.uid.as_raw() != current_uid
            || !executable.matches_peer(credentials.pid.as_raw_nonzero().get())
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "capability broker peer is not the cargo-fe2o3 executable",
            ));
        }
        let mut request = vec![0_u8; REQUEST_MAGIC.len() + 16];
        stream.read_exact(&mut request)?;
        let profile = request_profile(&request, session).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "capability broker request is not bound to this build session and profile",
            )
        })?;

        // SCM_RIGHTS preserves the open-file-description offset. Give each S09 wrapper an
        // independently opened description of the retained pinned image. Ordinary V1 clients
        // retain their historical two-descriptor response.
        let pinned_cargo_image = (profile == CapabilityProfileV1::S09)
            .then(|| File::open(format!("/proc/self/fd/{}", pinned_cargo_image.as_raw_fd())))
            .transpose()?;
        let mut descriptors = vec![backend.as_fd(), artifact.as_fd()];
        if let Some(pinned_cargo_image) = &pinned_cargo_image {
            descriptors.push(pinned_cargo_image.as_fd());
        }
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(io::Error::other("capability control buffer is too small"));
        }
        let response = [1_u8];
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

    fn request_bytes(session: BuildSession, profile: CapabilityProfileV1) -> Vec<u8> {
        let mut request = Vec::with_capacity(REQUEST_MAGIC.len() + 16);
        request.extend_from_slice(profile.request_magic());
        request.extend_from_slice(session.as_bytes());
        request
    }

    fn request_profile(request: &[u8], session: BuildSession) -> Option<CapabilityProfileV1> {
        [CapabilityProfileV1::Ordinary, CapabilityProfileV1::S09]
            .into_iter()
            .find(|profile| request == request_bytes(session, *profile))
    }

    fn endpoint_address(endpoint: &str) -> Result<SocketAddr, String> {
        if endpoint.len() != ENDPOINT_HEX_BYTES
            || endpoint
                .bytes()
                .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        {
            return Err("capability broker endpoint is not canonical lowercase hexadecimal".into());
        }
        SocketAddr::from_abstract_name(format!("fe2o3-cap-v1-{endpoint}").as_bytes())
            .map_err(|error| format!("invalid capability broker endpoint: {error}"))
    }

    fn random_endpoint() -> io::Result<String> {
        let mut bytes = [0_u8; ENDPOINT_BYTES];
        File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut endpoint = String::with_capacity(ENDPOINT_HEX_BYTES);
        for byte in bytes {
            endpoint.push(char::from(HEX[(byte >> 4) as usize]));
            endpoint.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
        Ok(endpoint)
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

        #[test]
        fn transfers_exact_capabilities_after_path_substitution() {
            let (temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let backend_sha = *backend.sha256();
            let cargo_sha = *pinned_cargo_image.sha256();
            let original_artifact = temp.0.join("artifact");
            let moved_artifact = temp.0.join("moved-artifact");
            let broker =
                CapabilityBroker::start(session, &backend, &artifact, &pinned_cargo_image).unwrap();

            fs::write(temp.0.join("backend.so"), b"replacement backend bytes").unwrap();
            fs::rename(&original_artifact, &moved_artifact).unwrap();
            fs::create_dir(&original_artifact).unwrap();

            let transferred =
                receive_from(broker.endpoint(), session, CapabilityProfileV1::S09).unwrap();
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
        fn ordinary_profile_preserves_the_two_descriptor_v1_contract() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let backend_sha = *backend.sha256();
            let broker =
                CapabilityBroker::start(session, &backend, &artifact, &pinned_cargo_image).unwrap();

            let transferred =
                receive_from(broker.endpoint(), session, CapabilityProfileV1::Ordinary).unwrap();
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
        fn rejects_wrong_session_and_serves_concurrent_exact_clients() {
            let (_temp, backend, artifact, pinned_cargo_image, session) = fixture();
            let backend_sha = *backend.sha256();
            let cargo_sha = *pinned_cargo_image.sha256();
            let broker = Arc::new(
                CapabilityBroker::start(session, &backend, &artifact, &pinned_cargo_image).unwrap(),
            );
            assert!(
                receive_from(
                    broker.endpoint(),
                    BuildSession::from_bytes([0x43; 16]),
                    CapabilityProfileV1::S09,
                )
                .is_err()
            );

            let clients = (0..8)
                .map(|_| {
                    let broker = Arc::clone(&broker);
                    std::thread::spawn(move || {
                        let transferred =
                            receive_from(broker.endpoint(), session, CapabilityProfileV1::S09)
                                .unwrap();
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
            let broker =
                CapabilityBroker::start(session, &backend, &artifact, &pinned_cargo_image).unwrap();
            let endpoint = broker.endpoint().to_owned();
            drop(broker);

            assert!(receive_from(&endpoint, session, CapabilityProfileV1::Ordinary).is_err());
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

    pub(crate) struct CapabilityBroker;

    impl CapabilityBroker {
        pub(crate) fn start(
            _session: BuildSession,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn endpoint(&self) -> &str {
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
        _profile: CapabilityProfileV1,
    ) -> Result<BrokeredCapabilities, String> {
        Err("Cargo capability transport requires Linux".to_string())
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::*;
