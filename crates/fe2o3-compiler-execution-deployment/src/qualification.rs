use std::fmt;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::os::unix::fs::FileExt as _;
use std::path::{Path, PathBuf};

use rustix::fs::{
    FileType, MemfdFlags, Mode, OFlags, ResolveFlags, SealFlags, fcntl_add_seals, fcntl_get_seals,
    fstat, memfd_create, openat2,
};
use sha2::{Digest as _, Sha256};

use super::install::{InstalledCompilerExecutionDeploymentV1, revalidate_installed_deployment};
use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, changed, invalid, io_error,
    lower_hex, parse_lower_hex_exact, require_no_xattrs, snapshot, std_io_error,
    validate_directory_mode, verify_directory_children,
};

const QUALIFICATION_PARENT_MODE_V1: u32 = 0o700;
const QUALIFICATION_BASE_IMAGE_MODE_V1: u32 = 0o444;
const QUALIFICATION_BASE_IMAGE_MAX_BYTES_V1: u64 = 512 * 1024 * 1024;
const COPY_BUFFER_BYTES_V1: usize = 64 * 1024;
const SQUASHFS_SUPERBLOCK_BYTES_V1: usize = 96;
const SQUASHFS_MAGIC_V1: u32 = 0x7371_7368;
const SQUASHFS_COMPRESSION_ZSTD_V1: u16 = 6;
const SQUASHFS_BLOCK_BYTES_V1: u32 = 128 * 1024;
const SQUASHFS_BLOCK_LOG_V1: u16 = 17;
const SQUASHFS_NO_XATTRS_FLAG_V1: u16 = 0x0100;
const SQUASHFS_MAJOR_V1: u16 = 4;
const SQUASHFS_MINOR_V1: u16 = 0;
const SQUASHFS_PADDING_BYTES_V1: u64 = 4096;

struct SealedQualificationBaseImageV1 {
    sha256: [u8; 32],
    byte_len: u64,
    created_epoch: u32,
    file: File,
}

/// Move-only, authority-free custody prepared for one disposable-root qualification.
///
/// The installed root has been freshly revalidated against its retained sealed deployment
/// sources. The base image is an exact caller-pinned SquashFS V4 image copied into a sealed
/// anonymous descriptor. The private qualification parent descriptor is retained for later
/// descriptor-relative staging.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_deployment::PreparedCompilerExecutionQualificationV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<PreparedCompilerExecutionQualificationV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_deployment::PreparedCompilerExecutionQualificationV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<PreparedCompilerExecutionQualificationV1>();
/// ```
pub struct PreparedCompilerExecutionQualificationV1 {
    installed: InstalledCompilerExecutionDeploymentV1,
    base: SealedQualificationBaseImageV1,
    parent: File,
    parent_path: PathBuf,
}

impl fmt::Debug for PreparedCompilerExecutionQualificationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCompilerExecutionQualificationV1")
            .field("git_commit", &self.installed.git_commit())
            .field("target", &self.installed.target())
            .field(
                "manifest_sha256",
                &lower_hex(&self.installed.manifest_sha256()),
            )
            .field("installed_root_name", &self.installed.root_name())
            .field("base_image_sha256", &lower_hex(&self.base.sha256))
            .field("base_image_byte_len", &self.base.byte_len)
            .field("base_image_created_epoch", &self.base.created_epoch)
            .field("authority", &"qualification-preparation-only")
            .finish_non_exhaustive()
    }
}

impl PreparedCompilerExecutionQualificationV1 {
    /// Returns the exact deployment commit retained by the installed root.
    pub fn git_commit(&self) -> &str {
        self.installed.git_commit()
    }

    /// Returns the exact deployment manifest digest retained by the installed root.
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.installed.manifest_sha256()
    }

    /// Returns the deterministic installed-root name.
    pub fn installed_root_name(&self) -> &str {
        self.installed.root_name()
    }

    /// Returns the independently pinned base-image digest.
    pub const fn base_image_sha256(&self) -> [u8; 32] {
        self.base.sha256
    }

    /// Returns the exact sealed base-image length.
    pub const fn base_image_byte_len(&self) -> u64 {
        self.base.byte_len
    }

    /// Returns the SquashFS creation epoch bound by the base image.
    pub const fn base_image_created_epoch(&self) -> u32 {
        self.base.created_epoch
    }

    /// Revalidates every retained object under the production root-owned policy.
    pub fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
        revalidate_prepared_qualification(self, (0, 0))
    }
}

