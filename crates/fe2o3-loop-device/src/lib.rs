//! Narrow Linux read-only autoclear loop-device boundary.

#![cfg(target_os = "linux")]
#![deny(missing_docs, unsafe_op_in_unsafe_fn)]

use std::error::Error;
use std::ffi::c_void;
use std::fmt;
use std::fs::File;
use std::os::fd::AsRawFd as _;

use rustix::fs::{
    FileType, Mode, OFlags, ResolveFlags, SealFlags, fcntl_get_seals, fstat, major, minor, openat2,
};
use rustix::ioctl::{Getter, Ioctl, IoctlOutput, Opcode, Setter};

const LOOP_CONTROL_PATH_V1: &str = "/dev/loop-control";
const LOOP_DEVICE_PREFIX_V1: &str = "/dev/loop";
const LOOP_CONTROL_MAJOR_V1: u32 = 10;
const LOOP_CONTROL_MINOR_V1: u32 = 237;
const LOOP_DEVICE_MAJOR_V1: u32 = 7;
const LOOP_CTL_GET_FREE_V1: Opcode = 0x4c82;
const LOOP_GET_STATUS64_V1: Opcode = 0x4c05;
const LOOP_CONFIGURE_V1: Opcode = 0x4c0a;
const LO_FLAGS_READ_ONLY_V1: u32 = 1;
const LO_FLAGS_AUTOCLEAR_V1: u32 = 4;
const REQUIRED_LOOP_FLAGS_V1: u32 = LO_FLAGS_READ_ONLY_V1 | LO_FLAGS_AUTOCLEAR_V1;
const CONFIGURATION_ATTEMPTS_V1: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LoopInfo64V1 {
    device: u64,
    inode: u64,
    rdevice: u64,
    offset: u64,
    size_limit: u64,
    number: u32,
    encryption_type: u32,
    encryption_key_size: u32,
    flags: u32,
    file_name: [u8; 64],
    crypt_name: [u8; 64],
    encryption_key: [u8; 32],
    init: [u64; 2],
}

impl Default for LoopInfo64V1 {
    fn default() -> Self {
        Self {
            device: 0,
            inode: 0,
            rdevice: 0,
            offset: 0,
            size_limit: 0,
            number: 0,
            encryption_type: 0,
            encryption_key_size: 0,
            flags: 0,
            file_name: [0; 64],
            crypt_name: [0; 64],
            encryption_key: [0; 32],
            init: [0; 2],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LoopConfigV1 {
    backing_fd: u32,
    block_size: u32,
    info: LoopInfo64V1,
    reserved: [u64; 8],
}

struct GetFreeLoopDeviceV1;

unsafe impl Ioctl for GetFreeLoopDeviceV1 {
    type Output = i32;

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        LOOP_CTL_GET_FREE_V1
    }

    fn as_ptr(&mut self) -> *mut c_void {
        std::ptr::null_mut()
    }

    unsafe fn output_from_ptr(
        output: IoctlOutput,
        _extract_output: *mut c_void,
    ) -> rustix::io::Result<Self::Output> {
        Ok(output)
    }
}

/// Stable category for a loop-device admission or configuration failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LoopDeviceErrorKindV1 {
    /// The backing descriptor is not one completely sealed read-only regular file.
    InvalidBacking,
    /// The loop-control or selected loop device node has an unexpected identity.
    InvalidDevice,
    /// A bounded Linux filesystem or `ioctl` operation failed.
    Io,
    /// All bounded atomic configuration attempts lost a loop-device race.
    Contended,
    /// Kernel-reported loop status differs from the requested exact configuration.
    StatusMismatch,
}

/// Error returned by the V1 loop-device boundary.
#[derive(Debug)]
pub struct LoopDeviceErrorV1 {
    kind: LoopDeviceErrorKindV1,
    message: String,
    source: Option<std::io::Error>,
}

impl LoopDeviceErrorV1 {
    /// Returns the stable failure category.
    pub const fn kind(&self) -> LoopDeviceErrorKindV1 {
        self.kind
    }
}

impl fmt::Display for LoopDeviceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(formatter, "{}: {source}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl Error for LoopDeviceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Move-only custody of one exact read-only, autoclear Linux loop device.
///
/// The loop remains configured while this value or a mount referencing it exists. Closing this
/// value requests automatic detachment as soon as the final mount is gone.
///
/// ```compile_fail
/// use fe2o3_loop_device::ReadOnlyAutoclearLoopDeviceV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ReadOnlyAutoclearLoopDeviceV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_loop_device::ReadOnlyAutoclearLoopDeviceV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ReadOnlyAutoclearLoopDeviceV1>();
/// ```
pub struct ReadOnlyAutoclearLoopDeviceV1 {
    index: u32,
    device: File,
    backing_device: u64,
    backing_inode: u64,
    backing_byte_len: u64,
}

impl fmt::Debug for ReadOnlyAutoclearLoopDeviceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadOnlyAutoclearLoopDeviceV1")
            .field("index", &self.index)
            .field("device_path", &self.device_path())
            .field("backing_byte_len", &self.backing_byte_len)
            .field("authority", &"read-only-autoclear-loop-custody")
            .finish_non_exhaustive()
    }
}

