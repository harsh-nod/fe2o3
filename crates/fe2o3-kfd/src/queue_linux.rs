//! Private Linux ioctl boundary for the native queue adapter foundation.
//!
//! There is deliberately no production backend yet. The memory owner does not
//! expose the typed mapped ring/control/EOP/CWSR authorities required to make
//! these calls sound. These functions keep the eventual unsafe boundary small
//! without making an fd or numeric address public.

use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{Ordering, fence};
use std::os::fd::BorrowedFd;

use fe2o3_kfd_uapi::{
    AMDKFD_IOC_CREATE_QUEUE, AMDKFD_IOC_DESTROY_QUEUE, AMDKFD_IOC_UPDATE_QUEUE,
    KFD_GFX942_DOORBELL_BYTES, KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES,
    KfdGfx942CreateQueueOutputs, KfdIoctlCreateQueueArgs, KfdIoctlDestroyQueueArgs,
    KfdIoctlUpdateQueueArgs,
};
use rustix::ioctl::{Opcode, Setter, Updater};
use rustix::mm::{Advice, MapFlags, MprotectFlags, ProtFlags};

const CREATE_QUEUE_OPCODE: Opcode = AMDKFD_IOC_CREATE_QUEUE as Opcode;
const DESTROY_QUEUE_OPCODE: Opcode = AMDKFD_IOC_DESTROY_QUEUE as Opcode;
#[allow(dead_code)]
const UPDATE_QUEUE_OPCODE: Opcode = AMDKFD_IOC_UPDATE_QUEUE as Opcode;

#[derive(Debug)]
pub(super) enum LinuxDoorbellErrorV1 {
    ProcessChanged,
    UnsupportedPageSize(usize),
    InvalidObservation(&'static str),
    Syscall {
        operation: &'static str,
        source: rustix::io::Errno,
    },
    #[cfg(feature = "live-validation")]
    IsolationRequired,
    #[cfg(feature = "live-validation")]
    ChildProbe(&'static str),
    #[cfg(feature = "live-validation")]
    MappingInherited,
}

impl core::fmt::Display for LinuxDoorbellErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ProcessChanged => formatter.write_str("doorbell mapping process changed"),
            Self::UnsupportedPageSize(size) => {
                write!(formatter, "unsupported doorbell host page size {size}")
            }
            Self::InvalidObservation(field) => {
                write!(formatter, "invalid doorbell mapping observation: {field}")
            }
            Self::Syscall { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            #[cfg(feature = "live-validation")]
            Self::IsolationRequired => formatter.write_str(
                "doorbell DONTFORK child probe requires an isolated single-threaded process",
            ),
            #[cfg(feature = "live-validation")]
            Self::ChildProbe(detail) => write!(formatter, "doorbell child probe failed: {detail}"),
            #[cfg(feature = "live-validation")]
            Self::MappingInherited => {
                formatter.write_str("doorbell VMA was inherited despite MADV_DONTFORK")
            }
        }
    }
}

impl std::error::Error for LinuxDoorbellErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DoorbellMmapPlanV1 {
    encoded_slice_offset: u64,
    queue_byte_offset: u64,
    slice_bytes: usize,
}

fn doorbell_mmap_plan(
    outputs: KfdGfx942CreateQueueOutputs,
) -> Result<DoorbellMmapPlanV1, LinuxDoorbellErrorV1> {
    let page_size = rustix::param::page_size();
    let slice_bytes = usize::try_from(KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES)
        .map_err(|_| LinuxDoorbellErrorV1::InvalidObservation("slice length"))?;
    if page_size != 4096 || !slice_bytes.is_multiple_of(page_size) {
        return Err(LinuxDoorbellErrorV1::UnsupportedPageSize(page_size));
    }
    let observation = outputs.doorbell_offset();
    let encoded_slice_offset = observation.encoded_process_slice_offset();
    let queue_byte_offset = observation.in_process_byte_offset();
    if !encoded_slice_offset.is_multiple_of(KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES)
        || queue_byte_offset
            .checked_add(KFD_GFX942_DOORBELL_BYTES)
            .is_none_or(|end| end > KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES)
        || !queue_byte_offset.is_multiple_of(KFD_GFX942_DOORBELL_BYTES)
        || observation.raw() != (encoded_slice_offset | queue_byte_offset)
    {
        return Err(LinuxDoorbellErrorV1::InvalidObservation(
            "encoded whole-slice offset",
        ));
    }
    Ok(DoorbellMmapPlanV1 {
        encoded_slice_offset,
        queue_byte_offset,
        slice_bytes,
    })
}

/// Non-Clone, non-address-exposing ownership of one complete process slice.
pub(super) struct LinuxDoorbellSliceV1 {
    address: NonNull<c_void>,
    plan: DoorbellMmapPlanV1,
    opener_pid: u32,
    active: bool,
}

