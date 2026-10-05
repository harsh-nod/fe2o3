//! Additional runtime checkpoints, installed only after the original exec gate.
//!
//! Existing compiler memory and namespace restrictions remain installed. This
//! filter adds stops; it neither admits a syscall nor grants runtime authority.
//! The original parent must arm its consuming trace before releasing that gate.

use std::mem::size_of;

const LD: u16 = 0x20;
const JEQ: u16 = 0x15;
const JGE: u16 = 0x35;
const RET: u16 = 0x06;
const KILL: u32 = 0x8000_0000;
const TRACE: u32 = 0x7ff0_0000;
const ALLOW: u32 = 0x7fff_0000;
const ARCH: u32 = 0xc000_003e;
const X32: u32 = 0x4000_0000;

// Proof boundaries plus descriptor imports/ioctl, whose exit must be inspected
// with every file-table sharer parked. clone3 remains ENOSYS and pidfd_getfd
// remains killed by the already-installed filters; neither policy is relaxed.
const CHECKPOINTS: [u32; 17] = [
    60, 231, 56, 57, 58, 9, 10, 25, 216, 329, 435, 157, 2, 257, 16, 47, 299,
];
// Indirect open flags and replacing the trace filter are not admitted bypasses.
const DENIED: [u32; 3] = [85, 317, 437]; // creat, seccomp, openat2.
pub(crate) const INSTRUCTIONS: usize = 6 + 2 * CHECKPOINTS.len() + 2 * DENIED.len() + 1;

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
    let mut result = [instruction(RET, KILL, 0, 0); INSTRUCTIONS];
    result[0] = instruction(LD, 4, 0, 0);
    result[1] = instruction(JEQ, ARCH, 1, 0);
    result[2] = instruction(RET, KILL, 0, 0);
    result[3] = instruction(LD, 0, 0, 0);
    result[4] = instruction(JGE, X32, 0, 1);
    result[5] = instruction(RET, KILL, 0, 0);
    let mut index = 0;
    while index < CHECKPOINTS.len() {
        result[6 + 2 * index] = instruction(JEQ, CHECKPOINTS[index], 0, 1);
        result[7 + 2 * index] = instruction(RET, TRACE, 0, 0);
        index += 1;
    }
    let start = 6 + 2 * CHECKPOINTS.len();
    index = 0;
    while index < DENIED.len() {
        result[start + 2 * index] = instruction(JEQ, DENIED[index], 0, 1);
        result[start + 2 * index + 1] = instruction(RET, KILL, 0, 0);
        index += 1;
    }
    result[INSTRUCTIONS - 1] = instruction(RET, ALLOW, 0, 0);
    result
}

static FILTER: [Instruction; INSTRUCTIONS] = program();

/// No allocation, callback or destructor after clone. NNP and the original
/// restrictions were established before READY. The only new syscall installs
/// this immutable filter; success proceeds directly to trusted FD remaps/exec.
pub(super) unsafe fn install() -> bool {
    let program = Program {
        length: INSTRUCTIONS as u16,
        instructions: FILTER.as_ptr(),
    };
    // SAFETY: the kernel copies the fixed immutable instructions and this live
    // header synchronously. The typed stage is used only by the owned child.
    unsafe { libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program, 0, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluate(architecture: u32, syscall: u32) -> u32 {
        let mut accumulator = 0;
        let mut pc = 0;
        for _ in 0..INSTRUCTIONS {
            let i = FILTER[pc];
            match i.code {
                LD => {
                    accumulator = match i.value {
                        0 => syscall,
                        4 => architecture,
                        _ => panic!("unknown load"),
                    }
                }
                JEQ | JGE => {
                    let yes = if i.code == JEQ {
                        accumulator == i.value
                    } else {
                        accumulator >= i.value
                    };
                    pc += usize::from(if yes { i.yes } else { i.no });
                }
                RET => return i.value,
                _ => panic!("unsupported filter instruction"),
            }
            pc += 1;
            assert!(pc < FILTER.len());
        }
        panic!("filter failed to terminate")
    }

    #[test]
    fn trace_filter_has_exact_sensitive_and_denied_rosters() {
        for syscall in 0..1024 {
            let expected = if CHECKPOINTS.contains(&syscall) {
                TRACE
            } else if DENIED.contains(&syscall) {
                KILL
            } else {
                ALLOW
            };
            assert_eq!(evaluate(ARCH, syscall), expected, "syscall {syscall}");
        }
        assert_eq!(CHECKPOINTS.len(), 17);
        assert_eq!(DENIED.len(), 3);
        assert!(CHECKPOINTS.iter().all(|value| !DENIED.contains(value)));
    }

    #[test]
    fn trace_filter_refuses_every_other_abi_and_high_syscall_number() {
        for architecture in [0, 3, 0x4000_0003, ARCH ^ 1, u32::MAX] {
            for syscall in [0, 9, 56, 60, 257, u32::MAX] {
                assert_eq!(evaluate(architecture, syscall), KILL);
            }
        }
        for syscall in [X32, X32 + 9, 0x8000_0000, u32::MAX] {
            assert_eq!(evaluate(ARCH, syscall), KILL);
        }
    }

    #[test]
    fn postgate_bootstrap_exec_and_fd_setup_remain_available() {
        for syscall in [1, 3, 32, 33, 59, 81, 292, 322, 436] {
            assert_eq!(evaluate(ARCH, syscall), ALLOW);
        }
        // Queries after installation are not silently exempted from runtime policy.
        assert_eq!(evaluate(ARCH, 157), TRACE);
        assert_eq!(evaluate(ARCH, 317), KILL);
    }
}