/// Prepares one exact installed deployment and pinned base image for disposable qualification.
///
/// The process must have effective UID 0. `qualification_parent` must be root-owned,
/// root-group, mode `0700`, and carry no extended attributes. The base image must be a
/// root-owned, root-group, single-link, mode `0444` regular file without extended attributes.
pub fn prepare_compiler_execution_qualification_v1(
    installed: InstalledCompilerExecutionDeploymentV1,
    base_image_path: &Path,
    expected_base_image_sha256: &str,
    qualification_parent: &Path,
) -> Result<PreparedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "compiler-execution qualification preparation requires effective UID 0",
        ));
    }
    prepare_for_owner(
        installed,
        base_image_path,
        expected_base_image_sha256,
        qualification_parent,
        (0, 0),
    )
}

#[cfg(test)]
pub(super) fn prepare_compiler_execution_qualification_for_test_v1(
    installed: InstalledCompilerExecutionDeploymentV1,
    base_image_path: &Path,
    expected_base_image_sha256: &str,
    qualification_parent: &Path,
    owner: (u32, u32),
) -> Result<PreparedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    prepare_for_owner(
        installed,
        base_image_path,
        expected_base_image_sha256,
        qualification_parent,
        owner,
    )
}

#[cfg(test)]
pub(super) fn revalidate_prepared_qualification_for_test_v1(
    prepared: &PreparedCompilerExecutionQualificationV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    revalidate_prepared_qualification(prepared, owner)
}

fn prepare_for_owner(
    installed: InstalledCompilerExecutionDeploymentV1,
    base_image_path: &Path,
    expected_base_image_sha256: &str,
    qualification_parent: &Path,
    owner: (u32, u32),
) -> Result<PreparedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    if !base_image_path.is_absolute() || !qualification_parent.is_absolute() {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification base-image and parent paths must be absolute",
        ));
    }
    let expected_digest = parse_lower_hex_exact(
        expected_base_image_sha256,
        32,
        "qualification base-image SHA-256",
    )?;
    let mut expected_sha256 = [0_u8; 32];
    expected_sha256.copy_from_slice(&expected_digest);
    revalidate_installed_deployment(&installed, owner)?;
    let parent = open_qualification_parent(qualification_parent, owner)?;
    let base = admit_qualification_base_image(base_image_path, expected_sha256, owner)?;
    let prepared = PreparedCompilerExecutionQualificationV1 {
        installed,
        base,
        parent,
        parent_path: qualification_parent.to_owned(),
    };
    revalidate_prepared_qualification(&prepared, owner)?;
    Ok(prepared)
}

fn revalidate_prepared_qualification(
    prepared: &PreparedCompilerExecutionQualificationV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    revalidate_installed_deployment(&prepared.installed, owner)?;
    validate_sealed_base_image(&prepared.base)?;
    validate_directory_mode(
        &prepared.parent,
        Some(owner),
        QUALIFICATION_PARENT_MODE_V1,
        "compiler-execution qualification parent",
    )?;
    let reopened = open_qualification_parent(&prepared.parent_path, owner)?;
    let retained = snapshot(
        &fstat(&prepared.parent)
            .map_err(|source| io_error("reinspect retained qualification parent", source))?,
    );
    let reopened = snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("reinspect canonical qualification parent", source))?,
    );
    if retained.device != reopened.device
        || retained.inode != reopened.inode
        || retained.mode != reopened.mode
        || retained.uid != reopened.uid
        || retained.gid != reopened.gid
    {
        return Err(changed(
            "qualification-parent pathname changed after preparation",
        ));
    }
    Ok(())
}

fn open_qualification_parent(
    path: &Path,
    owner: (u32, u32),
) -> Result<File, DeploymentVerificationErrorV1> {
    let parent = openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error("open compiler-execution qualification parent", source))?;
    validate_directory_mode(
        &parent,
        Some(owner),
        QUALIFICATION_PARENT_MODE_V1,
        "compiler-execution qualification parent",
    )?;
    verify_directory_children(&parent, &[], "compiler-execution qualification parent")?;
    Ok(parent)
}

