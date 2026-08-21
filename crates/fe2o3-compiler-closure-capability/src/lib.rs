//! Sealed transport for one canonical protected compiler-closure preimage.
//!
//! The descriptor carries coordination evidence only. It does not grant compiler, publication,
//! linking, loading, launch, or execution authority.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::fs::{self, File};
use std::io::Write;
use std::os::fd::{AsRawFd, BorrowedFd, IntoRawFd, RawFd};
use std::os::unix::fs::{FileExt, MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::process::Command;

use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};

const MAGIC: &[u8] = b"FE2O3-COMPILER-CLOSURE-CAPABILITY-V1\0";
const VERSION: u16 = 1;
const FLAGS: u16 = 0;
const CHECKSUM_DOMAIN: &[u8] = b"FE2O3/COMPILER-CLOSURE-CAPABILITY/CHECKSUM/V1\0";
const HEADER_BYTES: usize = MAGIC.len() + 2 + 2 + 4;
const CLOSURE_BYTES: usize = (6 * 32) + 2 + 2 + 32;
const CHECKSUM_BYTES: usize = 32;
const WIRE_BYTES: usize = HEADER_BYTES + CLOSURE_BYTES + CHECKSUM_BYTES;
const REQUIRED_SEALS: rustix::fs::SealFlags = rustix::fs::SealFlags::WRITE
    .union(rustix::fs::SealFlags::GROW)
    .union(rustix::fs::SealFlags::SHRINK)
    .union(rustix::fs::SealFlags::SEAL);

/// Reserved descriptor used to pass the protected compiler closure into rustc and its backend.
pub const COMPILER_CLOSURE_CHILD_FD_V1: RawFd = 199;

/// An immutable file capability containing one validated compiler closure.
pub struct CompilerClosureCapabilityV1 {
    closure: CompilerClosureV2,
    image: File,
    device: u64,
    inode: u64,
}

impl CompilerClosureCapabilityV1 {
    /// Creates and seals a canonical capability image.
    pub fn create(closure: CompilerClosureV2) -> Result<Self, String> {
        let bytes = encode(closure);
        let image = rustix::fs::memfd_create(
            "fe2o3-compiler-closure-capability-v1",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .map_err(|error| format!("cannot allocate compiler-closure capability: {error}"))?;
        image
            .set_permissions(fs::Permissions::from_mode(0o400))
            .map_err(|error| format!("cannot protect compiler-closure capability: {error}"))?;
        let mut writer = image
            .try_clone()
            .map_err(|error| format!("cannot clone compiler-closure capability: {error}"))?;
        writer
            .write_all(&bytes)
            .and_then(|()| writer.flush())
            .and_then(|()| writer.sync_all())
            .map_err(|error| format!("cannot write compiler-closure capability: {error}"))?;
        rustix::fs::fcntl_add_seals(
            &image,
            rustix::fs::SealFlags::WRITE
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::SHRINK,
        )
        .and_then(|()| rustix::fs::fcntl_add_seals(&image, rustix::fs::SealFlags::SEAL))
        .map_err(|error| format!("cannot seal compiler-closure capability: {error}"))?;
        Self::from_file(image)
    }

    /// Admits an already transferred capability file.
    pub fn from_file(image: File) -> Result<Self, String> {
        let metadata = validate_file(&image)?;
        let closure = decode(&read_exact_image(&image)?)?;
        let admitted = Self {
            closure,
            image,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits the exact descriptor inherited at the canonical child capability number.
    pub fn from_inherited_child() -> Result<Self, String> {
        Self::from_inherited_at(COMPILER_CLOSURE_CHILD_FD_V1)
    }

    /// Admits an inherited descriptor after retaining a private close-on-exec duplicate.
    pub fn from_inherited_at(child_fd: RawFd) -> Result<Self, String> {
        if child_fd < 3 {
            return Err("compiler-closure child descriptor overlaps stdio".to_owned());
        }
        // SAFETY: the descriptor is borrowed only for fcntl duplication and remains owned by the
        // current process.
        let inherited = unsafe { BorrowedFd::borrow_raw(child_fd) };
        let flags = rustix::io::fcntl_getfd(inherited).map_err(|error| {
            format!("cannot inspect inherited compiler-closure descriptor {child_fd}: {error}")
        })?;
        if flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(
                "inherited compiler-closure descriptor is unexpectedly close-on-exec".to_owned(),
            );
        }
        let retained = rustix::io::fcntl_dupfd_cloexec(inherited, 3).map_err(|error| {
            format!("cannot retain inherited compiler-closure descriptor {child_fd}: {error}")
        })?;
        Self::from_file(File::from(retained))
    }

    /// Returns the exact canonical closure carried by this descriptor.
    pub const fn closure(&self) -> CompilerClosureV2 {
        self.closure
    }

    /// Revalidates object identity, seals, bytes, and canonical closure equality.
    pub fn revalidate(&self) -> Result<(), String> {
        let metadata = validate_file(&self.image)?;
        if metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err("compiler-closure capability object identity changed".to_owned());
        }
        if decode(&read_exact_image(&self.image)?)? != self.closure {
            return Err("compiler-closure capability bytes changed".to_owned());
        }
        Ok(())
    }

    /// Clones the exact sealed descriptor for one broker transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        let cloned = self
            .image
            .try_clone()
            .map_err(|error| format!("cannot clone compiler-closure capability: {error}"))?;
        rustix::io::fcntl_setfd(&cloned, rustix::io::FdFlags::CLOEXEC)
            .map_err(|error| format!("cannot protect compiler-closure descriptor: {error}"))?;
        Ok(cloned)
    }

