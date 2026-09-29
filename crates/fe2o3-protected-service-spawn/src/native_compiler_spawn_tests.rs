//! Inert descriptor/argument mechanics, not approved compiler or deployment fixtures.
use super::*;
use crate::native_work;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::fd::{AsFd, AsRawFd};

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
        Stage::spawn_work_for(3, 63).unwrap() + native_work::COMPILER_CWD_WORK
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