fn admit_qualification_base_image(
    path: &Path,
    expected_sha256: [u8; 32],
    owner: (u32, u32),
) -> Result<SealedQualificationBaseImageV1, DeploymentVerificationErrorV1> {
    let mut source = openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error("open qualification base image", source))?;
    let initial = validate_base_image_source(&source, owner)?;
    let descriptor = memfd_create(
        c"fe2o3-qualification-base-v1",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(|source| io_error("create sealed qualification base image", source))?;
    let mut sealed = File::from(descriptor);
    let (first_sha256, byte_len) = copy_and_hash_base_image(&mut source, &mut sealed)?;
    if byte_len != initial.byte_len {
        return Err(changed(
            "qualification base-image length changed during first read",
        ));
    }
    require_unchanged_base_image(&source, initial, "after first read")?;
    sealed
        .flush()
        .map_err(|source| std_io_error("flush sealed qualification base image", source))?;
    rustix::fs::fchmod(
        &sealed,
        Mode::from_raw_mode(QUALIFICATION_BASE_IMAGE_MODE_V1),
    )
    .map_err(|source| io_error("set sealed qualification base-image mode", source))?;
    fcntl_add_seals(
        &sealed,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
    )
    .map_err(|source| io_error("seal qualification base image", source))?;
    compare_source_with_sealed(&mut source, &sealed, byte_len)?;
    require_unchanged_base_image(&source, initial, "after independent second read")?;
    if first_sha256 != expected_sha256 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            "qualification base image differs from its caller-supplied digest",
        ));
    }
    let reopened = openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error("reopen canonical qualification base image", source))?;
    if snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("reinspect canonical qualification base image", source))?,
    ) != initial
    {
        return Err(changed(
            "qualification base-image pathname changed during admission",
        ));
    }
    let created_epoch = validate_squashfs_profile(&sealed, byte_len)?;
    let retained = SealedQualificationBaseImageV1 {
        sha256: first_sha256,
        byte_len,
        created_epoch,
        file: sealed,
    };
    validate_sealed_base_image(&retained)?;
    Ok(retained)
}

fn validate_base_image_source(
    source: &File,
    owner: (u32, u32),
) -> Result<super::ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(source)
        .map_err(|source| io_error("inspect qualification base-image descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(source)
        .map_err(|source| io_error("inspect qualification base-image status flags", source))?;
    let observed = snapshot(
        &fstat(source)
            .map_err(|source| io_error("inspect qualification base-image metadata", source))?,
    );
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(observed.mode) != FileType::RegularFile
        || observed.mode & 0o7777 != QUALIFICATION_BASE_IMAGE_MODE_V1
        || (observed.uid, observed.gid) != owner
        || observed.links != 1
        || !(SQUASHFS_SUPERBLOCK_BYTES_V1 as u64..=QUALIFICATION_BASE_IMAGE_MAX_BYTES_V1)
            .contains(&observed.byte_len)
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            format!(
                "qualification base image has invalid metadata: mode={:04o} owner={}:{} links={} bytes={} fd_flags={descriptor_flags:?} status_flags={status:?}",
                observed.mode & 0o7777,
                observed.uid,
                observed.gid,
                observed.links,
                observed.byte_len,
            ),
        ));
    }
    require_no_xattrs(source, "qualification base image")?;
    Ok(observed)
}

fn copy_and_hash_base_image(
    source: &mut File,
    sealed: &mut File,
) -> Result<([u8; 32], u64), DeploymentVerificationErrorV1> {
    source
        .seek(SeekFrom::Start(0))
        .map_err(|source| std_io_error("rewind qualification base image", source))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_BYTES_V1];
    let mut byte_len = 0_u64;
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|source| std_io_error("read qualification base image", source))?;
        if count == 0 {
            break;
        }
        byte_len = byte_len.checked_add(count as u64).ok_or_else(|| {
            invalid(
                DeploymentVerificationErrorKindV1::InvalidMetadata,
                "qualification base-image length overflowed",
            )
        })?;
        if byte_len > QUALIFICATION_BASE_IMAGE_MAX_BYTES_V1 {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidMetadata,
                "qualification base image exceeds its byte bound",
            ));
        }
        hasher.update(&buffer[..count]);
        sealed
            .write_all(&buffer[..count])
            .map_err(|source| std_io_error("copy sealed qualification base image", source))?;
    }
    Ok((hasher.finalize().into(), byte_len))
}

fn compare_source_with_sealed(
    source: &mut File,
    sealed: &File,
    expected_byte_len: u64,
) -> Result<(), DeploymentVerificationErrorV1> {
    source
        .seek(SeekFrom::Start(0))
        .map_err(|source| std_io_error("rewind qualification base image", source))?;
    let mut source_buffer = [0_u8; COPY_BUFFER_BYTES_V1];
    let mut sealed_buffer = [0_u8; COPY_BUFFER_BYTES_V1];
    let mut offset = 0_u64;
    while offset < expected_byte_len {
        let remaining =
            usize::try_from((expected_byte_len - offset).min(COPY_BUFFER_BYTES_V1 as u64))
                .expect("bounded chunk length fits usize");
        source
            .read_exact(&mut source_buffer[..remaining])
            .map_err(|source| std_io_error("reread qualification base image", source))?;
        sealed
            .read_exact_at(&mut sealed_buffer[..remaining], offset)
            .map_err(|source| std_io_error("reread sealed qualification base image", source))?;
        if source_buffer[..remaining] != sealed_buffer[..remaining] {
            return Err(changed(
                "qualification base-image bytes changed between independent reads",
            ));
        }
        offset += remaining as u64;
    }
    let mut trailing = [0_u8; 1];
    if source
        .read(&mut trailing)
        .map_err(|source| std_io_error("check qualification base-image EOF", source))?
        != 0
    {
        return Err(changed(
            "qualification base-image length changed during second read",
        ));
    }
    Ok(())
}

