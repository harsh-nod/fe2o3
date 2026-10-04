use super::*;

// Interpret the actual fixed program, not a second admission predicate. Kernel
// installation and real syscalls are exercised by the isolated exec tests.
fn evaluate(arch: u32, nr: u32, args: [u64; 6]) -> u32 {
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
                    offset @ 16..=60 if offset % 4 == 0 => {
                        let argument = args[((offset - 16) / 8) as usize];
                        (argument >> ((offset % 8) * 8)) as u32
                    }
                    other => panic!("unexpected seccomp data offset {other}"),
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
            other => panic!("unexpected BPF opcode {other}"),
        }
    }
    panic!("fixed compiler filter did not terminate")
}

#[test]
fn compiler_filter_rejects_foreign_abis_and_fixed_memory_primitives() {
    for arch in [0, 0x4000_0003, 0xc000_00b7] {
        assert_eq!(evaluate(arch, 39, [0; 6]), KILL);
    }
    for nr in [0x4000_0000, 0x4000_0009, u32::MAX] {
        assert_eq!(evaluate(ARCH, nr, [0; 6]), KILL);
    }
    for nr in [
        101, 135, 323, 30, 134, 216, 206, 207, 208, 209, 210, 425, 426, 427, 310, 311, 438,
    ] {
        assert_eq!(evaluate(ARCH, nr, [0; 6]), KILL, "syscall {nr}");
        assert_eq!(evaluate(ARCH, nr, [u64::MAX; 6]), KILL);
    }
    // Ordinary I/O, creation and exec are not changed into proof authority.
    for nr in [
        0, 1, 2, 3, 25, 39, 56, 57, 58, 59, 60, 157, 231, 257, 317, 322, 435,
    ] {
        assert_eq!(evaluate(ARCH, nr, [0; 6]), ALLOW, "syscall {nr}");
    }
}

#[test]
fn compiler_filter_preserves_data_and_file_rx_but_denies_exec_transitions() {
    for prot in [0, 1, 2, 3] {
        assert_eq!(evaluate(ARCH, 9, [0, 4096, prot, 0x22, u64::MAX, 0]), ALLOW);
        for nr in [10, 329] {
            assert_eq!(evaluate(ARCH, nr, [0, 4096, prot, 0, 0, 0]), ALLOW);
        }
    }
    for prot in [4, 5, 6, 7, 0x1_0000_0004] {
        for nr in [10, 329] {
            assert_eq!(evaluate(ARCH, nr, [0, 4096, prot, 0, 0, 0]), KILL);
        }
        for (flags, fd) in [(0x22, 3), (2, u64::MAX), (2, u64::from(u32::MAX))] {
            assert_eq!(evaluate(ARCH, 9, [0, 4096, prot, flags, fd, 0]), KILL);
        }
    }
    assert_eq!(evaluate(ARCH, 9, [0, 4096, 5, 2, 3, 0]), ALLOW);
    assert_eq!(evaluate(ARCH, 9, [0, 4096, 7, 2, 3, 0]), KILL);
}

#[test]
fn compiler_filter_denies_personality_query_as_well_as_mutation() {
    // The exact-child proc read/close precedes installation; no syscall exception.
    for personality in [0, 0x0040_0000, u64::from(u32::MAX)] {
        assert_eq!(evaluate(ARCH, 135, [personality, 0, 0, 0, 0, 0]), KILL);
    }
}

#[test]
fn compiler_filter_is_fixed_forward_only_and_fully_quoted() {
    assert_eq!(size_of::<Instruction>(), 8);
    assert_eq!(size_of::<Program>(), 16);
    assert_eq!(INSTRUCTIONS, 57);
    assert_eq!(
        SCRATCH,
        728 + 6 * size_of::<libc::stat>()
            + 2 * size_of::<libc::statfs>()
            + 2 * 24
            + 2 * 12
            + 2 * 32
            + 2 * 10
            + 1024
    );
    for (pc, i) in FILTER.iter().enumerate() {
        if matches!(i.code, JEQ | JGE | JSET) {
            assert!(pc + 1 + usize::from(i.yes) < FILTER.len());
            assert!(pc + 1 + usize::from(i.no) < FILTER.len());
        }
    }
    assert_eq!(FILTER.last().unwrap().value, ALLOW);
}
