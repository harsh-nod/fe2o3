use std::path::Path;

use fe2o3_kfd_uapi::{AMDKFD_IOC_GET_VERSION, KfdIoctlGetVersionArgs, KfdUapiVersion};
use rustix::fd::OwnedFd;
use rustix::fs::{FileType, Mode, OFlags};
use rustix::ioctl::{Opcode, Updater};

use crate::{KfdAdapterError, KfdNodeObservation, OpenedKfd};

const GET_VERSION_OPCODE: Opcode = AMDKFD_IOC_GET_VERSION as Opcode;

pub(super) fn open_kfd(path: &Path) -> Result<OpenedKfd, KfdAdapterError> {
    let fd = rustix::fs::open(
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| KfdAdapterError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    let stat = rustix::fs::fstat(&fd).map_err(|source| KfdAdapterError::InspectDevice {
        path: path.to_path_buf(),
        source,
    })?;
    if !FileType::from_raw_mode(stat.st_mode).is_char_device() {
        return Err(KfdAdapterError::NotCharacterDevice(path.to_path_buf()));
    }

    Ok(OpenedKfd {
        fd,
        path: path.to_path_buf(),
        node: KfdNodeObservation {
            file_system_device: stat.st_dev,
            inode: stat.st_ino,
            character_device: stat.st_rdev,
        },
        opener_pid: std::process::id(),
        not_sync: core::marker::PhantomData,
    })
}

pub(super) fn observe_uapi(fd: &OwnedFd) -> Result<KfdUapiVersion, KfdAdapterError> {
    let mut output = KfdIoctlGetVersionArgs::zeroed();
    // SAFETY: `GET_VERSION_OPCODE` is pinned to the reviewed KFD UAPI schema,
    // and `output` is initialized C-layout storage whose size, alignment,
    // offsets, and request encoding have golden tests in `fe2o3-kfd-uapi`.
    // The owned fd and exclusive borrow of `output` remain live for the call.
    let request = unsafe { Updater::<GET_VERSION_OPCODE, _>::new(&mut output) };
    // SAFETY: The request's opcode/output contract is established above. A
    // successful result is still only a contracted kernel observation and is
    // separately subjected to exact version admission.
    unsafe { rustix::ioctl::ioctl(fd, request) }.map_err(KfdAdapterError::GetVersion)?;
    Ok(output.reported_version())
}
