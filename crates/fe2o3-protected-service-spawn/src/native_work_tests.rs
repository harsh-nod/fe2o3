use super::child_work;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

#[test]
fn mapping_gate_prepays_read_retries_and_both_closes() {
    assert_eq!(super::MAPPING_GATE_WORK, (64 + 2) * 1088 + 256);
}

#[test]
fn compiler_channel_prepays_all_extra_syscalls_and_wire_construction() {
    let calls = [
        "SO_PEERCRED",
        "getpid",
        "getppid",
        "socketpair",
        "F_DUPFD_CLOEXEC",
        "close original client",
        "sendmsg",
        "close service",
        "close transfer",
        "close high client",
    ];
    assert_eq!(
        super::COMPILER_CHANNEL_WORK,
        calls.len() * 1088 + 256 + (24 + 24) * 64
    );
    // Installation is charged once through the generated descriptor's count.
    assert_eq!(
        child_work(2, 63).unwrap() - child_work(1, 63).unwrap(),
        1088
    );
}

// Independent syscall transcript, without the implementation's aggregated counts or weights.
fn transcript_work(descriptors: usize, cap_last_cap: u32) -> usize {
    let mut work = 256;
    let mut operation = || work += 1024 + 64;
    for _ in 1..=64 {
        operation(); // Every signal slot, including SIGKILL and SIGSTOP allowances.
    }
    operation(); // rt_sigprocmask.
    for _ in 0..2 {
        for _ in ["getppid", "SET_PDEATHSIG", "getppid", "GET_PDEATHSIG"] {
            operation();
        }
    }
    for _ in ["umask", "prlimit64", "SET_SECUREBITS", "AMBIENT_CLEAR_ALL"] {
        operation();
    }
    for _ in 0..=cap_last_cap {
        operation(); // CAPBSET_DROP.
    }
    for _ in [
        "setgroups",
        "setresgid",
        "setresuid",
        "capset",
        "SET_NO_NEW_PRIVS",
        "SET_DUMPABLE",
        "getresuid",
        "getresgid",
        "setfsuid",
        "setfsgid",
        "getgroups",
        "capget",
    ] {
        operation();
    }
    for _ in 0..=cap_last_cap {
        operation(); // CAPBSET_READ.
        operation(); // AMBIENT_IS_SET.
    }
    for _ in [
        "GET_SECUREBITS",
        "GET_NO_NEW_PRIVS",
        "GET_DUMPABLE",
        "prlimit64",
        "umask",
        "ready write",
    ] {
        operation();
    }
    for _ in 0..64 {
        operation(); // Every gate attempt, including preceding EINTR refusals.
    }
    operation(); // close_range.
    for _ in 0..descriptors {
        operation(); // dup3.
    }
    for _ in [
        "close stdin",
        "close stdout",
        "close stderr",
        "execveat",
        "send",
        "_exit",
    ] {
        operation();
    }
    work
}

#[test]
fn every_supported_pair_matches_independent_transcript_and_formula() {
    assert_eq!(crate::MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1, 32);
    assert_eq!(crate::pre_exec::MAX_CHILD_GATE_ATTEMPTS_V2, 64);
    for descriptors in 1..=32 {
        for ceiling in 0..=63 {
            let transcript = transcript_work(descriptors, ceiling);
            let formula = (166 + descriptors + 3 * (ceiling as usize + 1)) * 1088 + 256;
            assert_eq!(transcript, formula);
            assert_eq!(child_work(descriptors, ceiling), Ok(transcript));
        }
    }
}

#[test]
fn exact_minimum_and_maximum_include_exec_failure_suffix() {
    assert_eq!(child_work(1, 0), Ok(185_216));
    assert_eq!(child_work(32, 63), Ok(424_576));
}

#[test]
fn descriptor_and_capability_increments_charge_all_operations() {
    for descriptors in 1..32 {
        for ceiling in 0..=63 {
            assert_eq!(
                child_work(descriptors + 1, ceiling).unwrap()
                    - child_work(descriptors, ceiling).unwrap(),
                1088,
            );
        }
    }
    for descriptors in 1..=32 {
        for ceiling in 0..63 {
            assert_eq!(
                child_work(descriptors, ceiling + 1).unwrap()
                    - child_work(descriptors, ceiling).unwrap(),
                3 * 1088,
            );
        }
    }
}

#[test]
fn unsupported_descriptor_counts_are_bounded_errors() {
    for descriptors in [0, 33, 64] {
        for ceiling in [0, 63] {
            assert_eq!(child_work(descriptors, ceiling), Err(Resource::Arithmetic));
        }
    }
}

#[test]
fn unsupported_capability_ceilings_are_bounded_errors() {
    for descriptors in [1, 32] {
        for ceiling in [64, 65, 255] {
            assert_eq!(child_work(descriptors, ceiling), Err(Resource::Arithmetic));
        }
    }
}

#[test]
fn overflow_sized_inputs_are_rejected_before_counting_or_weighting() {
    // Valid inputs cannot overflow usize on supported x86-64. Hostile inputs must
    // not wrap the capability count, descriptor sum, or final work multiplication.
    for descriptors in [usize::MAX / 1088 + 1, usize::MAX - 1, usize::MAX] {
        for ceiling in [0, 63, u32::MAX - 1, u32::MAX] {
            assert_eq!(child_work(descriptors, ceiling), Err(Resource::Arithmetic));
        }
    }
    for descriptors in [1, 32] {
        for ceiling in [u32::MAX - 1, u32::MAX] {
            assert_eq!(child_work(descriptors, ceiling), Err(Resource::Arithmetic));
        }
    }
}
