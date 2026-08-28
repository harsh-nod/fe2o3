use std::fs::{self, OpenOptions};
use std::io::{IoSlice, Write as _};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ed25519_dalek::SigningKey;
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV1;
use fe2o3_compiler_execution_client::PendingCompilerExecutionChildChannelV1;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionServiceLaunchManifestV1,
    CompilerExecutionServiceReadyV1, CompilerExecutionSupervisorHandoffV1,
};
use rustix::net::{
    AddressFamily, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags,
    SocketType, recv, sendmsg, socketpair,
};

use super::*;

struct Fixture {
    root: PathBuf,
    image: PathBuf,
    bytes: Vec<u8>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self::with_code(name, &[0xc3])
    }

    fn with_code(name: &str, code: &[u8]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fe2o3-supervisor-image-{name}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let image = root.join("entry");
        let bytes = static_elf_with_code(code);
        fs::write(&image, &bytes).unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o555)).unwrap();
        sealed_static_application_identity_v1(&bytes).unwrap();
        Self { root, image, bytes }
    }

    fn measurement(&self) -> ProvisionedStaticExecutableMeasurementV1 {
        ProvisionedStaticExecutableMeasurementV1::new(
            Sha256::digest(&self.bytes).into(),
            u64::try_from(self.bytes.len()).unwrap(),
        )
        .unwrap()
    }

    fn issuer_measurement(&self) -> CompilerExecutionIssuerMeasurementV1 {
        CompilerExecutionIssuerMeasurementV1::new(
            Sha256::digest(&self.bytes).into(),
            u64::try_from(self.bytes.len()).unwrap(),
        )
        .unwrap()
    }

    fn open(&self) -> File {
        File::open(&self.image).unwrap()
    }
}

