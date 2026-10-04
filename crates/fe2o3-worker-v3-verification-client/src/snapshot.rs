use std::fs::File;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::MetadataExt;

use fe2o3_worker_v3_verification_protocol::{
    WORKER_V3_VERIFICATION_FD_PAYLOADS_V1, WorkerV3VerificationFdPayloadDescriptorV1,
    WorkerV3VerificationFdPayloadKindV1, WorkerV3VerificationRequestIdentityV1,
    WorkerV3VerificationRequestV1,
};
use rustix::fs::SealFlags;
use sha2::{Digest, Sha256};

use crate::WorkerV3VerificationClientErrorV1;

const COPY_CHUNK_BYTES: usize = 64 * 1024;
const TMPFS_MAGIC: u64 = 0x0102_1994;
const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);
const REQUIRED_SEALS_WITH_FUTURE_WRITE: SealFlags = REQUIRED_SEALS.union(SealFlags::FUTURE_WRITE);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PayloadIdentityV1 {
    device: u64,
    inode: u64,
    mode: u32,
    byte_len: u64,
    owner_uid: u32,
    owner_gid: u32,
    link_count: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

/// Move-only custody of the exact two immutable request payload snapshots.
///
/// Admission consumes every supplied descriptor. Success retains exactly two sealed memfds in
/// protocol order and binds them to one request identity. This value conveys byte custody only;
/// it grants no theorem, load, launch, or peer authority.
pub struct WorkerV3VerificationPayloadSnapshotsV1 {
    request_identity: WorkerV3VerificationRequestIdentityV1,
    files: [File; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1],
    identities: [PayloadIdentityV1; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1],
}

impl std::fmt::Debug for WorkerV3VerificationPayloadSnapshotsV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationPayloadSnapshotsV1")
            .field("request_identity", &self.request_identity)
            .field("descriptor_count", &self.files.len())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl WorkerV3VerificationPayloadSnapshotsV1 {
    /// Consumes and admits exactly two immutable memfds in canonical protocol order.
    pub fn admit(
        request: &WorkerV3VerificationRequestV1,
        descriptors: Vec<OwnedFd>,
    ) -> Result<Self, WorkerV3VerificationClientErrorV1> {
        if descriptors.len() != WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 {
            return Err(WorkerV3VerificationClientErrorV1::DescriptorCount {
                expected: WORKER_V3_VERIFICATION_FD_PAYLOADS_V1,
                actual: descriptors.len(),
            });
        }
        let sources: [File; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1] = descriptors
            .into_iter()
            .map(File::from)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(
                |files: Vec<File>| WorkerV3VerificationClientErrorV1::DescriptorCount {
                    expected: WORKER_V3_VERIFICATION_FD_PAYLOADS_V1,
                    actual: files.len(),
                },
            )?;
        let source_first = validate_payload(&sources[0], &request.payloads()[0])?;
        let source_second = validate_payload(&sources[1], &request.payloads()[1])?;
        if (source_first.device, source_first.inode) == (source_second.device, source_second.inode)
        {
            return Err(WorkerV3VerificationClientErrorV1::DuplicatePayloadInode);
        }
        let files = [
            reopen_read_only(&sources[0], request.payloads()[0].kind())?,
            reopen_read_only(&sources[1], request.payloads()[1].kind())?,
        ];
        let first = validate_retained_payload(&files[0], &request.payloads()[0])?;
        let second = validate_retained_payload(&files[1], &request.payloads()[1])?;
        if first != source_first || second != source_second {
            return Err(WorkerV3VerificationClientErrorV1::PayloadChanged {
                kind: if first != source_first {
                    request.payloads()[0].kind()
                } else {
                    request.payloads()[1].kind()
                },
            });
        }
        drop(sources);
        Ok(Self {
            request_identity: request.identity(),
            files,
            identities: [first, second],
        })
    }

    /// Reports that immutable byte custody alone grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub(crate) fn borrowed_fds(&self) -> [BorrowedFd<'_>; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1] {
        [self.files[0].as_fd(), self.files[1].as_fd()]
    }

    pub(crate) fn revalidate(
        &self,
        request: &WorkerV3VerificationRequestV1,
    ) -> Result<(), WorkerV3VerificationClientErrorV1> {
        if self.request_identity != request.identity() {
            return Err(WorkerV3VerificationClientErrorV1::SnapshotRequestMismatch);
        }
        for index in 0..WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 {
            let identity =
                validate_retained_payload(&self.files[index], &request.payloads()[index])?;
            if identity != self.identities[index] {
                return Err(WorkerV3VerificationClientErrorV1::PayloadChanged {
                    kind: request.payloads()[index].kind(),
                });
            }
        }
        Ok(())
    }
}