impl LinuxDoorbellSliceV1 {
    pub(super) fn map(
        kfd: BorrowedFd<'_>,
        outputs: KfdGfx942CreateQueueOutputs,
        opener_pid: u32,
    ) -> Result<Self, LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        let plan = doorbell_mmap_plan(outputs)?;
        // SAFETY: the admitted KFD output fixes the mmap type/GPU hash and
        // complete 8192-byte slice offset. PROT_NONE prevents MMIO access
        // before the mandatory DONTFORK ordering point.
        let mapped = unsafe {
            rustix::mm::mmap(
                core::ptr::null_mut(),
                plan.slice_bytes,
                ProtFlags::empty(),
                MapFlags::SHARED,
                kfd,
                plan.encoded_slice_offset,
            )
        }
        .map_err(|source| LinuxDoorbellErrorV1::Syscall {
            operation: "mmap complete KFD doorbell slice",
            source,
        })?;
        let Some(address) = NonNull::new(mapped) else {
            // SAFETY: this is the exact successful mapping. Returning address
            // zero is outside the admitted process profile.
            if unsafe { rustix::mm::munmap(mapped, plan.slice_bytes) }.is_err() {
                std::process::abort();
            }
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell VMA address",
            ));
        };
        let cleanup_or_abort = || {
            // SAFETY: no MMIO access was enabled and the exact VMA is owned.
            if unsafe { rustix::mm::munmap(address.as_ptr(), plan.slice_bytes) }.is_err() {
                std::process::abort();
            }
        };
        // SAFETY: the exact mapping is exclusively owned and still PROT_NONE.
        if let Err(source) = unsafe {
            rustix::mm::madvise(address.as_ptr(), plan.slice_bytes, Advice::LinuxDontFork)
        } {
            cleanup_or_abort();
            return Err(LinuxDoorbellErrorV1::Syscall {
                operation: "madvise doorbell MADV_DONTFORK",
                source,
            });
        }
        // SAFETY: DONTFORK is installed, no reference exists, and the mapping
        // covers exactly the reviewed process slice. No safe MMIO accessor is
        // exposed by this capability.
        if let Err(source) = unsafe {
            rustix::mm::mprotect(
                address.as_ptr(),
                plan.slice_bytes,
                MprotectFlags::READ | MprotectFlags::WRITE,
            )
        } {
            cleanup_or_abort();
            return Err(LinuxDoorbellErrorV1::Syscall {
                operation: "mprotect doorbell read/write",
                source,
            });
        }
        Ok(Self {
            address,
            plan,
            opener_pid,
            active: true,
        })
    }

    pub(super) const fn slice_bytes(&self) -> usize {
        self.plan.slice_bytes
    }

    pub(super) const fn queue_byte_offset(&self) -> u64 {
        self.plan.queue_byte_offset
    }

    pub(super) fn store_packet_id_release(
        &mut self,
        packet_id: u64,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !self.active {
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell store state",
            ));
        }
        let offset = usize::try_from(self.plan.queue_byte_offset)
            .map_err(|_| LinuxDoorbellErrorV1::InvalidObservation("doorbell store offset"))?;
        let end = offset.checked_add(core::mem::size_of::<u64>()).ok_or(
            LinuxDoorbellErrorV1::InvalidObservation("doorbell store range"),
        )?;
        if end > self.plan.slice_bytes || !offset.is_multiple_of(core::mem::align_of::<u64>()) {
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell store range",
            ));
        }
        // This is one deliberately narrow CPU-memory/MMIO ordering boundary:
        // release-published packet bytes precede the WC MMIO notification.
        fence(Ordering::Release);
        #[cfg(target_arch = "x86_64")]
        // SAFETY: SFENCE has no memory operand and is universally available on
        // the admitted x86_64 platform.
        unsafe {
            core::arch::x86_64::_mm_sfence();
        }
        // SAFETY: the retained capability uniquely owns the live complete
        // process slice, the checked offset selects its exact aligned 8-byte
        // queue doorbell, and no pointer or reference escapes this call.
        unsafe {
            core::ptr::write_volatile(
                self.address.as_ptr().cast::<u8>().add(offset).cast::<u64>(),
                packet_id.to_le(),
            );
        }
        Ok(())
    }

    pub(super) fn release(mut self) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !self.active {
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell release state",
            ));
        }
        // SAFETY: the exact mapping remains linearly owned and no MMIO pointer
        // or reference can escape the capability.
        unsafe { rustix::mm::munmap(self.address.as_ptr(), self.plan.slice_bytes) }.map_err(
            |source| LinuxDoorbellErrorV1::Syscall {
                operation: "munmap complete KFD doorbell slice",
                source,
            },
        )?;
        self.active = false;
        Ok(())
    }

    #[cfg(feature = "live-validation")]
    pub(super) fn verify_dontfork_child_negative(&self) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != std::process::id() || !self.active {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        let tasks = std::fs::read_dir("/proc/self/task")
            .map_err(|_| LinuxDoorbellErrorV1::ChildProbe("read /proc/self/task"))?
            .take(2)
            .count();
        if tasks != 1 {
            return Err(LinuxDoorbellErrorV1::IsolationRequired);
        }
        // SAFETY: the isolated child only probes VMA existence and exits. It
        // never reads or writes MMIO and the parent waits synchronously.
        match unsafe { rustix::runtime::kernel_fork() }.map_err(|source| {
            LinuxDoorbellErrorV1::Syscall {
                operation: "fork doorbell DONTFORK probe",
                source,
            }
        })? {
            rustix::runtime::Fork::Child(_) => {
                let mut residency = [0_u8; 2];
                // SAFETY: mincore does not dereference the absent child VMA.
                let result = unsafe {
                    mincore(
                        self.address.as_ptr(),
                        self.plan.slice_bytes,
                        residency.as_mut_ptr(),
                    )
                };
                let code =
                    if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(12) {
                        0
                    } else if result == 0 {
                        1
                    } else {
                        2
                    };
                rustix::runtime::exit_group(code);
            }
            rustix::runtime::Fork::ParentOf(child) => {
                let (_, status) =
                    rustix::process::waitpid(Some(child), rustix::process::WaitOptions::empty())
                        .map_err(|source| LinuxDoorbellErrorV1::Syscall {
                            operation: "wait doorbell DONTFORK probe",
                            source,
                        })?
                        .ok_or(LinuxDoorbellErrorV1::ChildProbe("child was not waitable"))?;
                match status.exit_status() {
                    Some(0) => Ok(()),
                    Some(1) => Err(LinuxDoorbellErrorV1::MappingInherited),
                    _ => Err(LinuxDoorbellErrorV1::ChildProbe("child mincore protocol")),
                }
            }
        }
    }
}