fn static_elf_with_code(code: &[u8]) -> Vec<u8> {
    const HEADER: usize = 64;
    const PROGRAM: usize = 56;
    const PROGRAMS: usize = 4;
    const CODE_OFFSET: usize = 0x1000;
    assert!(!code.is_empty());
    let mut bytes = vec![0_u8; CODE_OFFSET + code.len()];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
    bytes[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
    bytes[54..56].copy_from_slice(&(PROGRAM as u16).to_le_bytes());
    bytes[56..58].copy_from_slice(&(PROGRAMS as u16).to_le_bytes());

    let table_size = (PROGRAM * PROGRAMS) as u64;
    write_program(
        &mut bytes,
        0,
        ProgramFixture {
            kind: 6,
            flags: 4,
            offset: HEADER as u64,
            virtual_address: 0x400040,
            file_size: table_size,
            memory_size: table_size,
            alignment: 8,
        },
    );
    write_program(
        &mut bytes,
        1,
        ProgramFixture {
            kind: 1,
            flags: 4,
            offset: 0,
            virtual_address: 0x400000,
            file_size: HEADER as u64 + table_size,
            memory_size: HEADER as u64 + table_size,
            alignment: 0x1000,
        },
    );
    write_program(
        &mut bytes,
        2,
        ProgramFixture {
            kind: 1,
            flags: 5,
            offset: CODE_OFFSET as u64,
            virtual_address: 0x401000,
            file_size: code.len() as u64,
            memory_size: code.len() as u64,
            alignment: 0x1000,
        },
    );
    write_program(
        &mut bytes,
        3,
        ProgramFixture {
            kind: 0x6474_e551,
            flags: 6,
            offset: 0,
            virtual_address: 0,
            file_size: 0,
            memory_size: 0,
            alignment: 16,
        },
    );
    bytes[CODE_OFFSET..].copy_from_slice(code);
    bytes
}

fn launched_probe_code(close_readiness: bool) -> Vec<u8> {
    // write(1, "LAUNCHED\n", 9), optionally close(209), then pause forever.
    let mut code = vec![
        0xb8, 1, 0, 0, 0, // mov eax, SYS_write
        0xbf, 1, 0, 0, 0, // mov edi, 1
        0x48, 0x8d, 0x35, 0, 0, 0, 0, // lea rsi, [rip + marker]
        0xba, 9, 0, 0, 0, // mov edx, 9
        0x0f, 0x05, // syscall
    ];
    if close_readiness {
        for descriptor in [9_u32, 209] {
            code.extend_from_slice(&[
                0xb8, 3, 0, 0, 0, // mov eax, SYS_close
                0xbf,
            ]);
            code.extend_from_slice(&descriptor.to_le_bytes());
            code.extend_from_slice(&[0x0f, 0x05]); // syscall
        }
    }
    let pause_offset = code.len();
    code.extend_from_slice(&[
        0xb8, 34, 0, 0, 0, // mov eax, SYS_pause
        0x0f, 0x05, // syscall
        0xeb, 0xf7, // jump back to mov eax
    ]);
    let marker_offset = code.len();
    code.extend_from_slice(b"LAUNCHED\n");
    let displacement = i32::try_from(marker_offset).unwrap() - 17;
    code[13..17].copy_from_slice(&displacement.to_le_bytes());
    assert_eq!(pause_offset + 9, marker_offset);
    code
}

struct ProgramFixture {
    kind: u32,
    flags: u32,
    offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

fn write_program(bytes: &mut [u8], index: usize, program: ProgramFixture) {
    const HEADER: usize = 64;
    const PROGRAM: usize = 56;
    let start = HEADER + index * PROGRAM;
    bytes[start..start + 4].copy_from_slice(&program.kind.to_le_bytes());
    bytes[start + 4..start + 8].copy_from_slice(&program.flags.to_le_bytes());
    bytes[start + 8..start + 16].copy_from_slice(&program.offset.to_le_bytes());
    bytes[start + 16..start + 24].copy_from_slice(&program.virtual_address.to_le_bytes());
    bytes[start + 32..start + 40].copy_from_slice(&program.file_size.to_le_bytes());
    bytes[start + 40..start + 48].copy_from_slice(&program.memory_size.to_le_bytes());
    bytes[start + 48..start + 56].copy_from_slice(&program.alignment.to_le_bytes());
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn policy(issuer: CompilerExecutionIssuerMeasurementV1) -> CompilerExecutionPolicyCapabilityV1 {
    let key = SigningKey::from_bytes(&[7; 32]);
    CompilerExecutionPolicyCapabilityV1::create(
        CompilerExecutionIssuerPolicyV1::new(
            1,
            issuer,
            sealed_static_issuer_runtime_measurement_v1(),
            key.verifying_key().to_bytes(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn credentials() -> Option<IssuerServiceCredentialProfileV1> {
    IssuerServiceCredentialProfileV1::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .ok()
}

fn signing_key(
    policy: &fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV1,
) -> CompilerExecutionSigningKeyCapabilityV1 {
    let mut seed = [7; 32];
    CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, policy).unwrap()
}

fn admitted_program(fixture: &Fixture) -> AdmittedIssuerProgramV1 {
    AdmittedIssuerProgramV1::provision(
        fixture.open(),
        fixture.measurement(),
        fixture.open(),
        policy(fixture.issuer_measurement()),
    )
    .unwrap()
}

fn bound_supervisor(fixture: &Fixture) -> Option<ProtectedIssuerSupervisorV1> {
    let profile = credentials()?;
    let program = admitted_program(fixture);
    let key = signing_key(program.policy());
    Some(
        ProtectedIssuerSupervisorV1::bind(
            program,
            profile,
            File::open(&fixture.root).unwrap(),
            key,
        )
        .unwrap(),
    )
}

fn seqpacket_pair() -> (OwnedFd, OwnedFd) {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap()
}

fn send_handoff_fixture(
    control: &OwnedFd,
    handoff: &CompilerExecutionSupervisorHandoffV1,
    descriptors: &[std::os::fd::BorrowedFd<'_>],
) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(SendAncillaryMessage::ScmRights(descriptors)));
    assert_eq!(
        sendmsg(
            control,
            &[IoSlice::new(handoff.canonical_bytes())],
            &mut ancillary,
            SendFlags::NOSIGNAL,
        )
        .unwrap(),
        handoff.canonical_bytes().len()
    );
}

fn live_launch() -> (
    std::process::Child,
    fe2o3_compiler_execution_client::CompilerExecutionServiceLaunchV1,
) {
    let mut command = Command::new("/bin/sleep");
    command
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let child = command.spawn().unwrap();
    let launch = pending.finish(child.id(), Duration::from_secs(2)).unwrap();
    (child, launch)
}

#[test]
fn exact_images_are_independently_sealed_before_authority_binding() {
    let fixture = Fixture::new("exact");
    let admitted = AdmittedIssuerProgramV1::provision(
        fixture.open(),
        fixture.measurement(),
        fixture.open(),
        policy(fixture.issuer_measurement()),
    )
    .unwrap();
    admitted.revalidate().unwrap();
    assert!(
        !admitted
            .launcher
            .snapshot
            .same_object_key(&admitted.issuer.snapshot)
    );
    for image in [&admitted.launcher, &admitted.issuer] {
        assert_eq!(
            rustix::fs::fcntl_getfl(&image.image).unwrap() & OFlags::ACCMODE,
            OFlags::RDONLY
        );
        assert!(
            rustix::fs::fcntl_get_seals(&image.image)
                .unwrap()
                .contains(REQUIRED_EXECUTABLE_SEALS_V1)
        );
        assert_eq!(image.snapshot.mode & 0o7777, EXECUTABLE_MODE_V1);
        assert!(rustix::fs::fchmod(&image.image, Mode::RUSR).is_err());
        assert!(image.image.set_len(0).is_err());
    }
    assert_ne!(
        admitted.launcher_object_identity().inode(),
        admitted.issuer_object_identity().inode()
    );
}

#[test]
fn wrong_launcher_and_issuer_measurements_reject() {
    let fixture = Fixture::new("wrong-measurement");
    let wrong_launcher = ProvisionedStaticExecutableMeasurementV1::new(
        [9; 32],
        u64::try_from(fixture.bytes.len()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        AdmittedIssuerProgramV1::provision(
            fixture.open(),
            wrong_launcher,
            fixture.open(),
            policy(fixture.issuer_measurement()),
        ),
        Err(IssuerProgramAdmissionErrorV1::MeasurementMismatch(
            "static launcher"
        ))
    ));

    let wrong_issuer = CompilerExecutionIssuerMeasurementV1::new(
        [8; 32],
        u64::try_from(fixture.bytes.len()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        AdmittedIssuerProgramV1::provision(
            fixture.open(),
            fixture.measurement(),
            fixture.open(),
            policy(wrong_issuer),
        ),
        Err(IssuerProgramAdmissionErrorV1::MeasurementMismatch(
            "compiler issuer"
        ))
    ));
}

#[test]
fn dynamic_and_invalid_source_descriptors_reject() {
    let fixture = Fixture::new("hostile-source");
    let current = std::env::current_exe().unwrap();
    let current_bytes = fs::read(&current).unwrap();
    let current_measurement = ProvisionedStaticExecutableMeasurementV1::new(
        Sha256::digest(&current_bytes).into(),
        u64::try_from(current_bytes.len()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        AdmittedIssuerProgramV1::provision(
            File::open(current).unwrap(),
            current_measurement,
            fixture.open(),
            policy(fixture.issuer_measurement()),
        ),
        Err(IssuerProgramAdmissionErrorV1::InvalidStaticImage {
            role: "static launcher",
            ..
        })
    ));

    fs::set_permissions(&fixture.image, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        AdmittedIssuerProgramV1::provision(
            fixture.open(),
            fixture.measurement(),
            fixture.open(),
            policy(fixture.issuer_measurement()),
        ),
        Err(IssuerProgramAdmissionErrorV1::InvalidSource(
            "static launcher"
        ))
    ));
}

#[test]
fn writable_and_inheritable_sources_reject() {
    let fixture = Fixture::new("descriptor-flags");
    fs::set_permissions(&fixture.image, fs::Permissions::from_mode(0o755)).unwrap();
    let writable = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fixture.image)
        .unwrap();
    assert!(matches!(
        PinnedSealedStaticExecutableV1::admit(writable, fixture.measurement(), "static launcher"),
        Err(IssuerProgramAdmissionErrorV1::InvalidSource(
            "static launcher"
        ))
    ));

    let inheritable = OpenOptions::new()
        .read(true)
        .custom_flags(0)
        .open(&fixture.image)
        .unwrap();
    rustix::io::fcntl_setfd(&inheritable, rustix::io::FdFlags::empty()).unwrap();
    assert_eq!(
        rustix::io::fcntl_getfd(&inheritable).unwrap(),
        rustix::io::FdFlags::empty()
    );
    assert!(matches!(
        PinnedSealedStaticExecutableV1::admit(
            inheritable,
            fixture.measurement(),
            "static launcher"
        ),
        Err(IssuerProgramAdmissionErrorV1::InvalidSource(
            "static launcher"
        ))
    ));
}

#[test]
fn invalid_measurements_and_runtime_policy_reject() {
    assert!(matches!(
        ProvisionedStaticExecutableMeasurementV1::new([0; 32], 1),
        Err(IssuerProgramAdmissionErrorV1::InvalidMeasurement)
    ));
    assert!(matches!(
        ProvisionedStaticExecutableMeasurementV1::new([1; 32], 0),
        Err(IssuerProgramAdmissionErrorV1::InvalidMeasurement)
    ));

    let fixture = Fixture::new("runtime-policy");
    let key = SigningKey::from_bytes(&[7; 32]);
    let wrong_runtime = CompilerExecutionIssuerMeasurementV1::new([4; 32], 1).unwrap();
    let wrong_policy = CompilerExecutionPolicyCapabilityV1::create(
        CompilerExecutionIssuerPolicyV1::new(
            1,
            fixture.issuer_measurement(),
            wrong_runtime,
            key.verifying_key().to_bytes(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        AdmittedIssuerProgramV1::provision(
            fixture.open(),
            fixture.measurement(),
            fixture.open(),
            wrong_policy,
        ),
        Err(IssuerProgramAdmissionErrorV1::RuntimePolicyMismatch)
    ));
}

#[test]
fn public_debug_contains_no_descriptor_or_source_path() {
    let fixture = Fixture::new("debug");
    let admitted = AdmittedIssuerProgramV1::provision(
        fixture.open(),
        fixture.measurement(),
        fixture.open(),
        policy(fixture.issuer_measurement()),
    )
    .unwrap();
    let rendered = format!("{admitted:?}");
    assert!(!rendered.contains("/proc/"));
    assert!(!rendered.contains(fixture.root.to_str().unwrap()));
    assert!(!rendered.contains(&format!("fd: {}", admitted.launcher.image.as_raw_fd())));
}

#[test]
fn exact_authority_inputs_bind_one_move_only_supervisor() {
    let Some(credentials) = credentials() else {
        return;
    };
    let fixture = Fixture::new("authority-bind");
    let program = admitted_program(&fixture);
    let key = signing_key(program.policy());
    let supervisor = ProtectedIssuerSupervisorV1::bind(
        program,
        credentials,
        File::open(&fixture.root).unwrap(),
        key,
    )
    .unwrap();
    supervisor.revalidate().unwrap();
    assert_eq!(supervisor.credentials(), credentials);
    assert_eq!(
        supervisor.policy().verifying_key(),
        SigningKey::from_bytes(&[7; 32]).verifying_key().as_bytes()
    );
}

#[test]
fn credential_profile_rejects_privileged_and_sentinel_identities() {
    assert_eq!(
        IssuerServiceCredentialProfileV1::new(0, 1),
        Err(IssuerServiceCredentialProfileErrorV1::InvalidUid)
    );
    assert_eq!(
        IssuerServiceCredentialProfileV1::new(u32::MAX, 1),
        Err(IssuerServiceCredentialProfileErrorV1::InvalidUid)
    );
    assert_eq!(
        IssuerServiceCredentialProfileV1::new(1, 0),
        Err(IssuerServiceCredentialProfileErrorV1::InvalidGid)
    );
    assert_eq!(
        IssuerServiceCredentialProfileV1::new(1, u32::MAX),
        Err(IssuerServiceCredentialProfileErrorV1::InvalidGid)
    );
    let profile = IssuerServiceCredentialProfileV1::new(1, 2).unwrap();
    assert_eq!(profile.uid(), 1);
    assert_eq!(profile.gid(), 2);
    assert_eq!(profile.securebits(), ISSUER_SERVICE_SECUREBITS_V1);
    assert_eq!(ISSUER_SERVICE_SECUREBITS_V1 & (1 << 4), 0);
}

#[test]
fn authority_binding_requires_the_configured_service_identity() {
    let fixture = Fixture::new("wrong-service-identity");
    let program = admitted_program(&fixture);
    let key = signing_key(program.policy());
    let wrong_uid = if rustix::process::geteuid().as_raw() == 1 {
        2
    } else {
        1
    };
    let service_gid = match rustix::process::getegid().as_raw() {
        0 | u32::MAX => 1,
        gid => gid,
    };
    let profile = IssuerServiceCredentialProfileV1::new(wrong_uid, service_gid).unwrap();
    assert!(matches!(
        ProtectedIssuerSupervisorV1::bind(
            program,
            profile,
            File::open(&fixture.root).unwrap(),
            key,
        ),
        Err(ProtectedIssuerSupervisorErrorV1::ServiceIdentityMismatch)
    ));
}

#[test]
fn hostile_root_shapes_and_metadata_drift_fail_closed() {
    let Some(profile) = credentials() else {
        return;
    };
    let fixture = Fixture::new("hostile-root");

    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o750)).unwrap();
    let program = admitted_program(&fixture);
    let key = signing_key(program.policy());
    assert!(matches!(
        ProtectedIssuerSupervisorV1::bind(
            program,
            profile,
            File::open(&fixture.root).unwrap(),
            key,
        ),
        Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "mode is not exactly 0700"
        ))
    ));
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o700)).unwrap();

    let ordinary = File::open(&fixture.image).unwrap();
    assert!(matches!(
        ProtectedIssuerSupervisorV1::bind(
            admitted_program(&fixture),
            profile,
            ordinary,
            signing_key(admitted_program(&fixture).policy()),
        ),
        Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "object is not a directory"
        ))
    ));

    let inheritable = File::open(&fixture.root).unwrap();
    rustix::io::fcntl_setfd(&inheritable, rustix::io::FdFlags::empty()).unwrap();
    let program = admitted_program(&fixture);
    assert!(matches!(
        ProtectedIssuerSupervisorV1::bind(
            program,
            profile,
            inheritable,
            signing_key(admitted_program(&fixture).policy()),
        ),
        Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "descriptor is inheritable"
        ))
    ));

    let path_only = rustix::fs::open(&fixture.root, OFlags::PATH | OFlags::CLOEXEC, Mode::empty())
        .map(File::from)
        .unwrap();
    let program = admitted_program(&fixture);
    let key = signing_key(program.policy());
    assert!(matches!(
        ProtectedIssuerSupervisorV1::bind(program, profile, path_only, key),
        Err(ProtectedIssuerSupervisorErrorV1::InvalidRoot(
            "descriptor is not read-only directory custody"
        ))
    ));

    let program = admitted_program(&fixture);
    let key = signing_key(program.policy());
    let supervisor = ProtectedIssuerSupervisorV1::bind(
        program,
        profile,
        File::open(&fixture.root).unwrap(),
        key,
    )
    .unwrap();
    fs::create_dir(fixture.root.join("changes-link-count")).unwrap();
    assert!(matches!(
        supervisor.revalidate(),
        Err(ProtectedIssuerSupervisorErrorV1::RootChanged)
    ));
}

