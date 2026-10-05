//! Inert descriptor/argument mechanics, not approved compiler or deployment fixtures.
use super::*;
use crate::native_work;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};

const SOURCE: usize = 1 << 20;
const LIMIT: usize = 1 << 32;

struct Fixture {
    directory: tempfile::TempDir,
    cwd: File,
    file: File,
    reader: File,
    writer: File,
    arguments: Vec<CString>,
    environment: Vec<CString>,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let cwd = File::open(directory.path()).unwrap();
        let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        Self {
            directory,
            cwd,
            file: File::open("/dev/null").unwrap(),
            reader: File::from(reader),
            writer: File::from(writer),
            arguments: ["captured-rustc", "--cfg", "", "--cfg", "two words"]
                .map(|v| CString::new(v).unwrap())
                .into(),
            environment: ["A=", "B=x=y"].map(|v| CString::new(v).unwrap()).into(),
        }
    }
    fn stage(
        &self,
        standard_io: [Option<BorrowedFd<'_>>; 3],
        bindings: &[Binding<'_>],
        cwd: &File,
        b: &mut Budget<'_>,
    ) -> Result<(Stage, Storage)> {
        // SOURCE covers these inert handles and owned input capacities. No ELF,
        // role, directory mapping or credential admission is claimed by this test.
        unsafe {
            Stage::stage_compiler(
                &self.file,
                &self.arguments,
                &self.environment,
                cwd.as_fd(),
                standard_io,
                bindings,
                self.writer.as_fd(),
                self.reader.as_fd(),
                self.writer.as_fd(),
                SOURCE,
                b,
            )
        }
    }
    fn streams(&self) -> [Option<BorrowedFd<'_>>; 3] {
        [Some(self.reader.as_fd()), Some(self.writer.as_fd()), None]
    }
    fn stage_channel(
        &self,
        standard_io: [Option<BorrowedFd<'_>>; 3],
        bindings: &[Binding<'_>],
        transfer: BorrowedFd<'_>,
        b: &mut Budget<'_>,
    ) -> Result<(Stage, Storage)> {
        // Inert staging only, never a claim of authenticated root custody.
        unsafe {
            Stage::stage_compiler_with_child_channel(
                &self.file,
                &self.arguments,
                &self.environment,
                self.cwd.as_fd(),
                standard_io,
                bindings,
                self.writer.as_fd(),
                self.reader.as_fd(),
                self.writer.as_fd(),
                transfer,
                SOURCE,
                b,
            )
        }
    }
}

fn transfer_pair(kind: rustix::net::SocketType) -> (OwnedFd, OwnedFd) {
    rustix::net::socketpair(
        rustix::net::AddressFamily::UNIX,
        kind,
        rustix::net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap()
}

#[test]
fn runtime_checkpoints_require_original_channel_stage_and_prepay_child_work() {
    let f = Fixture::new();
    let (_receiver, transfer) = transfer_pair(rustix::net::SocketType::SEQPACKET);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SOURCE).unwrap();
    let (plain, charge) = f.stage(f.streams(), &[], &f.cwd, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert!(!plain.has_runtime_checkpoints());
    assert!(plain.require_runtime_checkpoints(&mut b).is_err());
    b.release_storage(charge.additional_storage()).unwrap();

    let (stage, charge) = f
        .stage_channel(f.streams(), &[], transfer.as_fd(), &mut b)
        .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let before = stage.spawn_work(63).unwrap();
    assert!(!stage.has_runtime_checkpoints());
    let stage = stage.require_runtime_checkpoints(&mut b).unwrap();
    assert!(stage.has_runtime_checkpoints());
    assert_eq!(
        stage.spawn_work(63).unwrap(),
        before + native_work::COMPILER_TRACE_WORK
    );
    assert_eq!(stage.compiler_arguments(), Some(f.arguments.as_slice()));
    assert_eq!(stage.compiler_environment(), Some(f.environment.as_slice()));
    assert!(stage.require_runtime_checkpoints(&mut b).is_err());
    b.release_storage(charge.additional_storage()).unwrap();
}

#[test]
fn runtime_checkpoint_transition_refuses_unfunded_original_stage() {
    let f = Fixture::new();
    let (_receiver, transfer) = transfer_pair(rustix::net::SocketType::SEQPACKET);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SOURCE).unwrap();
    let (stage, charge) = f
        .stage_channel(f.streams(), &[], transfer.as_fd(), &mut b)
        .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let mut empty_work = Work::new(0);
    let mut empty = Budget::new(&mut empty_work, LIMIT);
    empty.reserve_storage(charge.additional_storage()).unwrap();
    assert!(matches!(
        stage.require_runtime_checkpoints(&mut empty),
        Err(Error::Resource(_))
    ));
    // The input descriptors and invocation remain owned by the original fixture.
    assert!(rustix::fs::fstat(&f.file).is_ok());
    assert!(rustix::fs::fstat(&f.cwd).is_ok());
    b.release_storage(charge.additional_storage()).unwrap();
}

#[test]
fn runtime_checkpoint_transition_checks_exact_floor_work_and_scratch() {
    let f = Fixture::new();
    let (_receiver, transfer) = transfer_pair(rustix::net::SocketType::SEQPACKET);
    for case in 0..4 {
        let mut staging_work = Work::new(LIMIT);
        let mut staging = Budget::new(&mut staging_work, LIMIT);
        staging.reserve_storage(SOURCE).unwrap();
        let (stage, charge) = f
            .stage_channel(f.streams(), &[], transfer.as_fd(), &mut staging)
            .unwrap();
        staging
            .reserve_storage(charge.additional_storage())
            .unwrap();
        let floor = stage.retained_storage();
        let work_limit = Stage::RUNTIME_CHECKPOINT_STAGING_WORK - usize::from(case == 1);
        let reserved = floor - usize::from(case == 2);
        let peak = floor + Stage::RUNTIME_CHECKPOINT_STAGING_SCRATCH;
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, peak - usize::from(case == 3));
        b.reserve_storage(reserved).unwrap();
        let result = stage.require_runtime_checkpoints(&mut b);
        assert_eq!(b.storage(), reserved);
        match case {
            0 => {
                let stage = result.unwrap();
                assert!(stage.has_runtime_checkpoints());
                assert_eq!(b.work(), Stage::RUNTIME_CHECKPOINT_STAGING_WORK);
                assert_eq!(b.peak_storage(), peak);
                assert!(b.failed_work().is_none());
                assert!(b.failed_storage().is_none());
            }
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            2 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            3 => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            _ => unreachable!(),
        }
        assert!(charge.additional_storage() > 0);
        assert!(rustix::fs::fstat(&f.file).is_ok());
        assert!(rustix::fs::fstat(&f.cwd).is_ok());
    }
}

#[test]
fn actual_stage_constructors_and_mapping_gate_select_mandatory_confinement() {
    let f = Fixture::new();
    let (_receiver, transfer) = transfer_pair(rustix::net::SocketType::SEQPACKET);
    let mapping = crate::native_spawn::namespace_spawn::MappingGate::new().unwrap();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SOURCE).unwrap();
    let ledger = b.work_ledger_identity_v1();
    // A destination that resembles a compiler channel is not role authority.
    for destination in [3, 195] {
        // SAFETY: inert files remain owned and fully funded; no child is created.
        let (service, charge) = unsafe {
            Stage::stage(
                &f.file,
                &[Binding::new(f.file.as_fd(), destination).unwrap()],
                f.writer.as_fd(),
                f.reader.as_fd(),
                f.writer.as_fd(),
                SOURCE,
                &mut b,
            )
        }
        .unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(!service.inner.requires_namespace_confinement(None));
        assert!(
            service
                .inner
                .requires_namespace_confinement(Some(mapping.child_ends()))
        );
        assert_eq!(
            service.spawn_work(63).unwrap(),
            Stage::spawn_work_for(1, 63).unwrap()
        );
        drop(service);
        b.release_storage(charge.additional_storage()).unwrap();
    }
    let (compiler, compiler_charge) = f.stage(f.streams(), &[], &f.cwd, &mut b).unwrap();
    b.reserve_storage(compiler_charge.additional_storage())
        .unwrap();
    let (channel, channel_charge) = f
        .stage_channel([None; 3], &[], transfer.as_fd(), &mut b)
        .unwrap();
    b.reserve_storage(channel_charge.additional_storage())
        .unwrap();
    for stage in [&compiler, &channel] {
        assert!(stage.inner.requires_namespace_confinement(None));
        assert!(
            stage
                .inner
                .requires_namespace_confinement(Some(mapping.child_ends()))
        );
    }
    drop(channel);
    drop(compiler);
    b.release_storage(channel_charge.additional_storage())
        .unwrap();
    b.release_storage(compiler_charge.additional_storage())
        .unwrap();
    assert_eq!(b.storage(), SOURCE);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert!(b.failed_work().is_none());
    assert!(b.failed_storage().is_none());
    // This only selects the private branch. Real mapping/exec is tested by the
    // ignored native control, not manufactured by this pipe or inert image.
}

#[test]
fn channel_staging_pins_exact_high_cloexec_transfer_and_charges_full_quota() {
    let f = Fixture::new();
    let (receiver, transfer) = transfer_pair(rustix::net::SocketType::SEQPACKET);
    let mut work = Work::new(Stage::COMPILER_CHILD_CHANNEL_STAGING_WORK);
    let mut b = Budget::new(
        &mut work,
        SOURCE + Stage::compiler_staging_scratch_for_sources(SOURCE).unwrap(),
    );
    b.reserve_storage(SOURCE).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (stage, charge) = f
        .stage_channel(f.streams(), &[], transfer.as_fd(), &mut b)
        .unwrap();
    assert_eq!(b.work(), Stage::COMPILER_CHILD_CHANNEL_STAGING_WORK);
    assert_eq!(b.storage(), SOURCE);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(
        b.peak_storage(),
        SOURCE + Stage::compiler_staging_scratch_for_sources(SOURCE).unwrap()
    );
    assert_eq!(charge.additional_storage(), stage.retained_storage());
    let actual = stage.compiler_child_channel_transfer().unwrap();
    assert!(actual.as_raw_fd() >= FLOOR);
    assert!(
        rustix::io::fcntl_getfd(actual)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_eq!(
        rustix::fs::fstat(actual).unwrap().st_ino,
        rustix::fs::fstat(&transfer).unwrap().st_ino
    );
    assert!(
        stage
            .binding(compiler_child_channel::COMPILER_SERVICE_FD)
            .is_none()
    );
    assert_eq!(
        stage.spawn_work(63).unwrap(),
        Stage::spawn_work_for(3, 63).unwrap()
            + native_work::COMPILER_CWD_WORK
            + native_work::COMPILER_RESTRICTION_WORK
            + native_work::COMPILER_CHANNEL_WORK
    );
    assert_eq!(
        stage.spawn_retaining_work::<()>(63, 0).unwrap(),
        Stage::SPAWN_WORK
            + native_work::child_work(3, 63).unwrap()
            + native_work::COMPILER_CWD_WORK
            + native_work::COMPILER_RESTRICTION_WORK
            + native_work::COMPILER_CHANNEL_WORK
            + crate::ProtectedServiceCleanupServiceV2::retained_launch_work::<()>(0).unwrap()
    );
    drop(transfer);
    rustix::net::send(actual, b"pinned", rustix::net::SendFlags::NOSIGNAL).unwrap();
    let mut bytes = [0; 6];
    assert_eq!(rustix::io::read(&receiver, &mut bytes).unwrap(), 6);
    assert_eq!(&bytes, b"pinned");
}

#[test]
fn channel_stage_refuses_underfunding_before_transferring_or_retiring_sources() {
    let f = Fixture::new();
    let (_receiver, transfer) = transfer_pair(rustix::net::SocketType::SEQPACKET);
    let peak = SOURCE + Stage::compiler_staging_scratch_for_sources(SOURCE).unwrap();
    let complete = Stage::COMPILER_CHILD_CHANNEL_STAGING_WORK;
    for (work_limit, storage_limit, reserved, category) in [
        (ENTRY - 1, peak, SOURCE, 0),
        (complete - 1, peak, SOURCE, 0),
        (complete, peak, SOURCE - 1, 1),
        (complete, peak - 1, SOURCE, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(reserved).unwrap();
        let error = f
            .stage_channel(f.streams(), &[], transfer.as_fd(), &mut b)
            .unwrap_err();
        assert_eq!(b.storage(), reserved);
        match category {
            0 => assert!(matches!(error, Error::Resource(Resource::Work(_)))),
            1 => assert!(matches!(error, Error::Resource(Resource::Accounting))),
            _ => assert!(matches!(error, Error::Resource(Resource::Storage(_)))),
        }
    }
}

#[test]
fn channel_slot_collisions_and_total_descriptor_limit_refuse_during_staging() {
    let f = Fixture::new();
    let (_receiver, transfer) = transfer_pair(rustix::net::SocketType::SEQPACKET);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SOURCE).unwrap();
    let collision = [Binding::new(f.file.as_fd(), 195).unwrap()];
    assert!(matches!(
        f.stage_channel(f.streams(), &collision, transfer.as_fd(), &mut b),
        Err(Error::State(
            "invalid or duplicate native compiler destination"
        ))
    ));
    let (old, _) = f.stage(f.streams(), &collision, &f.cwd, &mut b).unwrap();
    assert!(old.binding(195).is_some());
    assert!(old.compiler_child_channel_transfer().is_none());
    for (count, allowed) in [(29, true), (30, false)] {
        let bindings: Vec<_> = (3..count + 3)
            .map(|fd| Binding::new(f.file.as_fd(), fd).unwrap())
            .collect();
        assert_eq!(
            f.stage_channel(f.streams(), &bindings, transfer.as_fd(), &mut b)
                .is_ok(),
            allowed
        );
    }
    // With closed stdio and no normal bindings the one generated destination
    // still satisfies the common child's nonempty bounded table requirement.
    let (stage, _) = f
        .stage_channel([None; 3], &[], transfer.as_fd(), &mut b)
        .unwrap();
    assert_eq!(
        stage.spawn_work(63).unwrap(),
        Stage::spawn_work_for(1, 63).unwrap()
            + native_work::COMPILER_CWD_WORK
            + native_work::COMPILER_RESTRICTION_WORK
            + native_work::COMPILER_CHANNEL_WORK
    );
    assert_eq!(b.storage(), SOURCE);
}

#[test]
fn channel_stage_refuses_non_socket_wrong_type_and_unconnected_transfer() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SOURCE).unwrap();
    assert!(
        f.stage_channel(f.streams(), &[], f.file.as_fd(), &mut b)
            .is_err()
    );
    for kind in [
        rustix::net::SocketType::STREAM,
        rustix::net::SocketType::DGRAM,
    ] {
        let (_receiver, transfer) = transfer_pair(kind);
        assert!(matches!(
            f.stage_channel(f.streams(), &[], transfer.as_fd(), &mut b),
            Err(Error::State("compiler channel transfer is not seqpacket"))
        ));
    }
    let unconnected = rustix::net::socket_with(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        rustix::net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    assert!(
        f.stage_channel(f.streams(), &[], unconnected.as_fd(), &mut b)
            .is_err()
    );
    assert_eq!(b.storage(), SOURCE);
}

#[test]
fn compiler_stage_retains_exact_cwd_streams_strings_and_additional_child_work() {
    let fixture = Fixture::new();
    let bindings = [Binding::new(fixture.file.as_fd(), 198).unwrap()];
    let mut work = Work::new(Stage::COMPILER_STAGING_WORK);
    let mut b = Budget::new(
        &mut work,
        SOURCE + Stage::compiler_staging_scratch_for_sources(SOURCE).unwrap(),
    );
    b.reserve_storage(SOURCE).unwrap();
    let (stage, charge) = fixture
        .stage(fixture.streams(), &bindings, &fixture.cwd, &mut b)
        .unwrap();
    assert_eq!(b.storage(), SOURCE);
    assert_eq!(b.work(), Stage::COMPILER_STAGING_WORK);
    assert_eq!(charge.additional_storage(), stage.retained_storage());
    assert!(stage.retained_storage() > Stage::storage_for_sources(SOURCE).unwrap());
    assert_eq!(
        stage.compiler_arguments(),
        Some(fixture.arguments.as_slice())
    );
    assert_eq!(
        stage.compiler_environment(),
        Some(fixture.environment.as_slice())
    );
    assert!(stage.binding(2).is_none());
    for (actual, original) in [
        (stage.binding(0).unwrap(), &fixture.reader),
        (stage.binding(1).unwrap(), &fixture.writer),
        (stage.binding(198).unwrap(), &fixture.file),
        (stage.compiler_working_directory().unwrap(), &fixture.cwd),
    ] {
        assert!(actual.as_raw_fd() >= FLOOR);
        assert!(
            rustix::io::fcntl_getfd(actual)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let observed = rustix::fs::fstat(actual).unwrap();
        let expected = rustix::fs::fstat(original).unwrap();
        assert_eq!(
            (observed.st_dev, observed.st_ino, observed.st_mode),
            (expected.st_dev, expected.st_ino, expected.st_mode)
        );
        assert_eq!(
            rustix::fs::fcntl_getfl(actual).unwrap(),
            rustix::fs::fcntl_getfl(original).unwrap()
        );
    }
    assert_eq!(
        stage.spawn_work(63).unwrap(),
        Stage::spawn_work_for(3, 63).unwrap()
            + native_work::COMPILER_CWD_WORK
            + native_work::COMPILER_RESTRICTION_WORK
    );
    let expected_cwd = rustix::fs::fstat(&fixture.cwd).unwrap().st_ino;
    assert!(fixture.directory.path().is_dir());
    drop(fixture);
    assert_eq!(
        stage.compiler_arguments().unwrap()[0].as_bytes(),
        b"captured-rustc"
    );
    assert_eq!(
        rustix::fs::fstat(stage.compiler_working_directory().unwrap())
            .unwrap()
            .st_ino,
        expected_cwd
    );
}

#[test]
fn compiler_stage_refuses_one_short_floor_work_and_peak_without_releasing_inputs() {
    let f = Fixture::new();
    let peak = SOURCE + Stage::compiler_staging_scratch_for_sources(SOURCE).unwrap();
    for (work_limit, storage_limit, reserved, category) in [
        (ENTRY - 1, peak, SOURCE, 0),
        (Stage::COMPILER_STAGING_WORK - 1, peak, SOURCE, 0),
        (Stage::COMPILER_STAGING_WORK, peak, SOURCE - 1, 1),
        (Stage::COMPILER_STAGING_WORK, peak - 1, SOURCE, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(reserved).unwrap();
        let error = f.stage(f.streams(), &[], &f.cwd, &mut b).unwrap_err();
        assert_eq!(b.storage(), reserved);
        match category {
            0 => assert!(matches!(error, Error::Resource(Resource::Work(_)))),
            1 => assert!(matches!(error, Error::Resource(Resource::Accounting))),
            _ => assert!(matches!(error, Error::Resource(Resource::Storage(_)))),
        }
    }
    assert!(Stage::compiler_staging_scratch_for_sources(usize::MAX).is_err());
}

#[test]
fn compiler_descriptor_limit_includes_present_streams_and_rejects_duplicates() {
    let f = Fixture::new();
    for (count, duplicate, allowed) in [(30, false, true), (31, false, false), (2, true, false)] {
        let bindings: Vec<_> = (0..count)
            .map(|index| {
                Binding::new(f.file.as_fd(), if duplicate { 3 } else { index + 3 }).unwrap()
            })
            .collect();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(SOURCE).unwrap();
        assert_eq!(
            f.stage(f.streams(), &bindings, &f.cwd, &mut b).is_ok(),
            allowed
        );
        assert_eq!(b.storage(), SOURCE);
    }
    assert!(Binding::new(f.file.as_fd(), 0).is_err());
}

#[test]
fn non_directory_cwd_and_empty_descriptor_table_refuse() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SOURCE).unwrap();
    assert!(matches!(
        f.stage(f.streams(), &[], &f.file, &mut b),
        Err(Error::State("native compiler cwd is not a directory"))
    ));
    assert!(matches!(
        f.stage([None; 3], &[], &f.cwd, &mut b),
        Err(Error::State("invalid native compiler descriptor count"))
    ));
    assert_eq!(b.storage(), SOURCE);
}

#[test]
fn service_staging_keeps_fixed_abi_and_cannot_install_standard_streams() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SOURCE).unwrap();
    let bindings = [Binding::new(f.file.as_fd(), 3).unwrap()];
    let (stage, _) = unsafe {
        Stage::stage(
            &f.file,
            &bindings,
            f.writer.as_fd(),
            f.reader.as_fd(),
            f.writer.as_fd(),
            SOURCE,
            &mut b,
        )
    }
    .unwrap();
    assert!(stage.compiler_arguments().is_none());
    assert!(stage.compiler_environment().is_none());
    assert!(stage.compiler_working_directory().is_none());
    assert_eq!(
        stage.spawn_work(63).unwrap(),
        Stage::spawn_work_for(1, 63).unwrap()
    );
}