impl ReadOnlyAutoclearLoopDeviceV1 {
    /// Returns the canonical device pathname selected by Linux loop-control.
    pub fn device_path(&self) -> String {
        format!("{LOOP_DEVICE_PREFIX_V1}{}", self.index)
    }

    /// Returns the admitted backing-file length.
    pub const fn backing_byte_len(&self) -> u64 {
        self.backing_byte_len
    }

    /// Revalidates the retained device node and exact kernel loop status.
    pub fn revalidate(&self) -> Result<(), LoopDeviceErrorV1> {
        validate_loop_device(&self.device, self.index)?;
        let status = get_loop_status(&self.device)?;
        validate_loop_status(status, self.index, self.backing_device, self.backing_inode)
    }
}

/// Atomically attaches a completely sealed regular file to a free read-only loop device.
///
/// The backing descriptor must be read-capable, `CLOEXEC`, mode `0444`, nonempty, and sealed
/// against writes, growth, shrinkage, and seal changes. Configuration uses `LOOP_CONFIGURE`; the
/// older partial `LOOP_SET_FD` plus `LOOP_SET_STATUS64` sequence is never used.
pub fn attach_sealed_read_only_loop_device_v1(
    backing: &File,
) -> Result<ReadOnlyAutoclearLoopDeviceV1, LoopDeviceErrorV1> {
    let backing_status = validate_backing(backing)?;
    let control = open_device(LOOP_CONTROL_PATH_V1, "open Linux loop-control")?;
    validate_control_device(&control)?;
    let backing_fd = u32::try_from(backing.as_raw_fd()).map_err(|_| {
        invalid(
            LoopDeviceErrorKindV1::InvalidBacking,
            "sealed backing descriptor is outside the Linux loop ABI range",
        )
    })?;

    for _ in 0..CONFIGURATION_ATTEMPTS_V1 {
        let index = get_free_loop_index(&control)?;
        let path = format!("{LOOP_DEVICE_PREFIX_V1}{index}");
        let device = open_device(&path, "open selected Linux loop device")?;
        validate_loop_device(&device, index)?;
        let config = LoopConfigV1 {
            backing_fd,
            info: LoopInfo64V1 {
                flags: REQUIRED_LOOP_FLAGS_V1,
                ..LoopInfo64V1::default()
            },
            ..LoopConfigV1::default()
        };
        match configure_loop_device(&device, config) {
            Ok(()) => {
                let retained = ReadOnlyAutoclearLoopDeviceV1 {
                    index,
                    device,
                    backing_device: backing_status.st_dev,
                    backing_inode: backing_status.st_ino,
                    backing_byte_len: u64::try_from(backing_status.st_size).map_err(|_| {
                        invalid(
                            LoopDeviceErrorKindV1::InvalidBacking,
                            "sealed backing length is negative",
                        )
                    })?,
                };
                retained.revalidate()?;
                return Ok(retained);
            }
            Err(rustix::io::Errno::BUSY) => continue,
            Err(source) => {
                return Err(io_error("atomically configure Linux loop device", source));
            }
        }
    }
    Err(invalid(
        LoopDeviceErrorKindV1::Contended,
        "could not atomically acquire a free Linux loop device",
    ))
}

