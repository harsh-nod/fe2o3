use std::fs;
use std::path::{Path, PathBuf};

use fe2o3_drm_uapi::{
    AMDGPU_DRM_DRIVER_NAME, DRM_IOCTL_AMDGPU_INFO, DRM_IOCTL_VERSION, DrmAmdgpuDeviceIdentityV1,
    DrmAmdgpuInfo, DrmVersion,
};
use fe2o3_kfd_uapi::{
    AMDKFD_IOC_GET_PROCESS_APERTURES_NEW, AMDKFD_IOC_GET_VERSION, AMDKFD_IOC_SET_XNACK_MODE,
    KfdIoctlGetProcessAperturesNewArgs, KfdIoctlGetVersionArgs, KfdIoctlSetXnackModeArgs,
    KfdProcessDeviceApertures, KfdUapiVersion,
};
use fe2o3_runtime_model::DeviceNodeV1;
use rustix::fd::OwnedFd;
use rustix::fs::{FileType, Mode, OFlags};
use rustix::ioctl::{Opcode, Updater};

use crate::device::{
    DeviceBindingError, DrmIdentityObservation, MAX_PROCESS_APERTURES_V1, OpenedRender,
    PROCESS_APERTURE_QUERY_CAPACITY_V1, ProcessIncarnationObservation, RenderDescriptorObservation,
};
use crate::{KfdAdapterError, KfdNodeObservation, OpenedKfd};

const GET_VERSION_OPCODE: Opcode = AMDKFD_IOC_GET_VERSION as Opcode;
const GET_APERTURES_OPCODE: Opcode = AMDKFD_IOC_GET_PROCESS_APERTURES_NEW as Opcode;
const SET_XNACK_OPCODE: Opcode = AMDKFD_IOC_SET_XNACK_MODE as Opcode;
const DRM_VERSION_OPCODE: Opcode = DRM_IOCTL_VERSION as Opcode;
const DRM_INFO_OPCODE: Opcode = DRM_IOCTL_AMDGPU_INFO as Opcode;
const DRM_DEVICE_MAJOR: u32 = 226;
const MAX_DRM_DRIVER_NAME_BYTES: usize = 32;
const MAX_PROC_STAT_BYTES: usize = 4096;
const KFD_SYSFS_DEVICE: &str = "/sys/devices/virtual/kfd/kfd";
const KFD_SYSFS_CLASS: &str = "/sys/class/kfd/kfd";
const PROCESS_STAT: &str = "/proc/self/stat";
const MOUNT_NAMESPACE: &str = "/proc/self/ns/mnt";

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

fn binding_syscall(operation: &'static str, source: rustix::io::Errno) -> DeviceBindingError {
    DeviceBindingError::Syscall { operation, source }
}

fn binding_io(
    operation: &'static str,
    path: impl Into<PathBuf>,
    source: std::io::Error,
) -> DeviceBindingError {
    DeviceBindingError::Io {
        operation,
        path: path.into(),
        source,
    }
}

pub(super) fn observe_process_incarnation()
-> Result<ProcessIncarnationObservation, DeviceBindingError> {
    let bytes =
        fs::read(PROCESS_STAT).map_err(|source| binding_io("read", PROCESS_STAT, source))?;
    if bytes.is_empty() || bytes.len() > MAX_PROC_STAT_BYTES {
        return Err(DeviceBindingError::ProcessIncarnationChanged);
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| DeviceBindingError::ProcessIncarnationChanged)?;
    let close = text
        .rfind(')')
        .ok_or(DeviceBindingError::ProcessIncarnationChanged)?;
    let open = text[..close]
        .find('(')
        .ok_or(DeviceBindingError::ProcessIncarnationChanged)?;
    let reported_pid = text[..open]
        .trim()
        .parse::<u32>()
        .map_err(|_| DeviceBindingError::ProcessIncarnationChanged)?;
    let pid = std::process::id();
    if reported_pid != pid {
        return Err(DeviceBindingError::ProcessIncarnationChanged);
    }
    let start_time_ticks = text[close + 1..]
        .split_ascii_whitespace()
        .nth(19)
        .ok_or(DeviceBindingError::ProcessIncarnationChanged)?
        .parse::<u64>()
        .map_err(|_| DeviceBindingError::ProcessIncarnationChanged)?;
    if start_time_ticks == 0 {
        return Err(DeviceBindingError::ProcessIncarnationChanged);
    }
    let mount = fs::metadata(MOUNT_NAMESPACE)
        .map_err(|source| binding_io("inspect", MOUNT_NAMESPACE, source))?;
    use std::os::unix::fs::MetadataExt;
    Ok(ProcessIncarnationObservation {
        pid,
        start_time_ticks,
        mount_namespace_device: mount.dev(),
        mount_namespace_inode: mount.ino(),
    })
}

