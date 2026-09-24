use super::*;

use rustix::fs::{MemfdFlags, SeekFrom, fcntl_getfl, fstat, memfd_create, seek};
use rustix::io::{FdFlags, fcntl_dupfd_cloexec, fcntl_getfd, read};
use rustix::pipe::{PipeFlags, pipe_with};

struct Fixture {
    launcher: File,
    issuer: File,
    manifest: File,
    sources: [File; SOURCE_COUNT_V1],
    profile_ready: (OwnedFd, OwnedFd),
    gate: (OwnedFd, OwnedFd),
    exec_status: (OwnedFd, OwnedFd),
}

impl Fixture {
    fn new() -> Self {
        Self {
            launcher: anonymous_file(),
            issuer: anonymous_file(),
            manifest: anonymous_file(),
            sources: std::array::from_fn(|_| anonymous_file()),
            profile_ready: pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap(),
            gate: pipe_with(PipeFlags::CLOEXEC).unwrap(),
            exec_status: pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap(),
        }
    }

    fn input(&self) -> StagedLaunchInputV1<'_> {
        StagedLaunchInputV1 {
            launcher: &self.launcher,
            issuer: &self.issuer,
            manifest: &self.manifest,
            sources: &self.sources,
        }
    }

    fn stage(&self) -> StagedLaunchV1 {
        StagedLaunchV1::new(
            self.input(),
            &self.profile_ready.1,
            &self.gate.0,
            &self.exec_status.1,
        )
        .unwrap()
    }

    fn assert_originals_live(&self) {
        for descriptor in [&self.launcher, &self.issuer, &self.manifest]
            .into_iter()
            .chain(&self.sources)
        {
            fstat(descriptor).unwrap();
            assert_eq!(fcntl_getfd(descriptor).unwrap(), FdFlags::CLOEXEC);
        }
        for descriptor in [
            &self.profile_ready.0,
            &self.profile_ready.1,
            &self.gate.0,
            &self.gate.1,
            &self.exec_status.0,
            &self.exec_status.1,
        ] {
            fstat(descriptor).unwrap();
            assert_eq!(fcntl_getfd(descriptor).unwrap(), FdFlags::CLOEXEC);
        }
    }
}

fn anonymous_file() -> File {
    memfd_create(c"fe2o3-process-staging-test", MemfdFlags::CLOEXEC)
        .unwrap()
        .into()
}

fn assert_duplicate(source: &impl AsFd, staged: &OwnedFd) {
    let original = fstat(source).unwrap();
    let duplicate = fstat(staged).unwrap();
    assert_eq!(
        (duplicate.st_dev, duplicate.st_ino, duplicate.st_mode),
        (original.st_dev, original.st_ino, original.st_mode),
    );
    assert_ne!(source.as_fd().as_raw_fd(), staged.as_raw_fd());
    assert_eq!(fcntl_getfd(staged).unwrap(), FdFlags::CLOEXEC);
    assert_eq!(fcntl_getfl(staged).unwrap(), fcntl_getfl(source).unwrap());
}

fn assert_shared_offset(source: &File, staged: &OwnedFd, offset: u64) {
    seek(source, SeekFrom::Start(offset)).unwrap();
    assert_eq!(seek(staged, SeekFrom::Current(0)).unwrap(), offset);
    seek(staged, SeekFrom::Start(offset + 1)).unwrap();
    assert_eq!(seek(source, SeekFrom::Current(0)).unwrap(), offset + 1);
}