fn validate_backing(backing: &File) -> Result<rustix::fs::Stat, LoopDeviceErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(backing)
        .map_err(|source| io_error("inspect sealed backing descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(backing)
        .map_err(|source| io_error("inspect sealed backing status flags", source))?;
    let stat = fstat(backing).map_err(|source| io_error("inspect sealed backing", source))?;
    let access = status & OFlags::ACCMODE;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || (access != OFlags::RDONLY && access != OFlags::RDWR)
        || FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o7777 != 0o444
        || stat.st_size <= 0
    {
        return Err(invalid(
            LoopDeviceErrorKindV1::InvalidBacking,
            "loop backing is not one nonempty mode-0444 completely sealed read-capable file",
        ));
    }
    let seals = match fcntl_get_seals(backing) {
        Ok(seals) => seals,
        Err(rustix::io::Errno::INVAL) => {
            return Err(invalid(
                LoopDeviceErrorKindV1::InvalidBacking,
                "loop backing does not support Linux file seals",
            ));
        }
        Err(source) => return Err(io_error("inspect sealed backing seals", source)),
    };
    let required_seals = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
    if !seals.contains(required_seals) {
        return Err(invalid(
            LoopDeviceErrorKindV1::InvalidBacking,
            "loop backing is not sealed against every content mutation",
        ));
    }
    Ok(stat)
}

fn open_device(path: &str, operation: &'static str) -> Result<File, LoopDeviceErrorV1> {
    openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error(operation, source))
}

