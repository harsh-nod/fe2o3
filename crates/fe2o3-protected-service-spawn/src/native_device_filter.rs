//! A fixed deny-device program attached to an original, still-empty cgroup.
//! No caller-supplied code, kernel-program discovery, pins or detached links.
use crate::native_spawn::{ProtectedServiceSpawnErrorV2 as Error, Result, io};
use rustix::io::Errno;
use std::mem::size_of;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

const PROG_LOAD: u32 = 5;
const PROG_ATTACH: u32 = 8;
const CGROUP_DEVICE_PROGRAM: u32 = 15;
const CGROUP_DEVICE_ATTACH: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Instruction {
    code: u8,
    registers: u8,
    offset: i16,
    immediate: i32,
}

// BPF_ALU64 | BPF_MOV | BPF_K: r0 = 0; BPF_JMP | BPF_EXIT.
// Every device/access input is denied, without reading the context or a map.
static PROGRAM: [Instruction; 2] = [
    Instruction {
        code: 0xb7,
        registers: 0,
        offset: 0,
        immediate: 0,
    },
    Instruction {
        code: 0x95,
        registers: 0,
        offset: 0,
        immediate: 0,
    },
];

#[repr(C)]
struct Load {
    program_type: u32,
    instruction_count: u32,
    instructions: u64,
    license: u64,
    log_level: u32,
    log_size: u32,
    log_buffer: u64,
    kernel_version: u32,
    program_flags: u32,
    name: [u8; 16],
    interface_index: u32,
    expected_attach_type: u32,
}

#[repr(C)]
struct Attach {
    target: u32,
    program: u32,
    attach_type: u32,
    flags: u32,
    replace_program: u32,
}

pub(crate) const WORK: usize = 3 * (1024 + 64) + 256;
pub(crate) const SCRATCH: usize = size_of::<Load>() + size_of::<Attach>() + 256;

fn load_record() -> Load {
    Load {
        program_type: CGROUP_DEVICE_PROGRAM,
        instruction_count: PROGRAM.len() as u32,
        instructions: PROGRAM.as_ptr() as u64,
        license: c"GPL".as_ptr() as u64,
        log_level: 0,
        log_size: 0,
        log_buffer: 0,
        kernel_version: 0,
        program_flags: 0,
        name: *b"fe2o3_nodev\0\0\0\0\0",
        interface_index: 0,
        expected_attach_type: CGROUP_DEVICE_ATTACH,
    }
}

/// Caller holds the original empty root-controlled cgroup and excludes all
/// foreign attach/detach/migration/control writers until aggregate retirement.
/// The successful attachment persists after this temporary program FD closes;
/// removing the empty original cgroup retires it. No detach operation exists.
#[allow(unsafe_code)]
pub(crate) fn install(directory: BorrowedFd<'_>) -> Result<()> {
    let load = load_record();
    // SAFETY: the fixed native UAPI record and immutable program/license remain
    // live for this synchronous copy. No writable kernel log pointer is supplied.
    let raw =
        unsafe { libc::syscall(libc::SYS_bpf, PROG_LOAD, &raw const load, size_of::<Load>()) };
    if raw < 0 {
        return Err(io(
            "load fixed compiler device denial",
            Errno::last_os_error(),
        ));
    }
    let raw = i32::try_from(raw).map_err(|_| Error::State("invalid BPF program descriptor"))?;
    // SAFETY: successful BPF_PROG_LOAD returned this new, uniquely owned FD.
    let program = unsafe { OwnedFd::from_raw_fd(raw) };
    let attach = Attach {
        target: directory.as_raw_fd() as u32,
        program: program.as_raw_fd() as u32,
        attach_type: CGROUP_DEVICE_ATTACH,
        flags: 0,
        replace_program: 0,
    };
    // SAFETY: the complete fixed attach record contains only live owned/borrowed
    // FDs. Flags zero prohibit overriding this policy in a descendant cgroup.
    let result = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            PROG_ATTACH,
            &raw const attach,
            size_of::<Attach>(),
        )
    };
    if result != 0 {
        return Err(io(
            "attach fixed compiler device denial",
            Errno::last_os_error(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_program_denies_every_device_without_inputs_or_helpers() {
        assert_eq!(
            PROGRAM,
            [
                Instruction {
                    code: 0xb7,
                    registers: 0,
                    offset: 0,
                    immediate: 0
                },
                Instruction {
                    code: 0x95,
                    registers: 0,
                    offset: 0,
                    immediate: 0
                },
            ]
        );
        assert_eq!(size_of::<Instruction>(), 8);
        assert_eq!(size_of::<Load>(), 72);
        assert_eq!(size_of::<Attach>(), 20);
        assert_eq!(std::mem::offset_of!(Load, expected_attach_type), 68);
        let load = load_record();
        assert_eq!(load.program_type, 15);
        assert_eq!(load.expected_attach_type, 6);
        assert_eq!(load.instruction_count, 2);
        assert_eq!(load.instructions, PROGRAM.as_ptr() as u64);
        assert_eq!(
            load.log_level | load.log_size | load.program_flags | load.interface_index,
            0
        );
        assert_eq!(load.log_buffer, 0);
        assert_eq!(load.name, *b"fe2o3_nodev\0\0\0\0\0");
    }
}
