//! Inert original-root intake transcript, never invocation or process authority.
use crate::{
    CompilerExecutionIssuerPolicyV3 as Policy,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3;
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V3: usize = 224;
const N: usize = COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V3;
const VERSION: u32 = 3;
const DEBUG_NAME: &str = "CompilerExecutionRootIntakeRecordV3";
const WORK: usize = COMPILER_EXECUTION_ROOT_INTAKE_WORK_V3;
const SCRATCH: usize = COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V3;
const MAGIC: &[u8; 8] = b"F2O3CRI3";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-ROOT-INTAKE/V3\0";
const DIGEST: usize = N - 32;
const RETAINED: usize = size_of::<(CompilerExecutionRootIntakeRecordV3, Storage)>();
pub const COMPILER_EXECUTION_ROOT_INTAKE_WORK_V3: usize = resources::ENTRY_WORK + 32 * N;
pub const COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V3: usize =
    8 * RETAINED + 8 * N + 2 * size_of::<Sha256>() + 4096;

/// Closed transport phases. None grants authority or reports readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerExecutionRootIntakeKindV3 {
    Hello = 1,
    Challenge = 2,
    Input = 3,
    Ack = 4,
}
use CompilerExecutionRootIntakeKindV3 as Kind;

/// Exact descriptor ordering; input count follows the stdio mask, not a wire count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerExecutionRootIntakeRoleV3 {
    Invocation = 1,
    WorkingDirectory = 2,
    Stdin = 3,
    Stdout = 4,
    Stderr = 5,
}
use CompilerExecutionRootIntakeRoleV3 as Role;

/// Fixed canonical transcript bytes. Hashes detect corruption and bind frames;
/// they do NOT authenticate a peer, prove nonce freshness, admit an invocation,
/// establish original-root custody, or authorize an FD195 transition.
///
/// Hello/challenge/ACK carry zero rights; each ordered Input carries exactly one.
/// The receiver must enforce those transport facts, actual packet credentials,
/// the complete role sequence and retained ownership before sending a reply.
/// The sole ACK status is terminal RuntimeEnforcementUnavailable, NOT success.
/// EOF or a partial transcript is refusal, never an implicit ACK.
///
/// `invocation_bytes` always declares the complete sealed invocation file, even
/// on cwd/stdio frames. This codec checks conversion to usize and the existing
/// MAX_DESCRIPTOR_BYTES_V3; the consumer checks exact actual sealed length BEFORE
/// allocation/read. Framing grants no size allowance on the caller's account.
///
/// Constructors/decode return full UNRESERVED storage. Keep input owners prepaid
/// on the original account; work and denial history are never refunded.
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionRootIntakeRecordV3 {
    bytes: [u8; N],
}
use CompilerExecutionRootIntakeRecordV3 as Record;

impl Record {
    pub fn hello(
        policy: &Policy,
        invocation: [u8; 32],
        nonce: [u8; 32],
        stdio_mask: u8,
        invocation_bytes: u64,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(b, policy.retained_storage(), || {
            let mut bytes = [0; N];
            bytes[..8].copy_from_slice(MAGIC);
            bytes[8..12].copy_from_slice(&3u32.to_le_bytes());
            bytes[12..16].copy_from_slice(&(N as u32).to_le_bytes());
            bytes[16] = Kind::Hello as u8;
            bytes[18] = stdio_mask;
            bytes[24..56].copy_from_slice(&nonce);
            bytes[88..120].copy_from_slice(policy.identity().as_bytes());
            bytes[120..152].copy_from_slice(&invocation);
            bytes[184..192].copy_from_slice(&invocation_bytes.to_le_bytes());
            finish(bytes)
        })
    }
}

fn selected_roles(mask: u8) -> [Option<Role>; 6] {
    [
        Some(Role::Invocation),
        Some(Role::WorkingDirectory),
        (mask & 1 != 0).then_some(Role::Stdin),
        (mask & 2 != 0).then_some(Role::Stdout),
        (mask & 4 != 0).then_some(Role::Stderr),
        None,
    ]
}
fn role(byte: u8) -> Option<Role> {
    match byte {
        1 => Some(Role::Invocation),
        2 => Some(Role::WorkingDirectory),
        3 => Some(Role::Stdin),
        4 => Some(Role::Stdout),
        5 => Some(Role::Stderr),
        _ => None,
    }
}

include!("root_intake_body.rs");
pub use Error as CompilerExecutionRootIntakeErrorV3;

#[cfg(test)]
#[path = "root_intake_v3_tests.rs"]
mod tests;