fn descriptor_observation(stat: &rustix::fs::Stat) -> RenderDescriptorObservation {
    RenderDescriptorObservation {
        file_system_device: stat.st_dev,
        inode: stat.st_ino,
        character_device: stat.st_rdev,
        major: rustix::fs::major(stat.st_rdev),
        minor: rustix::fs::minor(stat.st_rdev),
    }
}

pub(super) fn validate_kfd_descriptor_and_sysfs(
    fd: &OwnedFd,
    expected: KfdNodeObservation,
) -> Result<DeviceNodeV1, DeviceBindingError> {
    revalidate_descriptor(fd, expected, "KFD")?;
    let stat = rustix::fs::fstat(fd).map_err(|source| binding_syscall("KFD fstat", source))?;
    let major = rustix::fs::major(stat.st_rdev);
    let minor = rustix::fs::minor(stat.st_rdev);
    if major == 0 || minor != 0 {
        return Err(DeviceBindingError::KfdDescriptorMismatch);
    }
    let char_path = PathBuf::from(format!("/sys/dev/char/{major}:{minor}"));
    let char_target = fs::canonicalize(&char_path)
        .map_err(|source| binding_io("canonicalize", char_path.clone(), source))?;
    let class_target = fs::canonicalize(KFD_SYSFS_CLASS)
        .map_err(|source| binding_io("canonicalize", KFD_SYSFS_CLASS, source))?;
    if char_target != Path::new(KFD_SYSFS_DEVICE) || class_target != char_target {
        return Err(DeviceBindingError::KfdSysfsMismatch);
    }
    let dev_path = char_target.join("dev");
    let dev = fs::read_to_string(&dev_path)
        .map_err(|source| binding_io("read", dev_path.clone(), source))?;
    if dev != format!("{major}:{minor}\n") {
        return Err(DeviceBindingError::KfdSysfsMismatch);
    }
    let uevent_path = char_target.join("uevent");
    let uevent = fs::read_to_string(&uevent_path)
        .map_err(|source| binding_io("read", uevent_path.clone(), source))?;
    let expected_uevent = format!("MAJOR={major}\nMINOR=0\nDEVNAME=kfd\n");
    if uevent != expected_uevent {
        return Err(DeviceBindingError::KfdSysfsMismatch);
    }
    Ok(DeviceNodeV1 { major, minor })
}

pub(super) fn revalidate_descriptor(
    fd: &OwnedFd,
    expected: KfdNodeObservation,
    operation: &'static str,
) -> Result<(), DeviceBindingError> {
    let stat = rustix::fs::fstat(fd).map_err(|source| binding_syscall(operation, source))?;
    if !FileType::from_raw_mode(stat.st_mode).is_char_device()
        || stat.st_dev != expected.file_system_device()
        || stat.st_ino != expected.inode()
        || stat.st_rdev != expected.character_device()
    {
        return Err(DeviceBindingError::KfdDescriptorMismatch);
    }
    Ok(())
}

pub(super) fn revalidate_render_descriptor(
    fd: &OwnedFd,
    expected: RenderDescriptorObservation,
) -> Result<(), DeviceBindingError> {
    let stat = rustix::fs::fstat(fd).map_err(|source| binding_syscall("render fstat", source))?;
    if !FileType::from_raw_mode(stat.st_mode).is_char_device()
        || descriptor_observation(&stat) != expected
    {
        return Err(DeviceBindingError::RenderDescriptorMismatch);
    }
    Ok(())
}