impl Drop for LinuxDoorbellSliceV1 {
    fn drop(&mut self) {
        // No implicit munmap, MMIO store, or queue operation. Ambiguous or
        // still-live mappings remain inaccessible until process teardown.
    }
}

#[cfg(feature = "live-validation")]
unsafe extern "C" {
    fn mincore(address: *mut c_void, length: usize, residency: *mut u8) -> i32;
}

/// Issues one CREATE_QUEUE call through an exclusively borrowed C-layout
/// record. The returned record and errno remain untrusted.
pub fn create_queue(
    kfd: BorrowedFd<'_>,
    args: &mut KfdIoctlCreateQueueArgs,
) -> Result<(), rustix::io::Errno> {
    // SAFETY: the reviewed x86_64 KFD 1.18 opcode and record layout are fixed
    // by fe2o3-kfd-uapi. The initialized record remains exclusively borrowed
    // for the complete ioctl. Kernel-written fields remain untrusted.
    let request = unsafe { Updater::<CREATE_QUEUE_OPCODE, _>::new(args) };
    // SAFETY: request construction establishes the lifetime and exclusive
    // output borrow. The caller still has to validate every returned field.
    unsafe { rustix::ioctl::ioctl(kfd, request) }.map(|_| ())
}

/// Issues one UPDATE_QUEUE call over a fully initialized input record.
#[allow(dead_code)]
pub fn update_queue(
    kfd: BorrowedFd<'_>,
    args: &KfdIoctlUpdateQueueArgs,
) -> Result<(), rustix::io::Errno> {
    // SAFETY: the exact write-only opcode and initialized input lifetime are
    // fixed here. Numeric queue/address fields are not authority by themselves.
    let request = unsafe { Setter::<UPDATE_QUEUE_OPCODE, _>::new(args) };
    // SAFETY: the setter borrows the initialized record for the complete call.
    unsafe { rustix::ioctl::ioctl(kfd, request) }.map(|_| ())
}

/// Issues one DESTROY_QUEUE call. The in/out record remains untrusted even
/// though the admitted driver profile does not define a useful output field.
pub fn destroy_queue(
    kfd: BorrowedFd<'_>,
    args: &mut KfdIoctlDestroyQueueArgs,
) -> Result<(), rustix::io::Errno> {
    // SAFETY: the reviewed x86_64 opcode and record layout are fixed, and the
    // initialized record remains exclusively borrowed for the complete call.
    let request = unsafe { Updater::<DESTROY_QUEUE_OPCODE, _>::new(args) };
    // SAFETY: request construction establishes the exclusive in/out borrow.
    unsafe { rustix::ioctl::ioctl(kfd, request) }.map(|_| ())
}
