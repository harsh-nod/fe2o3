//! Finite-attempt provisioner I/O sharing root-source metadata and byte checks.
use super::{Policy, Snapshot, equal_bytes, length, read_copy, snapshot, unchanged};
use crate::native_provisioner::{Failure, Result};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1 as Measurement;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_runtime_protocol::{
    SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1 as PARSER_STORAGE,
    sealed_static_application_identity_v1, sealed_static_application_work_bound_v1,
};
use rustix::fs::{self, AtFlags, FileType, FlockOperation, Mode, OFlags, RenameFlags};
use sha2::{Digest, Sha256};
use std::{fs::File, mem::size_of, os::unix::ffi::OsStrExt, path::Path};
use zeroize::Zeroizing;

pub(crate) const PATH_BYTES: usize = 4096;
pub(crate) const FILE_WORK: usize = 8 + 128 * 1024;
pub(crate) const FILE_SCRATCH: usize = 16384;
pub(crate) const SEED_STORAGE: usize = size_of::<Seed>() + 32 + 128;
const OPEN: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC)
    .union(OFlags::NONBLOCK);

#[derive(Clone, Copy)]
pub(crate) struct Owner {
    pub uid: u32,
    pub gid: u32,
}
impl Owner {
    pub const ROOT: Self = Self { uid: 0, gid: 0 };
    fn policy(self, mode: u32, length: usize) -> Policy {
        Policy {
            uid: self.uid,
            gid: self.gid,
            mode,
            length,
        }
    }
}

pub(crate) fn io(operation: &'static str, source: rustix::io::Errno) -> Failure {
    Failure::Io { operation, source }
}
fn invalid(reason: &'static str) -> Failure {
    Failure::Invalid(reason)
}
fn path(path: &Path) -> Result<()> {
    let bytes = path.as_os_str().as_bytes();
    if !path.is_absolute() || bytes.len() >= PATH_BYTES || bytes.contains(&0) {
        return Err(invalid("provisioning path is not bounded and absolute"));
    }
    Ok(())
}
fn open(pathname: &Path) -> Result<File> {
    path(pathname)?;
    fs::open(pathname, OPEN, Mode::empty())
        .map(File::from)
        .map_err(|e| io("open provisioning source", e))
}
fn file_size(file: &File, maximum: usize) -> Result<usize> {
    let stat = fs::fstat(file).map_err(|e| io("inspect provisioning source size", e))?;
    let n = usize::try_from(stat.st_size).map_err(|_| invalid("invalid source length"))?;
    if n == 0 || n > maximum {
        return Err(invalid("source length exceeds role bound"));
    }
    Ok(n)
}
fn bytes(n: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(n)
        .map_err(|_| Resource::Allocation)?;
    if bytes.capacity() > n.checked_mul(2).ok_or(Resource::Arithmetic)? {
        return Err(Resource::Allocation.into());
    }
    bytes.resize(n, 0);
    Ok(bytes)
}

