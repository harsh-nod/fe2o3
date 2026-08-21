//! Sealed transport for canonical protected compiler closures and rustc invocation descriptors.
//!
//! These descriptors carry coordination evidence only. They do not grant compiler, publication,
//! linking, loading, launch, or execution authority.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::fs::File;
use std::os::fd::RawFd;
use std::process::Command;

use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};

mod rustc_invocation;
mod sealed_image;

pub use rustc_invocation::{RUSTC_INVOCATION_CHILD_FD_V1, RustcInvocationCapabilityV1};
use sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const MAGIC: &[u8] = b"FE2O3-COMPILER-CLOSURE-CAPABILITY-V1\0";
const VERSION: u16 = 1;
const FLAGS: u16 = 0;
const CHECKSUM_DOMAIN: &[u8] = b"FE2O3/COMPILER-CLOSURE-CAPABILITY/CHECKSUM/V1\0";
const HEADER_BYTES: usize = MAGIC.len() + 2 + 2 + 4;
const CLOSURE_BYTES: usize = (6 * 32) + 2 + 2 + 32;
const CHECKSUM_BYTES: usize = 32;
const WIRE_BYTES: usize = HEADER_BYTES + CLOSURE_BYTES + CHECKSUM_BYTES;
const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-closure capability",
    memfd_name: "fe2o3-compiler-closure-capability-v1",
};
const LENGTH: ImageLength = ImageLength::Exact(WIRE_BYTES);

/// Reserved descriptor used to pass the protected compiler closure from its parent into a wrapper.
pub const COMPILER_CLOSURE_CHILD_FD_V1: RawFd = 199;

/// An immutable file capability containing one validated compiler closure.
pub struct CompilerClosureCapabilityV1 {
    closure: CompilerClosureV2,
    image: SealedCapabilityImage,
}