pub(super) fn open_and_observe_render(minor: u16) -> Result<OpenedRender, DeviceBindingError> {
    let path = PathBuf::from(format!("/dev/dri/renderD{minor}"));
    let fd = rustix::fs::open(
        &path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| binding_syscall("open render node", source))?;
    let stat = rustix::fs::fstat(&fd).map_err(|source| binding_syscall("render fstat", source))?;
    if !FileType::from_raw_mode(stat.st_mode).is_char_device() {
        return Err(DeviceBindingError::RenderDescriptorMismatch);
    }
    let descriptor = descriptor_observation(&stat);
    if descriptor.major() != DRM_DEVICE_MAJOR || descriptor.minor() != u32::from(minor) {
        return Err(DeviceBindingError::RenderDescriptorMismatch);
    }
    let drm = observe_drm_identity(&fd)?;
    Ok(OpenedRender {
        fd,
        path,
        descriptor,
        drm,
    })
}

fn observe_drm_identity(fd: &OwnedFd) -> Result<DrmIdentityObservation, DeviceBindingError> {
    let mut name = [0_u8; MAX_DRM_DRIVER_NAME_BYTES];
    let mut version = DrmVersion::zeroed();
    version.name_len = name.len() as u64;
    version.name = name.as_mut_ptr() as usize as u64;
    // SAFETY: the opcode and LP64 C layout are pinned by fe2o3-drm-uapi. The
    // initialized fixed buffer remains exclusively borrowed and live.
    let request = unsafe { Updater::<DRM_VERSION_OPCODE, _>::new(&mut version) };
    // SAFETY: the reviewed nested-pointer contract above remains live for the call.
    unsafe { rustix::ioctl::ioctl(fd, request) }
        .map_err(|source| binding_syscall("DRM_VERSION", source))?;
    let name_len =
        usize::try_from(version.name_len).map_err(|_| DeviceBindingError::InvalidDrmDriverName)?;
    if name_len != AMDGPU_DRM_DRIVER_NAME.len()
        || name_len > name.len()
        || &name[..name_len] != AMDGPU_DRM_DRIVER_NAME
    {
        return Err(DeviceBindingError::InvalidDrmDriverName);
    }

    let mut acceleration = 0_u32;
    let mut acceleration_query =
        DrmAmdgpuInfo::acceleration_status((&mut acceleration as *mut u32) as usize as u64);
    // SAFETY: the opcode/query layout and 4-byte output size are pinned; the
    // initialized output remains live through the call.
    let request = unsafe { Updater::<DRM_INFO_OPCODE, _>::new(&mut acceleration_query) };
    // SAFETY: the reviewed nested-pointer contract above remains live for the call.
    unsafe { rustix::ioctl::ioctl(fd, request) }
        .map_err(|source| binding_syscall("AMDGPU_INFO_ACCEL_WORKING", source))?;

    let mut device = DrmAmdgpuDeviceIdentityV1::default();
    let mut device_query =
        DrmAmdgpuInfo::device_identity_v1((&mut device as *mut _) as usize as u64);
    // SAFETY: the opcode/query layout and 20-byte output prefix are pinned; the
    // initialized output remains live through the call.
    let request = unsafe { Updater::<DRM_INFO_OPCODE, _>::new(&mut device_query) };
    // SAFETY: the reviewed nested-pointer contract above remains live for the call.
    unsafe { rustix::ioctl::ioctl(fd, request) }
        .map_err(|source| binding_syscall("AMDGPU_INFO_DEV_INFO", source))?;

    Ok(DrmIdentityObservation {
        driver_version: version.reported_driver_version(),
        acceleration_working: acceleration,
        device,
    })
}

trait KernelContractIo {
    fn query_xnack(&mut self) -> Result<i32, DeviceBindingError>;
    fn set_xnack_disabled(&mut self) -> Result<i32, DeviceBindingError>;
    fn aperture_count(&mut self) -> Result<usize, DeviceBindingError>;
    fn fill_apertures(
        &mut self,
        capacity: usize,
    ) -> Result<(Vec<KfdProcessDeviceApertures>, usize), DeviceBindingError>;
}

struct RawKfdContract<'a> {
    fd: &'a OwnedFd,
}