#[test]
fn key_from_another_policy_cannot_bind_to_the_program() {
    let Some(credentials) = credentials() else {
        return;
    };
    let fixture = Fixture::new("wrong-key-policy");
    let program = admitted_program(&fixture);
    let other_key = SigningKey::from_bytes(&[8; 32]);
    let other_policy = CompilerExecutionIssuerPolicyV1::new(
        1,
        fixture.issuer_measurement(),
        sealed_static_issuer_runtime_measurement_v1(),
        other_key.verifying_key().to_bytes(),
    )
    .unwrap();
    let mut other_seed = [8; 32];
    let other_capability =
        CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut other_seed, &other_policy)
            .unwrap();
    assert!(matches!(
        ProtectedIssuerSupervisorV1::bind(
            program,
            credentials,
            File::open(&fixture.root).unwrap(),
            other_capability,
        ),
        Err(ProtectedIssuerSupervisorErrorV1::SigningKey(_))
    ));
}

#[test]
fn supervisor_debug_exposes_no_descriptor_path_or_secret_seed() {
    let Some(credentials) = credentials() else {
        return;
    };
    let fixture = Fixture::new("supervisor-debug");
    let program = admitted_program(&fixture);
    let key = signing_key(program.policy());
    let supervisor = ProtectedIssuerSupervisorV1::bind(
        program,
        credentials,
        File::open(&fixture.root).unwrap(),
        key,
    )
    .unwrap();
    let rendered = format!("{supervisor:?}");
    assert!(!rendered.contains(fixture.root.to_str().unwrap()));
    assert!(!rendered.contains("fd:"));
    assert!(!rendered.contains("[7, 7, 7"));
}