pub(crate) struct Image<'a> {
    path: &'a Path,
    file: File,
    observed: Snapshot,
    owner: Owner,
    pub measurement: Measurement,
}
impl<'a> Image<'a> {
    pub fn quota(maximum: usize) -> Result<(usize, usize)> {
        length(maximum)?;
        let work = sealed_static_application_work_bound_v1(maximum)
            .and_then(|n| n.checked_add(maximum.checked_mul(32)?))
            .and_then(|n| n.checked_add(FILE_WORK))
            .ok_or(Resource::Arithmetic)?;
        let scratch = maximum
            .checked_mul(5)
            .and_then(|n| n.checked_add(PARSER_STORAGE + FILE_SCRATCH + 4 * size_of::<Self>()))
            .ok_or(Resource::Arithmetic)?;
        Ok((work, scratch))
    }
    pub fn measure(
        pathname: &'a Path,
        maximum: usize,
        owner: Owner,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        let (work, scratch) = Self::quota(maximum)?;
        b.with_prepaid_scope(0, 8, work, scratch, |_| {
            let file = open(pathname)?;
            let n = file_size(&file, maximum)?;
            let policy = owner.policy(0o555, n);
            let before = snapshot(&file, policy)?;
            let mut first = bytes(n)?;
            let mut second = bytes(n)?;
            read_copy(&file, &mut first)?;
            read_copy(&file, &mut second)?;
            equal_bytes(&first, &second)?;
            unchanged(before, snapshot(&file, policy)?)?;
            sealed_static_application_identity_v1(&first)?;
            let measurement = Measurement::new(Sha256::digest(&first).into(), n as u64)
                .map_err(|_| invalid("invalid measured executable"))?;
            Ok(Self {
                path: pathname,
                file,
                observed: before,
                owner,
                measurement,
            })
        })
    }
    pub fn retained_storage(&self) -> usize {
        self.measurement.byte_len() as usize + size_of::<Self>() + PATH_BYTES
    }
    pub fn revalidation_quota(n: usize) -> Result<(usize, usize)> {
        length(n)?;
        Ok((
            n.checked_mul(32)
                .and_then(|n| n.checked_add(FILE_WORK))
                .ok_or(Resource::Arithmetic)?,
            n.checked_mul(5)
                .and_then(|n| n.checked_add(FILE_SCRATCH))
                .ok_or(Resource::Arithmetic)?,
        ))
    }
    pub fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        let n = self.measurement.byte_len() as usize;
        let (work, scratch) = Self::revalidation_quota(n)?;
        b.with_prepaid_scope(self.retained_storage(), 8, work, scratch, |_| {
            let policy = self.owner.policy(0o555, n);
            unchanged(self.observed, snapshot(&self.file, policy)?)?;
            unchanged(self.observed, snapshot(&open(self.path)?, policy)?)?;
            // Metadata equality alone cannot establish unchanged contents.
            let mut first = bytes(n)?;
            let mut second = bytes(n)?;
            read_copy(&self.file, &mut first)?;
            read_copy(&self.file, &mut second)?;
            equal_bytes(&first, &second)?;
            equal_bytes(&Sha256::digest(&first), &self.measurement.sha256())?;
            unchanged(self.observed, snapshot(&self.file, policy)?)?;
            unchanged(self.observed, snapshot(&open(self.path)?, policy)?)?;
            Ok(())
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    links: u64,
}
fn directory_identity(file: &File, owner: Owner) -> Result<DirectoryIdentity> {
    let stat = fs::fstat(file).map_err(|e| io("inspect provisioning directory", e))?;
    let flags = fs::fcntl_getfl(file).map_err(|e| io("inspect directory flags", e))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != 0o755
        || stat.st_uid != owner.uid
        || stat.st_gid != owner.gid
        || stat.st_nlink == 0
        || flags & OFlags::ACCMODE != OFlags::RDONLY
        || flags.contains(OFlags::PATH)
        || rustix::io::fcntl_getfd(file).map_err(|e| io("inspect directory descriptor flags", e))?
            != rustix::io::FdFlags::CLOEXEC
    {
        return Err(invalid("invalid provisioning directory"));
    }
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        super::attribute_absent(fs::fgetxattr(file, attribute, &mut [0; 1]))?;
    }
    Ok(DirectoryIdentity {
        device: stat.st_dev,
        inode: stat.st_ino,
        uid: stat.st_uid,
        gid: stat.st_gid,
        mode: stat.st_mode,
        links: stat.st_nlink,
    })
}
pub(crate) struct Directory<'a> {
    path: &'a Path,
    file: File,
    identity: DirectoryIdentity,
    owner: Owner,
}
pub(crate) struct Seed<'a> {
    bytes: Zeroizing<[u8; 32]>,
    file: File,
    observed: Snapshot,
    name: &'a str,
}
impl std::ops::Deref for Seed<'_> {
    type Target = [u8; 32];
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}
impl<'a> Directory<'a> {
    pub const STORAGE: usize = size_of::<Self>() + PATH_BYTES;
    pub fn open(pathname: &'a Path, owner: Owner, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(0, 8, FILE_WORK, FILE_SCRATCH, |_| {
            path(pathname)?;
            let file = fs::open(pathname, OPEN | OFlags::DIRECTORY, Mode::empty())
                .map(File::from)
                .map_err(|e| io("open provisioning directory", e))?;
            let identity = directory_identity(&file, owner)?;
            fs::flock(&file, FlockOperation::NonBlockingLockExclusive)
                .map_err(|e| io("lock provisioning directory", e))?;
            Ok(Self {
                path: pathname,
                file,
                identity,
                owner,
            })
        })
    }
    pub fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(Self::STORAGE, 8, FILE_WORK, FILE_SCRATCH, |_| self.check())
    }
    fn check(&self) -> Result<()> {
        if directory_identity(&self.file, self.owner)? != self.identity
            || directory_identity(&open(self.path)?, self.owner)? != self.identity
        {
            return Err(invalid("provisioning directory identity changed"));
        }
        fs::flock(&self.file, FlockOperation::NonBlockingLockExclusive)
            .map_err(|e| io("recheck provisioning directory lock", e))
    }
    fn existing(&self, name: &str) -> Result<Option<File>> {
        if name.is_empty()
            || name.len() > 128
            || name.as_bytes().contains(&0)
            || name.contains('/')
            || name == "."
            || name == ".."
        {
            return Err(invalid("invalid fixed provisioning entry name"));
        }
        match fs::openat(&self.file, name, OPEN, Mode::empty()) {
            Ok(fd) => Ok(Some(File::from(fd))),
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(e) => Err(io("open provisioning record", e)),
        }
    }
    pub fn seed<'n>(&self, name: &'n str, b: &mut Budget<'_>) -> Result<Seed<'n>> {
        b.with_prepaid_scope(
            Self::STORAGE,
            8,
            record_work(32)?,
            record_scratch(32)?,
            |_| {
                self.check()?;
                let mut seed = Zeroizing::new([0; 32]);
                let file = if let Some(file) = self.existing(name)? {
                    self.read(&file, 0o400, &mut seed[..])?;
                    self.sync_named(name, &file, 0o400, 32)?;
                    file
                } else {
                    random(&mut seed[..])?;
                    self.publish_new(name, &seed[..], 0o400)?
                };
                let observed = snapshot(&file, self.owner.policy(0o400, 32))?;
                self.check()?;
                Ok(Seed {
                    bytes: seed,
                    file,
                    observed,
                    name,
                })
            },
        )
    }
    pub fn verify_seed(&self, seed: &Seed<'_>, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            Self::STORAGE + SEED_STORAGE,
            8,
            record_work(32)?,
            record_scratch(32)?,
            |_| {
                self.check()?;
                let policy = self.owner.policy(0o400, 32);
                unchanged(seed.observed, snapshot(&seed.file, policy)?)?;
                let file = self
                    .existing(seed.name)?
                    .ok_or(invalid("provisioned seed disappeared"))?;
                unchanged(seed.observed, snapshot(&file, policy)?)?;
                let mut actual = Zeroizing::new([0; 32]);
                self.read(&file, 0o400, &mut actual[..])?;
                equal_bytes(&actual[..], &seed.bytes[..])?;
                self.sync_named(seed.name, &file, 0o400, 32)?;
                self.check()
            },
        )
    }
    pub fn publish<const N: usize>(
        &self,
        name: &str,
        expected: &[u8; N],
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = Self::STORAGE.checked_add(N).ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, record_work(N)?, record_scratch(N)?, |_| {
            self.check()?;
            if let Some(file) = self.existing(name)? {
                self.verify_record(name, &file, expected)?;
            } else {
                self.publish_new(name, expected, 0o444)?;
            }
            self.check()
        })
    }
    pub fn verify<const N: usize>(
        &self,
        name: &str,
        expected: &[u8; N],
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = Self::STORAGE.checked_add(N).ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, record_work(N)?, record_scratch(N)?, |_| {
            self.check()?;
            let file = self
                .existing(name)?
                .ok_or(invalid("provisioned record disappeared"))?;
            self.verify_record(name, &file, expected)?;
            self.check()
        })
    }
    fn verify_record(&self, name: &str, file: &File, expected: &[u8]) -> Result<()> {
        let mut actual = bytes(expected.len())?;
        self.read(file, 0o444, &mut actual)?;
        equal_bytes(&actual, expected)?;
        self.sync_named(name, file, 0o444, expected.len())
    }
    fn sync_named(&self, name: &str, file: &File, mode: u32, n: usize) -> Result<()> {
        let policy = self.owner.policy(mode, n);
        let observed = snapshot(file, policy)?;
        let named = self
            .existing(name)?
            .ok_or(invalid("provisioned entry disappeared"))?;
        unchanged(observed, snapshot(&named, policy)?)?;
        // A prior rename may have succeeded before its directory sync failed.
        fs::fsync(file).map_err(|e| io("sync verified provisioning file", e))?;
        fs::fsync(&self.file).map_err(|e| io("sync verified provisioning directory", e))?;
        unchanged(observed, snapshot(file, policy)?)?;
        let named = self
            .existing(name)?
            .ok_or(invalid("provisioned entry disappeared"))?;
        unchanged(observed, snapshot(&named, policy)?)?;
        Ok(())
    }
    fn read(&self, file: &File, mode: u32, first: &mut [u8]) -> Result<()> {
        let policy = self.owner.policy(mode, first.len());
        let before = snapshot(file, policy)?;
        let mut second = Zeroizing::new(bytes(first.len())?);
        read_copy(file, first)?;
        read_copy(file, &mut second)?;
        equal_bytes(first, &second)?;
        unchanged(before, snapshot(file, policy)?)?;
        Ok(())
    }
    fn publish_new(&self, name: &str, payload: &[u8], mode: u32) -> Result<File> {
        let mut nonce = [0; 16];
        random(&mut nonce)?;
        let temporary = temporary_name(nonce);
        let temporary =
            std::str::from_utf8(&temporary).map_err(|_| invalid("invalid temporary name"))?;
        self.publish_new_named(name, payload, mode, temporary)
    }
    fn publish_new_named(
        &self,
        name: &str,
        payload: &[u8],
        mode: u32,
        temporary: &str,
    ) -> Result<File> {
        self.publish_with_sync(name, payload, mode, temporary, |directory| {
            fs::fsync(directory)
        })
    }
    fn publish_with_sync(
        &self,
        name: &str,
        payload: &[u8],
        mode: u32,
        temporary: &str,
        sync_directory: impl FnOnce(&File) -> rustix::io::Result<()>,
    ) -> Result<File> {
        let file = fs::openat(
            &self.file,
            temporary,
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::CLOEXEC
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )
        .map(File::from)
        .map_err(|e| io("create provisioning temporary", e))?;
        // Arm cleanup only after exclusive creation. A collision belongs to someone else.
        let mut pending = Pending {
            directory: &self.file,
            name: temporary,
            file,
            renamed: false,
        };
        exact_write(payload.len(), rustix::io::pwrite(&pending.file, payload, 0))?;
        fs::fchmod(&pending.file, Mode::from_raw_mode(mode))
            .map_err(|e| io("set provisioning mode", e))?;
        fs::fsync(&pending.file).map_err(|e| io("sync provisioning temporary", e))?;
        // Reopen read-only: the shared root-source policy excludes writable OFDs.
        let candidate = fs::openat(&self.file, temporary, OPEN, Mode::empty())
            .map(File::from)
            .map_err(|e| io("reopen temporary", e))?;
        let stat = fs::fstat(&pending.file).map_err(|e| io("inspect temporary identity", e))?;
        let observed = snapshot(&candidate, self.owner.policy(mode, payload.len()))?;
        if (observed.device, observed.inode) != (stat.st_dev, stat.st_ino) {
            return Err(invalid("temporary identity changed"));
        }
        self.check()?;
        fs::renameat_with(
            &self.file,
            temporary,
            &self.file,
            name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|e| io("publish provisioning file", e))?;
        pending.renamed = true;
        sync_directory(&self.file).map_err(|e| io("sync provisioning directory", e))?;
        let published = self
            .existing(name)?
            .ok_or_else(|| invalid("published record disappeared"))?;
        let published_observation = snapshot(&published, self.owner.policy(mode, payload.len()))?;
        // rename may update ctime; require the created inode, then a stable named read.
        if (observed.device, observed.inode)
            != (published_observation.device, published_observation.inode)
        {
            return Err(invalid("published inode differs from created temporary"));
        }
        let mut actual = Zeroizing::new(bytes(payload.len())?);
        self.read(&published, mode, &mut actual)?;
        equal_bytes(&actual, payload)?;
        unchanged(
            published_observation,
            snapshot(&published, self.owner.policy(mode, payload.len()))?,
        )?;
        Ok(published)
    }
}