impl KernelContractIo for RawKfdContract<'_> {
    fn query_xnack(&mut self) -> Result<i32, DeviceBindingError> {
        let mut args = KfdIoctlSetXnackModeArgs::query();
        // SAFETY: the opcode and 4-byte in/out layout are pinned by fe2o3-kfd-uapi.
        let request = unsafe { Updater::<SET_XNACK_OPCODE, _>::new(&mut args) };
        // SAFETY: args is initialized, exclusively borrowed, and live for the call.
        unsafe { rustix::ioctl::ioctl(self.fd, request) }
            .map_err(|source| binding_syscall("KFD query XNACK", source))?;
        Ok(args.xnack_enabled)
    }

    fn set_xnack_disabled(&mut self) -> Result<i32, DeviceBindingError> {
        let mut args = KfdIoctlSetXnackModeArgs::set(false);
        // SAFETY: the opcode and 4-byte in/out layout are pinned. The caller
        // performs the non-mutating disabled query before this request.
        let request = unsafe { Updater::<SET_XNACK_OPCODE, _>::new(&mut args) };
        // SAFETY: args is initialized, exclusively borrowed, and live for the call.
        unsafe { rustix::ioctl::ioctl(self.fd, request) }
            .map_err(|source| binding_syscall("KFD XNACK disabled no-queue barrier", source))?;
        Ok(args.xnack_enabled)
    }

    fn aperture_count(&mut self) -> Result<usize, DeviceBindingError> {
        let mut args = KfdIoctlGetProcessAperturesNewArgs::new(0, 0);
        // SAFETY: zero capacity is the pinned count-query form and does not
        // dereference the null output address under the reviewed KFD contract.
        let request = unsafe { Updater::<GET_APERTURES_OPCODE, _>::new(&mut args) };
        // SAFETY: args is initialized, exclusively borrowed, and live for the call.
        unsafe { rustix::ioctl::ioctl(self.fd, request) }
            .map_err(|source| binding_syscall("KFD aperture count", source))?;
        usize::try_from(args.num_of_nodes)
            .map_err(|_| DeviceBindingError::InvalidApertureCount(usize::MAX))
    }

    fn fill_apertures(
        &mut self,
        capacity: usize,
    ) -> Result<(Vec<KfdProcessDeviceApertures>, usize), DeviceBindingError> {
        if capacity != PROCESS_APERTURE_QUERY_CAPACITY_V1 {
            return Err(DeviceBindingError::InvalidApertureCount(capacity));
        }
        let mut output = [KfdProcessDeviceApertures::default(); PROCESS_APERTURE_QUERY_CAPACITY_V1];
        let mut args = KfdIoctlGetProcessAperturesNewArgs::new(
            output.as_mut_ptr() as usize as u64,
            PROCESS_APERTURE_QUERY_CAPACITY_V1 as u32,
        );
        // SAFETY: the opcode/layout are pinned and the initialized fixed output
        // array remains exclusively borrowed and live with one sentinel slot.
        let request = unsafe { Updater::<GET_APERTURES_OPCODE, _>::new(&mut args) };
        // SAFETY: the reviewed nested-pointer contract above remains live.
        unsafe { rustix::ioctl::ioctl(self.fd, request) }
            .map_err(|source| binding_syscall("KFD process apertures", source))?;
        let filled = usize::try_from(args.num_of_nodes)
            .map_err(|_| DeviceBindingError::InvalidApertureCount(usize::MAX))?;
        if filled > MAX_PROCESS_APERTURES_V1 {
            return Err(DeviceBindingError::InvalidApertureCount(filled));
        }
        Ok((output[..filled].to_vec(), filled))
    }
}

fn establish_xnack_with(io: &mut impl KernelContractIo) -> Result<(), DeviceBindingError> {
    if io.query_xnack()? != 0 {
        return Err(DeviceBindingError::UnsupportedXnackMode);
    }
    if io.set_xnack_disabled()? != 0 || io.query_xnack()? != 0 {
        return Err(DeviceBindingError::XnackChanged);
    }
    Ok(())
}

fn observe_apertures_with(
    io: &mut impl KernelContractIo,
) -> Result<Vec<KfdProcessDeviceApertures>, DeviceBindingError> {
    let count_before = io.aperture_count()?;
    if count_before == 0 || count_before > MAX_PROCESS_APERTURES_V1 {
        return Err(DeviceBindingError::InvalidApertureCount(count_before));
    }
    let (output, filled) = io.fill_apertures(PROCESS_APERTURE_QUERY_CAPACITY_V1)?;
    let count_after = io.aperture_count()?;
    if filled != count_before || count_after != count_before || output.len() != filled {
        return Err(DeviceBindingError::AperturesChanged);
    }
    Ok(output)
}

