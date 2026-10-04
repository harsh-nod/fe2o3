use super::*;

const LEGACY_STACK: u64 = 33_554_432;
const INTERPRETER_STACK: u64 = 1_073_741_824;
const POLICIES: [GeneratedProofProcessPolicyV2; 3] = [
    GeneratedProofProcessPolicyV2::LegacySingleSolverV1,
    GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV2,
    GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV3,
];
const ROLES: [TraceeRole; 4] = [
    TraceeRole::Verifier,
    TraceeRole::AuxiliaryVerifier,
    TraceeRole::Solver,
    TraceeRole::PendingExecutable,
];

fn thread(stack_size: u64) -> [u64; 11] {
    [
        0x003d_0f00,
        0x2000,
        0x2000,
        0x2000,
        0,
        0x10000,
        stack_size,
        0x3000,
        0,
        0,
        0,
    ]
}

fn process(stack_size: u64) -> [u64; 11] {
    [
        0x0000_0001_0000_4100,
        0,
        0,
        0,
        17,
        0x10000,
        stack_size,
        0,
        0,
        0,
        0,
    ]
}

#[test]
fn clone3_v3_stack_bounds_are_exact_and_only_authenticated_verifier_threads_expand() {
    for policy in POLICIES {
        for role in ROLES {
            let expanded = policy == GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV3
                && matches!(role, TraceeRole::Verifier | TraceeRole::AuxiliaryVerifier);
            for extent in [
                0,
                1,
                LEGACY_STACK,
                LEGACY_STACK + 1,
                0x3fff_cd40,
                INTERPRETER_STACK,
                INTERPRETER_STACK + 1,
            ] {
                let limit = if expanded {
                    INTERPRETER_STACK
                } else {
                    LEGACY_STACK
                };
                assert_eq!(
                    validate_clone3_arguments(thread(extent), policy, role).is_ok(),
                    (1..=limit).contains(&extent),
                    "{policy:?}/{role:?}/{extent}"
                );
                assert_eq!(
                    validate_clone3_arguments(process(extent), policy, role).is_ok(),
                    (1..=LEGACY_STACK).contains(&extent),
                    "process {policy:?}/{role:?}/{extent}"
                );
            }
        }
    }
}

#[test]
fn clone3_v3_stack_admission_preserves_every_abi_field_and_overflow_refusal() {
    let policy = GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV3;
    for role in ROLES {
        for base in [thread(LEGACY_STACK), process(LEGACY_STACK)] {
            assert!(validate_clone3_arguments(base, policy, role).is_ok());
            for (index, value) in [
                (0, base[0] ^ 1),
                (1, base[1] ^ 1),
                (2, base[2] ^ 1),
                (3, base[3] ^ 1),
                (4, base[4] ^ 1),
                (5, 0),
                (6, 0),
                (8, 1),
                (9, 1),
                (10, 1),
            ] {
                let mut mutant = base;
                mutant[index] = value;
                assert!(
                    validate_clone3_arguments(mutant, policy, role).is_err(),
                    "{role:?}/{index}"
                );
            }
            let mut tls = base;
            tls[7] = if base[7] == 0 { 1 } else { 0 };
            assert!(validate_clone3_arguments(tls, policy, role).is_err());
            let mut overflow = base;
            overflow[5] = u64::MAX - LEGACY_STACK + 1;
            assert!(validate_clone3_arguments(overflow, policy, role).is_err());
        }
    }
    let mut overflow = thread(INTERPRETER_STACK);
    overflow[5] = u64::MAX - INTERPRETER_STACK + 1;
    assert!(validate_clone3_arguments(overflow, policy, TraceeRole::Verifier).is_err());
    overflow[5] -= 1;
    assert!(validate_clone3_arguments(overflow, policy, TraceeRole::Verifier).is_ok());
}

#[test]
fn clone3_v3_stable_and_sensitive_entrypoints_use_the_same_explicit_role_and_policy() {
    let arguments = thread(INTERPRETER_STACK);
    let registers = UserRegistersX86_64 {
        orig_rax: CLONE3_SYSCALL as u64,
        rdi: arguments.as_ptr() as u64,
        rsi: 88,
        ..UserRegistersX86_64::default()
    };
    let pid = std::process::id() as i32;
    for policy in POLICIES {
        for role in ROLES {
            let accepted = policy == GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV3
                && matches!(role, TraceeRole::Verifier | TraceeRole::AuxiliaryVerifier);
            let birth = stable::birth_request(pid, &registers, policy, role);
            assert_eq!(birth.is_ok(), accepted);
            if accepted {
                assert!(matches!(birth.unwrap(), Some(stable::Birth::Thread)));
            }
            assert_eq!(
                validate_sensitive_registers_with_policy(pid, &registers, &[], false, policy, role)
                    .is_ok(),
                accepted
            );
        }
    }
    for (pointer, size) in [(0, 88), (registers.rdi, 80), (registers.rdi, 96)] {
        let bad = UserRegistersX86_64 {
            orig_rax: CLONE3_SYSCALL as u64,
            rdi: pointer,
            rsi: size,
            ..UserRegistersX86_64::default()
        };
        assert!(validate_clone3_request(pid, &bad, POLICIES[2], TraceeRole::Verifier).is_err());
    }
}
