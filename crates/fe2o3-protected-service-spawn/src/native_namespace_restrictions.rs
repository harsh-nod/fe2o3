//! Inherited namespace confinement for every native child, never its creator.
//!
//! This does not authenticate deployment provenance or exclude inherited cgroup
//! controls, migration or delegation. Those remain the caller's obligations.
//! clone3 has indirect arguments, so it returns ENOSYS rather than admitting an
//! uninspectable flags record. Approved libc thread/fork fallback needs separate
//! qualification; ordinary legacy clone, fork and vfork remain available.

use std::mem::size_of;

const LD: u16 = 0x20;
const JEQ: u16 = 0x15;
const JGE: u16 = 0x35;
const JSET: u16 = 0x45;
const RET: u16 = 0x06;
const KILL: u32 = 0x8000_0000;
const ALLOW: u32 = 0x7fff_0000;
const DENY: u32 = 0x0005_0001; // SECCOMP_RET_ERRNO | EPERM.
const UNIMPLEMENTED: u32 = 0x0005_0026; // SECCOMP_RET_ERRNO | ENOSYS.
const ARCH: u32 = 0xc000_003e;
const X32: u32 = 0x4000_0000;
// NEWNS, NEWCGROUP, NEWUTS, NEWIPC, NEWUSER, NEWPID, NEWNET and NEWTIME.
// NEWTIME overlaps the legacy clone exit-signal byte; valid exit signals do not.
const NAMESPACES: u32 = 0x7e02_0080;

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

pub(crate) const INSTRUCTIONS: usize = 16;
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

static FILTER: [Instruction; INSTRUCTIONS] = [
    instruction(LD, 4, 0, 0), // seccomp_data.arch
    instruction(JEQ, ARCH, 1, 0),
    instruction(RET, KILL, 0, 0),
    instruction(LD, 0, 0, 0), // seccomp_data.nr
    instruction(JGE, X32, 0, 1),
    instruction(RET, KILL, 0, 0),
    instruction(JEQ, 435, 0, 1), // clone3: never dereference its flags pointer.
    instruction(RET, UNIMPLEMENTED, 0, 0),
    instruction(JEQ, 272, 1, 0), // unshare, including a zero flags argument.
    instruction(JEQ, 308, 0, 1), // setns, including invalid FDs.
    instruction(RET, DENY, 0, 0),
    instruction(JEQ, 56, 0, 3), // legacy clone; otherwise ALLOW.
    instruction(LD, 16, 0, 0),  // args[0] flags, low word on native x86-64.
    instruction(JSET, NAMESPACES, 0, 1),
    instruction(RET, DENY, 0, 0),
    instruction(RET, ALLOW, 0, 0),
];

/// Install only in the cap-free, NNP direct child after required namespace maps
/// and credential setup, before READY or first exec. Parent prepays work/scratch.
/// Failure is terminal through the existing status/cleanup path, never an opt-out.
pub(super) unsafe fn install() -> bool {
    let program = Program {
        length: INSTRUCTIONS as u16,
        instructions: FILTER.as_ptr(),
    };
    // SAFETY: fixed native ABI and immutable static program; the kernel copies
    // the live header/program synchronously. No allocation or callback after clone.
    unsafe {
        libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) == 1
            && libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program, 0, 0) == 0
            && libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) == 2
    }
}

#[cfg(test)]
#[path = "native_namespace_restrictions_tests.rs"]
mod tests;
