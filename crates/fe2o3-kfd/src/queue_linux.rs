//! Private Linux ioctl boundary for the native queue adapter foundation.
//!
//! There is deliberately no production backend yet. The memory owner does not
//! expose the typed mapped ring/control/EOP/CWSR authorities required to make
//! these calls sound. These functions keep the eventual unsafe boundary small
//! without making an fd or numeric address public.

use std::os::fd::BorrowedFd;

use fe2o3_kfd_uapi::{
    AMDKFD_IOC_CREATE_QUEUE, AMDKFD_IOC_DESTROY_QUEUE, AMDKFD_IOC_UPDATE_QUEUE,
    KfdIoctlCreateQueueArgs, KfdIoctlDestroyQueueArgs, KfdIoctlUpdateQueueArgs,
};
use rustix::ioctl::{Opcode, Setter, Updater};

const CREATE_QUEUE_OPCODE: Opcode = AMDKFD_IOC_CREATE_QUEUE as Opcode;
const DESTROY_QUEUE_OPCODE: Opcode = AMDKFD_IOC_DESTROY_QUEUE as Opcode;
const UPDATE_QUEUE_OPCODE: Opcode = AMDKFD_IOC_UPDATE_QUEUE as Opcode;

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