fn validate_control_device(control: &File) -> Result<(), LoopDeviceErrorV1> {
    let stat = fstat(control).map_err(|source| io_error("inspect Linux loop-control", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::CharacterDevice
        || stat.st_uid != 0
        || major(stat.st_rdev) != LOOP_CONTROL_MAJOR_V1
        || minor(stat.st_rdev) != LOOP_CONTROL_MINOR_V1
    {
        return Err(invalid(
            LoopDeviceErrorKindV1::InvalidDevice,
            "Linux loop-control has an unexpected device identity",
        ));
    }
    Ok(())
}

fn validate_loop_device(device: &File, index: u32) -> Result<(), LoopDeviceErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(device)
        .map_err(|source| io_error("inspect Linux loop-device descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(device)
        .map_err(|source| io_error("inspect Linux loop-device status flags", source))?;
    let stat = fstat(device).map_err(|source| io_error("inspect Linux loop device", source))?;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDWR
        || FileType::from_raw_mode(stat.st_mode) != FileType::BlockDevice
        || stat.st_uid != 0
        || stat.st_nlink != 1
        || major(stat.st_rdev) != LOOP_DEVICE_MAJOR_V1
        || minor(stat.st_rdev) != index
    {
        return Err(invalid(
            LoopDeviceErrorKindV1::InvalidDevice,
            "selected Linux loop device has an unexpected identity",
        ));
    }
    Ok(())
}

fn get_free_loop_index(control: &File) -> Result<u32, LoopDeviceErrorV1> {
    // SAFETY: LOOP_CTL_GET_FREE takes no operand and returns one nonnegative loop index.
    let index = unsafe { rustix::ioctl::ioctl(control, GetFreeLoopDeviceV1) }
        .map_err(|source| io_error("select free Linux loop device", source))?;
    u32::try_from(index).map_err(|_| {
        invalid(
            LoopDeviceErrorKindV1::InvalidDevice,
            "Linux loop-control returned a negative device index",
        )
    })
}

fn configure_loop_device(device: &File, config: LoopConfigV1) -> rustix::io::Result<()> {
    // SAFETY: LOOP_CONFIGURE reads exactly one repr(C) loop_config value matching linux/loop.h.
    let request = unsafe { Setter::<LOOP_CONFIGURE_V1, LoopConfigV1>::new(config) };
    // SAFETY: the selected descriptor was validated as a Linux major-7 loop block device.
    unsafe { rustix::ioctl::ioctl(device, request) }
}

fn get_loop_status(device: &File) -> Result<LoopInfo64V1, LoopDeviceErrorV1> {
    // SAFETY: LOOP_GET_STATUS64 writes exactly one repr(C) loop_info64 value.
    let request = unsafe { Getter::<LOOP_GET_STATUS64_V1, LoopInfo64V1>::new() };
    // SAFETY: the selected descriptor was validated as a Linux major-7 loop block device.
    unsafe { rustix::ioctl::ioctl(device, request) }
        .map_err(|source| io_error("read configured Linux loop status", source))
}

fn validate_loop_status(
    status: LoopInfo64V1,
    index: u32,
    backing_device: u64,
    backing_inode: u64,
) -> Result<(), LoopDeviceErrorV1> {
    if status.device != backing_device
        || status.inode != backing_inode
        || status.rdevice != 0
        || status.offset != 0
        || status.size_limit != 0
        || status.number != index
        || status.encryption_type != 0
        || status.encryption_key_size != 0
        || status.flags != REQUIRED_LOOP_FLAGS_V1
        || status.file_name != [0; 64]
        || status.crypt_name != [0; 64]
        || status.encryption_key != [0; 32]
        || status.init != [0; 2]
    {
        return Err(invalid(
            LoopDeviceErrorKindV1::StatusMismatch,
            "kernel loop status differs from the exact read-only autoclear request",
        ));
    }
    Ok(())
}

fn invalid(kind: LoopDeviceErrorKindV1, message: impl Into<String>) -> LoopDeviceErrorV1 {
    LoopDeviceErrorV1 {
        kind,
        message: message.into(),
        source: None,
    }
}

fn io_error(operation: &'static str, source: rustix::io::Errno) -> LoopDeviceErrorV1 {
    LoopDeviceErrorV1 {
        kind: LoopDeviceErrorKindV1::Io,
        message: operation.to_owned(),
        source: Some(source.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::mem::{offset_of, size_of};
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[test]
    fn linux_loop_abi_layout_is_exact() {
        assert_eq!(size_of::<LoopInfo64V1>(), 232);
        assert_eq!(offset_of!(LoopInfo64V1, flags), 52);
        assert_eq!(offset_of!(LoopInfo64V1, file_name), 56);
        assert_eq!(offset_of!(LoopInfo64V1, init), 216);
        assert_eq!(size_of::<LoopConfigV1>(), 304);
        assert_eq!(offset_of!(LoopConfigV1, info), 8);
        assert_eq!(offset_of!(LoopConfigV1, reserved), 240);
        assert_eq!(LOOP_CTL_GET_FREE_V1, 0x4c82);
        assert_eq!(LOOP_GET_STATUS64_V1, 0x4c05);
        assert_eq!(LOOP_CONFIGURE_V1, 0x4c0a);
        assert_eq!(REQUIRED_LOOP_FLAGS_V1, 5);
    }

    #[test]
    fn ordinary_files_cannot_become_loop_backing() {
        let mut backing = tempfile::NamedTempFile::new().unwrap();
        backing.write_all(&[0_u8; 4096]).unwrap();
        backing
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o444))
            .unwrap();
        assert_eq!(
            attach_sealed_read_only_loop_device_v1(backing.as_file())
                .unwrap_err()
                .kind(),
            LoopDeviceErrorKindV1::InvalidBacking
        );
    }

    #[test]
    #[ignore = "requires effective root and Linux loop-control"]
    fn root_attaches_and_autoclears_a_sealed_memfd() {
        if rustix::process::geteuid().as_raw() != 0 {
            return;
        }
        let descriptor = rustix::fs::memfd_create(
            c"fe2o3-loop-device-test-v1",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap();
        let mut backing = File::from(descriptor);
        backing.write_all(&[0_u8; 4096]).unwrap();
        rustix::fs::fchmod(&backing, Mode::from_raw_mode(0o444)).unwrap();
        rustix::fs::fcntl_add_seals(
            &backing,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
        )
        .unwrap();
        let device = attach_sealed_read_only_loop_device_v1(&backing).unwrap();
        assert_eq!(device.backing_byte_len(), 4096);
        device.revalidate().unwrap();
    }
}
