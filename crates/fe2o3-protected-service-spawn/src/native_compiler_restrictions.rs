//! Fixed compiler pre-exec restrictions, not complete runtime enforcement.
//!
//! These scalar restrictions are inherited across exec and task creation. They
//! enforce an explicit-argument-bit subset of the proof controller's memory
//! denials without importing its ptrace/wait owner or read-only-open policy.
//! File-backed RX mappings, pathname opens, descendants and existing writable
//! descriptors are NOT authenticated here. In particular, ordinary compiler
//! output opens remain possible: this does not exclude procfs memory writers,
//! prove immutable loader/proc-macro backing, or satisfy an enforcement guard.
//! The actual child rejects inherited READ_IMPLIES_EXEC before installing the
//! filter; it never clears that state. ELF/exec-established personality and
//! initial mappings remain unchecked. This is not complete W^X enforcement.

use std::mem::size_of;

const LD: u16 = 0x20;
const JEQ: u16 = 0x15;
const JGE: u16 = 0x35;
const JSET: u16 = 0x45;
const RET: u16 = 0x06;
const KILL: u32 = 0x8000_0000; // SECCOMP_RET_KILL_PROCESS, including sibling threads.
const ALLOW: u32 = 0x7fff_0000;
const ARCH: u32 = 0xc000_003e;
const X32: u32 = 0x4000_0000;
const READ_IMPLIES_EXEC: libc::c_long = 0x0040_0000;

// x86-64 only; other audit architectures and the x32 ABI are refused first.
const DENIED: [u32; 17] = [
    101, // ptrace
    135, // personality, including READ_IMPLIES_EXEC
    323, // userfaultfd
    30, 134, 216, // shmat, uselib, remap_file_pages
    206, 207, 208, 209, 210, // asynchronous AIO
    425, 426, 427, // io_uring
    310, 311, // process_vm_readv / process_vm_writev
    438, // pidfd_getfd
];
pub(crate) const INSTRUCTIONS: usize = 22 + 2 * DENIED.len() + 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct Instruction {
    code: u16,
    yes: u8,
    no: u8,
    value: u32,
}

#[repr(C)]
struct Program {
    length: u16,
    instructions: *const Instruction,
}

// Includes the immutable program's bytes conservatively as well as child ABI
// frames, including the scalar personality result. No allocation, mutex,
// callback or fallible preparation after clone.
pub(crate) const SCRATCH: usize =
    INSTRUCTIONS * size_of::<Instruction>() + size_of::<Program>() + 256;

const fn instruction(code: u16, value: u32, yes: u8, no: u8) -> Instruction {
    Instruction {
        code,
        yes,
        no,
        value,
    }
}

const fn program() -> [Instruction; INSTRUCTIONS] {
    let mut p = [instruction(RET, KILL, 0, 0); INSTRUCTIONS];
    p[0] = instruction(LD, 4, 0, 0); // seccomp_data.arch
    p[1] = instruction(JEQ, ARCH, 1, 0);
    p[2] = instruction(RET, KILL, 0, 0);
    p[3] = instruction(LD, 0, 0, 0); // seccomp_data.nr
    p[4] = instruction(JGE, X32, 0, 1);
    p[5] = instruction(RET, KILL, 0, 0);
    p[6] = instruction(JEQ, 9, 0, 9); // mmap; otherwise 16
    p[7] = instruction(LD, 32, 0, 0); // args[2] protection, low word
    p[8] = instruction(JSET, 4, 0, 6); // non-EXEC -> ALLOW at 15
    p[9] = instruction(JSET, 2, 4, 0); // WRITE+EXEC -> KILL at 14
    p[10] = instruction(LD, 40, 0, 0); // args[3] flags, low word
    p[11] = instruction(JSET, 0x20, 2, 0); // MAP_ANONYMOUS -> 14
    p[12] = instruction(LD, 48, 0, 0); // args[4] fd; kernel consumes an int
    p[13] = instruction(JEQ, u32::MAX, 0, 1);
    p[14] = instruction(RET, KILL, 0, 0);
    p[15] = instruction(RET, ALLOW, 0, 0);
    p[16] = instruction(JEQ, 10, 1, 0); // mprotect -> 18
    p[17] = instruction(JEQ, 329, 0, 4); // pkey_mprotect; otherwise 22
    p[18] = instruction(LD, 32, 0, 0);
    p[19] = instruction(JSET, 4, 0, 1); // never add OR restore EXEC
    p[20] = instruction(RET, KILL, 0, 0);
    p[21] = instruction(RET, ALLOW, 0, 0);
    let mut i = 0;
    while i < DENIED.len() {
        p[22 + 2 * i] = instruction(JEQ, DENIED[i], 0, 1);
        p[23 + 2 * i] = instruction(RET, KILL, 0, 0);
        i += 1;
    }
    p[INSTRUCTIONS - 1] = instruction(RET, ALLOW, 0, 0);
    p
}

static FILTER: [Instruction; INSTRUCTIONS] = program();

/// Called only in the already cap-free, NNP direct child, before profile-ready
/// and first exec. The original parent prepays all work/scratch before clone.
/// Query errors or inherited READ_IMPLIES_EXEC refuse before filter installation.
/// Success means that inherited bit was absent and this fixed filter installed,
/// not code admission or validation of personality established by a later exec.
pub(super) unsafe fn install() -> bool {
    let program = Program {
        length: INSTRUCTIONS as u16,
        instructions: FILTER.as_ptr(),
    };
    // SAFETY: fixed native Linux ABI, immutable static filter and live stack
    // header. The personality sentinel only queries this actual child and must
    // precede the filter, which denies all later personality calls. The kernel
    // copies both filter records synchronously; no pointer escapes. NNP is
    // already established, and a successful filter cannot later be removed.
    unsafe {
        let personality = libc::syscall(libc::SYS_personality, u32::MAX as libc::c_ulong);
        personality >= 0
            && personality & READ_IMPLIES_EXEC == 0
            && libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) == 1
            && libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program, 0, 0) == 0
            && libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) == 2
    }
}

#[cfg(test)]
#[path = "native_compiler_restrictions_tests.rs"]
mod tests;