struct Pending<'a> {
    directory: &'a File,
    name: &'a str,
    file: File,
    renamed: bool,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.renamed {
            return;
        }
        let Ok(owned) = fs::fstat(&self.file) else {
            return;
        };
        let Ok(named) = fs::statat(self.directory, self.name, AtFlags::SYMLINK_NOFOLLOW) else {
            return;
        };
        if (owned.st_dev, owned.st_ino) == (named.st_dev, named.st_ino) {
            let _ = fs::unlinkat(self.directory, self.name, AtFlags::empty());
        }
    }
}
pub(crate) fn record_work(n: usize) -> Result<usize> {
    length(n)?;
    n.checked_mul(64)
        .and_then(|n| n.checked_add(4 * FILE_WORK))
        .ok_or(Resource::Arithmetic.into())
}
pub(crate) fn record_scratch(n: usize) -> Result<usize> {
    length(n)?;
    n.checked_mul(8)
        .and_then(|n| n.checked_add(FILE_SCRATCH))
        .ok_or(Resource::Arithmetic.into())
}
fn exact_write(n: usize, written: rustix::io::Result<usize>) -> Result<()> {
    if written.map_err(|e| io("write provisioning temporary", e))? != n {
        return Err(invalid("short provisioning write"));
    }
    Ok(())
}
fn random(bytes: &mut [u8]) -> Result<()> {
    let n = rustix::rand::getrandom(&mut *bytes, rustix::rand::GetRandomFlags::NONBLOCK)
        .map_err(|e| io("read provisioning randomness", e))?;
    if n != bytes.len() {
        return Err(invalid("short provisioning randomness"));
    }
    Ok(())
}
fn temporary_name(nonce: [u8; 16]) -> [u8; 46] {
    let mut name = [0; 46];
    name[..14].copy_from_slice(b".provisioning-");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, byte) in nonce.into_iter().enumerate() {
        name[14 + 2 * i] = HEX[usize::from(byte >> 4)];
        name[15 + 2 * i] = HEX[usize::from(byte & 15)];
    }
    name
}

pub(crate) fn listener_absent(pathname: &Path, b: &mut Budget<'_>) -> Result<()> {
    b.with_prepaid_scope(0, 8, FILE_WORK, FILE_SCRATCH, |_| {
        path(pathname)?;
        match fs::statat(fs::CWD, pathname, AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Ok(_) => Err(invalid("compiler execution listener is present")),
            Err(e) => Err(io("inspect compiler execution listener", e)),
        }
    })
}

#[cfg(test)]
#[path = "native_provisioning_io_tests.rs"]
mod tests;