    /// Installs this exact immutable image at a reserved descriptor for a child process.
    pub fn inherit_for_child_at(
        &self,
        command: &mut Command,
        child_fd: RawFd,
    ) -> Result<(), String> {
        self.revalidate()?;
        if child_fd < 3 {
            return Err("compiler-closure child descriptor overlaps stdio".to_owned());
        }
        // SAFETY: fcntl only probes the process-local descriptor number.
        let target = unsafe { BorrowedFd::borrow_raw(child_fd) };
        match rustix::io::fcntl_getfd(target) {
            Err(rustix::io::Errno::BADF) => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect reserved compiler-closure descriptor {child_fd}: {error}"
                ));
            }
            Ok(_) => {
                return Err(format!(
                    "reserved compiler-closure descriptor {child_fd} is already in use"
                ));
            }
        }
        let source_fd = self.image.as_raw_fd();
        let device = self.device;
        let inode = self.inode;
        // SAFETY: the image is owned by `self`, which outlives synchronous command spawning. The
        // callback performs only async-signal-safe descriptor operations.
        unsafe {
            command.pre_exec(move || {
                let source = BorrowedFd::borrow_raw(source_fd);
                if rustix::fs::fcntl_get_seals(source).map_err(std::io::Error::from)?
                    != REQUIRED_SEALS
                {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::PERM.raw_os_error(),
                    ));
                }
                let installed = rustix::io::fcntl_dupfd_cloexec(source, child_fd)
                    .map_err(std::io::Error::from)?;
                if installed.as_raw_fd() != child_fd {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::BUSY.raw_os_error(),
                    ));
                }
                let stat = rustix::fs::fstat(&installed).map_err(std::io::Error::from)?;
                if stat.st_dev != device || stat.st_ino != inode {
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

fn encode(closure: CompilerClosureV2) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(WIRE_BYTES);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&FLAGS.to_le_bytes());
    bytes.extend_from_slice(&(WIRE_BYTES as u32).to_le_bytes());
    for digest in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&closure.identity_sha256());
    let checksum = checksum(&bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), WIRE_BYTES);
    bytes
}

fn decode(bytes: &[u8]) -> Result<CompilerClosureV2, String> {
    if bytes.len() != WIRE_BYTES {
        return Err(format!(
            "compiler-closure capability has length {}, expected {WIRE_BYTES}",
            bytes.len()
        ));
    }
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err("compiler-closure capability has invalid magic".to_owned());
    }
    let mut decoder = Decoder::new(&bytes[MAGIC.len()..]);
    if decoder.u16()? != VERSION {
        return Err("compiler-closure capability has unsupported version".to_owned());
    }
    if decoder.u16()? != FLAGS {
        return Err("compiler-closure capability has unsupported flags".to_owned());
    }
    if decoder.u32()? != WIRE_BYTES as u32 {
        return Err("compiler-closure capability has noncanonical declared length".to_owned());
    }
    let cargo = decoder.array()?;
    let trampoline = decoder.array()?;
    let wrapper = decoder.array()?;
    let rustc = decoder.array()?;
    let runtime = decoder.array()?;
    let backend = decoder.array()?;
    let protocol = decoder.u16()?;
    if decoder.u16()? != 0 {
        return Err("compiler-closure capability has nonzero reserved bytes".to_owned());
    }
    let identity = decoder.array()?;
    let checksum_start = WIRE_BYTES - CHECKSUM_BYTES;
    if decoder.array::<CHECKSUM_BYTES>()? != checksum(&bytes[..checksum_start]) {
        return Err("compiler-closure capability checksum mismatch".to_owned());
    }
    if !decoder.finished() {
        return Err("compiler-closure capability has trailing bytes".to_owned());
    }
    CompilerClosureV2::from_pins_and_identity(
        cargo, trampoline, wrapper, rustc, runtime, backend, protocol, identity,
    )
    .map_err(|error| format!("compiler-closure capability is not canonical: {error}"))
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CHECKSUM_DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn validate_file(image: &File) -> Result<fs::Metadata, String> {
    let metadata = image
        .metadata()
        .map_err(|error| format!("cannot inspect compiler-closure capability: {error}"))?;
    if metadata.mode() & libc::S_IFMT != libc::S_IFREG
        || metadata.permissions().mode() & 0o777 != 0o400
        || metadata.len() != WIRE_BYTES as u64
    {
        return Err("compiler-closure capability has invalid type, mode, or length".to_owned());
    }
    if rustix::fs::fcntl_get_seals(image)
        .map_err(|error| format!("cannot inspect compiler-closure capability seals: {error}"))?
        != REQUIRED_SEALS
    {
        return Err("compiler-closure capability is not exactly immutable".to_owned());
    }
    if !rustix::io::fcntl_getfd(image)
        .map_err(|error| format!("cannot inspect compiler-closure descriptor flags: {error}"))?
        .contains(rustix::io::FdFlags::CLOEXEC)
    {
        return Err(
            "compiler-closure capability descriptor is unexpectedly inheritable".to_owned(),
        );
    }
    Ok(metadata)
}