#[test]
fn exact_cross_process_handoff_is_admitted_and_revalidated() {
    let fixture = Fixture::new("exact-handoff");
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (mut child, launch) = live_launch();
    let client = launch.client();
    let submitter = launch.submitter();
    let manifest = CompilerExecutionServiceLaunchManifestV1::new(client, supervisor.policy());
    let handoff = CompilerExecutionSupervisorHandoffV1::new(submitter, manifest.clone()).unwrap();
    let (service_peer, pidfd) = launch.into_descriptors();
    let (sender, receiver) = seqpacket_pair();
    send_handoff_fixture(&sender, &handoff, &[service_peer.as_fd(), pidfd.as_fd()]);

    let accepted = supervisor
        .accept_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    assert_eq!(accepted.manifest(), &manifest);
    assert_eq!(accepted.submitter().pid(), std::process::id());
    accepted.revalidate(&supervisor).unwrap();

    child.kill().unwrap();
    child.wait().unwrap();
    assert!(matches!(
        accepted.revalidate(&supervisor),
        Err(ProtectedIssuerHandoffErrorV1::Pidfd(_))
    ));
}

fn accepted_handoff(
    supervisor: &ProtectedIssuerSupervisorV1,
) -> (
    std::process::Child,
    OwnedFd,
    AcceptedCompilerExecutionHandoffV1,
) {
    let (child, launch) = live_launch();
    let manifest =
        CompilerExecutionServiceLaunchManifestV1::new(launch.client(), supervisor.policy());
    let handoff = CompilerExecutionSupervisorHandoffV1::new(launch.submitter(), manifest).unwrap();
    let (service_peer, pidfd) = launch.into_descriptors();
    let (sender, receiver) = seqpacket_pair();
    send_handoff_fixture(&sender, &handoff, &[service_peer.as_fd(), pidfd.as_fd()]);
    let accepted = supervisor
        .accept_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    (child, sender, accepted)
}

