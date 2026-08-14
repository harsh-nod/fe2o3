//! Authenticated occurrence evidence for a narrow Verus/solver controller protocol.
//!
//! V2 launches immutable, digest-pinned solver and Verus snapshots. Each tool
//! must implement the two-nonce `READY/START/RESULT/SEALED/DONE/ACK`
//! protocol described by this module; stock Verus and Z3 do not implement that
//! protocol yet. The controller runs the solver first and Verus second as
//! independent direct processes.
//! It does not establish that Verus invoked the solver or that either opaque result
//! is semantically a proof. A reviewed adapter for the real Verus CLI must preserve
//! these bindings before this path can provide end-to-end Verus evidence.
//!
//! On Linux x86_64, each target is created atomically with a
//! pidfd, runs with an empty environment, fixed working directory,
//! no-new-privileges, exact bounded address/data/file/core limits, bounded output,
//! and one deadline covering spawn and execution. A reviewed seccomp filter
//! installed before `exec` denies every process/thread creation syscall available
//! to the target ABI, credential and namespace mutation, and later `prctl` changes.
//! The calling thread and target must have equal non-root UID/GID tuples and no
//! active capabilities; inherited seccomp filters are rejected. The controller
//! seizes the gated pre-exec child and confirms an exact unresumable ptrace event
//! stop before both runtime observations. It reads every executable VMA's live
//! bytes through the stopped target's procfs memory file. File-backed live text is
//! compared byte-for-byte with the reviewed backing-object slice and zero fill;
//! the stable normalized result and explicit vDSO digest must match policy. The
//! exact unreadable x86_64 vsyscall VMA receives a typed marker because Linux
//! exposes no readable page bytes. Every anonymous mapping's range, class,
//! permissions, and size is included, W+X and shared-writable executable aliases
//! are rejected, and anonymous executable maps are limited to bounded kernel
//! vDSO/vsyscall exceptions. Policy pins the ASLR-stable file closure and executable
//! baseline, executable-byte bounds, exact seccomp program, fixed assembly child
//! trampoline, ptrace lifecycle constants, and mapping exceptions. The second
//! checkpoint must match the first exactly.
//!
//! A fixed x86_64 assembly launcher issues `clone3`: its child branch jumps directly
//! into the assembly trampoline, while only its parent branch returns to Rust. The
//! policy-bound instruction bytes use only scalar POD loads, branches, and direct
//! syscalls; there is no child-side Rust, PLT, allocator, panic, bounds-check, or
//! memcpy path after `clone3`. Pidfd signal permission is preflighted with signal
//! zero. Error cleanup
//! uses nonblocking pidfd waits and poll under a separate bounded cleanup deadline;
//! inability to confirm termination is a distinct fail-closed result, and `Drop`
//! uses the same bounded cleanup.
//!
//! `READY` is bound to a launch challenge. Only after the first frozen observation
//! and an empty control queue does the controller generate an unpredictable stage
//! nonce. `START`, sealed result envelope, `DONE`, `ACK`, and the receipt transcript
//! bind both values. This rejects a result or `DONE` prepared before the measured
//! first checkpoint.
//!
//! The executable pathname and live executable pages are authenticated only at
//! frozen checkpoints, with pathname polling while the target runs. The receipt
//! deliberately makes no exclusive-image-execution claim. An `exec` and return,
//! page modification and restoration, RW-to-RX-to-RW transition, mapping creation
//! and removal, or writable alias that exists entirely between observations is
//! outside the claim.

use std::fmt;

use fe2o3_artifacts::DigestAlgorithm;

use crate::{
    Digest, ExecutionLimits, MAX_PATH_BYTES, MAX_RESULT_BYTES, MAX_TIMEOUT_SECONDS,
    MeasuredToolIdentity, PlanError, ProofRequestV1, VerifierPolicy,
};

pub const MAX_VERUS_EXECUTION_SOURCE_BYTES_V2: usize = 16 * 1024 * 1024;
pub const MAX_VERUS_EXECUTION_DEPENDENCIES_V2: usize = 128;
pub const MAX_VERUS_EXECUTION_DEPENDENCY_BYTES_V2: usize = 256 * 1024 * 1024;
const TRANSCRIPT_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-EXECUTION/V2/PIDFD-NONCE2\0";
const POLICY_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-EXECUTION-POLICY/V2/PIDFD-NONCE2\0";
const DEPENDENCIES_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-DEPENDENCIES/V2\0";
const RUNTIME_CLOSURE_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-RUNTIME-CLOSURE/V2\0";
const RUNTIME_MAPPINGS_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-RUNTIME-MAPPINGS/V2\0";
const LIVE_EXECUTABLE_PAGES_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-LIVE-EXECUTABLE-PAGES/V2\0";
const EXECUTABLE_BASELINE_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-EXECUTABLE-BASELINE/V2\0";
const PROCESS_SECURITY_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-PROCESS-SECURITY/V2\0";
const CHILD_TRAMPOLINE_POLICY_DOMAIN: &[u8] =
    b"FE2O3/AUTHENTICATED-VERUS-CHILD-TRAMPOLINE-POLICY/V2\0";
const ANONYMOUS_MAPPING_POLICY_DOMAIN: &[u8] =
    b"FE2O3/AUTHENTICATED-VERUS-ANONYMOUS-MAPPING-POLICY/V2\0";
const SECCOMP_FILTER_POLICY_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-SECCOMP-FILTER-POLICY/V2\0";
const PTRACE_CHECKPOINT_POLICY_DOMAIN: &[u8] =
    b"FE2O3/AUTHENTICATED-VERUS-PTRACE-CHECKPOINT-POLICY/V2\0";
const RESULT_MAGIC: &str = "FE2O3-AUTHENTICATED-VERUS-RESULT-V2-PIDFD-NONCE2";
const CONTROL_MAGIC: &str = "FE2O3-VERUS-EXECUTION-V2-PIDFD-NONCE2";
const ADDRESS_SPACE_LIMIT_V2: u64 = 8 * 1024 * 1024 * 1024;
const DATA_LIMIT_V2: u64 = 4 * 1024 * 1024 * 1024;
const FILE_LIMIT_V2: u64 = 16 * 1024 * 1024;
const CORE_LIMIT_V2: u64 = 0;
const MAX_LIVE_EXECUTABLE_MAPPING_BYTES_V2: u64 = 512 * 1024 * 1024;
const MAX_LIVE_EXECUTABLE_TOTAL_BYTES_V2: u64 = 1024 * 1024 * 1024;
const MAX_RUNTIME_FILE_BYTES_V2: u64 = 1024 * 1024 * 1024;
const MAX_RUNTIME_TOTAL_BYTES_V2: u64 = 2 * 1024 * 1024 * 1024;
const MAX_VDSO_BYTES_V2: u64 = 64 * 1024;
const VSYSCALL_START_V2: u64 = 0xffff_ffff_ff60_0000;
const VSYSCALL_END_V2: u64 = VSYSCALL_START_V2 + 4096;
const AUDIT_ARCH_X86_64_V2: u32 = 0xc000_003e;
const X32_SYSCALL_BIT_V2: u32 = 0x4000_0000;
const BPF_LOAD_WORD_ABSOLUTE_V2: u16 = 0x20;
const BPF_JUMP_EQUAL_V2: u16 = 0x15;
const BPF_JUMP_GREATER_EQUAL_V2: u16 = 0x35;
const BPF_RETURN_V2: u16 = 0x06;
const SECCOMP_RETURN_KILL_PROCESS_V2: u32 = 0x8000_0000;
const SECCOMP_RETURN_ALLOW_V2: u32 = 0x7fff_0000;
const PTRACE_CONT_V2: u32 = 7;
const PTRACE_DETACH_V2: u32 = 17;
const PTRACE_SEIZE_V2: u32 = 0x4206;
const PTRACE_INTERRUPT_V2: u32 = 0x4207;
const PTRACE_EVENT_STOP_V2: u32 = 128;
const PTRACE_EVENT_EXEC_V2: u32 = 4;
const PTRACE_O_TRACEEXEC_V2: u32 = 0x0000_0010;
const PTRACE_O_EXITKILL_V2: u32 = 0x0010_0000;
const PTRACE_SIGTRAP_V2: u32 = 5;
const PTRACE_WAIT_NOHANG_V2: u32 = 0x0000_0001;
const PTRACE_WAITPID_WALL_V2: u32 = 0x4000_0000;
const DENIED_SYSCALLS_V2: [u32; 17] = [
    56, 57, 58, 435, // clone, fork, vfork, clone3
    105, 106, 113, 114, 117, 119, 116, 122, 123, // UID/GID/group mutation
    126, 157, // capset, prctl
    272, 308, // unshare, setns
];

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeccompInstructionV2 {
    code: u16,
    jump_true: u8,
    jump_false: u8,
    value: u32,
}

const fn bpf_statement_v2(code: u16, value: u32) -> SeccompInstructionV2 {
    SeccompInstructionV2 {
        code,
        jump_true: 0,
        jump_false: 0,
        value,
    }
}

const fn bpf_jump_v2(code: u16, value: u32, jump_true: u8, jump_false: u8) -> SeccompInstructionV2 {
    SeccompInstructionV2 {
        code,
        jump_true,
        jump_false,
        value,
    }
}

const fn seccomp_filter_v2() -> [SeccompInstructionV2; 41] {
    let mut filters = [bpf_statement_v2(BPF_RETURN_V2, SECCOMP_RETURN_KILL_PROCESS_V2); 41];
    filters[0] = bpf_statement_v2(BPF_LOAD_WORD_ABSOLUTE_V2, 4);
    filters[1] = bpf_jump_v2(BPF_JUMP_EQUAL_V2, AUDIT_ARCH_X86_64_V2, 1, 0);
    filters[2] = bpf_statement_v2(BPF_RETURN_V2, SECCOMP_RETURN_KILL_PROCESS_V2);
    filters[3] = bpf_statement_v2(BPF_LOAD_WORD_ABSOLUTE_V2, 0);
    filters[4] = bpf_jump_v2(BPF_JUMP_GREATER_EQUAL_V2, X32_SYSCALL_BIT_V2, 0, 1);
    filters[5] = bpf_statement_v2(BPF_RETURN_V2, SECCOMP_RETURN_KILL_PROCESS_V2);
    let mut denied = 0;
    while denied < DENIED_SYSCALLS_V2.len() {
        filters[6 + denied * 2] = bpf_jump_v2(BPF_JUMP_EQUAL_V2, DENIED_SYSCALLS_V2[denied], 0, 1);
        filters[7 + denied * 2] = bpf_statement_v2(BPF_RETURN_V2, SECCOMP_RETURN_KILL_PROCESS_V2);
        denied += 1;
    }
    filters[40] = bpf_statement_v2(BPF_RETURN_V2, SECCOMP_RETURN_ALLOW_V2);
    filters
}

const SECCOMP_FILTER_V2: [SeccompInstructionV2; 41] = seccomp_filter_v2();

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
core::arch::global_asm!(
    r#"
    .pushsection .text.fe2o3_authenticated_verus_child_v2,"ax",@progbits
    .balign 16
    .global fe2o3_authenticated_verus_clone_launcher_v2
    .hidden fe2o3_authenticated_verus_clone_launcher_v2
    .type fe2o3_authenticated_verus_clone_launcher_v2,@function
fe2o3_authenticated_verus_clone_launcher_v2:
    mov r8, rdi
    mov rdi, rsi
    mov rsi, rdx
    mov eax, 435
    syscall
    test rax, rax
    jnz .Lfe2o3_clone_parent
    mov rdi, r8
    jmp fe2o3_authenticated_verus_child_trampoline_v2
.Lfe2o3_clone_parent:
    ret
    .global fe2o3_authenticated_verus_clone_launcher_v2_end
    .hidden fe2o3_authenticated_verus_clone_launcher_v2_end
fe2o3_authenticated_verus_clone_launcher_v2_end:
    .size fe2o3_authenticated_verus_clone_launcher_v2, .-fe2o3_authenticated_verus_clone_launcher_v2

    .balign 16
    .global fe2o3_authenticated_verus_child_trampoline_v2
    .hidden fe2o3_authenticated_verus_child_trampoline_v2
    .type fe2o3_authenticated_verus_child_trampoline_v2,@function
fe2o3_authenticated_verus_child_trampoline_v2:
    mov r12, rdi

    xor eax, eax
    mov edi, DWORD PTR [r12 + 188]
    lea rsi, [r12 + 185]
    mov edx, 1
    syscall
    cmp rax, 1
    jne .Lfe2o3_child_fail

    mov eax, 33
    mov edi, DWORD PTR [r12 + 32]
    xor esi, esi
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 33
    mov edi, DWORD PTR [r12 + 36]
    mov esi, 1
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 33
    mov edi, DWORD PTR [r12 + 40]
    mov esi, 2
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 436
    mov edi, 3
    mov esi, 0xffffffff
    mov edx, 4
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 72
    mov edi, DWORD PTR [r12 + 44]
    mov esi, 2
    xor edx, edx
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 72
    mov edi, DWORD PTR [r12 + 48]
    mov esi, 2
    xor edx, edx
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov r13, QWORD PTR [r12 + 56]
    mov r14, QWORD PTR [r12 + 64]
    xor r15d, r15d
.Lfe2o3_child_inherited_loop:
    cmp r15, r14
    jae .Lfe2o3_child_inherited_done
    mov eax, 72
    mov edi, DWORD PTR [r13 + r15*4]
    mov esi, 2
    xor edx, edx
    syscall
    test rax, rax
    js .Lfe2o3_child_fail
    inc r15
    jmp .Lfe2o3_child_inherited_loop
.Lfe2o3_child_inherited_done:

    mov eax, 112
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 80
    mov rdi, QWORD PTR [r12 + 24]
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 160
    mov edi, 9
    lea rsi, [r12 + 72]
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 160
    mov edi, 2
    lea rsi, [r12 + 88]
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 160
    mov edi, 1
    lea rsi, [r12 + 104]
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 160
    mov edi, 4
    lea rsi, [r12 + 120]
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 126
    lea rdi, [r12 + 136]
    lea rsi, [r12 + 144]
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 157
    mov edi, 47
    mov esi, 4
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 157
    mov edi, 38
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 157
    mov edi, 22
    mov esi, 2
    lea rdx, [r12 + 168]
    xor r10d, r10d
    xor r8d, r8d
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 72
    mov edi, DWORD PTR [r12 + 44]
    mov esi, 2
    mov edx, 1
    syscall
    test rax, rax
    js .Lfe2o3_child_fail

    mov eax, 59
    mov rdi, QWORD PTR [r12]
    mov rsi, QWORD PTR [r12 + 8]
    mov rdx, QWORD PTR [r12 + 16]
    syscall

.Lfe2o3_child_fail:
    mov eax, 1
    mov edi, DWORD PTR [r12 + 44]
    lea rsi, [r12 + 184]
    mov edx, 1
    syscall
.Lfe2o3_child_exit:
    mov eax, 231
    mov edi, 127
    syscall
    jmp .Lfe2o3_child_exit

    .global fe2o3_authenticated_verus_child_trampoline_v2_end
    .hidden fe2o3_authenticated_verus_child_trampoline_v2_end
fe2o3_authenticated_verus_child_trampoline_v2_end:
    .size fe2o3_authenticated_verus_child_trampoline_v2, .-fe2o3_authenticated_verus_child_trampoline_v2
    .popsection
"#,
);

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe extern "C" {
    fn fe2o3_authenticated_verus_clone_launcher_v2(
        context: *const core::ffi::c_void,
        clone_arguments: *mut core::ffi::c_void,
        clone_arguments_size: usize,
    ) -> isize;
    static fe2o3_authenticated_verus_clone_launcher_v2_end: u8;
    fn fe2o3_authenticated_verus_child_trampoline_v2(context: *const core::ffi::c_void) -> !;
    static fe2o3_authenticated_verus_child_trampoline_v2_end: u8;
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn child_clone_launcher_bytes_v2() -> &'static [u8] {
    let start_pointer = fe2o3_authenticated_verus_clone_launcher_v2 as *const u8;
    let start = start_pointer.addr();
    let end = (&raw const fe2o3_authenticated_verus_clone_launcher_v2_end).addr();
    let length = end.saturating_sub(start);
    if length == 0 || length > 4096 {
        return &[];
    }
    // SAFETY: the linker symbols bound one immutable executable section in this image.
    unsafe { std::slice::from_raw_parts(start_pointer, length) }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn child_clone_launcher_bytes_v2() -> &'static [u8] {
    &[]
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn child_trampoline_bytes_v2() -> &'static [u8] {
    let start_pointer = fe2o3_authenticated_verus_child_trampoline_v2 as *const u8;
    let start = start_pointer.addr();
    let end = (&raw const fe2o3_authenticated_verus_child_trampoline_v2_end).addr();
    let length = end.saturating_sub(start);
    if length == 0 || length > 4096 {
        return &[];
    }
    // SAFETY: the linker symbols bound one immutable executable section in this image.
    unsafe { std::slice::from_raw_parts(start_pointer, length) }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn child_trampoline_bytes_v2() -> &'static [u8] {
    &[]
}

fn child_trampoline_policy_bytes_v2() -> Vec<u8> {
    child_trampoline_policy_bytes_from_v2(
        child_clone_launcher_bytes_v2(),
        child_trampoline_bytes_v2(),
    )
}

fn child_trampoline_policy_bytes_from_v2(launcher: &[u8], trampoline: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(CHILD_TRAMPOLINE_POLICY_DOMAIN);
    put_u32(&mut bytes, AUDIT_ARCH_X86_64_V2);
    put_blob(&mut bytes, launcher);
    put_blob(&mut bytes, trampoline);
    bytes
}

fn ptrace_checkpoint_policy_bytes_v2() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(PTRACE_CHECKPOINT_POLICY_DOMAIN);
    put_u32(&mut bytes, PTRACE_SEIZE_V2);
    put_u32(&mut bytes, PTRACE_INTERRUPT_V2);
    put_u32(&mut bytes, PTRACE_CONT_V2);
    put_u32(&mut bytes, PTRACE_DETACH_V2);
    put_u32(&mut bytes, PTRACE_O_EXITKILL_V2);
    put_u32(&mut bytes, PTRACE_O_TRACEEXEC_V2);
    put_u32(&mut bytes, PTRACE_EVENT_EXEC_V2);
    put_u32(&mut bytes, PTRACE_EVENT_STOP_V2);
    put_u32(&mut bytes, PTRACE_SIGTRAP_V2);
    put_u32(&mut bytes, PTRACE_WAIT_NOHANG_V2);
    put_u32(&mut bytes, PTRACE_WAITPID_WALL_V2);
    bytes
}