fn require_unchanged_base_image(
    source: &File,
    initial: super::ObjectSnapshotV1,
    phase: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    if snapshot(
        &fstat(source).map_err(|source| io_error("reinspect qualification base image", source))?,
    ) != initial
    {
        return Err(changed(format!("qualification base image changed {phase}")));
    }
    Ok(())
}

fn validate_sealed_base_image(
    retained: &SealedQualificationBaseImageV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    let expected_seals = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
    let observed = fstat(&retained.file)
        .map_err(|source| io_error("inspect sealed qualification base image", source))?;
    if FileType::from_raw_mode(observed.st_mode) != FileType::RegularFile
        || observed.st_mode & 0o7777 != QUALIFICATION_BASE_IMAGE_MODE_V1
        || u64::try_from(observed.st_size).unwrap_or(u64::MAX) != retained.byte_len
        || fcntl_get_seals(&retained.file)
            .map_err(|source| io_error("inspect qualification base-image seals", source))?
            != expected_seals
    {
        return Err(changed(
            "sealed qualification base image changed after admission",
        ));
    }
    Ok(())
}

fn validate_squashfs_profile(
    image: &File,
    byte_len: u64,
) -> Result<u32, DeploymentVerificationErrorV1> {
    let mut header = [0_u8; SQUASHFS_SUPERBLOCK_BYTES_V1];
    image
        .read_exact_at(&mut header, 0)
        .map_err(|source| std_io_error("read qualification SquashFS superblock", source))?;
    let inode_count = u32_at(&header, 4);
    let created_epoch = u32_at(&header, 8);
    let block_bytes = u32_at(&header, 12);
    let compression = u16_at(&header, 20);
    let block_log = u16_at(&header, 22);
    let flags = u16_at(&header, 24);
    let id_count = u16_at(&header, 26);
    let major = u16_at(&header, 28);
    let minor = u16_at(&header, 30);
    let bytes_used = u64_at(&header, 40);
    let id_table_start = u64_at(&header, 48);
    let xattr_table_start = u64_at(&header, 56);
    let inode_table_start = u64_at(&header, 64);
    let directory_table_start = u64_at(&header, 72);
    let padded_bytes = bytes_used
        .checked_add(SQUASHFS_PADDING_BYTES_V1 - 1)
        .map(|bytes| bytes / SQUASHFS_PADDING_BYTES_V1 * SQUASHFS_PADDING_BYTES_V1)
        .unwrap_or(u64::MAX);
    if u32_at(&header, 0) != SQUASHFS_MAGIC_V1
        || inode_count == 0
        || block_bytes != SQUASHFS_BLOCK_BYTES_V1
        || compression != SQUASHFS_COMPRESSION_ZSTD_V1
        || block_log != SQUASHFS_BLOCK_LOG_V1
        || flags & SQUASHFS_NO_XATTRS_FLAG_V1 == 0
        || id_count == 0
        || major != SQUASHFS_MAJOR_V1
        || minor != SQUASHFS_MINOR_V1
        || !(SQUASHFS_SUPERBLOCK_BYTES_V1 as u64..=byte_len).contains(&bytes_used)
        || padded_bytes != byte_len
        || !(SQUASHFS_SUPERBLOCK_BYTES_V1 as u64..bytes_used).contains(&id_table_start)
        || xattr_table_start != u64::MAX
        || !(SQUASHFS_SUPERBLOCK_BYTES_V1 as u64..bytes_used).contains(&inode_table_start)
        || !(SQUASHFS_SUPERBLOCK_BYTES_V1 as u64..bytes_used).contains(&directory_table_start)
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBase,
            "qualification base image is not the canonical SquashFS V1 profile",
        ));
    }
    let padding_len = usize::try_from(byte_len - bytes_used).map_err(|_| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBase,
            "qualification base-image padding length is not representable",
        )
    })?;
    let mut padding = vec![0_u8; padding_len];
    image
        .read_exact_at(&mut padding, bytes_used)
        .map_err(|source| std_io_error("read qualification base-image padding", source))?;
    if padding.iter().any(|&byte| byte != 0) {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBase,
            "qualification base image has nonzero trailing padding",
        ));
    }
    Ok(created_epoch)
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed header"))
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed header"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed header"))
}