#[test]
fn admitted_handoff_materializes_exact_sealed_ten_source_launch() {
    let fixture = Fixture::new("exact-prepared-launch");
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (mut child, _control_sender, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();

    assert_eq!(prepared.static_manifest().descriptors().len(), 10);
    assert_eq!(
        prepared.static_manifest().parent_pid(),
        std::process::id() as i32
    );
    assert_ne!(prepared.static_manifest().parent_start_time(), 0);
    assert_eq!(prepared.service_manifest(), prepared.accepted.manifest());
    assert_eq!(
        prepared
            .static_manifest()
            .descriptors()
            .iter()
            .map(|entry| entry.source_fd())
            .collect::<Vec<_>>(),
        (200..210).collect::<Vec<_>>()
    );
    assert_eq!(
        prepared
            .static_manifest()
            .descriptors()
            .iter()
            .map(|entry| entry.destination_fd())
            .collect::<Vec<_>>(),
        (0..10).collect::<Vec<_>>()
    );

    let required_manifest_seals =
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
    assert_eq!(
        rustix::fs::fcntl_get_seals(&prepared.static_manifest_file).unwrap(),
        required_manifest_seals
    );
    assert_eq!(
        prepared.static_manifest_file.metadata().unwrap().mode(),
        libc::S_IFREG | 0o400
    );
    assert_eq!(
        prepared.static_manifest_file.metadata().unwrap().len(),
        fe2o3_static_preexec_manifest::PREEXEC_MANIFEST_BYTES_V1 as u64
    );
    for source in &prepared.sources {
        assert!(
            rustix::io::fcntl_getfd(source)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
    for index in 0..prepared.sources.len() {
        let left = rustix::fs::fstat(&prepared.sources[index]).unwrap();
        for right in &prepared.sources[..index] {
            let right = rustix::fs::fstat(right).unwrap();
            assert_ne!((left.st_dev, left.st_ino), (right.st_dev, right.st_ino));
        }
    }
    assert_eq!(
        rustix::fs::fcntl_getfl(&prepared.sources[0]).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    for index in [1, 2, 9] {
        assert_eq!(
            rustix::fs::fcntl_getfl(&prepared.sources[index]).unwrap() & OFlags::ACCMODE,
            OFlags::WRONLY
        );
    }
    prepared.revalidate(&supervisor).unwrap();
    let rendered = format!("{prepared:?}");
    assert!(rendered.contains("descriptor_count: 10"));
    assert!(!rendered.contains("fd:"));
    assert!(!rendered.contains(fixture.root.to_str().unwrap()));

    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn prepared_launch_rejects_source_manifest_and_parent_substitution() {
    let fixture = Fixture::new("hostile-prepared-launch");
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };

    let (mut child, _control_sender, accepted) = accepted_handoff(&supervisor);
    let mut prepared = supervisor.prepare_launch(accepted).unwrap();
    prepared.sources.swap(1, 2);
    assert!(prepared.revalidate(&supervisor).is_err());
    child.kill().unwrap();
    child.wait().unwrap();

    let (mut child, _control_sender, accepted) = accepted_handoff(&supervisor);
    let mut prepared = supervisor.prepare_launch(accepted).unwrap();
    prepared.static_manifest_file = File::open("/dev/null").unwrap();
    assert!(prepared.revalidate(&supervisor).is_err());
    child.kill().unwrap();
    child.wait().unwrap();

    let (mut child, _control_sender, accepted) = accepted_handoff(&supervisor);
    let mut prepared = supervisor.prepare_launch(accepted).unwrap();
    let wrong_parent = prepared
        .static_manifest()
        .parent_pid()
        .checked_add(1)
        .unwrap();
    prepared.static_manifest = fe2o3_static_preexec_manifest::StaticPreexecManifestV1::new(
        wrong_parent,
        prepared.static_manifest().parent_start_time(),
        *prepared.static_manifest().executable(),
        prepared.static_manifest().descriptors().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        prepared.revalidate(&supervisor),
        Err(ProtectedIssuerLaunchPreparationErrorV1::ParentChanged)
    ));
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn prepared_launch_revalidation_detects_rustc_exit() {
    let fixture = Fixture::new("prepared-launch-rustc-exit");
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (mut child, _control_sender, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(matches!(
        prepared.revalidate(&supervisor),
        Err(ProtectedIssuerLaunchPreparationErrorV1::Handoff(
            ProtectedIssuerHandoffErrorV1::Pidfd(_)
        ))
    ));
}

#[test]
fn production_handoff_rejects_a_same_uid_submitter_before_receive() {
    let fixture = Fixture::new("same-uid-handoff");
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (sender, receiver) = seqpacket_pair();
    assert!(matches!(
        supervisor.accept_handoff(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::ClientAndSupervisorUidMatch)
    ));
    drop(sender);
}

#[test]
fn malformed_wrong_policy_and_extra_descriptors_fail_closed() {
    let fixture = Fixture::new("hostile-handoff");
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };

    let (sender, receiver) = seqpacket_pair();
    rustix::net::send(&sender, b"short", SendFlags::NOSIGNAL).unwrap();
    assert!(matches!(
        supervisor.accept_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer)
    ));

    let (mut child, launch) = live_launch();
    let actual_submitter = launch.submitter();
    let substituted_pid = if actual_submitter.pid() == 1 {
        2
    } else {
        actual_submitter.pid() - 1
    };
    let substituted_submitter =
        fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1::new(
            substituted_pid,
            actual_submitter.uid(),
            actual_submitter.gid(),
        )
        .unwrap();
    let manifest =
        CompilerExecutionServiceLaunchManifestV1::new(launch.client(), supervisor.policy());
    let substituted =
        CompilerExecutionSupervisorHandoffV1::new(substituted_submitter, manifest).unwrap();
    let (service_peer, pidfd) = launch.into_descriptors();
    let (sender, receiver) = seqpacket_pair();
    send_handoff_fixture(
        &sender,
        &substituted,
        &[service_peer.as_fd(), pidfd.as_fd()],
    );
    assert!(matches!(
        supervisor.accept_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::SubmitterCredentialsMismatch)
    ));
    child.kill().unwrap();
    child.wait().unwrap();

    let (mut child, launch) = live_launch();
    let wrong_key = SigningKey::from_bytes(&[8; 32]);
    let wrong_policy = CompilerExecutionIssuerPolicyV1::new(
        1,
        fixture.issuer_measurement(),
        sealed_static_issuer_runtime_measurement_v1(),
        wrong_key.verifying_key().to_bytes(),
    )
    .unwrap();
    let wrong_manifest =
        CompilerExecutionServiceLaunchManifestV1::new(launch.client(), &wrong_policy);
    let wrong_handoff =
        CompilerExecutionSupervisorHandoffV1::new(launch.submitter(), wrong_manifest).unwrap();
    let (service_peer, pidfd) = launch.into_descriptors();
    let (sender, receiver) = seqpacket_pair();
    send_handoff_fixture(
        &sender,
        &wrong_handoff,
        &[service_peer.as_fd(), pidfd.as_fd()],
    );
    assert!(matches!(
        supervisor.accept_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::PolicyMismatch)
    ));
    child.kill().unwrap();
    child.wait().unwrap();

    let (mut child, launch) = live_launch();
    let manifest =
        CompilerExecutionServiceLaunchManifestV1::new(launch.client(), supervisor.policy());
    let handoff = CompilerExecutionSupervisorHandoffV1::new(launch.submitter(), manifest).unwrap();
    let (service_peer, _pidfd) = launch.into_descriptors();
    let duplicate = rustix::io::fcntl_dupfd_cloexec(&service_peer, 0).unwrap();
    let (sender, receiver) = seqpacket_pair();
    send_handoff_fixture(
        &sender,
        &handoff,
        &[service_peer.as_fd(), duplicate.as_fd()],
    );
    assert!(matches!(
        supervisor.accept_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::DescriptorAlias)
    ));
    child.kill().unwrap();
    child.wait().unwrap();

    let (mut child, launch) = live_launch();
    let manifest =
        CompilerExecutionServiceLaunchManifestV1::new(launch.client(), supervisor.policy());
    let handoff = CompilerExecutionSupervisorHandoffV1::new(launch.submitter(), manifest).unwrap();
    let (service_peer, pidfd) = launch.into_descriptors();
    let extra = File::open("/dev/null").unwrap();
    let (sender, receiver) = seqpacket_pair();
    send_handoff_fixture(
        &sender,
        &handoff,
        &[service_peer.as_fd(), pidfd.as_fd(), extra.as_fd()],
    );
    assert!(matches!(
        supervisor.accept_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer)
    ));
    child.kill().unwrap();
    child.wait().unwrap();
}