fn read_exact_image(image: &File) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0_u8; WIRE_BYTES];
    let mut offset = 0;
    while offset < bytes.len() {
        let read = image
            .read_at(&mut bytes[offset..], offset as u64)
            .map_err(|error| format!("cannot read compiler-closure capability: {error}"))?;
        if read == 0 {
            return Err("compiler-closure capability ended before its declared length".to_owned());
        }
        offset += read;
    }
    Ok(bytes)
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| "compiler-closure capability length overflow".to_owned())?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| "compiler-closure capability is truncated".to_owned())?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.take(N)?
            .try_into()
            .map_err(|_| "compiler-closure capability is truncated".to_owned())
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    const fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsFd;

    fn closure() -> CompilerClosureV2 {
        CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap()
    }

    #[test]
    fn canonical_wire_round_trips_and_binds_every_byte() {
        let expected = closure();
        let bytes = encode(expected);
        assert_eq!(bytes.len(), WIRE_BYTES);
        assert_eq!(decode(&bytes), Ok(expected));

        for index in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[index] ^= 1;
            assert!(decode(&changed).is_err(), "wire byte {index} was not bound");
        }
    }

    #[test]
    fn canonical_wire_rejects_rechecksummed_role_aliases_and_unknown_protocols() {
        let mut aliased = encode(closure());
        let pins = HEADER_BYTES;
        aliased[pins + 32..pins + 64].copy_from_slice(&[1; 32]);
        let checksum_start = WIRE_BYTES - CHECKSUM_BYTES;
        let digest = checksum(&aliased[..checksum_start]);
        aliased[checksum_start..].copy_from_slice(&digest);
        assert!(decode(&aliased).is_err());

        let mut unknown_protocol = encode(closure());
        unknown_protocol[pins + 6 * 32..pins + 6 * 32 + 2].copy_from_slice(&2_u16.to_le_bytes());
        let digest = checksum(&unknown_protocol[..checksum_start]);
        unknown_protocol[checksum_start..].copy_from_slice(&digest);
        assert!(decode(&unknown_protocol).is_err());
    }

    #[test]
    fn sealed_image_is_exact_immutable_and_transferable() {
        let capability = CompilerClosureCapabilityV1::create(closure()).unwrap();
        assert_eq!(capability.closure(), closure());
        assert_eq!(
            rustix::fs::fcntl_get_seals(capability.image.as_fd()).unwrap(),
            REQUIRED_SEALS
        );
        assert!(capability.revalidate().is_ok());
        assert!(capability.image.set_len(0).is_err());
        let transferred = capability.try_clone_for_transfer().unwrap();
        let received = CompilerClosureCapabilityV1::from_file(transferred).unwrap();
        assert_eq!(received.closure(), closure());
    }

    #[test]
    fn ordinary_files_and_inheritable_descriptors_are_rejected() {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-closure-capability-hostile-{}",
            std::process::id()
        ));
        fs::write(&path, encode(closure())).unwrap();
        let file = File::open(&path).unwrap();
        assert!(CompilerClosureCapabilityV1::from_file(file).is_err());
        fs::remove_file(path).unwrap();

        let capability = CompilerClosureCapabilityV1::create(closure()).unwrap();
        let file = capability.try_clone_for_transfer().unwrap();
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        assert!(CompilerClosureCapabilityV1::from_file(file).is_err());
    }

    #[test]
    fn child_inherits_only_the_requested_exact_descriptor() {
        let capability = CompilerClosureCapabilityV1::create(closure()).unwrap();
        let child_fd = 511;
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(format!(
            "test $(wc -c </proc/self/fd/{child_fd}) -eq {WIRE_BYTES}"
        ));
        capability
            .inherit_for_child_at(&mut command, child_fd)
            .unwrap();
        assert!(command.status().unwrap().success());
        capability.revalidate().unwrap();
    }

    #[test]
    fn inherited_descriptor_is_retained_and_revalidated_before_use() {
        let capability = CompilerClosureCapabilityV1::create(closure()).unwrap();
        let child_fd = 511;
        let installed = rustix::io::fcntl_dupfd_cloexec(&capability.image, child_fd).unwrap();
        assert_eq!(installed.as_raw_fd(), child_fd);
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();
        let _installed = installed;

        let retained = CompilerClosureCapabilityV1::from_inherited_at(child_fd).unwrap();
        assert_eq!(retained.closure(), closure());
        retained.revalidate().unwrap();
        assert!(
            rustix::io::fcntl_getfd(&retained.image)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
}