fn seccomp_filter_policy_bytes_v2(instructions: &[SeccompInstructionV2]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(SECCOMP_FILTER_POLICY_DOMAIN);
    put_u32(&mut bytes, AUDIT_ARCH_X86_64_V2);
    put_u32(&mut bytes, X32_SYSCALL_BIT_V2);
    put_u32(&mut bytes, DENIED_SYSCALLS_V2.len() as u32);
    for syscall in DENIED_SYSCALLS_V2 {
        put_u32(&mut bytes, syscall);
    }
    put_u32(&mut bytes, instructions.len() as u32);
    for instruction in instructions {
        put_u16(&mut bytes, instruction.code);
        bytes.push(instruction.jump_true);
        bytes.push(instruction.jump_false);
        put_u32(&mut bytes, instruction.value);
    }
    bytes
}

fn anonymous_mapping_policy_bytes_v2() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(ANONYMOUS_MAPPING_POLICY_DOMAIN);
    put_u64(&mut bytes, MAX_VDSO_BYTES_V2);
    bytes.extend_from_slice(b"r-xp");
    put_u64(&mut bytes, VSYSCALL_START_V2);
    put_u64(&mut bytes, VSYSCALL_END_V2);
    bytes.extend_from_slice(b"--xp");
    bytes
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VerusExecutionRoleV2 {
    Solver,
    Verus,
}

impl VerusExecutionRoleV2 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Solver => "solver",
            Self::Verus => "verus",
        }
    }

    const fn memfd_name(self) -> &'static str {
        match self {
            Self::Solver => "fe2o3-solver-v2",
            Self::Verus => "fe2o3-verus-v2",
        }
    }
}

/// ASLR-stable identity of all file-backed mappings observed for one process.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeClosureMeasurementV2 {
    digest: Digest,
    file_count: u32,
    total_bytes: u64,
}

impl RuntimeClosureMeasurementV2 {
    pub const fn from_parts(digest: Digest, file_count: u32, total_bytes: u64) -> Self {
        Self {
            digest,
            file_count,
            total_bytes,
        }
    }

    pub const fn digest(self) -> Digest {
        self.digest
    }

    pub const fn file_count(self) -> u32 {
        self.file_count
    }

    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }
}

/// ASLR-independent reviewed identity of every executable VMA at a checkpoint.
///
/// File-backed executable pages are verified byte-for-byte against their pinned
/// backing-object slices before this identity is constructed. `vdso_digest`
/// separately pins the live kernel vDSO bytes represented in `digest`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeExecutableBaselineV2 {
    digest: Digest,
    mapping_count: u32,
    total_bytes: u64,
    vdso_digest: Digest,
}

impl RuntimeExecutableBaselineV2 {
    pub const fn from_parts(
        digest: Digest,
        mapping_count: u32,
        total_bytes: u64,
        vdso_digest: Digest,
    ) -> Self {
        Self {
            digest,
            mapping_count,
            total_bytes,
            vdso_digest,
        }
    }

    pub const fn digest(self) -> Digest {
        self.digest
    }

    pub const fn mapping_count(self) -> u32 {
        self.mapping_count
    }

    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }

    pub const fn vdso_digest(self) -> Digest {
        self.vdso_digest
    }
}

/// One named, exact dependency blob supplied to both tool stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusExecutionDependencyV2 {
    name: String,
    bytes: Vec<u8>,
}

impl AuthenticatedVerusExecutionDependencyV2 {
    pub fn new(
        name: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, AuthenticatedVerusExecutionErrorV2> {
        let name = name.into();
        if name.is_empty() || name.len() > u16::MAX as usize || name.chars().any(char::is_control) {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::InvalidDependencyName,
            ));
        }
        Ok(Self { name, bytes })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact paths and source closure for one V2 execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusExecutionInputsV2 {
    verus_program: String,
    solver_program: String,
    source: Vec<u8>,
    dependencies: Vec<AuthenticatedVerusExecutionDependencyV2>,
}

impl AuthenticatedVerusExecutionInputsV2 {
    pub fn new(
        verus_program: impl Into<String>,
        solver_program: impl Into<String>,
        source: Vec<u8>,
        mut dependencies: Vec<AuthenticatedVerusExecutionDependencyV2>,
    ) -> Result<Self, AuthenticatedVerusExecutionErrorV2> {
        let verus_program = checked_absolute_path(verus_program.into())?;
        let solver_program = checked_absolute_path(solver_program.into())?;
        if source.is_empty() || source.len() > MAX_VERUS_EXECUTION_SOURCE_BYTES_V2 {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::SourceSizeOutOfRange,
            ));
        }
        if dependencies.len() > MAX_VERUS_EXECUTION_DEPENDENCIES_V2 {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::DependencyClosureOutOfRange,
            ));
        }
        dependencies.sort_by(|left, right| left.name.cmp(&right.name));
        if dependencies
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
        {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::DuplicateDependency,
            ));
        }
        let total = dependencies.iter().try_fold(0_usize, |total, dependency| {
            total.checked_add(dependency.bytes.len())
        });
        if total.is_none_or(|total| total > MAX_VERUS_EXECUTION_DEPENDENCY_BYTES_V2) {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::DependencyClosureOutOfRange,
            ));
        }
        Ok(Self {
            verus_program,
            solver_program,
            source,
            dependencies,
        })
    }

    pub fn verus_program(&self) -> &str {
        &self.verus_program
    }

    pub fn solver_program(&self) -> &str {
        &self.solver_program
    }

    pub fn source(&self) -> &[u8] {
        &self.source
    }

    pub fn dependencies(&self) -> &[AuthenticatedVerusExecutionDependencyV2] {
        &self.dependencies
    }
}

/// Caller-provided deployment review record for the bounded V2 controller.
///
/// `review_digest` names an external review; this crate neither authenticates
/// the reviewer nor decides whether a measured runtime closure is acceptable.
/// Construction requires exact Verus and solver closure measurements so an
/// execution cannot silently calibrate its own allowlist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusExecutionPolicyV2 {
    verifier_policy: VerifierPolicy,
    review_digest: Digest,
    solver_runtime_closure: RuntimeClosureMeasurementV2,
    solver_executable_baseline: RuntimeExecutableBaselineV2,
    verus_runtime_closure: RuntimeClosureMeasurementV2,
    verus_executable_baseline: RuntimeExecutableBaselineV2,
    timeout_seconds: u32,
    output_limits: ExecutionLimits,
}

impl AuthenticatedVerusExecutionPolicyV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        verifier_policy: VerifierPolicy,
        review_digest: Digest,
        solver_runtime_closure: RuntimeClosureMeasurementV2,
        solver_executable_baseline: RuntimeExecutableBaselineV2,
        verus_runtime_closure: RuntimeClosureMeasurementV2,
        verus_executable_baseline: RuntimeExecutableBaselineV2,
        timeout_seconds: u32,
        output_limits: ExecutionLimits,
    ) -> Result<Self, AuthenticatedVerusExecutionErrorV2> {
        if review_digest.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::InvalidReviewDigest,
            ));
        }
        if timeout_seconds == 0
            || timeout_seconds > MAX_TIMEOUT_SECONDS
            || timeout_seconds > verifier_policy.max_timeout_seconds()
        {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::Plan(PlanError::TimeoutOutOfRange {
                    max: verifier_policy.max_timeout_seconds(),
                }),
            ));
        }
        Ok(Self {
            verifier_policy,
            review_digest,
            solver_runtime_closure,
            solver_executable_baseline,
            verus_runtime_closure,
            verus_executable_baseline,
            timeout_seconds,
            output_limits,
        })
    }

    pub const fn verifier_policy(&self) -> &VerifierPolicy {
        &self.verifier_policy
    }

    pub const fn review_digest(&self) -> Digest {
        self.review_digest
    }

    pub const fn solver_runtime_closure(&self) -> RuntimeClosureMeasurementV2 {
        self.solver_runtime_closure
    }

    pub const fn verus_runtime_closure(&self) -> RuntimeClosureMeasurementV2 {
        self.verus_runtime_closure
    }

    pub const fn solver_executable_baseline(&self) -> RuntimeExecutableBaselineV2 {
        self.solver_executable_baseline
    }

    pub const fn verus_executable_baseline(&self) -> RuntimeExecutableBaselineV2 {
        self.verus_executable_baseline
    }

    pub const fn timeout_seconds(&self) -> u32 {
        self.timeout_seconds
    }

    pub const fn output_limits(&self) -> ExecutionLimits {
        self.output_limits
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let verifier = self.verifier_policy.to_canonical_bytes();
        let mut bytes = Vec::with_capacity(verifier.len() + 256);
        bytes.extend_from_slice(POLICY_DOMAIN);
        put_u32(&mut bytes, verifier.len() as u32);
        bytes.extend_from_slice(&verifier);
        bytes.extend_from_slice(self.review_digest.as_bytes());
        for (closure, baseline) in [
            (self.solver_runtime_closure, self.solver_executable_baseline),
            (self.verus_runtime_closure, self.verus_executable_baseline),
        ] {
            bytes.extend_from_slice(closure.digest.as_bytes());
            put_u32(&mut bytes, closure.file_count);
            put_u64(&mut bytes, closure.total_bytes);
            bytes.extend_from_slice(baseline.digest.as_bytes());
            put_u32(&mut bytes, baseline.mapping_count);
            put_u64(&mut bytes, baseline.total_bytes);
            bytes.extend_from_slice(baseline.vdso_digest.as_bytes());
        }
        put_u32(&mut bytes, self.timeout_seconds);
        put_u64(&mut bytes, self.output_limits.max_stdout_bytes() as u64);
        put_u64(&mut bytes, self.output_limits.max_stderr_bytes() as u64);
        for limit in [
            ADDRESS_SPACE_LIMIT_V2,
            DATA_LIMIT_V2,
            FILE_LIMIT_V2,
            CORE_LIMIT_V2,
        ] {
            put_u64(&mut bytes, limit);
        }
        put_u64(&mut bytes, MAX_LIVE_EXECUTABLE_MAPPING_BYTES_V2);
        put_u64(&mut bytes, MAX_LIVE_EXECUTABLE_TOTAL_BYTES_V2);
        put_u64(&mut bytes, MAX_RUNTIME_FILE_BYTES_V2);
        put_u64(&mut bytes, MAX_RUNTIME_TOTAL_BYTES_V2);
        put_blob(
            &mut bytes,
            &seccomp_filter_policy_bytes_v2(&SECCOMP_FILTER_V2),
        );
        put_blob(&mut bytes, &child_trampoline_policy_bytes_v2());
        put_blob(&mut bytes, &ptrace_checkpoint_policy_bytes_v2());
        put_blob(&mut bytes, &anonymous_mapping_policy_bytes_v2());
        bytes
    }
}

/// Exact bounded bytes retained by the V2 receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundExecutionPayloadV2 {
    bytes: Vec<u8>,
    digest: Digest,
}

