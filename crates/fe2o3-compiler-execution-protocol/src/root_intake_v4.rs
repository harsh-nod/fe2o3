//! Mandatory original output-directory transport; no execution or namespace authority.
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

pub const COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V4: usize = 240;
pub const COMPILER_EXECUTION_ROOT_INTAKE_OUTPUT_FD_V4: i32 = 197;
const N: usize = COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V4;
const VERSION: u32 = 4;
const MAGIC: &[u8; 8] = b"F2O3CRI4";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-ROOT-INTAKE/V4\0";
const DEBUG_NAME: &str = "CompilerExecutionRootIntakeRecordV4";
const DIGEST: usize = N - 32;
const RETAINED: usize = size_of::<(CompilerExecutionRootIntakeRecordV4, Storage)>();
pub const COMPILER_EXECUTION_ROOT_INTAKE_WORK_V4: usize = resources::ENTRY_WORK + 32 * N;
pub const COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V4: usize =
    8 * RETAINED + 8 * N + 2 * size_of::<Sha256>() + 4096;
const WORK: usize = COMPILER_EXECUTION_ROOT_INTAKE_WORK_V4;
const SCRATCH: usize = COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V4;

pub use crate::root_intake_v3::CompilerExecutionRootIntakeKindV3 as CompilerExecutionRootIntakeKindV4;
use CompilerExecutionRootIntakeKindV4 as Kind;

/// Closed mandatory order: invocation, cwd, output directory, selected stdio.
/// Role numbers are not child descriptor numbers; only OutputDirectory binds 197.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerExecutionRootIntakeRoleV4 {
    Invocation = 1,
    WorkingDirectory = 2,
    Stdin = 3,
    Stdout = 4,
    Stderr = 5,
    OutputDirectory = 6,
}
use CompilerExecutionRootIntakeRoleV4 as Role;

/// Inert V4 transcript. V3 bytes retain their old meaning and are never upgraded.
/// Every frame binds output device/inode at bytes 192..208, followed by its
/// domain-separated digest. These scalar claims are NOT object authority: sender
/// and receiver must check the actual retained descriptors, kernel credentials,
/// exact sequence and ownership through the authenticated terminal ACK.
/// Directory custody is not immutability, publication admission or a source view.
/// Zero-right hello/challenge/ACK and one-right Input rules are unchanged.
/// The only ACK remains RuntimeEnforcementUnavailable, never Ready or FD195 release.
/// All constructor results are FULL/unreserved on the original account.
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionRootIntakeRecordV4 {
    bytes: [u8; N],
}
use CompilerExecutionRootIntakeRecordV4 as Record;

impl Record {
    #[allow(clippy::too_many_arguments)]
    pub fn hello(
        policy: &Policy,
        invocation: [u8; 32],
        nonce: [u8; 32],
        stdio_mask: u8,
        invocation_bytes: u64,
        output: (u64, u64),
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(b, policy.retained_storage(), || {
            let mut bytes = [0; N];
            bytes[..8].copy_from_slice(MAGIC);
            bytes[8..12].copy_from_slice(&VERSION.to_le_bytes());
            bytes[12..16].copy_from_slice(&(N as u32).to_le_bytes());
            bytes[16] = Kind::Hello as u8;
            bytes[18] = stdio_mask;
            bytes[24..56].copy_from_slice(&nonce);
            bytes[88..120].copy_from_slice(policy.identity().as_bytes());
            bytes[120..152].copy_from_slice(&invocation);
            bytes[184..192].copy_from_slice(&invocation_bytes.to_le_bytes());
            bytes[192..200].copy_from_slice(&output.0.to_le_bytes());
            bytes[200..208].copy_from_slice(&output.1.to_le_bytes());
            finish(bytes)
        })
    }
    pub fn output_identity(&self) -> (u64, u64) {
        (
            u64::from_le_bytes(fixed(&self.bytes[192..200])),
            u64::from_le_bytes(fixed(&self.bytes[200..208])),
        )
    }
}
fn selected_roles(mask: u8) -> [Option<Role>; 6] {
    [
        Some(Role::Invocation),
        Some(Role::WorkingDirectory),
        Some(Role::OutputDirectory),
        (mask & 1 != 0).then_some(Role::Stdin),
        (mask & 2 != 0).then_some(Role::Stdout),
        (mask & 4 != 0).then_some(Role::Stderr),
    ]
}
fn role(byte: u8) -> Option<Role> {
    match byte {
        1 => Some(Role::Invocation),
        2 => Some(Role::WorkingDirectory),
        3 => Some(Role::Stdin),
        4 => Some(Role::Stdout),
        5 => Some(Role::Stderr),
        6 => Some(Role::OutputDirectory),
        _ => None,
    }
}
include!("root_intake_body.rs");
pub use Error as CompilerExecutionRootIntakeErrorV4;

#[cfg(test)]
#[path = "root_intake_v4_tests.rs"]
mod tests;