impl CompilerClosureCapabilityV1 {
    /// Creates and seals a canonical capability image.
    pub fn create(closure: CompilerClosureV2) -> Result<Self, String> {
        let bytes = encode(closure);
        let image = SealedCapabilityImage::create(&bytes, ROLE, LENGTH)?;
        let admitted = Self { closure, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an already transferred capability file.
    pub fn from_file(image: File) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        let closure = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { closure, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits the exact descriptor inherited at the canonical child capability number.
    pub fn from_inherited_child() -> Result<Self, String> {
        Self::from_inherited_at(COMPILER_CLOSURE_CHILD_FD_V1)
    }

    /// Admits an inherited descriptor after retaining a private close-on-exec duplicate.
    pub fn from_inherited_at(child_fd: RawFd) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_inherited_at(child_fd, ROLE, LENGTH)?;
        let closure = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { closure, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Returns the exact canonical closure carried by this descriptor.
    pub const fn closure(&self) -> CompilerClosureV2 {
        self.closure
    }

    /// Revalidates object identity, seals, bytes, and canonical closure equality.
    pub fn revalidate(&self) -> Result<(), String> {
        if decode(&self.image.read_exact_bytes()?)? != self.closure {
            return Err("compiler-closure capability bytes changed".to_owned());
        }
        Ok(())
    }

    /// Clones the exact sealed descriptor for one broker transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }

    /// Installs this exact immutable image at a reserved descriptor for a child process.
    pub fn inherit_for_child_at(
        &self,
        command: &mut Command,
        child_fd: RawFd,
    ) -> Result<(), String> {
        self.revalidate()?;
        self.image.inherit_for_child_at(command, child_fd)
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
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write;
    use std::os::fd::{AsFd, AsRawFd};
    use std::os::unix::fs::{FileExt, MetadataExt, PermissionsExt};
    use std::sync::Mutex;

    use fe2o3_rustc_invocation::{
        CompileEnvironmentV2, MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV2,
        RustcInvocationDescriptorV3, RustcUnitV2, encode_descriptor_v3,
    };

    use crate::sealed_image::REQUIRED_SEALS;

    static FD_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn closure() -> CompilerClosureV2 {
        CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap()
    }

    fn invocation() -> RustcInvocationDescriptorV3 {
        let rustc = RustcUnitV2::new(
            "/workspace/fe2o3",
            vec![
                "/opt/fe2o3/rustc".into(),
                "--crate-name".into(),
                "capability_fixture".into(),
                "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
            ],
        )
        .unwrap();
        let environment = CompileEnvironmentV2::from_child_environment([
            (
                OsString::from("FE2O3_HSACO_DIR"),
                OsString::from("/workspace/fe2o3/target/fe2o3"),
            ),
            (
                OsString::from("FE2O3_TARGET"),
                OsString::from("gfx942:sramecc+:xnack-"),
            ),
        ])
        .unwrap();
        let v2 = RustcInvocationDescriptorV2::new([4; 32], [6; 32], rustc, environment).unwrap();
        RustcInvocationDescriptorV3::new(v2, closure()).unwrap()
    }

    fn sealed_file(bytes: &[u8], mode: u32, seals: rustix::fs::SealFlags, cloexec: bool) -> File {
        let file = rustix::fs::memfd_create(
            "fe2o3-capability-hostile-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .unwrap();
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .unwrap();
        let mut writer = file.try_clone().unwrap();
        writer.write_all(bytes).unwrap();
        writer.flush().unwrap();

        let mut initial_seals = seals;
        initial_seals.remove(rustix::fs::SealFlags::SEAL);
        if !initial_seals.is_empty() {
            rustix::fs::fcntl_add_seals(&file, initial_seals).unwrap();
        }
        if seals.contains(rustix::fs::SealFlags::SEAL) {
            rustix::fs::fcntl_add_seals(&file, rustix::fs::SealFlags::SEAL).unwrap();
        }
        if !cloexec {
            rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        }
        file
    }

    fn temporary_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fe2o3-compiler-capability-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
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
            rustix::fs::fcntl_get_seals(capability.image.as_file().as_fd()).unwrap(),
            REQUIRED_SEALS
        );
        assert!(capability.revalidate().is_ok());
        assert!(capability.image.as_file().set_len(0).is_err());
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
        let _guard = FD_TEST_LOCK.lock().unwrap();
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
        let _guard = FD_TEST_LOCK.lock().unwrap();
        let capability = CompilerClosureCapabilityV1::create(closure()).unwrap();
        let child_fd = 511;
        let installed =
            rustix::io::fcntl_dupfd_cloexec(capability.image.as_file(), child_fd).unwrap();
        assert_eq!(installed.as_raw_fd(), child_fd);
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();

        let retained = CompilerClosureCapabilityV1::from_inherited_at(child_fd).unwrap();
        drop(installed);
        assert_eq!(retained.closure(), closure());
        retained.revalidate().unwrap();
        assert!(
            rustix::io::fcntl_getfd(retained.image.as_file())
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }

    #[test]
    fn invocation_image_is_exact_immutable_and_transfers_the_same_object() {
        let expected = invocation();
        let bytes = encode_descriptor_v3(&expected).unwrap();
        let capability = RustcInvocationCapabilityV1::create(expected.clone()).unwrap();
        assert_eq!(capability.descriptor(), &expected);
        assert_eq!(
            capability.image.as_file().metadata().unwrap().mode(),
            libc::S_IFREG | 0o400
        );
        assert_eq!(
            rustix::fs::fcntl_get_seals(capability.image.as_file()).unwrap(),
            REQUIRED_SEALS
        );
        assert!(capability.image.as_file().set_len(0).is_err());
        assert!(capability.image.as_file().write_at(&[0], 0).is_err());

        let transferred = capability.try_clone_for_transfer().unwrap();
        let first_identity = rustix::fs::fstat(&transferred).unwrap();
        let received = RustcInvocationCapabilityV1::from_file(transferred).unwrap();
        let retransferred = received.try_clone_for_transfer().unwrap();
        let second_identity = rustix::fs::fstat(&retransferred).unwrap();
        assert_eq!(first_identity.st_dev, second_identity.st_dev);
        assert_eq!(first_identity.st_ino, second_identity.st_ino);
        assert_eq!(retransferred.metadata().unwrap().len(), bytes.len() as u64);
        assert_eq!(received.descriptor(), &expected);
    }

    #[test]
    fn invocation_admission_requires_exact_mode_seals_cloexec_and_bound() {
        let bytes = encode_descriptor_v3(&invocation()).unwrap();

        let path = temporary_path("ordinary-invocation");
        std::fs::write(&path, &bytes).unwrap();
        assert!(RustcInvocationCapabilityV1::from_file(File::open(&path).unwrap()).is_err());
        std::fs::remove_file(path).unwrap();

        for mode in [0o000, 0o600, 0o1400] {
            let file = sealed_file(&bytes, mode, REQUIRED_SEALS, true);
            assert!(RustcInvocationCapabilityV1::from_file(file).is_err());
        }
        let incomplete_seals = rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK;
        assert!(
            RustcInvocationCapabilityV1::from_file(sealed_file(
                &bytes,
                0o400,
                incomplete_seals,
                true,
            ))
            .is_err()
        );
        assert!(
            RustcInvocationCapabilityV1::from_file(sealed_file(
                &bytes,
                0o400,
                REQUIRED_SEALS,
                false,
            ))
            .is_err()
        );
        assert!(
            RustcInvocationCapabilityV1::from_file(sealed_file(
                &vec![0; MAX_DESCRIPTOR_BYTES_V3 + 1],
                0o400,
                REQUIRED_SEALS,
                true,
            ))
            .is_err()
        );
    }

    #[test]
    fn invocation_rejects_every_truncation_and_header_bit_mutation() {
        let bytes = encode_descriptor_v3(&invocation()).unwrap();
        for length in 0..bytes.len() {
            let file = sealed_file(&bytes[..length], 0o400, REQUIRED_SEALS, true);
            assert!(
                RustcInvocationCapabilityV1::from_file(file).is_err(),
                "truncation at {length} bytes was admitted"
            );
        }

        for index in 0..20 {
            for bit in 0..8 {
                let mut mutated = bytes.clone();
                mutated[index] ^= 1 << bit;
                let file = sealed_file(&mutated, 0o400, REQUIRED_SEALS, true);
                assert!(
                    RustcInvocationCapabilityV1::from_file(file).is_err(),
                    "header mutation at byte {index}, bit {bit} was admitted"
                );
            }
        }

        let mut trailing = bytes;
        trailing.push(0);
        let declared = u32::try_from(trailing.len()).unwrap();
        trailing[12..16].copy_from_slice(&declared.to_le_bytes());
        assert!(
            RustcInvocationCapabilityV1::from_file(sealed_file(
                &trailing,
                0o400,
                REQUIRED_SEALS,
                true,
            ))
            .is_err()
        );
    }

    #[test]
    fn invocation_object_identity_substitution_is_rejected() {
        let descriptor = invocation();
        let bytes = encode_descriptor_v3(&descriptor).unwrap();
        let mut capability = RustcInvocationCapabilityV1::create(descriptor).unwrap();
        capability
            .image
            .replace_file_for_test(sealed_file(&bytes, 0o400, REQUIRED_SEALS, true));
        assert!(capability.revalidate().is_err());
    }

    #[test]
    fn canonical_inherited_invocation_is_retained_after_source_close() {
        let _guard = FD_TEST_LOCK.lock().unwrap();
        assert_eq!(RUSTC_INVOCATION_CHILD_FD_V1, 199);
        let expected = invocation();
        let capability = RustcInvocationCapabilityV1::create(expected.clone()).unwrap();
        let source = capability.try_clone_for_transfer().unwrap();
        let installed =
            rustix::io::fcntl_dupfd_cloexec(&source, RUSTC_INVOCATION_CHILD_FD_V1).unwrap();
        assert_eq!(installed.as_raw_fd(), RUSTC_INVOCATION_CHILD_FD_V1);
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();
        drop(source);

        let retained = RustcInvocationCapabilityV1::from_inherited_child().unwrap();
        drop(installed);
        retained.revalidate().unwrap();
        assert_eq!(retained.descriptor(), &expected);
        assert!(
            rustix::io::fcntl_getfd(retained.image.as_file())
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }

    #[test]
    fn invocation_child_installation_uses_fd_199_and_exact_bytes() {
        let _guard = FD_TEST_LOCK.lock().unwrap();
        let descriptor = invocation();
        let bytes = encode_descriptor_v3(&descriptor).unwrap();
        let path = temporary_path("expected-invocation");
        std::fs::write(&path, &bytes).unwrap();

        let capability = RustcInvocationCapabilityV1::create(descriptor).unwrap();
        let mut command = Command::new("/usr/bin/cmp");
        command
            .arg("-s")
            .arg(format!("/proc/self/fd/{RUSTC_INVOCATION_CHILD_FD_V1}"))
            .arg(&path);
        capability.inherit_for_child(&mut command).unwrap();
        drop(capability);
        assert!(command.status().unwrap().success());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn closure_and_invocation_roles_cannot_be_confused() {
        let closure_capability = CompilerClosureCapabilityV1::create(closure()).unwrap();
        assert!(
            RustcInvocationCapabilityV1::from_file(
                closure_capability.try_clone_for_transfer().unwrap()
            )
            .is_err()
        );

        let invocation_capability = RustcInvocationCapabilityV1::create(invocation()).unwrap();
        assert!(
            CompilerClosureCapabilityV1::from_file(
                invocation_capability.try_clone_for_transfer().unwrap()
            )
            .is_err()
        );
    }
}
