//! Actual descriptor/image staging and inert codec tests only. No retained
//! runtime/backing/child or positive deployment is fabricated in these tests.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
};

const LIMIT: usize = 1 << 34;
const SESSION: [u8; 32] = [11; 32];
const RUNTIME: [u8; 32] = [12; 32];

#[test]
fn closed_launch_signature_consumes_actual_backing_and_original_custody() {
    let _: unsafe fn(
        Backing,
        [u8; 32],
        Credentials,
        Duration,
        &mut Cleanup,
        &mut Budget<'_>,
    ) -> Result<(ManagedProofHelper, usize)> = launch;
    let _: fn(ManagedProofHelper, Duration, &mut Budget<'_>) -> Result<CleanupPoll> =
        ManagedProofHelper::finish;
    fn retained<T: Send + 'static>() {}
    retained::<ManagedProofHelper>();
}

fn clone_bootstrap(channels: &Channels) -> File {
    rustix::io::fcntl_dupfd_cloexec(&channels.child, 0)
        .map(File::from)
        .unwrap()
}

#[test]
fn both_actual_bootstrap_ends_receive_credentials_before_staging() {
    let channels = helper_channels().unwrap();
    let fds = [
        channels.root.as_raw_fd(),
        channels.child.as_raw_fd(),
        channels.exec_reader.as_raw_fd(),
        channels.exec_writer.as_raw_fd(),
        channels.profile_reader.as_raw_fd(),
        channels.profile_writer.as_raw_fd(),
        channels.gate_reader.as_raw_fd(),
        channels.gate_writer.as_raw_fd(),
    ];
    for (index, fd) in fds.iter().enumerate() {
        assert!(!fds[..index].contains(fd));
    }
    assert!(net::sockopt::socket_passcred(&channels.root).unwrap());
    assert!(net::sockopt::socket_passcred(&channels.child).unwrap());
    validate_bootstrap(&clone_bootstrap(&channels), &channels).unwrap();
    assert_eq!(Channels::STORAGE, 8 * native::FILE_STORAGE);
}

#[test]
fn staged_bootstrap_rejects_other_endpoint_or_equal_shape_socket() {
    let channels = helper_channels().unwrap();
    let wrong_end = rustix::io::fcntl_dupfd_cloexec(&channels.root, 0)
        .map(File::from)
        .unwrap();
    let other = helper_channels().unwrap();
    for wrong in [wrong_end, clone_bootstrap(&other)] {
        assert!(matches!(
            validate_bootstrap(&wrong, &channels),
            Err(ProofHelperLaunchError::Invalid(
                "proof helper bootstrap object changed"
            ))
        ));
    }
    validate_bootstrap(&clone_bootstrap(&channels), &channels).unwrap();
}

#[test]
fn staged_bootstrap_refuses_mutated_flags_on_each_retained_endpoint() {
    for mode in 0..6 {
        let channels = helper_channels().unwrap();
        let alias = clone_bootstrap(&channels);
        match mode {
            0 => net::sockopt::set_socket_passcred(&channels.root, false).unwrap(),
            1 => net::sockopt::set_socket_passcred(&channels.child, false).unwrap(),
            2 => rustix::io::fcntl_setfd(&alias, FdFlags::empty()).unwrap(),
            3 => rustix::fs::fcntl_setfl(&channels.root, rustix::fs::OFlags::empty()).unwrap(),
            4 => rustix::fs::fcntl_setfl(&alias, rustix::fs::OFlags::empty()).unwrap(),
            _ => rustix::fs::fcntl_setfl(
                &alias,
                rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::APPEND,
            )
            .unwrap(),
        }
        assert!(matches!(
            validate_bootstrap(&alias, &channels),
            Err(ProofHelperLaunchError::Invalid(
                "proof helper bootstrap flags changed"
            ))
        ));
    }
}

#[test]
fn source_charge_includes_full_image_four_channels_and_binding() {
    for length in [1, 8192, 1024 * 1024 * 1024] {
        let m = Measurement::new([13; 32], length, 1024 * 1024 * 1024).unwrap();
        let source = source_storage(m).unwrap();
        assert_eq!(
            source,
            Image::file_storage(m).unwrap() + 4 * native::FILE_STORAGE + BINDING_STORAGE
        );
        assert!(Stage::storage_for_sources(source).unwrap() > source);
    }
}

#[test]
#[allow(unsafe_code)]
fn real_sealed_stage_binds_only_fd3_and_holds_exec_eof_until_drop() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("helper");
    let bytes = crate::provisioning_entrypoint::static_pause_elf(73);
    fs::write(&path, &bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
    let measurement = Measurement::new(
        Sha256::digest(&bytes).into(),
        bytes.len() as u64,
        1024 * 1024,
    )
    .unwrap();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Channels::STORAGE + Image::file_storage(measurement).unwrap())
        .unwrap();
    let channels = helper_channels().unwrap();
    let (image, charge) = Image::seal_source_for_owner(
        File::open(path).unwrap(),
        measurement,
        fe2o3_protected_static_executable::ProtectedStaticExecutableOwnerV1::current(),
        "private proof helper staging fixture",
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (transfer, charge) = image.try_clone_for_exec(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    b.reserve_storage(BINDING_STORAGE).unwrap();
    let bindings = [Binding::new(channels.child.as_fd(), BOOTSTRAP_FD).unwrap()];
    // SAFETY: these real, exclusively owned image/channel inputs are fully paid;
    // this test never spawns or fabricates deployment, backing or child approval.
    let (stage, charge) = unsafe {
        Stage::stage(
            &transfer,
            &bindings,
            channels.profile_writer.as_fd(),
            channels.gate_reader.as_fd(),
            channels.exec_writer.as_fd(),
            source_storage(measurement).unwrap(),
            &mut b,
        )
    }
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    image
        .revalidate_exec_clone(stage.executable(), &mut b)
        .unwrap();
    validate_bootstrap(stage.binding(3).unwrap(), &channels).unwrap();
    for destination in 4..32 {
        assert!(stage.binding(destination).is_none());
    }
    let Channels {
        exec_reader,
        exec_writer,
        ..
    } = channels;
    drop(exec_writer);
    let mut buffer = [0; 1];
    assert_eq!(
        net::recv(&exec_reader, &mut buffer, net::RecvFlags::DONTWAIT),
        Err(rustix::io::Errno::AGAIN)
    );
    drop(stage);
    assert_eq!(
        net::recv(&exec_reader, &mut buffer, net::RecvFlags::DONTWAIT).unwrap(),
        (0, 0)
    );
}

fn encoded(
    kind: Kind,
    pid: Pid,
    session: [u8; 32],
    runtime: [u8; 32],
    b: &mut Budget<'_>,
) -> [u8; WIRE] {
    let (record, charge) =
        Record::new(kind, native::pid_u32(pid).unwrap(), session, runtime, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let bytes = *record.canonical_bytes();
    drop(record);
    b.release_storage(charge.additional_storage()).unwrap();
    bytes
}

#[test]
fn record_association_rejects_each_independent_mutation() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(WIRE).unwrap();
    let pid = rustix::process::getpid();
    let bytes = encoded(Kind::Ready, pid, SESSION, RUNTIME, &mut b);
    check_record(&bytes, Kind::Ready, pid, SESSION, RUNTIME, &mut b).unwrap();
    let other_pid = Pid::from_raw(if pid.as_raw_pid() == 1 { 2 } else { 1 }).unwrap();
    for (kind, expected_pid, session, runtime) in [
        (Kind::Finished, pid, SESSION, RUNTIME),
        (Kind::Ready, other_pid, SESSION, RUNTIME),
        (Kind::Ready, pid, [14; 32], RUNTIME),
        (Kind::Ready, pid, SESSION, [15; 32]),
    ] {
        assert!(matches!(
            check_record(&bytes, kind, expected_pid, session, runtime, &mut b),
            Err(ProofHelperLaunchError::Invalid(
                "proof helper bootstrap association mismatch"
            ))
        ));
        assert_eq!(b.storage(), WIRE);
    }
}

#[test]
fn finished_record_is_distinct_from_ready_and_never_implies_a_reap() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(WIRE).unwrap();
    let pid = rustix::process::getpid();
    let bytes = encoded(Kind::Finished, pid, SESSION, RUNTIME, &mut b);
    check_record(&bytes, Kind::Finished, pid, SESSION, RUNTIME, &mut b).unwrap();
    assert!(matches!(
        check_record(&bytes, Kind::Ready, pid, SESSION, RUNTIME, &mut b),
        Err(ProofHelperLaunchError::Invalid(_))
    ));
    // No Child or CleanupPoll was constructed; inert framing provides neither.
    assert_eq!(b.storage(), WIRE);
}

#[test]
fn record_input_floor_and_full_decode_match_work_are_required() {
    let pid = rustix::process::getpid();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(WIRE).unwrap();
    let bytes = encoded(Kind::Ready, pid, SESSION, RUNTIME, &mut b);
    let start = b.work();
    check_record(&bytes, Kind::Ready, pid, SESSION, RUNTIME, &mut b).unwrap();
    let needed = b.work() - start;
    assert_eq!(
        needed,
        2 * fe2o3_compiler_execution_protocol::PROOF_EXECUTOR_BOOTSTRAP_WORK_V1
    );
    for mode in 0..3 {
        let mut work = Work::new(needed - usize::from(mode == 2));
        let mut b = Budget::new(&mut work, LIMIT);
        let floor = WIRE - usize::from(mode == 1);
        b.reserve_storage(floor).unwrap();
        let result = b.with_prepaid_scope(floor, 0, 0, 0, |b| {
            check_record(&bytes, Kind::Ready, pid, SESSION, RUNTIME, b)
        });
        match mode {
            0 => result.unwrap(),
            1 => assert!(matches!(
                result,
                Err(ProofHelperLaunchError::Record(RecordError::Resource(
                    Resource::Accounting
                )))
            )),
            _ => assert!(matches!(
                result,
                Err(ProofHelperLaunchError::Record(RecordError::Resource(
                    Resource::Work(_)
                )))
            )),
        }
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn expired_boundary_refuses_instead_of_performing_a_later_phase() {
    assert!(matches!(
        ensure_deadline(Instant::now(), "private boundary"),
        Err(ProofHelperLaunchError::Native(NativeError::Transport(
            launch_io::Failure::Timeout("private boundary")
        )))
    ));
}