pub(super) fn query_xnack_mode(fd: &OwnedFd) -> Result<i32, DeviceBindingError> {
    RawKfdContract { fd }.query_xnack()
}

pub(super) fn establish_xnack_disabled_no_queue_barrier(
    fd: &OwnedFd,
) -> Result<(), DeviceBindingError> {
    establish_xnack_with(&mut RawKfdContract { fd })
}

pub(super) fn observe_process_apertures(
    fd: &OwnedFd,
) -> Result<Vec<KfdProcessDeviceApertures>, DeviceBindingError> {
    observe_apertures_with(&mut RawKfdContract { fd })
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    struct FakeKernelContract {
        xnack: Vec<i32>,
        set_calls: usize,
        counts: Vec<usize>,
        fill: Vec<KfdProcessDeviceApertures>,
        returned_fill_count: usize,
    }

    impl KernelContractIo for FakeKernelContract {
        fn query_xnack(&mut self) -> Result<i32, DeviceBindingError> {
            if self.xnack.is_empty() {
                return Err(DeviceBindingError::XnackChanged);
            }
            Ok(self.xnack.remove(0))
        }

        fn set_xnack_disabled(&mut self) -> Result<i32, DeviceBindingError> {
            self.set_calls += 1;
            Ok(0)
        }

        fn aperture_count(&mut self) -> Result<usize, DeviceBindingError> {
            if self.counts.is_empty() {
                return Err(DeviceBindingError::AperturesChanged);
            }
            Ok(self.counts.remove(0))
        }

        fn fill_apertures(
            &mut self,
            capacity: usize,
        ) -> Result<(Vec<KfdProcessDeviceApertures>, usize), DeviceBindingError> {
            assert_eq!(capacity, PROCESS_APERTURE_QUERY_CAPACITY_V1);
            Ok((self.fill.clone(), self.returned_fill_count))
        }
    }

    fn aperture(gpu_id: u32) -> KfdProcessDeviceApertures {
        KfdProcessDeviceApertures {
            gpu_id,
            ..KfdProcessDeviceApertures::default()
        }
    }

    #[test]
    fn enabled_xnack_is_rejected_without_mutation() {
        let mut fake = FakeKernelContract {
            xnack: vec![1],
            set_calls: 0,
            counts: vec![],
            fill: vec![],
            returned_fill_count: 0,
        };
        assert!(matches!(
            establish_xnack_with(&mut fake),
            Err(DeviceBindingError::UnsupportedXnackMode)
        ));
        assert_eq!(fake.set_calls, 0);
    }

    #[test]
    fn disabled_xnack_runs_set_barrier_and_requeries() {
        let mut fake = FakeKernelContract {
            xnack: vec![0, 0],
            set_calls: 0,
            counts: vec![],
            fill: vec![],
            returned_fill_count: 0,
        };
        establish_xnack_with(&mut fake).unwrap();
        assert_eq!(fake.set_calls, 1);
    }

    #[test]
    fn aperture_count_growth_and_truncation_fail_closed() {
        let mut growth = FakeKernelContract {
            xnack: vec![],
            set_calls: 0,
            counts: vec![1, 2],
            fill: vec![aperture(7)],
            returned_fill_count: 1,
        };
        assert!(matches!(
            observe_apertures_with(&mut growth),
            Err(DeviceBindingError::AperturesChanged)
        ));

        let mut truncation = FakeKernelContract {
            xnack: vec![],
            set_calls: 0,
            counts: vec![2, 2],
            fill: vec![aperture(7)],
            returned_fill_count: 1,
        };
        assert!(matches!(
            observe_apertures_with(&mut truncation),
            Err(DeviceBindingError::AperturesChanged)
        ));

        let mut saturation = FakeKernelContract {
            xnack: vec![],
            set_calls: 0,
            counts: vec![MAX_PROCESS_APERTURES_V1, MAX_PROCESS_APERTURES_V1],
            fill: (1..=PROCESS_APERTURE_QUERY_CAPACITY_V1)
                .map(|gpu_id| aperture(gpu_id as u32))
                .collect(),
            returned_fill_count: PROCESS_APERTURE_QUERY_CAPACITY_V1,
        };
        assert!(matches!(
            observe_apertures_with(&mut saturation),
            Err(DeviceBindingError::AperturesChanged)
        ));
    }
}
