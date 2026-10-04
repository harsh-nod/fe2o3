use super::*;

// Execute the actual immutable program, not a second policy predicate.
fn evaluate(arch: u32, nr: u32, flags: u64) -> u32 {
    let mut pc = 0;
    let mut a = 0;
    for _ in 0..INSTRUCTIONS {
        let i = FILTER[pc];
        pc += 1;
        match i.code {
            LD => {
                a = match i.value {
                    0 => nr,
                    4 => arch,
                    16 => flags as u32,
                    other => panic!("unexpected namespace-filter offset {other}"),
                }
            }
            JEQ | JGE | JSET => {
                let yes = match i.code {
                    JEQ => a == i.value,
                    JGE => a >= i.value,
                    JSET => a & i.value != 0,
                    _ => unreachable!(),
                };
                pc += usize::from(if yes { i.yes } else { i.no });
            }
            RET => return i.value,
            other => panic!("unexpected namespace-filter opcode {other}"),
        }
    }
    panic!("namespace filter did not terminate")
}

#[test]
fn namespace_filter_denies_every_namespace_bit_and_unconditional_entrypoints() {
    for bit in [
        0x80, 0x20000, 0x2000000, 0x4000000, 0x8000000, 0x10000000, 0x20000000, 0x40000000,
    ] {
        for flags in [bit, bit | 17, bit | 0x1_0000_0000] {
            assert_eq!(evaluate(ARCH, 56, flags), DENY, "flags {flags:#x}");
        }
    }
    for flags in [0, 17, u64::MAX] {
        assert_eq!(evaluate(ARCH, 272, flags), DENY);
        assert_eq!(evaluate(ARCH, 308, flags), DENY);
        assert_eq!(evaluate(ARCH, 435, flags), UNIMPLEMENTED);
    }
    for bit in 0..64 {
        let expected = if matches!(bit, 7 | 17 | 25..=30) {
            DENY
        } else {
            ALLOW
        };
        assert_eq!(evaluate(ARCH, 56, 1_u64 << bit), expected, "bit {bit}");
    }
}

#[test]
fn namespace_filter_preserves_native_thread_fork_exec_and_helper_primitives() {
    // Legacy pthread flags, vfork-style flags, SIGCHLD and an upper-word flag.
    for flags in [0, 17, 0x003d_0f00, 0x4111, 0x1_0000_0011] {
        assert_eq!(evaluate(ARCH, 56, flags), ALLOW, "flags {flags:#x}");
    }
    for nr in [
        0, 1, 3, 9, 10, 39, 57, 58, 59, 60, 101, 157, 231, 257, 317, 322,
    ] {
        assert_eq!(evaluate(ARCH, nr, u64::MAX), ALLOW, "syscall {nr}");
    }
    for arch in [0, 0x4000_0003, 0xc000_00b7] {
        assert_eq!(evaluate(arch, 56, 17), KILL);
    }
    for nr in [0x4000_0000, 0x4000_0038, u32::MAX] {
        assert_eq!(evaluate(ARCH, nr, 17), KILL);
    }
}

#[test]
fn namespace_filter_is_fixed_forward_only_and_fully_quoted() {
    assert_eq!(size_of::<Instruction>(), 8);
    assert_eq!(size_of::<Program>(), 16);
    assert_eq!(INSTRUCTIONS, 16);
    assert_eq!(SCRATCH, 400);
    for (pc, i) in FILTER.iter().enumerate() {
        if matches!(i.code, JEQ | JGE | JSET) {
            assert!(pc + 1 + usize::from(i.yes) < FILTER.len());
            assert!(pc + 1 + usize::from(i.no) < FILTER.len());
        }
    }
}