fn validate_payload(
    file: &File,
    descriptor: &WorkerV3VerificationFdPayloadDescriptorV1,
) -> Result<PayloadIdentityV1, WorkerV3VerificationClientErrorV1> {
    let kind = descriptor.kind();
    set_close_on_exec(file, kind)?;
    let before = capture_identity(file, kind)?;
    if before.byte_len < descriptor.byte_len() {
        return Err(WorkerV3VerificationClientErrorV1::PayloadLengthMismatch {
            kind,
            expected: descriptor.byte_len(),
            actual: before.byte_len,
        });
    }
    if before.byte_len > descriptor.byte_len() {
        return Err(WorkerV3VerificationClientErrorV1::TrailingPayloadBytes {
            kind,
            declared: descriptor.byte_len(),
            actual: before.byte_len,
        });
    }

    let mut digest = Sha256::new();
    let mut buffer = [0_u8; COPY_CHUNK_BYTES];
    let mut offset = 0_u64;
    while offset < descriptor.byte_len() {
        let remaining = descriptor.byte_len() - offset;
        let limit = usize::try_from(remaining.min(COPY_CHUNK_BYTES as u64))
            .expect("copy chunk bound fits usize");
        let count = rustix::io::pread(file, &mut buffer[..limit], offset)
            .map_err(|source| descriptor_error("pread payload", source.into()))?;
        if count == 0 {
            return Err(WorkerV3VerificationClientErrorV1::PayloadLengthMismatch {
                kind,
                expected: descriptor.byte_len(),
                actual: offset,
            });
        }
        digest.update(&buffer[..count]);
        offset = offset.checked_add(count as u64).ok_or(
            WorkerV3VerificationClientErrorV1::PayloadLengthMismatch {
                kind,
                expected: descriptor.byte_len(),
                actual: u64::MAX,
            },
        )?;
    }
    let mut trailing = [0_u8; 1];
    if rustix::io::pread(file, &mut trailing, descriptor.byte_len())
        .map_err(|source| descriptor_error("bound payload", source.into()))?
        != 0
    {
        return Err(WorkerV3VerificationClientErrorV1::TrailingPayloadBytes {
            kind,
            declared: descriptor.byte_len(),
            actual: descriptor.byte_len().saturating_add(1),
        });
    }
    let actual_digest: [u8; 32] = digest.finalize().into();
    if &actual_digest != descriptor.sha256() {
        return Err(WorkerV3VerificationClientErrorV1::PayloadDigestMismatch { kind });
    }
    let after = capture_identity(file, kind)?;
    if before != after {
        return Err(WorkerV3VerificationClientErrorV1::PayloadChanged { kind });
    }
    Ok(after)
}

fn validate_retained_payload(
    file: &File,
    descriptor: &WorkerV3VerificationFdPayloadDescriptorV1,
) -> Result<PayloadIdentityV1, WorkerV3VerificationClientErrorV1> {
    require_read_only(file, descriptor.kind())?;
    validate_payload(file, descriptor)
}