fn read_exact_nonblocking(descriptor: &OwnedFd, expected: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut observed = vec![0_u8; expected.len()];
    let mut used = 0;
    while used < observed.len() {
        match rustix::io::read(descriptor, &mut observed[used..]) {
            Ok(0) => panic!("probe stdout ended before its exact marker"),
            Ok(count) => used += count,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                assert!(Instant::now() < deadline, "probe stdout timed out");
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("probe stdout failed: {error}"),
        }
    }
    assert_eq!(observed, expected);
}

fn read_published_readiness(control: &OwnedFd, expected: &CompilerExecutionServiceReadyV1) {
    let mut published = [0_u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1];
    assert_eq!(
        recv(control, &mut published, RecvFlags::empty()).unwrap().0,
        published.len()
    );
    assert_eq!(published.as_slice(), expected.canonical_bytes());
    let mut trailing = [0_u8; 1];
    assert_eq!(
        recv(control, &mut trailing, RecvFlags::empty()).unwrap().0,
        0
    );
}

fn assert_reaped(pid: rustix::process::Pid) {
    assert!(matches!(
        rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn clone3_pidfd_launch_admits_exact_readiness_and_reaps_once() {
    let fixture = Fixture::with_code("pidfd-ready", &launched_probe_code(true));
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (mut rustc_child, control_sender, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();
    let injected_readiness = rustix::io::fcntl_dupfd_cloexec(&prepared.sources[9], 0).unwrap();
    let launch_manifest = prepared.service_manifest().clone();

    let launched = supervisor
        .launch_inner::<false>(prepared, Duration::from_secs(2))
        .unwrap();
    assert!(launched.is_live().unwrap());
    let pid = rustix::process::Pid::from_raw(launched.pid() as i32).unwrap();
    read_exact_nonblocking(launched.stdout_reader_for_test(), b"LAUNCHED\n");

    let readiness =
        CompilerExecutionServiceReadyV1::new(launched.pid(), &launch_manifest, supervisor.policy())
            .unwrap();
    assert_eq!(
        rustix::io::write(&injected_readiness, readiness.canonical_bytes()).unwrap(),
        readiness.canonical_bytes().len()
    );
    drop(injected_readiness);
    let ready = launched.await_readiness(Duration::from_secs(2)).unwrap();
    assert_eq!(ready.readiness(), &readiness);
    ready.revalidate().unwrap();
    let rendered = format!("{ready:?}");
    assert!(rendered.contains(&format!("pid: {}", pid.as_raw_pid())));
    assert!(!rendered.contains("fd:"));
    let serving = ready.publish_readiness(Duration::from_secs(2)).unwrap();
    read_published_readiness(&control_sender, &readiness);
    assert_eq!(serving.readiness(), &readiness);
    serving.revalidate().unwrap();
    let rendered = format!("{serving:?}");
    assert!(rendered.contains("announced-live-issuer-custody-only"));
    assert!(!rendered.contains("fd:"));
    serving.cancel().unwrap();
    assert_reaped(pid);

    rustc_child.kill().unwrap();
    rustc_child.wait().unwrap();
}

#[test]
fn closed_cargo_control_fails_publication_and_reaps_the_issuer() {
    let fixture = Fixture::with_code("closed-readiness-control", &launched_probe_code(true));
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (mut rustc_child, control_sender, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();
    let injected_readiness = rustix::io::fcntl_dupfd_cloexec(&prepared.sources[9], 0).unwrap();
    let launch_manifest = prepared.service_manifest().clone();
    let launched = supervisor
        .launch_inner::<false>(prepared, Duration::from_secs(2))
        .unwrap();
    let pid = rustix::process::Pid::from_raw(launched.pid() as i32).unwrap();
    let readiness =
        CompilerExecutionServiceReadyV1::new(launched.pid(), &launch_manifest, supervisor.policy())
            .unwrap();
    assert_eq!(
        rustix::io::write(&injected_readiness, readiness.canonical_bytes()).unwrap(),
        readiness.canonical_bytes().len()
    );
    drop(injected_readiness);
    let ready = launched.await_readiness(Duration::from_secs(2)).unwrap();
    drop(control_sender);
    assert!(matches!(
        ready.publish_readiness(Duration::from_secs(1)),
        Err(ProtectedIssuerLaunchErrorV1::Io {
            operation: "publish protected issuer readiness to Cargo",
            ..
        })
    ));
    std::thread::sleep(Duration::from_millis(100));
    assert_reaped(pid);
    rustc_child.kill().unwrap();
    rustc_child.wait().unwrap();
}

#[test]
fn readiness_pid_substitution_and_trailing_bytes_fail_closed() {
    for trailing in [false, true] {
        let name = if trailing {
            "readiness-trailing"
        } else {
            "readiness-pid-substitution"
        };
        let fixture = Fixture::with_code(name, &launched_probe_code(true));
        let Some(supervisor) = bound_supervisor(&fixture) else {
            return;
        };
        let (mut rustc_child, _control_sender, accepted) = accepted_handoff(&supervisor);
        let prepared = supervisor.prepare_launch(accepted).unwrap();
        let injected_readiness = rustix::io::fcntl_dupfd_cloexec(&prepared.sources[9], 0).unwrap();
        let launch_manifest = prepared.service_manifest().clone();
        let launched = supervisor
            .launch_inner::<false>(prepared, Duration::from_secs(2))
            .unwrap();
        let pid = rustix::process::Pid::from_raw(launched.pid() as i32).unwrap();
        let readiness_pid = if trailing {
            launched.pid()
        } else {
            launched.pid().checked_add(1).unwrap()
        };
        let readiness = CompilerExecutionServiceReadyV1::new(
            readiness_pid,
            &launch_manifest,
            supervisor.policy(),
        )
        .unwrap();
        assert_eq!(
            rustix::io::write(&injected_readiness, readiness.canonical_bytes()).unwrap(),
            readiness.canonical_bytes().len()
        );
        if trailing {
            assert_eq!(rustix::io::write(&injected_readiness, &[0x7f]).unwrap(), 1);
        }
        drop(injected_readiness);
        let error = launched
            .await_readiness(Duration::from_secs(2))
            .unwrap_err();
        if trailing {
            assert!(matches!(
                error,
                ProtectedIssuerLaunchErrorV1::ReadinessTrailingBytes
            ));
        } else {
            assert!(matches!(
                error,
                ProtectedIssuerLaunchErrorV1::ReadinessMismatch
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
        assert_reaped(pid);
        rustc_child.kill().unwrap();
        rustc_child.wait().unwrap();
    }
}

#[test]
fn readiness_timeout_kills_reaps_and_allows_a_fresh_launch() {
    let fixture = Fixture::with_code("readiness-timeout", &launched_probe_code(false));
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (mut first_rustc, _first_control, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();
    let launched = supervisor
        .launch_inner::<false>(prepared, Duration::from_secs(2))
        .unwrap();
    let first_pid = rustix::process::Pid::from_raw(launched.pid() as i32).unwrap();
    assert!(matches!(
        launched.await_readiness(Duration::from_millis(20)),
        Err(ProtectedIssuerLaunchErrorV1::Timeout(
            "exact issuer readiness"
        ))
    ));
    std::thread::sleep(Duration::from_millis(100));
    assert_reaped(first_pid);
    first_rustc.kill().unwrap();
    first_rustc.wait().unwrap();

    let (mut second_rustc, _second_control, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();
    let launched = supervisor
        .launch_inner::<false>(prepared, Duration::from_secs(2))
        .unwrap();
    let second_pid = rustix::process::Pid::from_raw(launched.pid() as i32).unwrap();
    launched.cancel().unwrap();
    assert_reaped(second_pid);
    second_rustc.kill().unwrap();
    second_rustc.wait().unwrap();
}

#[test]
#[ignore = "requires FE2O3_STATIC_PREEXEC_LAUNCHER from the freestanding CMake build"]
fn real_static_launcher_crosses_both_exec_boundaries() {
    let launcher_path = std::env::var_os("FE2O3_STATIC_PREEXEC_LAUNCHER")
        .expect("FE2O3_STATIC_PREEXEC_LAUNCHER must name the qualified static launcher");
    let launcher_bytes = fs::read(&launcher_path).unwrap();
    let launcher_measurement = ProvisionedStaticExecutableMeasurementV1::new(
        Sha256::digest(&launcher_bytes).into(),
        launcher_bytes.len() as u64,
    )
    .unwrap();
    let fixture = Fixture::with_code("real-static-launcher", &launched_probe_code(true));
    let Some(credentials) = credentials() else {
        return;
    };
    let program = AdmittedIssuerProgramV1::provision(
        File::open(launcher_path).unwrap(),
        launcher_measurement,
        fixture.open(),
        policy(fixture.issuer_measurement()),
    )
    .unwrap();
    let key = signing_key(program.policy());
    let supervisor = ProtectedIssuerSupervisorV1::bind(
        program,
        credentials,
        File::open(&fixture.root).unwrap(),
        key,
    )
    .unwrap();
    let (mut rustc_child, control_sender, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();
    let injected_readiness = rustix::io::fcntl_dupfd_cloexec(&prepared.sources[9], 0).unwrap();
    let launch_manifest = prepared.service_manifest().clone();
    let launched = supervisor
        .launch_inner::<false>(prepared, Duration::from_secs(2))
        .unwrap();
    read_exact_nonblocking(launched.stdout_reader_for_test(), b"LAUNCHED\n");
    let readiness =
        CompilerExecutionServiceReadyV1::new(launched.pid(), &launch_manifest, supervisor.policy())
            .unwrap();
    assert_eq!(
        rustix::io::write(&injected_readiness, readiness.canonical_bytes()).unwrap(),
        readiness.canonical_bytes().len()
    );
    drop(injected_readiness);
    let ready = launched.await_readiness(Duration::from_secs(2)).unwrap();
    ready.revalidate().unwrap();
    let serving = ready.publish_readiness(Duration::from_secs(2)).unwrap();
    read_published_readiness(&control_sender, &readiness);
    serving.revalidate().unwrap();
    serving.cancel().unwrap();
    rustc_child.kill().unwrap();
    rustc_child.wait().unwrap();
}

#[test]
#[ignore = "subprocess helper for abrupt supervisor-parent death"]
fn clone3_parent_death_helper() {
    let fixture = Fixture::with_code("parent-death-helper", &launched_probe_code(false));
    let Some(supervisor) = bound_supervisor(&fixture) else {
        return;
    };
    let (mut rustc_child, _control_sender, accepted) = accepted_handoff(&supervisor);
    let prepared = supervisor.prepare_launch(accepted).unwrap();
    let launched = supervisor
        .launch_inner::<false>(prepared, Duration::from_secs(2))
        .unwrap();
    rustc_child.kill().unwrap();
    rustc_child.wait().unwrap();
    fs::remove_dir_all(&fixture.root).unwrap();
    println!("FE2O3_ISSUER_CHILD_PID={}", launched.pid());
    std::io::stdout().flush().unwrap();
    std::process::exit(0);
}

#[test]
fn gated_child_cannot_outlive_an_abrupt_supervisor_parent_exit() {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "tests::clone3_parent_death_helper",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "helper stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let pid = stdout
        .lines()
        .find_map(|line| {
            line.split_once("FE2O3_ISSUER_CHILD_PID=")
                .map(|(_, pid)| pid)
        })
        .unwrap_or_else(|| panic!("helper omitted child PID: {stdout}"))
        .parse::<u32>()
        .unwrap();
    let process = PathBuf::from(format!("/proc/{pid}"));
    let deadline = Instant::now() + Duration::from_secs(2);
    while process.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !process.exists(),
        "pidfd child survived its abrupt supervisor-parent exit"
    );
}