impl BoundExecutionPayloadV2 {
    fn new(bytes: Vec<u8>) -> Self {
        let digest = sha256(&bytes);
        Self { bytes, digest }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusProcessOccurrenceV2 {
    role: VerusExecutionRoleV2,
    execution_nonce: Digest,
    executable: MeasuredToolIdentity,
    runtime_closure: RuntimeClosureMeasurementV2,
    executable_baseline: RuntimeExecutableBaselineV2,
    runtime_mappings_digest: Digest,
    executable_pages_before_digest: Digest,
    executable_pages_after_digest: Digest,
    process_security_digest: Digest,
}

impl AuthenticatedVerusProcessOccurrenceV2 {
    pub const fn role(&self) -> VerusExecutionRoleV2 {
        self.role
    }

    /// Nonce generated only after this target's first frozen observation.
    pub const fn execution_nonce(&self) -> Digest {
        self.execution_nonce
    }

    pub const fn executable(&self) -> &MeasuredToolIdentity {
        &self.executable
    }

    pub const fn runtime_closure(&self) -> RuntimeClosureMeasurementV2 {
        self.runtime_closure
    }

    /// Stable reviewed executable mapping identity. Unlike the raw checkpoint
    /// page aggregates, this value excludes ASLR addresses.
    pub const fn executable_baseline(&self) -> RuntimeExecutableBaselineV2 {
        self.executable_baseline
    }

    pub const fn runtime_mappings_digest(&self) -> Digest {
        self.runtime_mappings_digest
    }

    /// Digest of live bytes for readable executable VMAs at READY, plus the typed
    /// unreadable marker for the exact kernel vsyscall VMA.
    pub const fn executable_pages_before_digest(&self) -> Digest {
        self.executable_pages_before_digest
    }

    /// Digest of live bytes for readable executable VMAs at DONE, plus the typed
    /// unreadable marker for the exact kernel vsyscall VMA.
    pub const fn executable_pages_after_digest(&self) -> Digest {
        self.executable_pages_after_digest
    }

    /// Digest of the exact limits, capability, seccomp, and thread observations.
    pub const fn process_security_digest(&self) -> Digest {
        self.process_security_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusToolExecutionV2 {
    occurrence: AuthenticatedVerusProcessOccurrenceV2,
    stdout: BoundExecutionPayloadV2,
    stderr: BoundExecutionPayloadV2,
    result_envelope: BoundExecutionPayloadV2,
    result_payload: BoundExecutionPayloadV2,
}

impl AuthenticatedVerusToolExecutionV2 {
    pub const fn occurrence(&self) -> &AuthenticatedVerusProcessOccurrenceV2 {
        &self.occurrence
    }

    pub const fn stdout(&self) -> &BoundExecutionPayloadV2 {
        &self.stdout
    }

    pub const fn stderr(&self) -> &BoundExecutionPayloadV2 {
        &self.stderr
    }

    pub const fn result_envelope(&self) -> &BoundExecutionPayloadV2 {
        &self.result_envelope
    }

    pub const fn result_payload(&self) -> &BoundExecutionPayloadV2 {
        &self.result_payload
    }
}

/// Non-duplicable evidence that both pinned protocol-tool occurrences completed.
///
/// The receipt is not `Clone` and has no public constructor. It authenticates
/// direct process occurrence at the authenticated checkpoints and exact opaque
/// outputs only. It does not claim that no transient executable substitution
/// occurred between observations. It grants no proof, artifact publication,
/// module load, or kernel launch authority.
///
/// ```compile_fail
/// fn duplicate(receipt: fe2o3_verifier::AuthenticatedVerusExecutionReceiptV2) {
///     let _copy = receipt.clone();
/// }
/// ```
///
/// ```compile_fail
/// fn cannot_publish(receipt: fe2o3_verifier::AuthenticatedVerusExecutionReceiptV2) {
///     receipt.publish();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusExecutionReceiptV2 {
    challenge: Digest,
    request: BoundExecutionPayloadV2,
    policy_digest: Digest,
    source: BoundExecutionPayloadV2,
    dependencies_digest: Digest,
    solver: AuthenticatedVerusToolExecutionV2,
    verus: AuthenticatedVerusToolExecutionV2,
    transcript: Vec<u8>,
    transcript_digest: Digest,
}

impl AuthenticatedVerusExecutionReceiptV2 {
    pub const fn challenge(&self) -> Digest {
        self.challenge
    }

    pub const fn request(&self) -> &BoundExecutionPayloadV2 {
        &self.request
    }

    pub const fn policy_digest(&self) -> Digest {
        self.policy_digest
    }

    pub const fn source(&self) -> &BoundExecutionPayloadV2 {
        &self.source
    }

    pub const fn dependencies_digest(&self) -> Digest {
        self.dependencies_digest
    }

    pub const fn solver(&self) -> &AuthenticatedVerusToolExecutionV2 {
        &self.solver
    }

    pub const fn verus(&self) -> &AuthenticatedVerusToolExecutionV2 {
        &self.verus
    }

    /// The direct Verus process completed the authenticated checkpoint protocol.
    /// This is not an exclusive measured-image-execution claim.
    pub const fn authenticates_verus_process_occurrence(&self) -> bool {
        true
    }

    /// The direct solver process completed the authenticated checkpoint protocol.
    /// This is not an exclusive measured-image-execution claim.
    pub const fn authenticates_solver_process_occurrence(&self) -> bool {
        true
    }

    /// Executable path and page bytes are authenticated at frozen checkpoints,
    /// but executable changes entirely between observations are outside the claim.
    pub const fn authenticates_exclusive_measured_image_execution(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn transcript_digest(&self) -> Digest {
        self.transcript_digest
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        self.transcript.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessFailureV2 {
    Spawn,
    Observe,
    PrivilegedController,
    ControlProtocol,
    OutputTooLarge,
    Timeout,
    ProcessPolicyMismatch,
    ExecutableSubstitution,
    AnonymousExecutableMapping,
    WritableExecutableMapping,
    WritableExecutableAlias,
    ExecutablePageUnreadable,
    ExecutableMappingTooLarge,
    ExecutablePagesChanged,
    RuntimeClosureChanged,
    ExecutableBaselineViolation,
    UnexpectedPtraceStop,
    TerminationUnconfirmed,
    ExitFailure,
    UnexpectedStderr,
    ResultEnvelope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthenticatedVerusExecutionErrorKindV2 {
    UnsupportedPlatform,
    InvalidPath,
    InvalidDependencyName,
    DuplicateDependency,
    SourceSizeOutOfRange,
    DependencyClosureOutOfRange,
    InvalidReviewDigest,
    InvalidChallenge,
    Plan(PlanError),
    SourceRequestMismatch {
        expected: Digest,
        measured: Digest,
    },
    DependencyRequestMismatch {
        expected: Digest,
        measured: Digest,
    },
    ExecutableDigestMismatch {
        role: VerusExecutionRoleV2,
        expected: Digest,
        measured: Digest,
    },
    RuntimeClosureMismatch {
        role: VerusExecutionRoleV2,
        expected: RuntimeClosureMeasurementV2,
        measured: RuntimeClosureMeasurementV2,
    },
    RuntimeExecutableBaselineMismatch {
        role: VerusExecutionRoleV2,
        expected: Box<RuntimeExecutableBaselineV2>,
        measured: Box<RuntimeExecutableBaselineV2>,
    },
    Process {
        role: VerusExecutionRoleV2,
        failure: ProcessFailureV2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusExecutionErrorV2 {
    kind: AuthenticatedVerusExecutionErrorKindV2,
    io_kind: Option<std::io::ErrorKind>,
}

impl AuthenticatedVerusExecutionErrorV2 {
    fn plain(kind: AuthenticatedVerusExecutionErrorKindV2) -> Self {
        Self {
            kind,
            io_kind: None,
        }
    }

    fn io(kind: AuthenticatedVerusExecutionErrorKindV2, error: std::io::Error) -> Self {
        Self {
            kind,
            io_kind: Some(error.kind()),
        }
    }

    pub const fn kind(&self) -> &AuthenticatedVerusExecutionErrorKindV2 {
        &self.kind
    }

    pub const fn io_kind(&self) -> Option<std::io::ErrorKind> {
        self.io_kind
    }
}

impl fmt::Display for AuthenticatedVerusExecutionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "authenticated Verus execution V2 failed: {:?}",
            self.kind
        )
    }
}

impl std::error::Error for AuthenticatedVerusExecutionErrorV2 {}

fn checked_absolute_path(path: String) -> Result<String, AuthenticatedVerusExecutionErrorV2> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.chars().any(char::is_control)
        || !std::path::Path::new(&path).is_absolute()
    {
        Err(AuthenticatedVerusExecutionErrorV2::plain(
            AuthenticatedVerusExecutionErrorKindV2::InvalidPath,
        ))
    } else {
        Ok(path)
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_blob(bytes: &mut Vec<u8>, value: &[u8]) {
    put_u64(bytes, value.len() as u64);
    bytes.extend_from_slice(value);
}

fn put_text(bytes: &mut Vec<u8>, value: &str) {
    put_u16(bytes, value.len() as u16);
    bytes.extend_from_slice(value.as_bytes());
}

fn dependencies_digest(dependencies: &[AuthenticatedVerusExecutionDependencyV2]) -> Digest {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(DEPENDENCIES_DOMAIN);
    put_u32(&mut canonical, dependencies.len() as u32);
    for dependency in dependencies {
        put_text(&mut canonical, &dependency.name);
        put_u64(&mut canonical, dependency.bytes.len() as u64);
        canonical.extend_from_slice(sha256(&dependency.bytes).as_bytes());
    }
    sha256(&canonical)
}

/// Executes the solver and Verus V2 protocol stages under one fresh challenge.
///
/// On Linux x86_64 this API authenticates pidfd-owned direct process occurrence,
/// frozen runtime/security checkpoints, exact inputs, and immutable opaque
/// result bytes. It does not parse a Verus proof result, does not integrate the
/// stock Verus CLI, and intentionally grants no downstream authority.
pub fn execute_authenticated_verus_v2(
    request: ProofRequestV1,
    inputs: AuthenticatedVerusExecutionInputsV2,
    policy: &AuthenticatedVerusExecutionPolicyV2,
) -> Result<AuthenticatedVerusExecutionReceiptV2, AuthenticatedVerusExecutionErrorV2> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        platform::execute(request, inputs, policy)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (request, inputs, policy);
        Err(AuthenticatedVerusExecutionErrorV2::plain(
            AuthenticatedVerusExecutionErrorKindV2::UnsupportedPlatform,
        ))
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod platform {
    use super::*;
    use sha2::{Digest as _, Sha256};
    use std::{
        collections::BTreeMap,
        ffi::CString,
        fs::{File, Metadata, OpenOptions},
        io::{self, Read, Seek, SeekFrom, Write},
        mem::{offset_of, size_of},
        os::{
            fd::{AsRawFd, FromRawFd, OwnedFd},
            raw::{c_char, c_int, c_long, c_uint, c_void},
            unix::{
                fs::{FileExt, MetadataExt},
                net::UnixStream,
            },
        },
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    const RANDOM_SOURCE: &str = "/dev/urandom";
    const IO_CHUNK_BYTES: usize = 64 * 1024;
    const POLL_INTERVAL: Duration = Duration::from_millis(2);
    const CAPTURE_GRACE: Duration = Duration::from_millis(200);
    const CONTROL_EOF_CLASSIFICATION_GRACE: Duration = Duration::from_millis(100);
    const CLEANUP_TIMEOUT: Duration = Duration::from_millis(500);
    const MAX_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;
    const MAX_RUNTIME_FILES: usize = 256;
    const MAX_RUNTIME_MAPPINGS: usize = 4096;
    const MAX_PROC_MAPS_BYTES: usize = 1024 * 1024;
    const MAX_CONTROL_FRAME_BYTES: usize = 256;
    const MAX_SUPPLEMENTARY_GROUPS: usize = 64;
    const SIGKILL: c_int = 9;
    const SIGTRAP: c_int = PTRACE_SIGTRAP_V2 as c_int;
    const RLIMIT_FSIZE: c_int = 1;
    const RLIMIT_DATA: c_int = 2;
    const RLIMIT_CORE: c_int = 4;
    const RLIMIT_AS: c_int = 9;
    const WAIT_NOHANG: c_int = 0x0000_0001;
    const WAIT_EXITED: c_int = 0x0000_0004;
    const WAIT_NOWAIT: c_int = 0x0100_0000;
    const CLD_EXITED: c_int = 1;
    const CLD_KILLED: c_int = 2;
    const CLD_DUMPED: c_int = 3;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Snapshot {
        device: u64,
        inode: u64,
        mode: u32,
        size: u64,
        modified_seconds: i64,
        modified_nanoseconds: i64,
        changed_seconds: i64,
        changed_nanoseconds: i64,
    }

    impl Snapshot {
        fn from_metadata(metadata: &Metadata) -> Self {
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                mode: metadata.mode(),
                size: metadata.len(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
            }
        }
    }

    struct SealedExecutable {
        file: File,
        measurement: MeasuredToolIdentity,
        byte_len: u64,
        snapshot: Snapshot,
    }

    impl SealedExecutable {
        fn capture(
            role: VerusExecutionRoleV2,
            path: &str,
            expected: &MeasuredToolIdentity,
        ) -> Result<Self, AuthenticatedVerusExecutionErrorV2> {
            let mut source = File::open(path).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Spawn),
                    error,
                )
            })?;
            let initial = Snapshot::from_metadata(&source.metadata().map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Observe),
                    error,
                )
            })?);
            if initial.size == 0 || initial.size > MAX_EXECUTABLE_BYTES {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::Observe,
                )));
            }
            let mut file = create_memfd(role.memfd_name()).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Spawn),
                    error,
                )
            })?;
            let mut hasher = Sha256::new();
            let mut total = 0_u64;
            let mut buffer = [0_u8; IO_CHUNK_BYTES];
            loop {
                let count = read_retry(&mut source, &mut buffer).map_err(|error| {
                    AuthenticatedVerusExecutionErrorV2::io(
                        process_error(role, ProcessFailureV2::Observe),
                        error,
                    )
                })?;
                if count == 0 {
                    break;
                }
                total = total.checked_add(count as u64).ok_or_else(|| {
                    AuthenticatedVerusExecutionErrorV2::plain(process_error(
                        role,
                        ProcessFailureV2::Observe,
                    ))
                })?;
                if total > MAX_EXECUTABLE_BYTES {
                    return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                        role,
                        ProcessFailureV2::Observe,
                    )));
                }
                hasher.update(&buffer[..count]);
                file.write_all(&buffer[..count]).map_err(|error| {
                    AuthenticatedVerusExecutionErrorV2::io(
                        process_error(role, ProcessFailureV2::Observe),
                        error,
                    )
                })?;
            }
            if total != initial.size
                || Snapshot::from_metadata(&source.metadata().map_err(|error| {
                    AuthenticatedVerusExecutionErrorV2::io(
                        process_error(role, ProcessFailureV2::Observe),
                        error,
                    )
                })?) != initial
            {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ExecutableSubstitution,
                )));
            }
            file.flush().map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Observe),
                    error,
                )
            })?;
            seal(&file).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Observe),
                    error,
                )
            })?;
            let measured = Digest::from_bytes(hasher.finalize().into());
            if measured != expected.executable_digest() {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(
                    AuthenticatedVerusExecutionErrorKindV2::ExecutableDigestMismatch {
                        role,
                        expected: expected.executable_digest(),
                        measured,
                    },
                ));
            }
            let snapshot = Snapshot::from_metadata(&file.metadata().map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Observe),
                    error,
                )
            })?);
            Ok(Self {
                file,
                measurement: expected.clone(),
                byte_len: total,
                snapshot,
            })
        }

        fn path(&self) -> String {
            format!("/proc/self/fd/{}", self.file.as_raw_fd())
        }
    }

    struct SealedBlob {
        file: File,
    }

    impl SealedBlob {
        fn immutable(name: &str, bytes: &[u8]) -> Result<Self, AuthenticatedVerusExecutionErrorV2> {
            let mut value = Self::mutable(name)?;
            value.file.write_all(bytes).map_err(data_io)?;
            value.file.flush().map_err(data_io)?;
            seal(&value.file).map_err(data_io)?;
            value.file.seek(SeekFrom::Start(0)).map_err(data_io)?;
            Ok(value)
        }

        fn mutable(name: &str) -> Result<Self, AuthenticatedVerusExecutionErrorV2> {
            Ok(Self {
                file: create_memfd(name).map_err(data_io)?,
            })
        }

        fn path(&self) -> String {
            format!("/proc/self/fd/{}", self.file.as_raw_fd())
        }

        fn raw_fd(&self) -> c_int {
            self.file.as_raw_fd()
        }

        fn read_immutably_sealed(
            &mut self,
            max: usize,
            role: VerusExecutionRoleV2,
        ) -> Result<Vec<u8>, AuthenticatedVerusExecutionErrorV2> {
            if !is_immutably_sealed(&self.file) {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ResultEnvelope,
                )));
            }
            self.file.seek(SeekFrom::Start(0)).map_err(data_io)?;
            let mut bytes = Vec::with_capacity(max.min(8192));
            Read::by_ref(&mut self.file)
                .take((max + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(data_io)?;
            if bytes.len() > max {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ResultEnvelope,
                )));
            }
            Ok(bytes)
        }
    }

    struct NamedSealedDependency {
        name: String,
        blob: SealedBlob,
    }

    #[derive(Clone, Copy)]
    struct Bindings {
        challenge: Digest,
        request: Digest,
        policy: Digest,
        source: Digest,
        dependencies: Digest,
        verus: Digest,
        solver: Digest,
        predecessor: Digest,
    }

    struct RawStage {
        occurrence: AuthenticatedVerusProcessOccurrenceV2,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        result_envelope: Vec<u8>,
        result_payload: Vec<u8>,
    }

    pub(super) fn execute(
        request: ProofRequestV1,
        inputs: AuthenticatedVerusExecutionInputsV2,
        policy: &AuthenticatedVerusExecutionPolicyV2,
    ) -> Result<AuthenticatedVerusExecutionReceiptV2, AuthenticatedVerusExecutionErrorV2> {
        validate_request_policy(&request, policy)?;
        validate_controller_security()?;
        let request_bytes = request.to_canonical_bytes();
        let request_payload = BoundExecutionPayloadV2::new(request_bytes.clone());
        let source_payload = BoundExecutionPayloadV2::new(inputs.source.clone());
        if source_payload.digest != request.target().source_tree_digest {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::SourceRequestMismatch {
                    expected: request.target().source_tree_digest,
                    measured: source_payload.digest,
                },
            ));
        }
        let dependencies = dependencies_digest(&inputs.dependencies);
        if dependencies != request.target().crate_graph_digest {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::DependencyRequestMismatch {
                    expected: request.target().crate_graph_digest,
                    measured: dependencies,
                },
            ));
        }

        let challenge = fresh_challenge()?;
        let policy_digest = sha256(&policy.to_canonical_bytes());
        let solver_image = SealedExecutable::capture(
            VerusExecutionRoleV2::Solver,
            &inputs.solver_program,
            policy.verifier_policy.expected_tools().solver(),
        )?;
        let verus_image = SealedExecutable::capture(
            VerusExecutionRoleV2::Verus,
            &inputs.verus_program,
            policy.verifier_policy.expected_tools().verifier(),
        )?;
        let request_file = SealedBlob::immutable("fe2o3-verus-request-v2", &request_bytes)?;
        let source_file = SealedBlob::immutable("fe2o3-verus-source-v2", &inputs.source)?;
        let dependency_files = inputs
            .dependencies
            .iter()
            .enumerate()
            .map(|(index, dependency)| {
                Ok(NamedSealedDependency {
                    name: dependency.name.clone(),
                    blob: SealedBlob::immutable(
                        &format!("fe2o3-verus-dependency-v2-{index}"),
                        &dependency.bytes,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, AuthenticatedVerusExecutionErrorV2>>()?;

        let common = Bindings {
            challenge,
            request: request_payload.digest,
            policy: policy_digest,
            source: source_payload.digest,
            dependencies,
            verus: verus_image.measurement.executable_digest(),
            solver: solver_image.measurement.executable_digest(),
            predecessor: Digest::from_bytes([0; 32]),
        };
        let solver = run_stage(StageContext {
            role: VerusExecutionRoleV2::Solver,
            image: &solver_image,
            expected_closure: policy.solver_runtime_closure,
            expected_baseline: policy.solver_executable_baseline,
            request: &request_file,
            source: &source_file,
            dependencies: &dependency_files,
            predecessor: None,
            bindings: common,
            timeout_seconds: policy.timeout_seconds,
            limits: policy.output_limits,
        })?;
        let solver_envelope =
            SealedBlob::immutable("fe2o3-solver-result-v2", &solver.result_envelope)?;
        let mut verus_bindings = common;
        verus_bindings.predecessor = sha256(&solver.result_envelope);
        let verus = run_stage(StageContext {
            role: VerusExecutionRoleV2::Verus,
            image: &verus_image,
            expected_closure: policy.verus_runtime_closure,
            expected_baseline: policy.verus_executable_baseline,
            request: &request_file,
            source: &source_file,
            dependencies: &dependency_files,
            predecessor: Some(&solver_envelope),
            bindings: verus_bindings,
            timeout_seconds: policy.timeout_seconds,
            limits: policy.output_limits,
        })?;

        let solver = finish_stage(solver);
        let verus = finish_stage(verus);
        let transcript = canonical_transcript(
            challenge,
            &request_payload,
            policy_digest,
            &source_payload,
            dependencies,
            &solver,
            &verus,
        );
        let transcript_digest = sha256(&transcript);
        Ok(AuthenticatedVerusExecutionReceiptV2 {
            challenge,
            request: request_payload,
            policy_digest,
            source: source_payload,
            dependencies_digest: dependencies,
            solver,
            verus,
            transcript,
            transcript_digest,
        })
    }

    fn validate_request_policy(
        request: &ProofRequestV1,
        policy: &AuthenticatedVerusExecutionPolicyV2,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        let verifier = &policy.verifier_policy;
        if request.configuration() != verifier.expected_configuration() {
            return Err(plan_error(PlanError::ConfigurationPolicyMismatch));
        }
        if request.model() != verifier.expected_model() {
            return Err(plan_error(PlanError::ModelPolicyMismatch));
        }
        verifier
            .axiom_policy()
            .validate(request.trusted_items())
            .map_err(|error| plan_error(PlanError::Model(error)))
    }

    struct StageContext<'a> {
        role: VerusExecutionRoleV2,
        image: &'a SealedExecutable,
        expected_closure: RuntimeClosureMeasurementV2,
        expected_baseline: RuntimeExecutableBaselineV2,
        request: &'a SealedBlob,
        source: &'a SealedBlob,
        dependencies: &'a [NamedSealedDependency],
        predecessor: Option<&'a SealedBlob>,
        bindings: Bindings,
        timeout_seconds: u32,
        limits: ExecutionLimits,
    }

    fn run_stage(
        context: StageContext<'_>,
    ) -> Result<RawStage, AuthenticatedVerusExecutionErrorV2> {
        let StageContext {
            role,
            image,
            expected_closure,
            expected_baseline,
            request,
            source,
            dependencies,
            predecessor,
            bindings,
            timeout_seconds,
            limits,
        } = context;
        validate_sealed_image(role, image)?;
        let mut result = SealedBlob::mutable(&format!("fe2o3-{}-result-v2", role.as_str()))?;
        let (mut controller, worker_control) = UnixStream::pair().map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                error,
            )
        })?;
        controller.set_nonblocking(true).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::ControlProtocol),
                error,
            )
        })?;
        let control_fd = worker_control.as_raw_fd();
        let mut arguments = Vec::new();
        for argument in [
            format!("fe2o3-{}-v2", role.as_str()),
            "--fe2o3-authenticated-execution-v2".to_owned(),
            "--role".to_owned(),
            role.as_str().to_owned(),
            "--challenge".to_owned(),
            bindings.challenge.to_hex(),
            "--request".to_owned(),
            request.path(),
            "--request-digest".to_owned(),
            bindings.request.to_hex(),
            "--policy-digest".to_owned(),
            bindings.policy.to_hex(),
            "--source".to_owned(),
            source.path(),
            "--source-digest".to_owned(),
            bindings.source.to_hex(),
            "--dependencies-digest".to_owned(),
            bindings.dependencies.to_hex(),
            "--verus-digest".to_owned(),
            bindings.verus.to_hex(),
            "--solver-digest".to_owned(),
            bindings.solver.to_hex(),
            "--predecessor-digest".to_owned(),
            bindings.predecessor.to_hex(),
            "--result".to_owned(),
            result.path(),
            "--control-fd".to_owned(),
            control_fd.to_string(),
        ] {
            arguments.push(checked_c_string(role, argument)?);
        }
        if let Some(predecessor) = predecessor {
            arguments.push(checked_c_string(role, "--predecessor-result")?);
            arguments.push(checked_c_string(role, predecessor.path())?);
        }
        for dependency in dependencies {
            arguments.push(checked_c_string(role, "--dependency")?);
            arguments.push(checked_c_string(role, &dependency.name)?);
            arguments.push(checked_c_string(role, dependency.blob.path())?);
        }
        let mut inherited = [-1; MAX_VERUS_EXECUTION_DEPENDENCIES_V2 + 4];
        let mut inherited_count = 0;
        for descriptor in [request.raw_fd(), source.raw_fd(), result.raw_fd()] {
            inherited[inherited_count] = descriptor;
            inherited_count += 1;
        }
        if let Some(predecessor) = predecessor {
            inherited[inherited_count] = predecessor.raw_fd();
            inherited_count += 1;
        }
        for dependency in dependencies {
            inherited[inherited_count] = dependency.blob.raw_fd();
            inherited_count += 1;
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
            .ok_or_else(|| {
                AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::Timeout,
                ))
            })?;
        let tracer_id = controller_thread_id(role)?;
        let expected_security = expected_process_security(role)?;
        let spawned = spawn_target(
            role,
            image,
            &arguments,
            control_fd,
            &inherited[..inherited_count],
            expected_security.limits,
            deadline,
        )?;
        drop(worker_control);
        let mut child = ChildGuard::new(
            spawned.pidfd,
            spawned.proc_dir,
            spawned.process_id,
            tracer_id,
            role,
        );
        let stdout = CaptureTask::spawn(spawned.stdout, limits.max_stdout_bytes());
        let stderr = CaptureTask::spawn(spawned.stderr, limits.max_stderr_bytes());
        let execution = (|| {
            wait_for_frame(
                &mut child,
                image,
                &mut controller,
                &control_frame("READY", role, bindings.challenge),
                deadline,
                &stdout,
                &stderr,
            )?;
            child.interrupt_and_confirm(deadline)?;
            let before = observe_process(&child, image, expected_security, deadline)?;
            if before.runtime_closure != expected_closure {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(
                    AuthenticatedVerusExecutionErrorKindV2::RuntimeClosureMismatch {
                        role,
                        expected: expected_closure,
                        measured: before.runtime_closure,
                    },
                ));
            }
            if before.executable_baseline != expected_baseline {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(
                    AuthenticatedVerusExecutionErrorKindV2::RuntimeExecutableBaselineMismatch {
                        role,
                        expected: Box::new(expected_baseline),
                        measured: Box::new(before.executable_baseline),
                    },
                ));
            }
            reject_pending_control(role, &controller)?;
            let execution_nonce = fresh_challenge()?;
            controller
                .write_all(&bound_control_frame(
                    "START",
                    role,
                    bindings.challenge,
                    execution_nonce,
                ))
                .map_err(|error| control_io(role, error))?;
            child.continue_target()?;
            wait_for_frame(
                &mut child,
                image,
                &mut controller,
                &bound_control_frame("RESULT", role, bindings.challenge, execution_nonce),
                deadline,
                &stdout,
                &stderr,
            )?;
            child.interrupt_and_confirm(deadline)?;
            reject_pending_control(role, &controller)?;
            let result_envelope = result.read_immutably_sealed(MAX_RESULT_BYTES, role)?;
            let result_payload = parse_result(role, &result_envelope, bindings, execution_nonce)?;
            controller
                .write_all(&bound_control_frame(
                    "SEALED",
                    role,
                    bindings.challenge,
                    execution_nonce,
                ))
                .map_err(|error| control_io(role, error))?;
            child.continue_target()?;
            wait_for_frame(
                &mut child,
                image,
                &mut controller,
                &bound_control_frame("DONE", role, bindings.challenge, execution_nonce),
                deadline,
                &stdout,
                &stderr,
            )?;
            child.interrupt_and_confirm(deadline)?;
            if result.read_immutably_sealed(MAX_RESULT_BYTES, role)? != result_envelope {
                return Err(result_error(role));
            }
            let after = observe_process(&child, image, expected_security, deadline)?;
            if before.executable_pages_digest != after.executable_pages_digest {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ExecutablePagesChanged,
                )));
            }
            if before != after {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::RuntimeClosureChanged,
                )));
            }
            controller
                .write_all(&bound_control_frame(
                    "ACK",
                    role,
                    bindings.challenge,
                    execution_nonce,
                ))
                .map_err(|error| control_io(role, error))?;
            child.detach_and_continue()?;
            let status = wait_for_exit(&mut child, image, deadline, &stdout, &stderr)?;
            if !status.success {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ExitFailure,
                )));
            }
            Ok((
                before,
                after,
                execution_nonce,
                result_envelope,
                result_payload,
            ))
        })();
        if execution.is_err() {
            child.terminate_and_confirm(cleanup_deadline())?;
        }
        let capture = finish_capture(role, stdout, stderr, deadline);
        let (before, after, execution_nonce, result_envelope, result_payload) = execution?;
        let capture = capture?;
        if !capture.1.is_empty() {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::UnexpectedStderr,
            )));
        }
        Ok(RawStage {
            occurrence: AuthenticatedVerusProcessOccurrenceV2 {
                role,
                execution_nonce,
                executable: image.measurement.clone(),
                runtime_closure: before.runtime_closure,
                executable_baseline: before.executable_baseline,
                runtime_mappings_digest: before.runtime_mappings_digest,
                executable_pages_before_digest: before.executable_pages_digest,
                executable_pages_after_digest: after.executable_pages_digest,
                process_security_digest: before.process_security_digest,
            },
            stdout: capture.0,
            stderr: capture.1,
            result_envelope,
            result_payload,
        })
    }

    fn finish_stage(stage: RawStage) -> AuthenticatedVerusToolExecutionV2 {
        AuthenticatedVerusToolExecutionV2 {
            occurrence: stage.occurrence,
            stdout: BoundExecutionPayloadV2::new(stage.stdout),
            stderr: BoundExecutionPayloadV2::new(stage.stderr),
            result_envelope: BoundExecutionPayloadV2::new(stage.result_envelope),
            result_payload: BoundExecutionPayloadV2::new(stage.result_payload),
        }
    }

    fn canonical_transcript(
        challenge: Digest,
        request: &BoundExecutionPayloadV2,
        policy: Digest,
        source: &BoundExecutionPayloadV2,
        dependencies: Digest,
        solver: &AuthenticatedVerusToolExecutionV2,
        verus: &AuthenticatedVerusToolExecutionV2,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(TRANSCRIPT_DOMAIN);
        bytes.extend_from_slice(challenge.as_bytes());
        put_payload(&mut bytes, request);
        bytes.extend_from_slice(policy.as_bytes());
        put_payload(&mut bytes, source);
        bytes.extend_from_slice(dependencies.as_bytes());
        for stage in [solver, verus] {
            let occurrence = &stage.occurrence;
            bytes.push(match occurrence.role {
                VerusExecutionRoleV2::Solver => 1,
                VerusExecutionRoleV2::Verus => 2,
            });
            bytes.extend_from_slice(occurrence.execution_nonce.as_bytes());
            let tool = &occurrence.executable;
            put_text(&mut bytes, tool.name().as_str());
            put_text(&mut bytes, tool.version().as_str());
            bytes.extend_from_slice(tool.executable_digest().as_bytes());
            bytes.extend_from_slice(tool.configuration_digest().as_bytes());
            bytes.extend_from_slice(occurrence.runtime_closure.digest.as_bytes());
            put_u32(&mut bytes, occurrence.runtime_closure.file_count);
            put_u64(&mut bytes, occurrence.runtime_closure.total_bytes);
            bytes.extend_from_slice(occurrence.executable_baseline.digest.as_bytes());
            put_u32(&mut bytes, occurrence.executable_baseline.mapping_count);
            put_u64(&mut bytes, occurrence.executable_baseline.total_bytes);
            bytes.extend_from_slice(occurrence.executable_baseline.vdso_digest.as_bytes());
            bytes.extend_from_slice(occurrence.runtime_mappings_digest.as_bytes());
            bytes.extend_from_slice(occurrence.executable_pages_before_digest.as_bytes());
            bytes.extend_from_slice(occurrence.executable_pages_after_digest.as_bytes());
            bytes.extend_from_slice(occurrence.process_security_digest.as_bytes());
            for payload in [
                &stage.stdout,
                &stage.stderr,
                &stage.result_envelope,
                &stage.result_payload,
            ] {
                put_payload(&mut bytes, payload);
            }
        }
        bytes
    }

    fn put_payload(bytes: &mut Vec<u8>, payload: &BoundExecutionPayloadV2) {
        put_blob(bytes, &payload.bytes);
        bytes.extend_from_slice(payload.digest.as_bytes());
    }

    fn parse_result(
        role: VerusExecutionRoleV2,
        bytes: &[u8],
        bindings: Bindings,
        execution_nonce: Digest,
    ) -> Result<Vec<u8>, AuthenticatedVerusExecutionErrorV2> {
        let text = std::str::from_utf8(bytes).map_err(|_| result_error(role))?;
        let mut remainder = text;
        if take_line(&mut remainder).map_err(|_| result_error(role))? != RESULT_MAGIC {
            return Err(result_error(role));
        }
        let expected = [
            ("role", role.as_str().to_owned()),
            ("challenge", bindings.challenge.to_hex()),
            ("execution-nonce", execution_nonce.to_hex()),
            ("request", bindings.request.to_hex()),
            ("policy", bindings.policy.to_hex()),
            ("source", bindings.source.to_hex()),
            ("dependencies", bindings.dependencies.to_hex()),
            ("verus", bindings.verus.to_hex()),
            ("solver", bindings.solver.to_hex()),
            ("predecessor", bindings.predecessor.to_hex()),
        ];
        for (field, expected) in expected {
            let line = take_line(&mut remainder).map_err(|_| result_error(role))?;
            let actual = line
                .strip_prefix(field)
                .and_then(|value| value.strip_prefix('='))
                .ok_or_else(|| result_error(role))?;
            if actual != expected {
                return Err(result_error(role));
            }
        }
        let length = take_line(&mut remainder).map_err(|_| result_error(role))?;
        let length = length
            .strip_prefix("payload-bytes=")
            .filter(|value| {
                !value.is_empty()
                    && (value.len() == 1 || !value.starts_with('0'))
                    && value.bytes().all(|byte| byte.is_ascii_digit())
            })
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| result_error(role))?;
        if length != remainder.len() {
            return Err(result_error(role));
        }
        Ok(remainder.as_bytes().to_vec())
    }

    fn take_line<'a>(remainder: &mut &'a str) -> Result<&'a str, ()> {
        let (line, rest) = remainder.split_once('\n').ok_or(())?;
        *remainder = rest;
        Ok(line)
    }

    fn control_frame(kind: &str, role: VerusExecutionRoleV2, challenge: Digest) -> Vec<u8> {
        format!(
            "{CONTROL_MAGIC} {kind} {} {}\n",
            role.as_str(),
            challenge.to_hex()
        )
        .into_bytes()
    }

    fn bound_control_frame(
        kind: &str,
        role: VerusExecutionRoleV2,
        challenge: Digest,
        execution_nonce: Digest,
    ) -> Vec<u8> {
        format!(
            "{CONTROL_MAGIC} {kind} {} {} {}\n",
            role.as_str(),
            challenge.to_hex(),
            execution_nonce.to_hex(),
        )
        .into_bytes()
    }

    fn reject_pending_control(
        role: VerusExecutionRoleV2,
        control: &UnixStream,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        const MSG_PEEK: c_int = 0x2;
        const MSG_DONTWAIT: c_int = 0x40;
        let mut byte = [0_u8; 1];
        // SAFETY: `byte` is writable and the control descriptor is a live socket.
        let count = unsafe {
            linux_recv(
                control.as_raw_fd(),
                byte.as_mut_ptr().cast(),
                byte.len(),
                MSG_PEEK | MSG_DONTWAIT,
            )
        };
        if count < 0 {
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::WouldBlock => Ok(()),
                io::ErrorKind::Interrupted => reject_pending_control(role, control),
                _ => Err(control_io(role, error)),
            }
        } else {
            Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::ControlProtocol,
            )))
        }
    }

    fn wait_for_frame(
        child: &mut ChildGuard,
        image: &SealedExecutable,
        control: &mut UnixStream,
        expected: &[u8],
        deadline: Instant,
        stdout: &CaptureTask,
        stderr: &CaptureTask,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        let role = child.role;
        let mut frame = Vec::with_capacity(expected.len());
        let mut buffer = [0_u8; 128];
        loop {
            supervise_iteration(child, image, deadline, stdout, stderr)?;
            match control.read(&mut buffer) {
                Ok(0) => return classify_control_eof(child, image, deadline),
                Ok(count) => {
                    if frame.len() + count > MAX_CONTROL_FRAME_BYTES {
                        return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                            role,
                            ProcessFailureV2::ControlProtocol,
                        )));
                    }
                    frame.extend_from_slice(&buffer[..count]);
                    if frame.ends_with(b"\n") {
                        if frame == expected {
                            return Ok(());
                        }
                        return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                            role,
                            ProcessFailureV2::ControlProtocol,
                        )));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(POLL_INTERVAL);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(control_io(role, error)),
            }
        }
    }

    fn classify_control_eof(
        child: &mut ChildGuard,
        image: &SealedExecutable,
        deadline: Instant,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        let role = child.role;
        let classification_deadline = Instant::now()
            .checked_add(CONTROL_EOF_CLASSIFICATION_GRACE)
            .map_or(deadline, |candidate| candidate.min(deadline));
        loop {
            if child.exit_pending()? {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ControlProtocol,
                )));
            }
            validate_process_executable(child, image)?;
            if let Some(status) = child.ptrace_stop_pending()? {
                let failure = if ptrace_exec_status(status) {
                    ProcessFailureV2::ExecutableSubstitution
                } else {
                    ProcessFailureV2::UnexpectedPtraceStop
                };
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role, failure,
                )));
            }
            if Instant::now() >= classification_deadline {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ControlProtocol,
                )));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn wait_for_exit(
        child: &mut ChildGuard,
        image: &SealedExecutable,
        deadline: Instant,
        stdout: &CaptureTask,
        stderr: &CaptureTask,
    ) -> Result<ProcessExit, AuthenticatedVerusExecutionErrorV2> {
        let role = child.role;
        loop {
            if Instant::now() >= deadline {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::Timeout,
                )));
            }
            if stdout.exceeded() || stderr.exceeded() {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::OutputTooLarge,
                )));
            }
            if let Some(status) = child.try_reap()? {
                return Ok(status);
            }
            if let Err(error) = validate_process_executable(child, image) {
                if error.io_kind() == Some(io::ErrorKind::NotFound)
                    && let Some(status) =
                        wait_for_terminal_after_executable_disappeared(child, deadline)?
                {
                    return Ok(status);
                }
                return Err(error);
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn wait_for_terminal_after_executable_disappeared(
        child: &mut ChildGuard,
        deadline: Instant,
    ) -> Result<Option<ProcessExit>, AuthenticatedVerusExecutionErrorV2> {
        loop {
            if let Some(status) = child.try_reap()? {
                return Ok(Some(status));
            }
            match std::fs::read_link(child.proc_path("exe")) {
                Ok(_) => return Ok(None),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AuthenticatedVerusExecutionErrorV2::io(
                        process_error(child.role, ProcessFailureV2::ExecutableSubstitution),
                        error,
                    ));
                }
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn supervise_iteration(
        child: &mut ChildGuard,
        image: &SealedExecutable,
        deadline: Instant,
        stdout: &CaptureTask,
        stderr: &CaptureTask,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        let role = child.role;
        if Instant::now() >= deadline {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::Timeout,
            )));
        }
        if stdout.exceeded() || stderr.exceeded() {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::OutputTooLarge,
            )));
        }
        if child.exit_pending()? {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::ControlProtocol,
            )));
        }
        if let Some(status) = child.ptrace_stop_pending()? {
            let failure = if ptrace_exec_status(status) {
                ProcessFailureV2::ExecutableSubstitution
            } else {
                ProcessFailureV2::UnexpectedPtraceStop
            };
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role, failure,
            )));
        }
        if let Err(error) = validate_process_executable(child, image) {
            if error.io_kind() == Some(io::ErrorKind::NotFound) {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ControlProtocol,
                )));
            }
            return Err(error);
        }
        Ok(())
    }

    struct CaptureTask {
        handle: JoinHandle<io::Result<Vec<u8>>>,
        exceeded: Arc<AtomicBool>,
    }

    impl CaptureTask {
        fn spawn<R: Read + Send + 'static>(mut reader: R, max: usize) -> Self {
            let exceeded = Arc::new(AtomicBool::new(false));
            let thread_exceeded = Arc::clone(&exceeded);
            let handle = thread::spawn(move || {
                let mut retained = Vec::with_capacity(max.min(8192));
                let mut buffer = [0_u8; 8192];
                loop {
                    let count = read_retry(&mut reader, &mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    let remaining = max.saturating_sub(retained.len());
                    retained.extend_from_slice(&buffer[..count.min(remaining)]);
                    if count > remaining {
                        thread_exceeded.store(true, Ordering::Release);
                    }
                }
                Ok(retained)
            });
            Self { handle, exceeded }
        }

        fn exceeded(&self) -> bool {
            self.exceeded.load(Ordering::Acquire)
        }

        fn is_finished(&self) -> bool {
            self.handle.is_finished()
        }

        fn finish(
            self,
            role: VerusExecutionRoleV2,
        ) -> Result<Vec<u8>, AuthenticatedVerusExecutionErrorV2> {
            self.handle
                .join()
                .map_err(|_| {
                    AuthenticatedVerusExecutionErrorV2::plain(process_error(
                        role,
                        ProcessFailureV2::Observe,
                    ))
                })?
                .map_err(|error| {
                    AuthenticatedVerusExecutionErrorV2::io(
                        process_error(role, ProcessFailureV2::Observe),
                        error,
                    )
                })
        }
    }

    fn finish_capture(
        role: VerusExecutionRoleV2,
        stdout: CaptureTask,
        stderr: CaptureTask,
        deadline: Instant,
    ) -> Result<(Vec<u8>, Vec<u8>), AuthenticatedVerusExecutionErrorV2> {
        let drain_deadline = deadline.max(Instant::now() + CAPTURE_GRACE);
        while (!stdout.is_finished() || !stderr.is_finished()) && Instant::now() < drain_deadline {
            thread::sleep(POLL_INTERVAL);
        }
        if !stdout.is_finished() || !stderr.is_finished() {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::Timeout,
            )));
        }
        if stdout.exceeded() || stderr.exceeded() {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::OutputTooLarge,
            )));
        }
        Ok((stdout.finish(role)?, stderr.finish(role)?))
    }

    #[derive(Clone, Copy)]
    struct ProcessExit {
        success: bool,
    }

    struct ChildGuard {
        pidfd: OwnedFd,
        proc_dir: File,
        process_id: c_int,
        tracer_id: u32,
        role: VerusExecutionRoleV2,
        traced: bool,
        reaped: bool,
    }

    impl ChildGuard {
        fn new(
            pidfd: OwnedFd,
            proc_dir: File,
            process_id: c_int,
            tracer_id: u32,
            role: VerusExecutionRoleV2,
        ) -> Self {
            Self {
                pidfd,
                proc_dir,
                process_id,
                tracer_id,
                role,
                traced: true,
                reaped: false,
            }
        }

        fn proc_path(&self, relative: &str) -> String {
            format!("/proc/self/fd/{}/{}", self.proc_dir.as_raw_fd(), relative)
        }

        fn interrupt_and_confirm(
            &mut self,
            deadline: Instant,
        ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
            if !self.traced {
                return Err(observe_error(self.role));
            }
            if self.ptrace_stop_pending()?.is_some() {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    self.role,
                    ProcessFailureV2::UnexpectedPtraceStop,
                )));
            }
            ptrace_request(PTRACE_INTERRUPT_V2, self.process_id).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(self.role, ProcessFailureV2::Observe),
                    error,
                )
            })?;
            loop {
                ensure_deadline(self.role, deadline)?;
                if let Some(status) = self.ptrace_stop_pending()? {
                    if ptrace_interrupt_status(status)
                        && process_is_tracing_stopped(
                            &self.proc_path("status"),
                            self.role,
                            self.tracer_id,
                        )?
                    {
                        return Ok(());
                    }
                    return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                        self.role,
                        ProcessFailureV2::UnexpectedPtraceStop,
                    )));
                }
                if self.exit_pending()? {
                    return Err(observe_error(self.role));
                }
                thread::sleep(POLL_INTERVAL);
            }
        }

        fn continue_target(&self) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
            if !self.traced {
                return Err(observe_error(self.role));
            }
            ptrace_request(PTRACE_CONT_V2, self.process_id).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(self.role, ProcessFailureV2::Observe),
                    error,
                )
            })
        }

        fn detach_and_continue(&mut self) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
            if !self.traced {
                return Err(observe_error(self.role));
            }
            ptrace_request(PTRACE_DETACH_V2, self.process_id).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(self.role, ProcessFailureV2::Observe),
                    error,
                )
            })?;
            self.traced = false;
            Ok(())
        }

        fn ptrace_stop_pending(&self) -> Result<Option<c_int>, AuthenticatedVerusExecutionErrorV2> {
            if !self.traced {
                return Ok(None);
            }
            let status = waitpid_ptrace(self.process_id).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(self.role, ProcessFailureV2::Observe),
                    error,
                )
            })?;
            let Some(status) = status else {
                return Ok(None);
            };
            if status & 0xff != 0x7f {
                return Err(observe_error(self.role));
            }
            Ok(Some(status))
        }

        fn exit_pending(&self) -> Result<bool, AuthenticatedVerusExecutionErrorV2> {
            waitid_pidfd_terminal(
                self.pidfd.as_raw_fd(),
                WAIT_EXITED | WAIT_NOHANG | WAIT_NOWAIT,
            )
            .map(|event| event.is_some())
            .map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(self.role, ProcessFailureV2::Observe),
                    error,
                )
            })
        }

        fn try_reap(&mut self) -> Result<Option<ProcessExit>, AuthenticatedVerusExecutionErrorV2> {
            let event = waitid_pidfd_terminal(self.pidfd.as_raw_fd(), WAIT_EXITED | WAIT_NOHANG)
                .map_err(|error| {
                    AuthenticatedVerusExecutionErrorV2::io(
                        process_error(self.role, ProcessFailureV2::Observe),
                        error,
                    )
                })?;
            let Some(event) = event else {
                return Ok(None);
            };
            self.reaped = true;
            Ok(Some(ProcessExit {
                success: event.code == CLD_EXITED && event.status == 0,
            }))
        }

        fn terminate_and_confirm(
            &mut self,
            deadline: Instant,
        ) -> Result<Option<ProcessExit>, AuthenticatedVerusExecutionErrorV2> {
            if self.reaped {
                return Ok(None);
            }
            let event = terminate_pidfd_bounded(self.role, self.pidfd.as_raw_fd(), deadline)?;
            self.traced = false;
            self.reaped = true;
            Ok(Some(ProcessExit {
                success: event.code == CLD_EXITED && event.status == 0,
            }))
        }
    }

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.terminate_and_confirm(cleanup_deadline());
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ProcessObservation {
        runtime_closure: RuntimeClosureMeasurementV2,
        executable_baseline: RuntimeExecutableBaselineV2,
        runtime_mappings_digest: Digest,
        executable_pages_digest: Digest,
        process_security_digest: Digest,
    }

    struct RuntimeFile {
        file: File,
        path: String,
        digest: Digest,
        length: u64,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RuntimeMapping {
        start: u64,
        end: u64,
        permissions: [u8; 4],
        file_offset: u64,
        key: (u32, u32, u64),
        path: String,
        digest: Digest,
        length: u64,
        live_executable_digest: Option<Digest>,
        live_executable_readable: bool,
    }

    struct LiveExecutableMappingView<'a> {
        path: &'a str,
        start: u64,
        end: u64,
        permissions: [u8; 4],
        file_offset: u64,
    }

    fn observe_process(
        child: &ChildGuard,
        image: &SealedExecutable,
        expected_security: ProcessSecurityObservation,
        deadline: Instant,
    ) -> Result<ProcessObservation, AuthenticatedVerusExecutionErrorV2> {
        ensure_deadline(child.role, deadline)?;
        validate_process_executable(child, image)?;
        let security = process_security_observation(child)?;
        if security != expected_security {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                child.role,
                ProcessFailureV2::ProcessPolicyMismatch,
            )));
        }
        let (
            runtime_closure,
            executable_baseline,
            runtime_mappings_digest,
            executable_pages_digest,
        ) = runtime_closure(child, image, deadline)?;
        validate_process_executable(child, image)?;
        if process_security_observation(child)? != security {
            return Err(observe_error(child.role));
        }
        Ok(ProcessObservation {
            runtime_closure,
            executable_baseline,
            runtime_mappings_digest,
            executable_pages_digest,
            process_security_digest: security.digest(),
        })
    }

    fn validate_process_executable(
        child: &ChildGuard,
        image: &SealedExecutable,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        let path = child.proc_path("exe");
        let link = std::fs::read_link(path).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(child.role, ProcessFailureV2::ExecutableSubstitution),
                error,
            )
        })?;
        let expected = format!("/memfd:{} (deleted)", child.role.memfd_name());
        if link.to_str() != Some(&expected) {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                child.role,
                ProcessFailureV2::ExecutableSubstitution,
            )));
        }
        validate_sealed_image(child.role, image)
    }

    fn validate_sealed_image(
        role: VerusExecutionRoleV2,
        image: &SealedExecutable,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        let metadata = image.file.metadata().map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::ExecutableSubstitution),
                error,
            )
        })?;
        if Snapshot::from_metadata(&metadata) != image.snapshot || writable_alias(&image.file) {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::ExecutableSubstitution,
            )));
        }
        Ok(())
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct LimitPair {
        soft: u64,
        hard: u64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ProcessLimits {
        address_space: LimitPair,
        data: LimitPair,
        file: LimitPair,
        core: LimitPair,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct SupplementaryGroups {
        values: [u32; MAX_SUPPLEMENTARY_GROUPS],
        count: u16,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ProcessSecurityObservation {
        uids: [u32; 4],
        gids: [u32; 4],
        supplementary_groups: SupplementaryGroups,
        no_new_privileges: bool,
        seccomp_mode: u32,
        seccomp_filters: u32,
        threads: u32,
        capability_inheritable: u64,
        capability_permitted: u64,
        capability_effective: u64,
        capability_bounding: u64,
        capability_ambient: u64,
        inherited_securebits: u32,
        ptrace_owned: bool,
        limits: ProcessLimits,
    }

    impl ProcessSecurityObservation {
        fn digest(self) -> Digest {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(PROCESS_SECURITY_DOMAIN);
            for id in self.uids {
                put_u32(&mut bytes, id);
            }
            for id in self.gids {
                put_u32(&mut bytes, id);
            }
            put_u16(&mut bytes, self.supplementary_groups.count);
            for group in
                &self.supplementary_groups.values[..usize::from(self.supplementary_groups.count)]
            {
                put_u32(&mut bytes, *group);
            }
            bytes.push(u8::from(self.no_new_privileges));
            put_u32(&mut bytes, self.seccomp_mode);
            put_u32(&mut bytes, self.seccomp_filters);
            put_u32(&mut bytes, self.threads);
            for capability in [
                self.capability_inheritable,
                self.capability_permitted,
                self.capability_effective,
                self.capability_bounding,
                self.capability_ambient,
            ] {
                put_u64(&mut bytes, capability);
            }
            put_u32(&mut bytes, self.inherited_securebits);
            bytes.push(u8::from(self.ptrace_owned));
            for limit in [
                self.limits.address_space,
                self.limits.data,
                self.limits.file,
                self.limits.core,
            ] {
                put_u64(&mut bytes, limit.soft);
                put_u64(&mut bytes, limit.hard);
            }
            sha256(&bytes)
        }
    }

    fn validate_controller_security() -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        if child_clone_launcher_bytes_v2().is_empty() || child_trampoline_bytes_v2().is_empty() {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                VerusExecutionRoleV2::Verus,
                ProcessFailureV2::Spawn,
            )));
        }
        let status = read_bounded("/proc/thread-self/status", 16 * 1024).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(
                    VerusExecutionRoleV2::Verus,
                    ProcessFailureV2::PrivilegedController,
                ),
                error,
            )
        })?;
        let status = std::str::from_utf8(&status).map_err(|_| {
            AuthenticatedVerusExecutionErrorV2::plain(process_error(
                VerusExecutionRoleV2::Verus,
                ProcessFailureV2::PrivilegedController,
            ))
        })?;
        if !controller_status_allows_spawn(status)
            || controller_securebits() != Some(0)
            || !sigchld_policy_is_reapable()
        {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                VerusExecutionRoleV2::Verus,
                ProcessFailureV2::PrivilegedController,
            )));
        }
        Ok(())
    }

    fn controller_status_allows_spawn(status: &str) -> bool {
        controller_credentials(status).is_some()
            && status_u32(status, "Seccomp:") == Some(0)
            && status_u32(status, "Seccomp_filters:") == Some(0)
    }

    fn controller_credentials(
        status: &str,
    ) -> Option<([u32; 4], [u32; 4], SupplementaryGroups, u64)> {
        let uids = status_id_tuple(status, "Uid:")?;
        let gids = status_id_tuple(status, "Gid:")?;
        let groups = status_groups(status)?;
        let capabilities =
            ["CapInh:", "CapPrm:", "CapEff:", "CapAmb:"].map(|name| status_hex_u64(status, name));
        if uids[0] == 0
            || gids[0] == 0
            || uids.iter().any(|uid| *uid != uids[0])
            || gids.iter().any(|gid| *gid != gids[0])
            || groups.values[..usize::from(groups.count)].contains(&0)
            || capabilities.into_iter().any(|value| value != Some(0))
        {
            return None;
        }
        Some((uids, gids, groups, status_hex_u64(status, "CapBnd:")?))
    }

    fn expected_process_security(
        role: VerusExecutionRoleV2,
    ) -> Result<ProcessSecurityObservation, AuthenticatedVerusExecutionErrorV2> {
        let status = read_bounded("/proc/thread-self/status", 16 * 1024).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Observe),
                error,
            )
        })?;
        let status = std::str::from_utf8(&status).map_err(|_| observe_error(role))?;
        let (uids, gids, supplementary_groups, capability_bounding) =
            controller_credentials(status).ok_or_else(|| {
                AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::PrivilegedController,
                ))
            })?;
        let securebits = controller_securebits().ok_or_else(|| observe_error(role))?;
        if securebits != 0 {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::PrivilegedController,
            )));
        }
        if !controller_status_allows_spawn(status) {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::PrivilegedController,
            )));
        }
        Ok(ProcessSecurityObservation {
            uids,
            gids,
            supplementary_groups,
            no_new_privileges: true,
            seccomp_mode: 2,
            seccomp_filters: 1,
            threads: 1,
            capability_inheritable: 0,
            capability_permitted: 0,
            capability_effective: 0,
            capability_bounding,
            capability_ambient: 0,
            inherited_securebits: securebits,
            ptrace_owned: true,
            limits: expected_process_limits(role)?,
        })
    }

    fn process_security_observation(
        child: &ChildGuard,
    ) -> Result<ProcessSecurityObservation, AuthenticatedVerusExecutionErrorV2> {
        let status = read_bounded(&child.proc_path("status"), 16 * 1024).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(child.role, ProcessFailureV2::Observe),
                error,
            )
        })?;
        let status = std::str::from_utf8(&status).map_err(|_| observe_error(child.role))?;
        let ptrace_owned = status_u32(status, "TracerPid:") == Some(child.tracer_id);
        if !process_status_is_tracing_stopped(status) || !ptrace_owned {
            return Err(observe_error(child.role));
        }
        Ok(ProcessSecurityObservation {
            uids: status_id_tuple(status, "Uid:").ok_or_else(|| observe_error(child.role))?,
            gids: status_id_tuple(status, "Gid:").ok_or_else(|| observe_error(child.role))?,
            supplementary_groups: status_groups(status).ok_or_else(|| observe_error(child.role))?,
            no_new_privileges: status_u32(status, "NoNewPrivs:") == Some(1),
            seccomp_mode: status_u32(status, "Seccomp:")
                .ok_or_else(|| observe_error(child.role))?,
            seccomp_filters: status_u32(status, "Seccomp_filters:")
                .ok_or_else(|| observe_error(child.role))?,
            threads: status_u32(status, "Threads:").ok_or_else(|| observe_error(child.role))?,
            capability_inheritable: status_hex_u64(status, "CapInh:")
                .ok_or_else(|| observe_error(child.role))?,
            capability_permitted: status_hex_u64(status, "CapPrm:")
                .ok_or_else(|| observe_error(child.role))?,
            capability_effective: status_hex_u64(status, "CapEff:")
                .ok_or_else(|| observe_error(child.role))?,
            capability_bounding: status_hex_u64(status, "CapBnd:")
                .ok_or_else(|| observe_error(child.role))?,
            capability_ambient: status_hex_u64(status, "CapAmb:")
                .ok_or_else(|| observe_error(child.role))?,
            // The calling thread is required to start at securebits zero. That
            // value survives the initial exec, and the executable filter kills
            // every later prctl before it can mutate securebits.
            inherited_securebits: 0,
            ptrace_owned,
            limits: read_process_limits(child)?,
        })
    }

    fn process_status_is_tracing_stopped(status: &str) -> bool {
        status_field(status, "State:").and_then(|value| value.trim().as_bytes().first().copied())
            == Some(b't')
    }

    fn process_is_tracing_stopped(
        path: &str,
        role: VerusExecutionRoleV2,
        tracer_id: u32,
    ) -> Result<bool, AuthenticatedVerusExecutionErrorV2> {
        let status = read_bounded(path, 16 * 1024).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Observe),
                error,
            )
        })?;
        let status = std::str::from_utf8(&status).map_err(|_| observe_error(role))?;
        Ok(process_status_is_tracing_stopped(status)
            && status_u32(status, "TracerPid:") == Some(tracer_id))
    }

    fn status_field<'a>(status: &'a str, name: &str) -> Option<&'a str> {
        status.lines().find_map(|line| line.strip_prefix(name))
    }

    fn status_u32(status: &str, name: &str) -> Option<u32> {
        status_field(status, name)?.trim().parse().ok()
    }

    fn status_hex_u64(status: &str, name: &str) -> Option<u64> {
        u64::from_str_radix(status_field(status, name)?.trim(), 16).ok()
    }

    fn status_id_tuple(status: &str, name: &str) -> Option<[u32; 4]> {
        let mut values = status_field(status, name)?.split_ascii_whitespace();
        let tuple = [
            values.next()?.parse().ok()?,
            values.next()?.parse().ok()?,
            values.next()?.parse().ok()?,
            values.next()?.parse().ok()?,
        ];
        if values.next().is_some() {
            return None;
        }
        Some(tuple)
    }

    fn status_groups(status: &str) -> Option<SupplementaryGroups> {
        let mut groups = SupplementaryGroups {
            values: [0; MAX_SUPPLEMENTARY_GROUPS],
            count: 0,
        };
        for value in status_field(status, "Groups:")?.split_ascii_whitespace() {
            let index = usize::from(groups.count);
            if index >= groups.values.len() {
                return None;
            }
            groups.values[index] = value.parse().ok()?;
            groups.count += 1;
        }
        Some(groups)
    }

    fn controller_securebits() -> Option<u32> {
        const PR_GET_SECUREBITS: c_int = 27;
        // SAFETY: PR_GET_SECUREBITS has no pointer arguments.
        let value = unsafe { linux_prctl(PR_GET_SECUREBITS) };
        u32::try_from(value).ok()
    }

    fn expected_process_limits(
        role: VerusExecutionRoleV2,
    ) -> Result<ProcessLimits, AuthenticatedVerusExecutionErrorV2> {
        Ok(ProcessLimits {
            address_space: expected_limit(role, RLIMIT_AS, ADDRESS_SPACE_LIMIT_V2)?,
            data: expected_limit(role, RLIMIT_DATA, DATA_LIMIT_V2)?,
            file: expected_limit(role, RLIMIT_FSIZE, FILE_LIMIT_V2)?,
            core: expected_limit(role, RLIMIT_CORE, CORE_LIMIT_V2)?,
        })
    }

    fn expected_limit(
        role: VerusExecutionRoleV2,
        resource: c_int,
        requested: u64,
    ) -> Result<LimitPair, AuthenticatedVerusExecutionErrorV2> {
        let mut existing = LinuxRlimit {
            current: 0,
            maximum: 0,
        };
        // SAFETY: `existing` is writable and `resource` is one fixed RLIMIT value.
        if unsafe { linux_getrlimit(resource, &mut existing) } < 0 {
            return Err(AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Observe),
                io::Error::last_os_error(),
            ));
        }
        let exact = existing.maximum.min(requested);
        Ok(LimitPair {
            soft: exact,
            hard: exact,
        })
    }

    fn read_process_limits(
        child: &ChildGuard,
    ) -> Result<ProcessLimits, AuthenticatedVerusExecutionErrorV2> {
        let bytes = read_bounded(&child.proc_path("limits"), 16 * 1024).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(child.role, ProcessFailureV2::Observe),
                error,
            )
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|_| observe_error(child.role))?;
        Ok(ProcessLimits {
            address_space: parse_limit(text, "Max address space")
                .ok_or_else(|| observe_error(child.role))?,
            data: parse_limit(text, "Max data size").ok_or_else(|| observe_error(child.role))?,
            file: parse_limit(text, "Max file size").ok_or_else(|| observe_error(child.role))?,
            core: parse_limit(text, "Max core file size")
                .ok_or_else(|| observe_error(child.role))?,
        })
    }

    fn parse_limit(limits: &str, name: &str) -> Option<LimitPair> {
        let values = limits
            .lines()
            .find_map(|line| line.strip_prefix(name))?
            .split_ascii_whitespace()
            .take(2)
            .map(|value| {
                if value == "unlimited" {
                    Some(u64::MAX)
                } else {
                    value.parse::<u64>().ok()
                }
            })
            .collect::<Option<Vec<_>>>()?;
        if values.len() != 2 {
            return None;
        }
        Some(LimitPair {
            soft: values[0],
            hard: values[1],
        })
    }

    fn runtime_closure(
        child: &ChildGuard,
        image: &SealedExecutable,
        deadline: Instant,
    ) -> Result<
        (
            RuntimeClosureMeasurementV2,
            RuntimeExecutableBaselineV2,
            Digest,
            Digest,
        ),
        AuthenticatedVerusExecutionErrorV2,
    > {
        let role = child.role;
        let maps =
            read_bounded(&child.proc_path("maps"), MAX_PROC_MAPS_BYTES).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Observe),
                    error,
                )
            })?;
        let maps = std::str::from_utf8(&maps).map_err(|_| {
            AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::Observe,
            ))
        })?;
        reject_writable_executable_aliases(role, maps)?;
        let mut files = BTreeMap::<(u32, u32, u64), RuntimeFile>::new();
        let mut mappings = Vec::new();
        let mut total_bytes = 0_u64;
        let mut live_executable_bytes = 0_u64;
        let process_memory = File::open(child.proc_path("mem")).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::ExecutablePageUnreadable),
                error,
            )
        })?;
        let process_executable = File::open(child.proc_path("exe")).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Observe),
                error,
            )
        })?;
        let executable_metadata = process_executable.metadata().map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Observe),
                error,
            )
        })?;
        let executable_key = (
            device_major(executable_metadata.dev()),
            device_minor(executable_metadata.dev()),
            executable_metadata.ino(),
        );
        for line in maps.lines() {
            ensure_deadline(role, deadline)?;
            if line.is_empty() || mappings.len() >= MAX_RUNTIME_MAPPINGS {
                return Err(observe_error(role));
            }
            let mut fields = line.split_ascii_whitespace();
            let range = fields.next().ok_or_else(|| observe_error(role))?;
            let permissions = fields.next().ok_or_else(|| observe_error(role))?;
            let offset = fields.next().ok_or_else(|| observe_error(role))?;
            let device = fields.next().ok_or_else(|| observe_error(role))?;
            let inode = fields.next().ok_or_else(|| observe_error(role))?;
            let path = fields.collect::<Vec<_>>().join(" ");
            let (start, end) = range.split_once('-').ok_or_else(|| observe_error(role))?;
            let start = u64::from_str_radix(start, 16).map_err(|_| observe_error(role))?;
            let end = u64::from_str_radix(end, 16).map_err(|_| observe_error(role))?;
            if start >= end {
                return Err(observe_error(role));
            }
            let permissions: [u8; 4] = permissions
                .as_bytes()
                .try_into()
                .map_err(|_| observe_error(role))?;
            if !matches!(permissions[0], b'r' | b'-')
                || !matches!(permissions[1], b'w' | b'-')
                || !matches!(permissions[2], b'x' | b'-')
                || !matches!(permissions[3], b'p' | b's')
            {
                return Err(observe_error(role));
            }
            if permissions[1] == b'w' && permissions[2] == b'x' {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::WritableExecutableMapping,
                )));
            }
            let file_offset = u64::from_str_radix(offset, 16).map_err(|_| observe_error(role))?;
            let (major_text, minor_text) =
                device.split_once(':').ok_or_else(|| observe_error(role))?;
            let major = u32::from_str_radix(major_text, 16).map_err(|_| observe_error(role))?;
            let minor = u32::from_str_radix(minor_text, 16).map_err(|_| observe_error(role))?;
            let inode = inode.parse::<u64>().map_err(|_| observe_error(role))?;
            let key = (major, minor, inode);
            if inode == 0 {
                if key != (0, 0, 0) {
                    return Err(observe_error(role));
                }
                let length = end - start;
                let class =
                    anonymous_mapping_class(role, &path, start, end, permissions, file_offset)?;
                let live_executable = live_executable_mapping_digest(
                    role,
                    &process_memory,
                    &LiveExecutableMappingView {
                        path: &class,
                        start,
                        end,
                        permissions,
                        file_offset,
                    },
                    None,
                    &mut live_executable_bytes,
                    deadline,
                )?;
                mappings.push(RuntimeMapping {
                    start,
                    end,
                    permissions,
                    file_offset,
                    key,
                    path: class,
                    digest: Digest::from_bytes([0; 32]),
                    length,
                    live_executable_digest: live_executable.map(|measurement| measurement.0),
                    live_executable_readable: live_executable
                        .is_some_and(|measurement| measurement.1),
                });
                continue;
            }
            let (digest, length, live_executable) = {
                if path.len() > u16::MAX as usize
                    || (!path.starts_with('/') && !path.starts_with("/memfd:"))
                {
                    return Err(observe_error(role));
                }
                if !files.contains_key(&key) {
                    if files.len() >= MAX_RUNTIME_FILES {
                        return Err(observe_error(role));
                    }
                    let mut file = if key == executable_key {
                        process_executable.try_clone().map_err(|error| {
                            AuthenticatedVerusExecutionErrorV2::io(
                                process_error(role, ProcessFailureV2::Observe),
                                error,
                            )
                        })?
                    } else {
                        open_mapped_file(child, range, &path, key).map_err(|error| {
                            AuthenticatedVerusExecutionErrorV2::io(
                                process_error(role, ProcessFailureV2::Observe),
                                error,
                            )
                        })?
                    };
                    let metadata = file.metadata().map_err(|error| {
                        AuthenticatedVerusExecutionErrorV2::io(
                            process_error(role, ProcessFailureV2::Observe),
                            error,
                        )
                    })?;
                    if !metadata.is_file()
                        || metadata.len() > MAX_RUNTIME_FILE_BYTES_V2
                        || total_bytes > MAX_RUNTIME_TOTAL_BYTES_V2.saturating_sub(metadata.len())
                        || writable_alias(&file)
                    {
                        return Err(observe_error(role));
                    }
                    let snapshot = Snapshot::from_metadata(&metadata);
                    let digest = hash_file(role, &mut file, metadata.len(), deadline)?;
                    if key == executable_key
                        && (metadata.len() != image.byte_len
                            || digest != image.measurement.executable_digest())
                    {
                        return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                            role,
                            ProcessFailureV2::ExecutableSubstitution,
                        )));
                    }
                    if Snapshot::from_metadata(&file.metadata().map_err(|error| {
                        AuthenticatedVerusExecutionErrorV2::io(
                            process_error(role, ProcessFailureV2::Observe),
                            error,
                        )
                    })?) != snapshot
                    {
                        return Err(observe_error(role));
                    }
                    total_bytes += metadata.len();
                    files.insert(
                        key,
                        RuntimeFile {
                            file,
                            path: path.clone(),
                            digest,
                            length: metadata.len(),
                        },
                    );
                }
                let file = files.get(&key).ok_or_else(|| observe_error(role))?;
                if file.path != path {
                    return Err(observe_error(role));
                }
                let live_executable = live_executable_mapping_digest(
                    role,
                    &process_memory,
                    &LiveExecutableMappingView {
                        path: &path,
                        start,
                        end,
                        permissions,
                        file_offset,
                    },
                    Some(file),
                    &mut live_executable_bytes,
                    deadline,
                )?;
                (file.digest, file.length, live_executable)
            };
            mappings.push(RuntimeMapping {
                start,
                end,
                permissions,
                file_offset,
                key,
                path,
                digest,
                length,
                live_executable_digest: live_executable.map(|measurement| measurement.0),
                live_executable_readable: live_executable.is_some_and(|measurement| measurement.1),
            });
        }
        if files.is_empty() {
            return Err(observe_error(role));
        }
        let mut records = files
            .values()
            .map(|file| (file.path.as_str(), file.digest, file.length))
            .collect::<Vec<_>>();
        records.sort_unstable();
        let mut closure = Vec::new();
        closure.extend_from_slice(RUNTIME_CLOSURE_DOMAIN);
        put_u32(&mut closure, records.len() as u32);
        for (path, digest, length) in records {
            put_text(&mut closure, path);
            closure.extend_from_slice(digest.as_bytes());
            put_u64(&mut closure, length);
        }
        let mut baseline_records = Vec::new();
        let mut vdso_digest = None;
        for mapping in &mappings {
            let Some(live_digest) = mapping.live_executable_digest else {
                continue;
            };
            let mut record = Vec::new();
            let file_backed = mapping.key != (0, 0, 0);
            record.push(if file_backed { 1 } else { 2 });
            put_text(&mut record, &mapping.path);
            record.extend_from_slice(&mapping.permissions);
            put_u64(&mut record, mapping.file_offset);
            put_u64(&mut record, mapping.end - mapping.start);
            record.extend_from_slice(mapping.digest.as_bytes());
            put_u64(&mut record, mapping.length);
            record.push(u8::from(mapping.live_executable_readable));
            record.extend_from_slice(live_digest.as_bytes());
            if mapping.path == "kernel-vdso"
                && (vdso_digest.replace(live_digest).is_some() || !mapping.live_executable_readable)
            {
                return Err(observe_error(role));
            }
            baseline_records.push(record);
        }
        let vdso_digest = vdso_digest.ok_or_else(|| observe_error(role))?;
        baseline_records.sort_unstable();
        let mut baseline_bytes = Vec::new();
        baseline_bytes.extend_from_slice(EXECUTABLE_BASELINE_DOMAIN);
        put_u32(&mut baseline_bytes, baseline_records.len() as u32);
        put_u64(&mut baseline_bytes, live_executable_bytes);
        for record in &baseline_records {
            put_blob(&mut baseline_bytes, record);
        }
        let executable_baseline = RuntimeExecutableBaselineV2 {
            digest: sha256(&baseline_bytes),
            mapping_count: baseline_records.len() as u32,
            total_bytes: live_executable_bytes,
            vdso_digest,
        };
        mappings.sort_by(|left, right| {
            (
                left.start,
                left.end,
                left.permissions,
                left.file_offset,
                left.key,
                left.path.as_str(),
            )
                .cmp(&(
                    right.start,
                    right.end,
                    right.permissions,
                    right.file_offset,
                    right.key,
                    right.path.as_str(),
                ))
        });
        let mut mapping_bytes = Vec::new();
        let mut executable_page_bytes = Vec::new();
        mapping_bytes.extend_from_slice(RUNTIME_MAPPINGS_DOMAIN);
        executable_page_bytes.extend_from_slice(LIVE_EXECUTABLE_PAGES_DOMAIN);
        put_u32(&mut mapping_bytes, mappings.len() as u32);
        put_u64(&mut executable_page_bytes, live_executable_bytes);
        put_u32(
            &mut executable_page_bytes,
            mappings
                .iter()
                .filter(|mapping| mapping.live_executable_digest.is_some())
                .count() as u32,
        );
        for mapping in mappings {
            put_u64(&mut mapping_bytes, mapping.start);
            put_u64(&mut mapping_bytes, mapping.end);
            mapping_bytes.extend_from_slice(&mapping.permissions);
            put_u64(&mut mapping_bytes, mapping.file_offset);
            put_u32(&mut mapping_bytes, mapping.key.0);
            put_u32(&mut mapping_bytes, mapping.key.1);
            put_u64(&mut mapping_bytes, mapping.key.2);
            put_text(&mut mapping_bytes, &mapping.path);
            mapping_bytes.extend_from_slice(mapping.digest.as_bytes());
            put_u64(&mut mapping_bytes, mapping.length);
            match mapping.live_executable_digest {
                Some(digest) => {
                    mapping_bytes.push(1);
                    mapping_bytes.push(u8::from(mapping.live_executable_readable));
                    mapping_bytes.extend_from_slice(digest.as_bytes());
                    put_u64(&mut executable_page_bytes, mapping.start);
                    put_u64(&mut executable_page_bytes, mapping.end);
                    executable_page_bytes.extend_from_slice(&mapping.permissions);
                    executable_page_bytes.push(u8::from(mapping.live_executable_readable));
                    executable_page_bytes.extend_from_slice(digest.as_bytes());
                }
                None => mapping_bytes.push(0),
            }
        }
        Ok((
            RuntimeClosureMeasurementV2 {
                digest: sha256(&closure),
                file_count: files.len() as u32,
                total_bytes,
            },
            executable_baseline,
            sha256(&mapping_bytes),
            sha256(&executable_page_bytes),
        ))
    }

    fn reject_writable_executable_aliases(
        role: VerusExecutionRoleV2,
        maps: &str,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        let mut access = BTreeMap::<(u32, u32, u64), (bool, bool)>::new();
        for line in maps.lines() {
            let mut fields = line.split_ascii_whitespace();
            let _range = fields.next().ok_or_else(|| observe_error(role))?;
            let permissions: [u8; 4] = fields
                .next()
                .ok_or_else(|| observe_error(role))?
                .as_bytes()
                .try_into()
                .map_err(|_| observe_error(role))?;
            let _offset = fields.next().ok_or_else(|| observe_error(role))?;
            let device = fields.next().ok_or_else(|| observe_error(role))?;
            let inode = fields
                .next()
                .ok_or_else(|| observe_error(role))?
                .parse::<u64>()
                .map_err(|_| observe_error(role))?;
            if inode == 0 {
                continue;
            }
            let (major, minor) = device.split_once(':').ok_or_else(|| observe_error(role))?;
            let key = (
                u32::from_str_radix(major, 16).map_err(|_| observe_error(role))?,
                u32::from_str_radix(minor, 16).map_err(|_| observe_error(role))?,
                inode,
            );
            let entry = access.entry(key).or_insert((false, false));
            entry.0 |= permissions[2] == b'x';
            entry.1 |= permissions[1] == b'w' && permissions[3] == b's';
            if entry.0 && entry.1 {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::WritableExecutableAlias,
                )));
            }
        }
        Ok(())
    }

    fn live_executable_mapping_digest(
        role: VerusExecutionRoleV2,
        memory: &File,
        mapping: &LiveExecutableMappingView<'_>,
        backing: Option<&RuntimeFile>,
        total: &mut u64,
        deadline: Instant,
    ) -> Result<Option<(Digest, bool)>, AuthenticatedVerusExecutionErrorV2> {
        if mapping.permissions[2] != b'x' {
            return Ok(None);
        }
        let length = mapping.end - mapping.start;
        if length > MAX_LIVE_EXECUTABLE_MAPPING_BYTES_V2
            || *total > MAX_LIVE_EXECUTABLE_TOTAL_BYTES_V2.saturating_sub(length)
        {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::ExecutableMappingTooLarge,
            )));
        }
        *total += length;
        if mapping.path == "kernel-vsyscall"
            && mapping.permissions == *b"--xp"
            && mapping.start == VSYSCALL_START_V2
            && mapping.end == VSYSCALL_END_V2
        {
            let mut marker = Vec::new();
            marker.extend_from_slice(LIVE_EXECUTABLE_PAGES_DOMAIN);
            marker.extend_from_slice(b"kernel-emulated-unreadable-vsyscall");
            put_u64(&mut marker, mapping.start);
            put_u64(&mut marker, mapping.end);
            marker.extend_from_slice(&mapping.permissions);
            return Ok(Some((sha256(&marker), false)));
        }
        let mut hasher = Sha256::new();
        let mut offset = 0_u64;
        let mut live_buffer = [0_u8; IO_CHUNK_BYTES];
        let mut backing_buffer = [0_u8; IO_CHUNK_BYTES];
        if let Some(backing) = backing
            && mapping.file_offset > backing.length
        {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::ExecutableBaselineViolation,
            )));
        }
        while offset < length {
            ensure_deadline(role, deadline)?;
            let wanted = usize::try_from((length - offset).min(live_buffer.len() as u64))
                .map_err(|_| observe_error(role))?;
            let count = loop {
                match memory.read_at(&mut live_buffer[..wanted], mapping.start + offset) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        return Err(AuthenticatedVerusExecutionErrorV2::io(
                            process_error(role, ProcessFailureV2::ExecutablePageUnreadable),
                            error,
                        ));
                    }
                    Ok(count) => break count,
                }
            };
            if count == 0 {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::ExecutablePageUnreadable,
                )));
            }
            if let Some(backing) = backing {
                backing_buffer[..count].fill(0);
                let backing_offset = mapping
                    .file_offset
                    .checked_add(offset)
                    .ok_or_else(|| observe_error(role))?;
                let available = backing.length.saturating_sub(backing_offset);
                let expected_file_bytes = usize::try_from(available.min(count as u64))
                    .map_err(|_| observe_error(role))?;
                let mut filled = 0;
                while filled < expected_file_bytes {
                    let read_offset = backing_offset
                        .checked_add(filled as u64)
                        .ok_or_else(|| observe_error(role))?;
                    let read = match backing.file.read_at(
                        &mut backing_buffer[filled..expected_file_bytes],
                        read_offset,
                    ) {
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => {
                            return Err(AuthenticatedVerusExecutionErrorV2::io(
                                process_error(role, ProcessFailureV2::ExecutableBaselineViolation),
                                error,
                            ));
                        }
                        Ok(read) => read,
                    };
                    if read == 0 {
                        return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                            role,
                            ProcessFailureV2::ExecutableBaselineViolation,
                        )));
                    }
                    filled += read;
                }
                if live_buffer[..count] != backing_buffer[..count] {
                    return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                        role,
                        ProcessFailureV2::ExecutableBaselineViolation,
                    )));
                }
            }
            hasher.update(&live_buffer[..count]);
            offset += count as u64;
        }
        Ok(Some((Digest::from_bytes(hasher.finalize().into()), true)))
    }

    fn anonymous_mapping_class(
        role: VerusExecutionRoleV2,
        path: &str,
        start: u64,
        end: u64,
        permissions: [u8; 4],
        file_offset: u64,
    ) -> Result<String, AuthenticatedVerusExecutionErrorV2> {
        if file_offset != 0 || path.len() > u16::MAX as usize || path.chars().any(char::is_control)
        {
            return Err(observe_error(role));
        }
        let length = end - start;
        let executable = permissions[2] == b'x';
        match path {
            "[vdso]" if permissions == *b"r-xp" && length > 0 && length <= MAX_VDSO_BYTES_V2 => {
                Ok("kernel-vdso".to_owned())
            }
            "[vsyscall]"
                if permissions == *b"--xp"
                    && start == VSYSCALL_START_V2
                    && end == VSYSCALL_END_V2 =>
            {
                Ok("kernel-vsyscall".to_owned())
            }
            _ if executable => Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::AnonymousExecutableMapping,
            ))),
            "" => Ok("anonymous".to_owned()),
            "[heap]" => Ok("heap".to_owned()),
            "[stack]" => Ok("stack".to_owned()),
            "[vvar]" => Ok("kernel-vvar".to_owned()),
            "[vvar_vclock]" => Ok("kernel-vvar-vclock".to_owned()),
            _ if path.starts_with("[anon:") && path.ends_with(']') => {
                Ok(format!("named-anonymous:{path}"))
            }
            _ => Err(observe_error(role)),
        }
    }

    fn open_mapped_file(
        child: &ChildGuard,
        range: &str,
        path: &str,
        expected: (u32, u32, u64),
    ) -> io::Result<File> {
        let map_path = child.proc_path(&format!("map_files/{range}"));
        let file = File::open(&map_path).or_else(|map_error| {
            if path.ends_with(" (deleted)") {
                return Err(map_error);
            }
            File::open(child.proc_path(&format!("root{path}")))
        })?;
        let metadata = file.metadata()?;
        if (
            device_major(metadata.dev()),
            device_minor(metadata.dev()),
            metadata.ino(),
        ) != expected
            || !metadata.is_file()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mapped file identity changed",
            ));
        }
        Ok(file)
    }

    fn hash_file(
        role: VerusExecutionRoleV2,
        file: &mut File,
        expected: u64,
        deadline: Instant,
    ) -> Result<Digest, AuthenticatedVerusExecutionErrorV2> {
        hash_file_bounded(role, file, expected, MAX_RUNTIME_FILE_BYTES_V2, deadline)
    }

    fn hash_file_bounded(
        role: VerusExecutionRoleV2,
        file: &mut File,
        expected: u64,
        maximum: u64,
        deadline: Instant,
    ) -> Result<Digest, AuthenticatedVerusExecutionErrorV2> {
        if expected > maximum {
            return Err(observe_error(role));
        }
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Observe),
                error,
            )
        })?;
        let mut hasher = Sha256::new();
        let mut total = 0_u64;
        let mut buffer = [0_u8; IO_CHUNK_BYTES];
        loop {
            ensure_deadline(role, deadline)?;
            let count = read_retry(file, &mut buffer).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Observe),
                    error,
                )
            })?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .ok_or_else(|| observe_error(role))?;
            if total > expected {
                return Err(observe_error(role));
            }
            hasher.update(&buffer[..count]);
        }
        if total != expected {
            return Err(observe_error(role));
        }
        Ok(Digest::from_bytes(hasher.finalize().into()))
    }

    fn writable_alias(file: &File) -> bool {
        if is_immutably_sealed(file) {
            return false;
        }
        OpenOptions::new()
            .write(true)
            .open(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .is_ok()
    }

    const fn device_major(device: u64) -> u32 {
        (((device >> 8) & 0x0000_0fff) | ((device >> 32) & 0xffff_f000)) as u32
    }

    const fn device_minor(device: u64) -> u32 {
        ((device & 0x0000_00ff) | ((device >> 12) & 0xffff_ff00)) as u32
    }

    fn is_immutably_sealed(file: &File) -> bool {
        const F_GET_SEALS: c_int = 1034;
        const ALL_IMMUTABLE_SEALS: c_int = 0x0001 | 0x0002 | 0x0004 | 0x0008;
        // SAFETY: F_GET_SEALS takes no variadic argument and the descriptor is live.
        let seals = unsafe { linux_fcntl(file.as_raw_fd(), F_GET_SEALS) };
        seals == ALL_IMMUTABLE_SEALS
    }

    fn read_bounded(path: &str, max: usize) -> io::Result<Vec<u8>> {
        let file = File::open(path)?;
        let mut bytes = Vec::with_capacity(max.min(8192));
        file.take((max + 1) as u64).read_to_end(&mut bytes)?;
        if bytes.len() > max {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "proc record exceeds bound",
            ));
        }
        Ok(bytes)
    }

    struct SpawnedTarget {
        pidfd: OwnedFd,
        proc_dir: File,
        process_id: c_int,
        stdout: File,
        stderr: File,
    }

    #[repr(C)]
    struct ChildExecContext {
        program: *const c_char,
        arguments: *const *const c_char,
        environment: *const *const c_char,
        root: *const c_char,
        stdin_fd: c_int,
        stdout_fd: c_int,
        stderr_fd: c_int,
        error_fd: c_int,
        control_fd: c_int,
        inherited: *const c_int,
        inherited_count: usize,
        limits: ProcessLimits,
        capability_header: CapabilityHeader,
        capability_data: [CapabilityData; 2],
        seccomp_program: SocketFilterProgram,
        error_byte: u8,
        gate_byte: u8,
        gate_fd: c_int,
    }

    const _: () = {
        assert!(offset_of!(ChildExecContext, program) == 0);
        assert!(offset_of!(ChildExecContext, arguments) == 8);
        assert!(offset_of!(ChildExecContext, environment) == 16);
        assert!(offset_of!(ChildExecContext, root) == 24);
        assert!(offset_of!(ChildExecContext, stdin_fd) == 32);
        assert!(offset_of!(ChildExecContext, stdout_fd) == 36);
        assert!(offset_of!(ChildExecContext, stderr_fd) == 40);
        assert!(offset_of!(ChildExecContext, error_fd) == 44);
        assert!(offset_of!(ChildExecContext, control_fd) == 48);
        assert!(offset_of!(ChildExecContext, inherited) == 56);
        assert!(offset_of!(ChildExecContext, inherited_count) == 64);
        assert!(offset_of!(ChildExecContext, limits) == 72);
        assert!(offset_of!(ChildExecContext, capability_header) == 136);
        assert!(offset_of!(ChildExecContext, capability_data) == 144);
        assert!(offset_of!(ChildExecContext, seccomp_program) == 168);
        assert!(offset_of!(ChildExecContext, error_byte) == 184);
        assert!(offset_of!(ChildExecContext, gate_byte) == 185);
        assert!(offset_of!(ChildExecContext, gate_fd) == 188);
        assert!(size_of::<ChildExecContext>() == 192);
    };

    #[repr(C)]
    struct CloneArguments {
        flags: u64,
        pidfd: u64,
        child_tid: u64,
        parent_tid: u64,
        exit_signal: u64,
        stack: u64,
        stack_size: u64,
        tls: u64,
        set_tid: u64,
        set_tid_size: u64,
        cgroup: u64,
    }

    fn clone3_result_error(result: isize) -> Option<io::Error> {
        if result >= 0 {
            return None;
        }
        let errno = result
            .checked_neg()
            .and_then(|value| c_int::try_from(value).ok())
            .filter(|value| (1..=4095).contains(value))
            .unwrap_or(5);
        Some(io::Error::from_raw_os_error(errno))
    }

    fn checked_c_string(
        role: VerusExecutionRoleV2,
        value: impl AsRef<str>,
    ) -> Result<CString, AuthenticatedVerusExecutionErrorV2> {
        CString::new(value.as_ref()).map_err(|_| {
            AuthenticatedVerusExecutionErrorV2::plain(process_error(role, ProcessFailureV2::Spawn))
        })
    }

    fn spawn_target(
        role: VerusExecutionRoleV2,
        image: &SealedExecutable,
        arguments: &[CString],
        control_fd: c_int,
        inherited: &[c_int],
        limits: ProcessLimits,
        deadline: Instant,
    ) -> Result<SpawnedTarget, AuthenticatedVerusExecutionErrorV2> {
        const CLONE_PIDFD: u64 = 0x0000_1000;
        const SIGCHLD: u64 = 17;
        let program = checked_c_string(role, image.path())?;
        let root = checked_c_string(role, "/")?;
        let mut argument_pointers = arguments
            .iter()
            .map(|argument| argument.as_ptr())
            .collect::<Vec<_>>();
        argument_pointers.push(std::ptr::null());
        let environment = [std::ptr::null::<c_char>()];
        let stdin = File::open("/dev/null").map_err(|error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                error,
            )
        })?;
        let (stdout_read, stdout_write) = cloexec_pipe(role, false)?;
        let (stderr_read, stderr_write) = cloexec_pipe(role, false)?;
        let (error_read, error_write) = cloexec_pipe(role, true)?;
        let (gate_read, gate_write) = cloexec_pipe(role, false)?;
        let child_context = ChildExecContext {
            program: program.as_ptr(),
            arguments: argument_pointers.as_ptr(),
            environment: environment.as_ptr(),
            root: root.as_ptr(),
            stdin_fd: stdin.as_raw_fd(),
            stdout_fd: stdout_write.as_raw_fd(),
            stderr_fd: stderr_write.as_raw_fd(),
            error_fd: error_write.as_raw_fd(),
            control_fd,
            inherited: inherited.as_ptr(),
            inherited_count: inherited.len(),
            limits,
            capability_header: CapabilityHeader {
                version: 0x2008_0522,
                process: 0,
            },
            capability_data: [CapabilityData {
                effective: 0,
                permitted: 0,
                inheritable: 0,
            }; 2],
            seccomp_program: SocketFilterProgram {
                length: SECCOMP_FILTER_V2.len() as u16,
                filters: SECCOMP_FILTER_V2.as_ptr(),
            },
            error_byte: 1,
            gate_byte: 0,
            gate_fd: gate_read.as_raw_fd(),
        };
        let mut pidfd: c_int = -1;
        let mut clone_arguments = CloneArguments {
            flags: CLONE_PIDFD,
            pidfd: (&raw mut pidfd).addr() as u64,
            child_tid: 0,
            parent_tid: 0,
            exit_signal: SIGCHLD,
            stack: 0,
            stack_size: 0,
            tls: 0,
            set_tid: 0,
            set_tid_size: 0,
            cgroup: 0,
        };
        // SAFETY: the fixed assembly launcher issues clone3 over this initialized
        // record. Its child branch jumps directly into the audited trampoline;
        // only the parent branch returns to Rust.
        let clone_result = unsafe {
            fe2o3_authenticated_verus_clone_launcher_v2(
                (&raw const child_context).cast(),
                (&raw mut clone_arguments).cast(),
                size_of::<CloneArguments>(),
            )
        };
        if let Some(error) = clone3_result_error(clone_result) {
            return Err(AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                error,
            ));
        }
        debug_assert_ne!(clone_result, 0);
        drop(stdout_write);
        drop(stderr_write);
        drop(error_write);
        drop(gate_read);
        let process_id = c_int::try_from(clone_result).map_err(|_| {
            AuthenticatedVerusExecutionErrorV2::plain(process_error(role, ProcessFailureV2::Spawn))
        })?;
        if pidfd < 0 {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::Spawn,
            )));
        }
        // SAFETY: successful CLONE_PIDFD initialized one new owned descriptor.
        let pidfd = unsafe { OwnedFd::from_raw_fd(pidfd) };
        if let Err(error) = pidfd_send_signal(pidfd.as_raw_fd(), 0) {
            terminate_pidfd_bounded(role, pidfd.as_raw_fd(), cleanup_deadline())?;
            return Err(AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                error,
            ));
        }
        let proc_dir = match File::open(format!("/proc/{process_id}")) {
            Ok(proc_dir) => proc_dir,
            Err(error) => {
                terminate_pidfd_bounded(role, pidfd.as_raw_fd(), cleanup_deadline())?;
                return Err(AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Spawn),
                    error,
                ));
            }
        };
        if let Err(error) = ptrace_seize(process_id) {
            terminate_pidfd_bounded(role, pidfd.as_raw_fd(), cleanup_deadline())?;
            return Err(AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                error,
            ));
        }
        let mut gate_write = File::from(gate_write);
        if let Err(error) = gate_write.write_all(&[1]) {
            terminate_pidfd_bounded(role, pidfd.as_raw_fd(), cleanup_deadline())?;
            return Err(AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                error,
            ));
        }
        drop(gate_write);
        let mut child_error = File::from(error_read);
        if let Err(error) = await_exec_status(role, &mut child_error, deadline) {
            terminate_pidfd_bounded(role, pidfd.as_raw_fd(), cleanup_deadline())?;
            return Err(error);
        }
        if let Err(error) = await_initial_exec_stop(role, pidfd.as_raw_fd(), process_id, deadline) {
            terminate_pidfd_bounded(role, pidfd.as_raw_fd(), cleanup_deadline())?;
            return Err(error);
        }
        if let Err(error) = ptrace_request(PTRACE_CONT_V2, process_id) {
            terminate_pidfd_bounded(role, pidfd.as_raw_fd(), cleanup_deadline())?;
            return Err(AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                error,
            ));
        }
        Ok(SpawnedTarget {
            pidfd,
            proc_dir,
            process_id,
            stdout: File::from(stdout_read),
            stderr: File::from(stderr_read),
        })
    }

    fn cloexec_pipe(
        role: VerusExecutionRoleV2,
        nonblocking: bool,
    ) -> Result<(OwnedFd, OwnedFd), AuthenticatedVerusExecutionErrorV2> {
        const O_CLOEXEC: c_int = 0o2000000;
        const O_NONBLOCK: c_int = 0o0004000;
        let mut descriptors: [c_int; 2] = [-1; 2];
        // SAFETY: `descriptors` has storage for the two descriptors returned by pipe2.
        let flags = O_CLOEXEC | if nonblocking { O_NONBLOCK } else { 0 };
        if unsafe { linux_pipe2(descriptors.as_mut_ptr(), flags) } < 0 {
            return Err(AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::Spawn),
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: successful pipe2 returned two distinct owned descriptors.
        Ok(unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        })
    }

    #[repr(C)]
    struct LinuxPollFd {
        descriptor: c_int,
        events: i16,
        returned_events: i16,
    }

    fn await_exec_status(
        role: VerusExecutionRoleV2,
        status: &mut File,
        deadline: Instant,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        const POLLIN: i16 = 0x0001;
        const POLLERR: i16 = 0x0008;
        const POLLHUP: i16 = 0x0010;
        let mut byte = [0_u8; 1];
        loop {
            if Instant::now() >= deadline {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::Timeout,
                )));
            }
            match status.read(&mut byte) {
                Ok(0) => return Ok(()),
                Ok(_) => {
                    return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                        role,
                        ProcessFailureV2::Spawn,
                    )));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    return Err(AuthenticatedVerusExecutionErrorV2::io(
                        process_error(role, ProcessFailureV2::Spawn),
                        error,
                    ));
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::Timeout,
                )));
            }
            let timeout_millis = remaining.as_millis().clamp(1, 50) as c_int;
            let mut descriptor = LinuxPollFd {
                descriptor: status.as_raw_fd(),
                events: POLLIN | POLLERR | POLLHUP,
                returned_events: 0,
            };
            // SAFETY: `descriptor` is one writable pollfd and the timeout is bounded.
            let polled = unsafe { linux_poll(&mut descriptor, 1, timeout_millis) };
            if polled < 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(AuthenticatedVerusExecutionErrorV2::io(
                        process_error(role, ProcessFailureV2::Spawn),
                        error,
                    ));
                }
            }
        }
    }

    #[repr(C)]
    struct LinuxRlimit {
        current: u64,
        maximum: u64,
    }

    #[repr(C)]
    struct LinuxSigAction {
        handler: usize,
        mask: [u64; 16],
        flags: c_int,
        restorer: usize,
    }

    fn sigchld_policy_is_reapable() -> bool {
        const SIGCHLD: c_int = 17;
        const SA_NOCLDWAIT: c_int = 2;
        let mut action = LinuxSigAction {
            handler: 0,
            mask: [0; 16],
            flags: 0,
            restorer: 0,
        };
        // Require the default disposition: an ignored, custom, or auto-reaping
        // SIGCHLD policy could race this controller's pidfd-specific wait.
        (unsafe { linux_sigaction(SIGCHLD, std::ptr::null(), &mut action) }) == 0
            && action.handler == 0
            && action.flags & SA_NOCLDWAIT == 0
    }

    #[repr(C)]
    struct CapabilityHeader {
        version: u32,
        process: c_int,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CapabilityData {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }

    #[repr(C)]
    struct SocketFilterProgram {
        length: u16,
        filters: *const SeccompInstructionV2,
    }

    #[repr(C, align(8))]
    struct LinuxSigInfo {
        bytes: [u8; 128],
    }

    #[derive(Clone, Copy, Debug)]
    struct WaitEvent {
        code: c_int,
        status: c_int,
    }

    impl WaitEvent {
        fn is_terminal(self) -> bool {
            matches!(self.code, CLD_EXITED | CLD_KILLED | CLD_DUMPED)
        }
    }

    fn waitid_pidfd(pidfd: c_int, options: c_int) -> io::Result<Option<WaitEvent>> {
        const P_PIDFD: c_int = 3;
        let mut info = LinuxSigInfo { bytes: [0; 128] };
        // SAFETY: `info` is one correctly sized and aligned Linux x86_64 siginfo_t.
        if unsafe { linux_waitid(P_PIDFD, pidfd as c_uint, &mut info, options) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let integer = |offset: usize| {
            c_int::from_ne_bytes(
                info.bytes[offset..offset + size_of::<c_int>()]
                    .try_into()
                    .unwrap(),
            )
        };
        if integer(0) == 0 {
            Ok(None)
        } else {
            Ok(Some(WaitEvent {
                code: integer(8),
                status: integer(24),
            }))
        }
    }

    fn waitid_pidfd_terminal(pidfd: c_int, options: c_int) -> io::Result<Option<WaitEvent>> {
        waitid_pidfd(pidfd, options).map(|event| event.filter(|event| event.is_terminal()))
    }

    fn ptrace_seize(process_id: c_int) -> io::Result<()> {
        ptrace_with_data(
            PTRACE_SEIZE_V2,
            process_id,
            (PTRACE_O_EXITKILL_V2 | PTRACE_O_TRACEEXEC_V2) as usize,
        )
    }

    fn await_initial_exec_stop(
        role: VerusExecutionRoleV2,
        pidfd: c_int,
        process_id: c_int,
        deadline: Instant,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        loop {
            ensure_deadline(role, deadline)?;
            let status = waitpid_ptrace(process_id).map_err(|error| {
                AuthenticatedVerusExecutionErrorV2::io(
                    process_error(role, ProcessFailureV2::Spawn),
                    error,
                )
            })?;
            if let Some(status) = status {
                if ptrace_exec_status(status) {
                    return Ok(());
                }
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::UnexpectedPtraceStop,
                )));
            }
            if waitid_pidfd_terminal(pidfd, WAIT_EXITED | WAIT_NOHANG | WAIT_NOWAIT)
                .map_err(|error| {
                    AuthenticatedVerusExecutionErrorV2::io(
                        process_error(role, ProcessFailureV2::Spawn),
                        error,
                    )
                })?
                .is_some()
            {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::Spawn,
                )));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn ptrace_request(request: u32, process_id: c_int) -> io::Result<()> {
        ptrace_with_data(request, process_id, 0)
    }

    fn ptrace_with_data(request: u32, process_id: c_int, data: usize) -> io::Result<()> {
        // SAFETY: the pid is the unreaped CLONE_PIDFD child and ptrace interprets
        // null address plus the integer-valued data argument for these requests.
        if unsafe {
            linux_ptrace(
                request,
                process_id,
                std::ptr::null_mut(),
                data as *mut c_void,
            )
        } < 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn waitpid_ptrace(process_id: c_int) -> io::Result<Option<c_int>> {
        let mut status = 0;
        // SAFETY: `status` is writable and this waits nonblockingly for exactly
        // the unreaped child whose pidfd remains owned by the controller.
        let result = unsafe {
            linux_waitpid(
                process_id,
                &mut status,
                (PTRACE_WAIT_NOHANG_V2 | PTRACE_WAITPID_WALL_V2) as c_int,
            )
        };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else if result == 0 {
            Ok(None)
        } else if result != process_id {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "waitpid returned a different tracee",
            ))
        } else {
            Ok(Some(status))
        }
    }

    fn ptrace_interrupt_status(status: c_int) -> bool {
        status & 0xff == 0x7f
            && (status >> 8) & 0xff == SIGTRAP
            && ((status as u32) >> 16) == PTRACE_EVENT_STOP_V2
    }

    fn ptrace_exec_status(status: c_int) -> bool {
        status & 0xff == 0x7f
            && (status >> 8) & 0xff == SIGTRAP
            && ((status as u32) >> 16) == PTRACE_EVENT_EXEC_V2
    }

    fn cleanup_deadline() -> Instant {
        Instant::now()
            .checked_add(CLEANUP_TIMEOUT)
            .unwrap_or_else(Instant::now)
    }

    fn terminate_pidfd_bounded(
        role: VerusExecutionRoleV2,
        pidfd: c_int,
        deadline: Instant,
    ) -> Result<WaitEvent, AuthenticatedVerusExecutionErrorV2> {
        terminate_with_operations(
            role,
            deadline,
            || pidfd_send_signal(pidfd, SIGKILL),
            || waitid_pidfd_terminal(pidfd, WAIT_EXITED | WAIT_NOHANG),
            |timeout| poll_pidfd(pidfd, timeout),
        )
    }

    fn terminate_with_operations<Signal, Wait, Poll>(
        role: VerusExecutionRoleV2,
        deadline: Instant,
        mut signal: Signal,
        mut wait: Wait,
        mut poll: Poll,
    ) -> Result<WaitEvent, AuthenticatedVerusExecutionErrorV2>
    where
        Signal: FnMut() -> io::Result<()>,
        Wait: FnMut() -> io::Result<Option<WaitEvent>>,
        Poll: FnMut(Duration) -> io::Result<()>,
    {
        let termination_error = |error| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::TerminationUnconfirmed),
                error,
            )
        };
        if let Some(event) = wait().map_err(&termination_error)? {
            return Ok(event);
        }
        if let Err(signal_error) = signal() {
            if let Some(event) = wait().map_err(&termination_error)? {
                return Ok(event);
            }
            return Err(termination_error(signal_error));
        }
        loop {
            if let Some(event) = wait().map_err(&termination_error)? {
                return Ok(event);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                    role,
                    ProcessFailureV2::TerminationUnconfirmed,
                )));
            }
            poll(remaining.min(Duration::from_millis(50))).map_err(&termination_error)?;
        }
    }

    fn poll_pidfd(pidfd: c_int, timeout: Duration) -> io::Result<()> {
        const POLLIN: i16 = 0x0001;
        const POLLERR: i16 = 0x0008;
        const POLLHUP: i16 = 0x0010;
        const POLLNVAL: i16 = 0x0020;
        let mut descriptor = LinuxPollFd {
            descriptor: pidfd,
            events: POLLIN | POLLERR | POLLHUP,
            returned_events: 0,
        };
        let timeout_millis = timeout.as_millis().clamp(1, 50) as c_int;
        // SAFETY: `descriptor` is one writable pollfd and timeout is hard bounded.
        let result = unsafe { linux_poll(&mut descriptor, 1, timeout_millis) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(error);
        }
        if descriptor.returned_events & POLLNVAL != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "pidfd became invalid during cleanup",
            ));
        }
        Ok(())
    }

    fn pidfd_send_signal(pidfd: c_int, signal: c_int) -> io::Result<()> {
        const SYS_PIDFD_SEND_SIGNAL: c_long = 424;
        // SAFETY: the pidfd is live; no siginfo payload or flags are supplied.
        if unsafe {
            linux_syscall(
                SYS_PIDFD_SEND_SIGNAL,
                pidfd,
                signal,
                std::ptr::null::<c_void>(),
                0_u32,
            )
        } < 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn create_memfd(name: &str) -> io::Result<File> {
        const MFD_CLOEXEC: c_uint = 0x0001;
        const MFD_ALLOW_SEALING: c_uint = 0x0002;
        let name = CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid memfd name"))?;
        // SAFETY: `name` is a live NUL-terminated string and flags are valid.
        let descriptor =
            unsafe { linux_memfd_create(name.as_ptr(), MFD_CLOEXEC | MFD_ALLOW_SEALING) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful memfd_create returns a new owned descriptor.
        Ok(File::from(unsafe { OwnedFd::from_raw_fd(descriptor) }))
    }

    fn seal(file: &File) -> io::Result<()> {
        const F_ADD_SEALS: c_int = 1033;
        const ALL_IMMUTABLE_SEALS: c_int = 0x0001 | 0x0002 | 0x0004 | 0x0008;
        // SAFETY: the descriptor is live and F_ADD_SEALS accepts this integer bitset.
        if unsafe { linux_fcntl(file.as_raw_fd(), F_ADD_SEALS, ALL_IMMUTABLE_SEALS) } < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    unsafe extern "C" {
        #[link_name = "memfd_create"]
        fn linux_memfd_create(name: *const c_char, flags: c_uint) -> c_int;

        #[link_name = "fcntl"]
        fn linux_fcntl(fd: c_int, command: c_int, ...) -> c_int;

        #[link_name = "pipe2"]
        fn linux_pipe2(descriptors: *mut c_int, flags: c_int) -> c_int;

        #[link_name = "poll"]
        fn linux_poll(descriptors: *mut LinuxPollFd, count: usize, timeout: c_int) -> c_int;

        #[link_name = "recv"]
        fn linux_recv(fd: c_int, bytes: *mut c_void, length: usize, flags: c_int) -> isize;

        #[link_name = "prctl"]
        fn linux_prctl(option: c_int, ...) -> c_int;

        #[link_name = "sigaction"]
        fn linux_sigaction(
            signal: c_int,
            action: *const LinuxSigAction,
            previous: *mut LinuxSigAction,
        ) -> c_int;

        #[link_name = "waitid"]
        fn linux_waitid(
            id_type: c_int,
            id: c_uint,
            information: *mut LinuxSigInfo,
            options: c_int,
        ) -> c_int;

        #[link_name = "waitpid"]
        fn linux_waitpid(process_id: c_int, status: *mut c_int, options: c_int) -> c_int;

        #[link_name = "ptrace"]
        fn linux_ptrace(
            request: c_uint,
            process_id: c_int,
            address: *mut c_void,
            data: *mut c_void,
        ) -> c_long;

        #[link_name = "syscall"]
        fn linux_syscall(number: c_long, ...) -> c_long;

        #[link_name = "getrlimit"]
        fn linux_getrlimit(resource: c_int, value: *mut LinuxRlimit) -> c_int;

    }

    fn fresh_challenge() -> Result<Digest, AuthenticatedVerusExecutionErrorV2> {
        let mut bytes = [0_u8; 32];
        File::open(RANDOM_SOURCE)
            .and_then(|mut source| source.read_exact(&mut bytes))
            .map_err(data_io)?;
        if bytes == [0; 32] {
            return Err(AuthenticatedVerusExecutionErrorV2::plain(
                AuthenticatedVerusExecutionErrorKindV2::InvalidChallenge,
            ));
        }
        Ok(Digest::from_bytes(bytes))
    }

    fn controller_thread_id(
        role: VerusExecutionRoleV2,
    ) -> Result<u32, AuthenticatedVerusExecutionErrorV2> {
        const SYS_GETTID: c_long = 186;
        // SAFETY: gettid has no arguments and returns the calling Linux thread id.
        let result = unsafe { linux_syscall(SYS_GETTID) };
        u32::try_from(result).map_err(|_| {
            AuthenticatedVerusExecutionErrorV2::io(
                process_error(role, ProcessFailureV2::PrivilegedController),
                io::Error::last_os_error(),
            )
        })
    }

    fn read_retry(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match reader.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }

    fn ensure_deadline(
        role: VerusExecutionRoleV2,
        deadline: Instant,
    ) -> Result<(), AuthenticatedVerusExecutionErrorV2> {
        if Instant::now() >= deadline {
            Err(AuthenticatedVerusExecutionErrorV2::plain(process_error(
                role,
                ProcessFailureV2::Timeout,
            )))
        } else {
            Ok(())
        }
    }

    fn process_error(
        role: VerusExecutionRoleV2,
        failure: ProcessFailureV2,
    ) -> AuthenticatedVerusExecutionErrorKindV2 {
        AuthenticatedVerusExecutionErrorKindV2::Process { role, failure }
    }

    fn observe_error(role: VerusExecutionRoleV2) -> AuthenticatedVerusExecutionErrorV2 {
        AuthenticatedVerusExecutionErrorV2::plain(process_error(role, ProcessFailureV2::Observe))
    }

    fn control_io(
        role: VerusExecutionRoleV2,
        error: io::Error,
    ) -> AuthenticatedVerusExecutionErrorV2 {
        AuthenticatedVerusExecutionErrorV2::io(
            process_error(role, ProcessFailureV2::ControlProtocol),
            error,
        )
    }

    fn result_error(role: VerusExecutionRoleV2) -> AuthenticatedVerusExecutionErrorV2 {
        AuthenticatedVerusExecutionErrorV2::plain(process_error(
            role,
            ProcessFailureV2::ResultEnvelope,
        ))
    }

    fn data_io(error: io::Error) -> AuthenticatedVerusExecutionErrorV2 {
        AuthenticatedVerusExecutionErrorV2::io(
            AuthenticatedVerusExecutionErrorKindV2::Process {
                role: VerusExecutionRoleV2::Verus,
                failure: ProcessFailureV2::Observe,
            },
            error,
        )
    }

    fn plan_error(error: PlanError) -> AuthenticatedVerusExecutionErrorV2 {
        AuthenticatedVerusExecutionErrorV2::plain(AuthenticatedVerusExecutionErrorKindV2::Plan(
            error,
        ))
    }

    #[cfg(test)]
    mod tests {
        use sha2::{Digest as _, Sha256};
        use std::{
            cell::Cell,
            fs::File,
            io,
            time::{Duration, Instant},
        };

        use super::super::{
            ProcessFailureV2, SECCOMP_FILTER_V2, SECCOMP_RETURN_ALLOW_V2, VerusExecutionRoleV2,
            child_clone_launcher_bytes_v2, child_trampoline_bytes_v2,
            child_trampoline_policy_bytes_from_v2, ptrace_checkpoint_policy_bytes_v2,
            seccomp_filter_policy_bytes_v2, sha256,
        };
        use super::{
            LimitPair, WaitEvent, await_exec_status, cloexec_pipe, clone3_result_error,
            controller_credentials, controller_status_allows_spawn, create_memfd,
            hash_file_bounded, parse_limit, ptrace_exec_status, ptrace_interrupt_status,
            terminate_with_operations,
        };

        const UNPRIVILEGED: &str = "Uid:\t1000\t1000\t1000\t1000\n\
Gid:\t1000\t1000\t1000\t1000\n\
Groups:\t1000 1001\n\
CapInh:\t0000000000000000\n\
CapPrm:\t0000000000000000\n\
CapEff:\t0000000000000000\n\
CapBnd:\t000001ffffffffff\n\
CapAmb:\t0000000000000000\n\
NoNewPrivs:\t0\n\
Seccomp:\t0\n\
Seccomp_filters:\t0\n";

        #[test]
        fn controller_status_rejects_root_unequal_credentials_and_active_capabilities() {
            assert!(controller_credentials(UNPRIVILEGED).is_some());
            assert!(
                controller_credentials(
                    &UNPRIVILEGED.replace("Uid:\t1000\t1000\t1000\t1000", "Uid:\t0\t0\t0\t0")
                )
                .is_none()
            );
            assert!(
                controller_credentials(&UNPRIVILEGED.replace(
                    "Uid:\t1000\t1000\t1000\t1000",
                    "Uid:\t1000\t1001\t1000\t1000",
                ))
                .is_none()
            );
            assert!(
                controller_credentials(
                    &UNPRIVILEGED.replace("Gid:\t1000\t1000\t1000\t1000", "Gid:\t0\t0\t0\t0")
                )
                .is_none()
            );
            assert!(
                controller_credentials(&UNPRIVILEGED.replace(
                    "Gid:\t1000\t1000\t1000\t1000",
                    "Gid:\t1000\t1001\t1000\t1000",
                ))
                .is_none()
            );
            assert!(
                controller_credentials(
                    &UNPRIVILEGED.replace("Groups:\t1000 1001", "Groups:\t0 1000")
                )
                .is_none()
            );
            for field in ["CapInh:", "CapPrm:", "CapEff:", "CapAmb:"] {
                let privileged = UNPRIVILEGED.replace(
                    &format!("{field}\t0000000000000000"),
                    &format!("{field}\t0000000000000001"),
                );
                assert!(controller_credentials(&privileged).is_none());
            }
            assert!(controller_credentials(&UNPRIVILEGED.replace("CapBnd:", "Missing:")).is_none());
            assert!(
                controller_credentials(
                    &UNPRIVILEGED.replace("Groups:\t1000 1001", "Groups:\t1000 invalid")
                )
                .is_none()
            );
        }

        #[test]
        fn controller_status_rejects_every_inherited_seccomp_filter() {
            assert!(controller_status_allows_spawn(UNPRIVILEGED));
            assert!(!controller_status_allows_spawn(
                &UNPRIVILEGED.replace("Seccomp:\t0", "Seccomp:\t2")
            ));
            assert!(!controller_status_allows_spawn(
                &UNPRIVILEGED.replace("Seccomp_filters:\t0", "Seccomp_filters:\t1")
            ));
        }

        #[test]
        fn kill_failure_never_enters_a_blocking_wait_or_poll() {
            let wait_calls = Cell::new(0_u32);
            let poll_called = Cell::new(false);
            let error = terminate_with_operations(
                VerusExecutionRoleV2::Solver,
                Instant::now() + Duration::from_secs(1),
                || Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected")),
                || {
                    wait_calls.set(wait_calls.get() + 1);
                    Ok(None)
                },
                |_| {
                    poll_called.set(true);
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(wait_calls.get(), 2);
            assert!(!poll_called.get());
            assert!(matches!(
                error.kind(),
                super::super::AuthenticatedVerusExecutionErrorKindV2::Process {
                    failure: ProcessFailureV2::TerminationUnconfirmed,
                    ..
                }
            ));
        }

        #[test]
        fn canonical_filter_bytes_change_when_process_denial_is_weakened() {
            let expected = sha256(&seccomp_filter_policy_bytes_v2(&SECCOMP_FILTER_V2));
            let mut syscall_weakened = SECCOMP_FILTER_V2;
            let clone_comparison = syscall_weakened
                .iter_mut()
                .find(|instruction| instruction.value == 56)
                .unwrap();
            clone_comparison.value = u32::MAX;
            assert_ne!(
                sha256(&seccomp_filter_policy_bytes_v2(&syscall_weakened)),
                expected
            );
            let mut action_weakened = SECCOMP_FILTER_V2;
            action_weakened[7].value = SECCOMP_RETURN_ALLOW_V2;
            assert_ne!(
                sha256(&seccomp_filter_policy_bytes_v2(&action_weakened)),
                expected
            );
        }

        #[test]
        fn canonical_policy_binds_every_child_trampoline_byte() {
            let launcher = child_clone_launcher_bytes_v2();
            let trampoline = child_trampoline_bytes_v2();
            assert!(!launcher.is_empty());
            assert!(!trampoline.is_empty());
            let expected = sha256(&child_trampoline_policy_bytes_from_v2(launcher, trampoline));
            let mut weakened_launcher = launcher.to_vec();
            let launcher_middle = weakened_launcher.len() / 2;
            weakened_launcher[launcher_middle] ^= 1;
            assert_ne!(
                sha256(&child_trampoline_policy_bytes_from_v2(
                    &weakened_launcher,
                    trampoline,
                )),
                expected
            );
            let mut weakened = trampoline.to_vec();
            let middle = weakened.len() / 2;
            weakened[middle] ^= 1;
            assert_ne!(
                sha256(&child_trampoline_policy_bytes_from_v2(launcher, &weakened)),
                expected
            );
        }

        #[test]
        fn clone3_raw_negative_results_preserve_exact_errno() {
            assert!(clone3_result_error(17).is_none());
            assert_eq!(clone3_result_error(-1).unwrap().raw_os_error(), Some(1));
            assert_eq!(clone3_result_error(-38).unwrap().raw_os_error(), Some(38));
            assert_eq!(
                clone3_result_error(-4095).unwrap().raw_os_error(),
                Some(4095)
            );
        }

        #[test]
        fn ptrace_checkpoint_accepts_only_exact_interrupt_event_stop() {
            let exact = ((super::PTRACE_EVENT_STOP_V2 as i32) << 16) | (super::SIGTRAP << 8) | 0x7f;
            assert!(ptrace_interrupt_status(exact));
            assert!(!ptrace_interrupt_status(exact ^ (1 << 16)));
            assert!(!ptrace_interrupt_status(exact ^ (1 << 8)));
            assert!(!ptrace_interrupt_status(exact ^ 1));

            let exec = ((super::PTRACE_EVENT_EXEC_V2 as i32) << 16) | (super::SIGTRAP << 8) | 0x7f;
            assert!(ptrace_exec_status(exec));
            assert!(!ptrace_exec_status(exact));
        }

        #[test]
        fn pidfd_ptrace_notifications_are_not_terminal_events() {
            assert!(
                WaitEvent {
                    code: super::CLD_EXITED,
                    status: 0,
                }
                .is_terminal()
            );
            assert!(
                WaitEvent {
                    code: super::CLD_KILLED,
                    status: super::SIGKILL,
                }
                .is_terminal()
            );
            assert!(
                WaitEvent {
                    code: super::CLD_DUMPED,
                    status: super::SIGTRAP,
                }
                .is_terminal()
            );
            assert!(
                !WaitEvent {
                    code: 4,
                    status: ((super::PTRACE_EVENT_STOP_V2 as i32) << 8) | super::SIGTRAP,
                }
                .is_terminal()
            );
        }

        #[test]
        fn canonical_policy_binds_exact_ptrace_checkpoint_bytes() {
            let exact = ptrace_checkpoint_policy_bytes_v2();
            let digest = sha256(&exact);
            for index in 0..exact.len() {
                let mut weakened = exact.clone();
                weakened[index] ^= 1;
                assert_ne!(sha256(&weakened), digest);
            }
        }

        #[test]
        fn sparse_near_limit_file_hashing_uses_a_fixed_controller_buffer() {
            const TEST_LIMIT: u64 = 32 * 1024 * 1024;
            let mut sparse = create_memfd("fe2o3-v2-sparse-near-limit").unwrap();
            sparse.set_len(TEST_LIMIT).unwrap();
            let measured = hash_file_bounded(
                VerusExecutionRoleV2::Solver,
                &mut sparse,
                TEST_LIMIT,
                TEST_LIMIT,
                Instant::now() + Duration::from_secs(30),
            )
            .unwrap();
            let mut expected = Sha256::new();
            let zeros = [0_u8; super::IO_CHUNK_BYTES];
            for _ in 0..TEST_LIMIT / zeros.len() as u64 {
                expected.update(zeros);
            }
            assert_eq!(measured.as_bytes(), expected.finalize().as_slice());

            let mut over = create_memfd("fe2o3-v2-sparse-over-limit").unwrap();
            over.set_len(TEST_LIMIT + 1).unwrap();
            assert!(
                hash_file_bounded(
                    VerusExecutionRoleV2::Solver,
                    &mut over,
                    TEST_LIMIT + 1,
                    TEST_LIMIT,
                    Instant::now() + Duration::from_secs(30),
                )
                .is_err()
            );
        }

        #[test]
        fn pre_exec_status_wait_is_bounded_by_the_stage_deadline() {
            let (reader, _writer) = cloexec_pipe(VerusExecutionRoleV2::Solver, true).unwrap();
            let mut reader = File::from(reader);
            let error = await_exec_status(
                VerusExecutionRoleV2::Solver,
                &mut reader,
                Instant::now() + Duration::from_millis(10),
            )
            .unwrap_err();
            assert!(matches!(
                error.kind(),
                super::super::AuthenticatedVerusExecutionErrorKindV2::Process {
                    failure: ProcessFailureV2::Timeout,
                    ..
                }
            ));

            let (reader, writer) = cloexec_pipe(VerusExecutionRoleV2::Solver, true).unwrap();
            drop(writer);
            let mut reader = File::from(reader);
            let expired = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
            let error =
                await_exec_status(VerusExecutionRoleV2::Solver, &mut reader, expired).unwrap_err();
            assert!(matches!(
                error.kind(),
                super::super::AuthenticatedVerusExecutionErrorKindV2::Process {
                    failure: ProcessFailureV2::Timeout,
                    ..
                }
            ));
        }

        #[test]
        fn limit_parser_rejects_missing_values_and_preserves_exact_soft_hard_limits() {
            let limits =
                "Max file size             1048576              2097152              bytes\n";
            assert_eq!(
                parse_limit(limits, "Max file size"),
                Some(LimitPair {
                    soft: 1_048_576,
                    hard: 2_097_152,
                })
            );
            assert_eq!(parse_limit("Max file size 1\n", "Max file size"), None);
        }
    }
}