fn capture_identity(
    file: &File,
    kind: WorkerV3VerificationFdPayloadKindV1,
) -> Result<PayloadIdentityV1, WorkerV3VerificationClientErrorV1> {
    let metadata = file
        .metadata()
        .map_err(|source| descriptor_error("fstat payload", source))?;
    if !metadata.file_type().is_file() {
        return Err(WorkerV3VerificationClientErrorV1::PayloadNotRegular { kind });
    }
    if metadata.nlink() != 0 {
        return Err(WorkerV3VerificationClientErrorV1::PayloadLinked {
            kind,
            actual: metadata.nlink(),
        });
    }
    let expected_uid = rustix::process::geteuid().as_raw();
    if metadata.uid() != expected_uid {
        return Err(WorkerV3VerificationClientErrorV1::PayloadOwnerMismatch {
            kind,
            expected: expected_uid,
            actual: metadata.uid(),
        });
    }
    let expected_gid = rustix::process::getegid().as_raw();
    if metadata.gid() != expected_gid {
        return Err(WorkerV3VerificationClientErrorV1::PayloadGroupMismatch {
            kind,
            expected: expected_gid,
            actual: metadata.gid(),
        });
    }
    let filesystem = rustix::fs::fstatfs(file)
        .map_err(|source| descriptor_error("fstatfs payload", source.into()))?;
    if filesystem.f_type as u64 != TMPFS_MAGIC {
        return Err(WorkerV3VerificationClientErrorV1::PayloadNotMemfd { kind });
    }
    let seals = rustix::fs::fcntl_get_seals(file)
        .map_err(|source| descriptor_error("inspect payload seals", source.into()))?;
    if seals != REQUIRED_SEALS && seals != REQUIRED_SEALS_WITH_FUTURE_WRITE {
        return Err(WorkerV3VerificationClientErrorV1::PayloadNotImmutable {
            kind,
            actual_seal_bits: seals.bits(),
        });
    }
    Ok(PayloadIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        byte_len: metadata.len(),
        owner_uid: metadata.uid(),
        owner_gid: metadata.gid(),
        link_count: metadata.nlink(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}

fn set_close_on_exec(
    file: &File,
    kind: WorkerV3VerificationFdPayloadKindV1,
) -> Result<(), WorkerV3VerificationClientErrorV1> {
    rustix::io::fcntl_setfd(file, rustix::io::FdFlags::CLOEXEC)
        .map_err(|source| descriptor_error("set payload close-on-exec", source.into()))?;
    let actual = rustix::io::fcntl_getfd(file)
        .map_err(|source| descriptor_error("inspect payload descriptor flags", source.into()))?;
    if actual != rustix::io::FdFlags::CLOEXEC {
        return Err(WorkerV3VerificationClientErrorV1::PayloadDescriptorFlags {
            kind,
            actual_bits: actual.bits(),
        });
    }
    Ok(())
}

fn reopen_read_only(
    source: &File,
    kind: WorkerV3VerificationFdPayloadKindV1,
) -> Result<File, WorkerV3VerificationClientErrorV1> {
    let path = format!("/proc/self/fd/{}", source.as_raw_fd());
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|source| descriptor_error("reopen payload read-only", source.into()))?;
    let file = File::from(descriptor);
    require_read_only(&file, kind)?;
    Ok(file)
}

fn require_read_only(
    file: &File,
    kind: WorkerV3VerificationFdPayloadKindV1,
) -> Result<(), WorkerV3VerificationClientErrorV1> {
    let status = rustix::fs::fcntl_getfl(file)
        .map_err(|source| descriptor_error("inspect read-only payload status", source.into()))?;
    if status & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
        || status.intersects(
            rustix::fs::OFlags::APPEND
                | rustix::fs::OFlags::ASYNC
                | rustix::fs::OFlags::DIRECT
                | rustix::fs::OFlags::PATH,
        )
    {
        return Err(WorkerV3VerificationClientErrorV1::PayloadNotReadOnly {
            kind,
            actual_status_bits: status.bits(),
        });
    }
    Ok(())
}

fn descriptor_error(
    operation: &'static str,
    source: std::io::Error,
) -> WorkerV3VerificationClientErrorV1 {
    WorkerV3VerificationClientErrorV1::Descriptor { operation, source }
}
