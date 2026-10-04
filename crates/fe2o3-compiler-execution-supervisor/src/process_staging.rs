//! Fixed-capacity descriptor staging shared by native and legacy process launch.

use std::fs::File;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};

use fe2o3_static_preexec_manifest::{
    PREEXEC_EXECUTABLE_FD, PREEXEC_MANIFEST_FD, PREEXEC_MAX_DESCRIPTORS, PREEXEC_SOURCE_FD_BASE,
};
use rustix::io::Errno;

use crate::launch_checks::SOURCE_COUNT_V1;

const DESCRIPTOR_COUNT: usize = SOURCE_COUNT_V1 + 2;
const STAGED_DESCRIPTOR_FLOOR: i32 = PREEXEC_SOURCE_FD_BASE + PREEXEC_MAX_DESCRIPTORS as i32;

const _: () = assert!(DESCRIPTOR_COUNT == 14);
const _: () = assert!(PREEXEC_MANIFEST_FD == 198);
const _: () = assert!(PREEXEC_EXECUTABLE_FD == 199);
const _: () = assert!(PREEXEC_SOURCE_FD_BASE == 200);
const _: () = assert!(STAGED_DESCRIPTOR_FLOOR == 216);

/// Borrowed descriptor custody only; callers retain admission and lifecycle policy.
pub(super) struct StagedLaunchInputV1<'a> {
    pub(super) launcher: &'a File,
    pub(super) issuer: &'a File,
    pub(super) manifest: &'a File,
    pub(super) sources: &'a [File; SOURCE_COUNT_V1],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StagedLaunchErrorV1 {
    InvalidProcessState(&'static str),
    Io {
        operation: &'static str,
        source: Errno,
    },
}

pub(super) struct StagedDescriptorV1 {
    pub(super) source: OwnedFd,
    pub(super) target: i32,
}

pub(super) struct StagedLaunchV1 {
    pub(super) launcher: OwnedFd,
    pub(super) descriptors: [StagedDescriptorV1; DESCRIPTOR_COUNT],
    pub(super) stdio_sources: [i32; 3],
    pub(super) profile_ready_writer: OwnedFd,
    pub(super) gate_reader: OwnedFd,
    pub(super) exec_status_writer: OwnedFd,
}

impl StagedLaunchV1 {
    pub(super) fn new(
        input: StagedLaunchInputV1<'_>,
        profile_ready_writer: &OwnedFd,
        gate_reader: &OwnedFd,
        exec_status_writer: &OwnedFd,
    ) -> Result<Self, StagedLaunchErrorV1> {
        Self::new_with_duplicate(
            input,
            profile_ready_writer,
            gate_reader,
            exec_status_writer,
            |source, floor| rustix::io::fcntl_dupfd_cloexec(source, floor),
        )
    }

    fn new_with_duplicate(
        input: StagedLaunchInputV1<'_>,
        profile_ready_writer: &OwnedFd,
        gate_reader: &OwnedFd,
        exec_status_writer: &OwnedFd,
        mut duplicate: impl FnMut(BorrowedFd<'_>, i32) -> Result<OwnedFd, Errno>,
    ) -> Result<Self, StagedLaunchErrorV1> {
        let mut next = STAGED_DESCRIPTOR_FLOOR;
        let launcher = duplicate_above(
            input.launcher,
            &mut next,
            "stage static launcher",
            &mut duplicate,
        )?;
        let inputs = [
            (
                input.manifest,
                PREEXEC_MANIFEST_FD,
                "stage static launch manifest",
            ),
            (
                input.issuer,
                PREEXEC_EXECUTABLE_FD,
                "stage issuer executable",
            ),
        ]
        .into_iter()
        .chain(
            input
                .sources
                .iter()
                .zip(PREEXEC_SOURCE_FD_BASE..)
                .map(|(source, target)| (source, target, "stage issuer source descriptor")),
        );

        // Each filled slot owns its duplicate even if a later syscall fails.
        let mut descriptors: [Option<StagedDescriptorV1>; DESCRIPTOR_COUNT] =
            [const { None }; DESCRIPTOR_COUNT];
        for (slot, (source, target, operation)) in descriptors.iter_mut().zip(inputs) {
            *slot = Some(StagedDescriptorV1 {
                source: duplicate_above(source, &mut next, operation, &mut duplicate)?,
                target,
            });
        }
        let [
            Some(manifest),
            Some(issuer),
            Some(source0),
            Some(source1),
            Some(source2),
            Some(source3),
            Some(source4),
            Some(source5),
            Some(source6),
            Some(source7),
            Some(source8),
            Some(source9),
            Some(source10),
            Some(source11),
        ] = descriptors
        else {
            return Err(StagedLaunchErrorV1::InvalidProcessState(
                "staged descriptor table is incomplete",
            ));
        };
        let stdio_sources = [
            source0.source.as_raw_fd(),
            source1.source.as_raw_fd(),
            source2.source.as_raw_fd(),
        ];
        let descriptors = [
            manifest, issuer, source0, source1, source2, source3, source4, source5, source6,
            source7, source8, source9, source10, source11,
        ];
        let profile_ready_writer = duplicate_above(
            profile_ready_writer,
            &mut next,
            "stage child-profile writer",
            &mut duplicate,
        )?;
        let gate_reader = duplicate_above(
            gate_reader,
            &mut next,
            "stage launch-gate reader",
            &mut duplicate,
        )?;
        let exec_status_writer = duplicate_above(
            exec_status_writer,
            &mut next,
            "stage exec-status writer",
            &mut duplicate,
        )?;
        Ok(Self {
            launcher,
            descriptors,
            stdio_sources,
            profile_ready_writer,
            gate_reader,
            exec_status_writer,
        })
    }
}

fn duplicate_above(
    source: &impl AsFd,
    next: &mut i32,
    operation: &'static str,
    duplicate: &mut impl FnMut(BorrowedFd<'_>, i32) -> Result<OwnedFd, Errno>,
) -> Result<OwnedFd, StagedLaunchErrorV1> {
    let descriptor = duplicate(source.as_fd(), *next)
        .map_err(|source| StagedLaunchErrorV1::Io { operation, source })?;
    *next =
        descriptor
            .as_raw_fd()
            .checked_add(1)
            .ok_or(StagedLaunchErrorV1::InvalidProcessState(
                "staged descriptor range overflowed",
            ))?;
    Ok(descriptor)
}

#[cfg(test)]
#[path = "process_staging_tests.rs"]
mod tests;