#[test]
fn fixed_targets_stdio_and_duplicates_preserve_the_child_contract() {
    let mut fixture = Fixture::new();
    let occupied = fcntl_dupfd_cloexec(&fixture.launcher, STAGED_DESCRIPTOR_FLOOR).unwrap();
    fixture.sources[7] = fcntl_dupfd_cloexec(&fixture.sources[7], occupied.as_raw_fd() + 4)
        .unwrap()
        .into();
    let staged = fixture.stage();

    assert_eq!(staged.descriptors.len(), DESCRIPTOR_COUNT);
    assert_eq!(staged.descriptors[0].target, 198);
    assert_eq!(staged.descriptors[1].target, 199);
    assert_duplicate(&fixture.manifest, &staged.descriptors[0].source);
    assert_duplicate(&fixture.issuer, &staged.descriptors[1].source);
    for (index, (source, descriptor)) in fixture
        .sources
        .iter()
        .zip(&staged.descriptors[2..])
        .enumerate()
    {
        assert_eq!(descriptor.target, 200 + index as i32);
        assert_duplicate(source, &descriptor.source);
        assert_shared_offset(source, &descriptor.source, index as u64 + 10);
    }
    assert_eq!(
        staged.stdio_sources,
        [
            staged.descriptors[2].source.as_raw_fd(),
            staged.descriptors[3].source.as_raw_fd(),
            staged.descriptors[4].source.as_raw_fd(),
        ],
    );
    assert_duplicate(&fixture.launcher, &staged.launcher);
    assert_duplicate(&fixture.profile_ready.1, &staged.profile_ready_writer);
    assert_duplicate(&fixture.gate.0, &staged.gate_reader);
    assert_duplicate(&fixture.exec_status.1, &staged.exec_status_writer);
    assert_shared_offset(&fixture.launcher, &staged.launcher, 1);
    assert_shared_offset(&fixture.manifest, &staged.descriptors[0].source, 2);
    assert_shared_offset(&fixture.issuer, &staged.descriptors[1].source, 3);

    let mut previous = 215;
    for descriptor in std::iter::once(&staged.launcher)
        .chain(staged.descriptors.iter().map(|entry| &entry.source))
        .chain([
            &staged.profile_ready_writer,
            &staged.gate_reader,
            &staged.exec_status_writer,
        ])
    {
        let raw = descriptor.as_raw_fd();
        assert!(raw >= 216);
        assert!(raw > previous, "staging must be strictly increasing");
        assert_ne!(raw, occupied.as_raw_fd());
        assert_ne!(raw, fixture.sources[7].as_raw_fd());
        assert!(staged.descriptors.iter().all(|entry| raw != entry.target));
        previous = raw;
    }
    assert_duplicate(&fixture.launcher, &occupied);
    drop(staged);
    fixture.assert_originals_live();
}

const DUPLICATION_OPERATIONS: [&str; 18] = [
    "stage static launcher",
    "stage static launch manifest",
    "stage issuer executable",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage issuer source descriptor",
    "stage child-profile writer",
    "stage launch-gate reader",
    "stage exec-status writer",
];

fn cleanup_probes() -> [(OwnedFd, OwnedFd); 18] {
    std::array::from_fn(|_| pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap())
}

fn assert_probes_closed(probes: [(OwnedFd, OwnedFd); 18]) {
    // EOF proves every staged writer closed; unrelated descriptor reuse cannot affect it.
    for (index, (reader, writer)) in probes.into_iter().enumerate() {
        drop(writer);
        assert_eq!(
            read(&reader, &mut [0_u8]),
            Ok(0),
            "staged duplicate {index} leaked",
        );
    }
}

#[test]
fn every_duplication_failure_closes_partial_staging_and_preserves_borrowed_inputs() {
    let fixture = Fixture::new();
    for (fail_at, operation) in DUPLICATION_OPERATIONS.into_iter().enumerate() {
        let probes = cleanup_probes();
        let mut calls = 0;
        let result = StagedLaunchV1::new_with_duplicate(
            fixture.input(),
            &fixture.profile_ready.1,
            &fixture.gate.0,
            &fixture.exec_status.1,
            |_, floor| {
                let index = calls;
                calls += 1;
                if index == fail_at {
                    return Err(Errno::MFILE);
                }
                fcntl_dupfd_cloexec(&probes[index].1, floor)
            },
        );
        assert_eq!(
            result.err(),
            Some(StagedLaunchErrorV1::Io {
                operation,
                source: Errno::MFILE,
            }),
        );
        assert_eq!(
            calls,
            fail_at + 1,
            "duplication must stop at the first error"
        );
        assert_probes_closed(probes);
        fixture.assert_originals_live();
    }
}

#[test]
fn dropping_completed_staging_closes_all_eighteen_duplicates() {
    let fixture = Fixture::new();
    let probes = cleanup_probes();
    let mut calls = 0;
    let staged = StagedLaunchV1::new_with_duplicate(
        fixture.input(),
        &fixture.profile_ready.1,
        &fixture.gate.0,
        &fixture.exec_status.1,
        |_, floor| {
            let index = calls;
            calls += 1;
            fcntl_dupfd_cloexec(&probes[index].1, floor)
        },
    )
    .unwrap();
    assert_eq!(calls, 18);
    for (reader, _) in &probes {
        assert_eq!(read(reader, &mut [0_u8]), Err(Errno::AGAIN));
    }
    drop(staged);
    assert_probes_closed(probes);
    fixture.assert_originals_live();
}
