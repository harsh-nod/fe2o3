//! Immutable workspace target projection for the authority-free check wrapper.

use rustix::fs::{FileType, MemfdFlags, SealFlags, fcntl_get_seals, fstat};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::FileExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const MAGIC: &[u8] = b"fe2o3-binding-check-target-projection-v1\0";
const MEMFD_NAME: &str = "fe2o3-binding-check-projection-v1";
const MAX_PROJECTION_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TARGETS: usize = 65_536;
const MAX_FIELD_BYTES: usize = 1024 * 1024;
const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObjectIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
    pub(crate) mode: u64,
    pub(crate) size: u64,
    pub(crate) modified_seconds: i64,
    pub(crate) modified_nanoseconds: i64,
    pub(crate) changed_seconds: i64,
    pub(crate) changed_nanoseconds: i64,
}

impl ObjectIdentity {
    pub(crate) fn from_stat(stat: &rustix::fs::Stat) -> Result<Self, String> {
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode.into(),
            size: u64::try_from(stat.st_size)
                .map_err(|_| "workspace target source has a negative size".to_owned())?,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: i64::try_from(stat.st_mtime_nsec)
                .map_err(|_| "workspace target source has invalid mtime nanoseconds".to_owned())?,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: i64::try_from(stat.st_ctime_nsec)
                .map_err(|_| "workspace target source has invalid ctime nanoseconds".to_owned())?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetSource {
    pub(crate) package_name: String,
    pub(crate) package_root: PathBuf,
    pub(crate) package_device: u64,
    pub(crate) package_inode: u64,
    pub(crate) source_path: PathBuf,
    pub(crate) source_identity: ObjectIdentity,
    pub(crate) managed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Projection {
    pub(crate) workspace_root: PathBuf,
    pub(crate) workspace_device: u64,
    pub(crate) workspace_inode: u64,
    pub(crate) targets: Vec<TargetSource>,
}

impl Projection {
    pub(crate) fn managed_package_names(&self) -> Vec<String> {
        self.targets
            .iter()
            .filter(|target| target.managed)
            .map(|target| target.package_name.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(crate) fn validate_and_encode(&self) -> Result<Vec<u8>, String> {
        validate_absolute_normal(&self.workspace_root, "workspace root")?;
        if self.workspace_root.to_str().is_none() {
            return Err("workspace projection root is not UTF-8".to_owned());
        }
        if self.workspace_device == 0 || self.workspace_inode == 0 {
            return Err("workspace projection has a reserved root identity".to_owned());
        }
        if self.targets.is_empty() || self.targets.len() > MAX_TARGETS {
            return Err("workspace projection has an invalid target count".to_owned());
        }
        for pair in self.targets.windows(2) {
            if pair[0].source_path.as_os_str().as_bytes()
                >= pair[1].source_path.as_os_str().as_bytes()
            {
                return Err(format!(
                    "workspace target sources are duplicated or not canonically ordered: {}",
                    pair[1].source_path.display()
                ));
            }
        }
        let mut object_owners = std::collections::HashMap::new();
        object_owners
            .try_reserve(self.targets.len())
            .map_err(|_| "failed to reserve bounded target ownership map".to_owned())?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        field(&mut bytes, self.workspace_root.as_os_str().as_bytes())?;
        u64_field(&mut bytes, self.workspace_device);
        u64_field(&mut bytes, self.workspace_inode);
        u32_field(
            &mut bytes,
            u32::try_from(self.targets.len()).map_err(|_| "target count overflow".to_owned())?,
        );
        for target in &self.targets {
            validate_package_name(&target.package_name)?;
            validate_absolute_normal(&target.package_root, "package root")?;
            validate_absolute_normal(&target.source_path, "target source")?;
            if target.package_device == 0
                || target.package_inode == 0
                || target.package_root.to_str().is_none()
                || target.source_path.to_str().is_none()
                || FileType::from_raw_mode(target.source_identity.mode as _)
                    != FileType::RegularFile
                || target.source_identity.device == 0
                || target.source_identity.inode == 0
                || target.source_identity.size > 8 * 1024 * 1024
                || !(0..1_000_000_000).contains(&target.source_identity.modified_nanoseconds)
                || !(0..1_000_000_000).contains(&target.source_identity.changed_nanoseconds)
                || !target.package_root.starts_with(&self.workspace_root)
                || !target.source_path.starts_with(&target.package_root)
                || target.source_path.extension() != Some(OsStr::new("rs"))
            {
                return Err(format!(
                    "workspace target source is outside the UTF-8 .rs package contract: {}",
                    target.source_path.display()
                ));
            }
            if let Some(owner) = object_owners.insert(
                (target.source_identity.device, target.source_identity.inode),
                target.package_name.as_str(),
            ) && owner != target.package_name
            {
                return Err(format!(
                    "workspace target object is claimed by packages `{owner}` and `{}`",
                    target.package_name
                ));
            }
            field(&mut bytes, target.package_name.as_bytes())?;
            field(&mut bytes, target.package_root.as_os_str().as_bytes())?;
            u64_field(&mut bytes, target.package_device);
            u64_field(&mut bytes, target.package_inode);
            field(&mut bytes, target.source_path.as_os_str().as_bytes())?;
            encode_identity(&mut bytes, target.source_identity);
            bytes.push(u8::from(target.managed));
            if bytes.len() as u64 > MAX_PROJECTION_BYTES {
                return Err("workspace target projection exceeds its byte bound".to_owned());
            }
        }
        Ok(bytes)
    }
}

pub(crate) struct SealedProjection {
    file: File,
    identity: (u64, u64, u64),
}

impl SealedProjection {
    pub(crate) fn new(projection: &Projection) -> Result<Self, String> {
        let bytes = projection.validate_and_encode()?;
        let mut file =
            rustix::fs::memfd_create(MEMFD_NAME, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
                .map(File::from)
                .map_err(|error| format!("failed to create binding projection memfd: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("failed to write binding projection: {error}"))?;
        fe2o3_process_identity::seal_immutable_memfd_v1(
            &file,
            fe2o3_process_identity::ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
        )
        .map_err(|error| format!("failed to seal binding projection: {error}"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind binding projection: {error}"))?;
        let stat = fstat(&file)
            .map_err(|error| format!("failed to inspect binding projection: {error}"))?;
        if fcntl_get_seals(&file)
            .map_err(|error| format!("failed to inspect binding projection seals: {error}"))?
            != REQUIRED_SEALS
        {
            return Err("binding projection does not have exact immutable seals".to_owned());
        }
        if stat.st_size as usize != bytes.len() {
            return Err("binding projection length mismatch after sealing".to_owned());
        }
        let mut sealed_bytes = Vec::with_capacity(bytes.len());
        file.read_to_end(&mut sealed_bytes)
            .map_err(|error| format!("failed to revalidate sealed binding projection: {error}"))?;
        if sealed_bytes != bytes {
            return Err("binding projection content mismatch after sealing".to_owned());
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind sealed binding projection: {error}"))?;
        Ok(Self {
            file,
            identity: (stat.st_dev, stat.st_ino, stat.st_size as u64),
        })
    }

    pub(crate) fn inherit_for_child_at(
        &self,
        command: &mut Command,
        target_fd: std::os::fd::RawFd,
    ) -> Result<(), String> {
        if target_fd < 3 || std::fs::symlink_metadata(format!("/proc/self/fd/{target_fd}")).is_ok()
        {
            return Err(format!(
                "binding projection child descriptor {target_fd} is unavailable"
            ));
        }
        let source_fd = self.file.as_raw_fd();
        let expected = self.identity;
        // SAFETY: the retained sealed file remains alive through spawn; the callback only
        // duplicates and validates descriptors in the child before exec.
        unsafe {
            command.pre_exec(move || {
                let source = BorrowedFd::borrow_raw(source_fd);
                let installed = rustix::io::fcntl_dupfd_cloexec(source, target_fd)
                    .map_err(std::io::Error::from)?;
                if installed.as_raw_fd() != target_fd {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::BUSY.raw_os_error(),
                    ));
                }
                let stat = fstat(&installed).map_err(std::io::Error::from)?;
                if (stat.st_dev, stat.st_ino, stat.st_size as u64) != expected {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::STALE.raw_os_error(),
                    ));
                }
                rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty())
                    .map_err(std::io::Error::from)?;
                let _ = installed.into_raw_fd();
                Ok(())
            });
        }
        Ok(())
    }
}

pub(crate) fn consume_inherited() -> Result<Projection, String> {
    let descriptor = crate::CARGO_BINDING_CHECK_PROJECTION_CHILD_FD;
    // SAFETY: the fixed descriptor is validated before ownership is consumed.
    let borrowed = unsafe { BorrowedFd::borrow_raw(descriptor) };
    rustix::io::fcntl_getfd(borrowed)
        .map_err(|error| format!("binding projection descriptor is unavailable: {error}"))?;
    rustix::io::fcntl_setfd(borrowed, rustix::io::FdFlags::CLOEXEC)
        .map_err(|error| format!("failed to make binding projection close-on-exec: {error}"))?;
    // SAFETY: this wrapper invocation owns its inherited descriptor copy.
    let file = unsafe { File::from_raw_fd(descriptor) };
    let stat = fstat(&file)
        .map_err(|error| format!("failed to inspect inherited binding projection: {error}"))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_nlink != 0
        || stat.st_size <= 0
        || stat.st_size as u64 > MAX_PROJECTION_BYTES
    {
        return Err(
            "inherited binding projection is not a bounded anonymous regular file".to_owned(),
        );
    }
    if fcntl_get_seals(&file)
        .map_err(|error| format!("failed to inspect inherited binding projection seals: {error}"))?
        != REQUIRED_SEALS
    {
        return Err("inherited binding projection does not have exact immutable seals".to_owned());
    }
    let link = std::fs::read_link(format!("/proc/self/fd/{descriptor}"))
        .map_err(|error| format!("failed to inspect binding projection proc link: {error}"))?;
    let expected = format!("/memfd:{MEMFD_NAME} (deleted)");
    let alternate = format!("memfd:{MEMFD_NAME} (deleted)");
    if link != Path::new(&expected) && link != Path::new(&alternate) {
        return Err(format!(
            "inherited binding projection is not the exact named memfd: {}",
            link.display()
        ));
    }
    let bytes = read_projection_at_zero(&file, stat.st_size as usize)?;
    decode(&bytes)
}

fn read_projection_at_zero(file: &File, size: usize) -> Result<Vec<u8>, String> {
    if size == 0 || size as u64 > MAX_PROJECTION_BYTES {
        return Err("binding projection read has an invalid size".to_owned());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| "failed to reserve bounded binding projection".to_owned())?;
    bytes.resize(size, 0);
    file.read_exact_at(&mut bytes, 0)
        .map_err(|error| format!("failed to positionally read binding projection: {error}"))?;
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<Projection, String> {
    let mut reader = Reader { bytes, offset: 0 };
    reader.exact(MAGIC)?;
    let workspace_root = PathBuf::from(OsString::from_vec(reader.field()?.to_vec()));
    validate_absolute_normal(&workspace_root, "workspace root")?;
    let workspace_device = reader.u64()?;
    let workspace_inode = reader.u64()?;
    if workspace_device == 0 || workspace_inode == 0 {
        return Err("binding projection has a reserved workspace identity".to_owned());
    }
    let count = reader.u32()? as usize;
    if count == 0 || count > MAX_TARGETS {
        return Err("binding projection has an invalid target count".to_owned());
    }
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(count)
        .map_err(|_| "failed to reserve bounded binding targets".to_owned())?;
    for _ in 0..count {
        let package_name = std::str::from_utf8(reader.field()?)
            .map_err(|_| "binding projection package name is not UTF-8".to_owned())?
            .to_owned();
        validate_package_name(&package_name)?;
        let package_root = PathBuf::from(OsString::from_vec(reader.field()?.to_vec()));
        let package_device = reader.u64()?;
        let package_inode = reader.u64()?;
        let source_path = PathBuf::from(OsString::from_vec(reader.field()?.to_vec()));
        let source_identity = decode_identity(&mut reader)?;
        let managed = match reader.byte()? {
            0 => false,
            1 => true,
            _ => return Err("binding projection has a noncanonical managed flag".to_owned()),
        };
        targets.push(TargetSource {
            package_name,
            package_root,
            package_device,
            package_inode,
            source_path,
            source_identity,
            managed,
        });
    }
    if reader.offset != bytes.len() {
        return Err("binding projection has trailing bytes".to_owned());
    }
    let projection = Projection {
        workspace_root,
        workspace_device,
        workspace_inode,
        targets,
    };
    if projection.validate_and_encode()? != bytes {
        return Err("binding projection is not canonically ordered and encoded".to_owned());
    }
    Ok(projection)
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 256
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(format!("invalid binding projection package name `{name}`"));
    }
    Ok(())
}

fn validate_absolute_normal(path: &Path, kind: &str) -> Result<(), String> {
    if !path.is_absolute() || path.as_os_str().as_bytes().contains(&0) {
        return Err(format!(
            "binding projection {kind} is not an absolute normal path"
        ));
    }
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(format!(
                    "binding projection {kind} is not an absolute normal path"
                ));
            }
        }
    }
    if normalized.as_os_str().as_bytes() != path.as_os_str().as_bytes() {
        return Err(format!(
            "binding projection {kind} does not use canonical path spelling"
        ));
    }
    Ok(())
}

fn field(output: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_FIELD_BYTES {
        return Err("binding projection field has an invalid length".to_owned());
    }
    u32_field(
        output,
        u32::try_from(value.len()).map_err(|_| "projection field length overflow".to_owned())?,
    );
    output.extend_from_slice(value);
    Ok(())
}

fn u32_field(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn u64_field(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn i64_field(output: &mut Vec<u8>, value: i64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn encode_identity(output: &mut Vec<u8>, identity: ObjectIdentity) {
    for value in [
        identity.device,
        identity.inode,
        identity.mode,
        identity.size,
    ] {
        u64_field(output, value);
    }
    for value in [
        identity.modified_seconds,
        identity.modified_nanoseconds,
        identity.changed_seconds,
        identity.changed_nanoseconds,
    ] {
        i64_field(output, value);
    }
}

fn decode_identity(reader: &mut Reader<'_>) -> Result<ObjectIdentity, String> {
    Ok(ObjectIdentity {
        device: reader.u64()?,
        inode: reader.u64()?,
        mode: reader.u64()?,
        size: reader.u64()?,
        modified_seconds: reader.i64()?,
        modified_nanoseconds: reader.i64()?,
        changed_seconds: reader.i64()?,
        changed_nanoseconds: reader.i64()?,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| "binding projection is truncated".to_owned())?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn exact(&mut self, expected: &[u8]) -> Result<(), String> {
        if self.take(expected.len())? != expected {
            return Err("binding projection has the wrong domain/version".to_owned());
        }
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("exact u32 field"),
        ))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("exact u64 field"),
        ))
    }

    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(
            self.take(8)?.try_into().expect("exact i64 field"),
        ))
    }

    fn field(&mut self) -> Result<&'a [u8], String> {
        let count = self.u32()? as usize;
        if count == 0 || count > MAX_FIELD_BYTES {
            return Err("binding projection field has an invalid length".to_owned());
        }
        self.take(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Projection {
        Projection {
            workspace_root: PathBuf::from("/workspace"),
            workspace_device: 1,
            workspace_inode: 2,
            targets: vec![TargetSource {
                package_name: "managed".to_owned(),
                package_root: PathBuf::from("/workspace/managed"),
                package_device: 3,
                package_inode: 4,
                source_path: PathBuf::from("/workspace/managed/src/lib.rs"),
                source_identity: ObjectIdentity {
                    device: 5,
                    inode: 6,
                    mode: 0o100644,
                    size: 7,
                    modified_seconds: 8,
                    modified_nanoseconds: 9,
                    changed_seconds: 10,
                    changed_nanoseconds: 11,
                },
                managed: true,
            }],
        }
    }

    #[test]
    fn canonical_projection_round_trips_and_rejects_malformed_encodings() {
        let bytes = fixture().validate_and_encode().unwrap();
        assert_eq!(decode(&bytes).unwrap(), fixture());
        let mut wrong_domain = bytes.clone();
        wrong_domain[0] ^= 0x80;
        assert!(decode(&wrong_domain).is_err());
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode(&trailing).is_err());
    }

    #[test]
    fn duplicate_target_ownership_and_non_rs_targets_fail_closed() {
        let mut duplicate = fixture();
        duplicate.targets.push(duplicate.targets[0].clone());
        assert!(duplicate.validate_and_encode().is_err());
        let mut unordered = fixture();
        let mut second = unordered.targets[0].clone();
        second.package_name = "earlier".to_owned();
        second.package_root = PathBuf::from("/workspace/earlier");
        second.source_path = PathBuf::from("/workspace/earlier/src/lib.rs");
        unordered.targets.push(second);
        assert!(unordered.validate_and_encode().is_err());
        let mut cross_package_hardlink = fixture();
        let mut second = cross_package_hardlink.targets[0].clone();
        second.package_name = "second".to_owned();
        second.package_root = PathBuf::from("/workspace/second");
        second.source_path = PathBuf::from("/workspace/second/src/lib.rs");
        cross_package_hardlink.targets.push(second);
        assert!(cross_package_hardlink.validate_and_encode().is_err());
        let mut non_rs = fixture();
        non_rs.targets[0].source_path = PathBuf::from("/workspace/managed/src/lib.kernel");
        assert!(non_rs.validate_and_encode().is_err());
        let mut noncanonical = fixture();
        noncanonical.targets[0].package_root = PathBuf::from("/workspace//managed");
        assert!(noncanonical.validate_and_encode().is_err());
        let mut nul = fixture();
        nul.targets[0].source_path = PathBuf::from(OsString::from_vec(
            b"/workspace/managed/src/lib.rs\0ignored".to_vec(),
        ));
        assert!(nul.validate_and_encode().is_err());
    }

    #[test]
    fn duplicated_projection_consumers_never_share_a_stream_offset() {
        let expected = fixture();
        let mut sealed = SealedProjection::new(&expected).unwrap();
        sealed.file.seek(SeekFrom::End(0)).unwrap();
        let size = sealed.identity.2 as usize;
        let mut workers = Vec::new();
        for _ in 0..32 {
            let file = sealed.file.try_clone().unwrap();
            let expected = expected.clone();
            workers.push(std::thread::spawn(move || {
                let bytes = read_projection_at_zero(&file, size).unwrap();
                assert_eq!(decode(&bytes).unwrap(), expected);
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(sealed.file.stream_position().unwrap(), size as u64);
    }

    #[test]
    fn sealed_projection_has_exact_immutable_seals_and_revalidates_content() {
        let expected = fixture();
        let sealed = SealedProjection::new(&expected).unwrap();
        let seals = fcntl_get_seals(&sealed.file).unwrap();
        assert_eq!(seals, REQUIRED_SEALS);
        assert_eq!(
            sealed.identity.2,
            expected.validate_and_encode().unwrap().len() as u64
        );
    }
}
