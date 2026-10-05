//! Inert terminal result for an exact original-root intake, never execution authority.
use crate::{
    CompilerExecutionRootIntakeKindV4 as IntakeKind, CompilerExecutionRootIntakeRecordV4 as Intake,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3;
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_ROOT_COMPLETION_BYTES_V1: usize = 240;
const N: usize = COMPILER_EXECUTION_ROOT_COMPLETION_BYTES_V1;
const DIGEST: usize = N - 32;
const MAGIC: &[u8; 8] = b"F2O3CRC1";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-ROOT-COMPLETION/V1\0";
const RETAINED: usize = size_of::<(Record, Storage)>();
pub const COMPILER_EXECUTION_ROOT_COMPLETION_WORK_V1: usize = resources::ENTRY_WORK + 32 * N;
pub const COMPILER_EXECUTION_ROOT_COMPLETION_STORAGE_V1: usize =
    8 * RETAINED + 8 * N + 2 * size_of::<Sha256>() + 4096;
const WORK: usize = COMPILER_EXECUTION_ROOT_COMPLETION_WORK_V1;
const SCRATCH: usize = COMPILER_EXECUTION_ROOT_COMPLETION_STORAGE_V1;

/// A reported wait result, not evidence of its occurrence or of successful proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionRootTerminationV1 {
    Exited(u8),
    Signaled(u8),
}
use CompilerExecutionRootTerminationV1 as Termination;

/// Distinct from the V4 refusal ACK; old decoders must reject these bytes.
/// Every constructor/decoder is authority-free. An actual root must retain the
/// original compiler through terminal wait and trace retirement before sending.
/// A receiver must authenticate that root on its retained endpoint and compare
/// its exact last intake before using the result. Exit zero alone never admits
/// a publication, proof, executable, load or launch.
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionRootCompletionRecordV1 {
    bytes: [u8; N],
}
use CompilerExecutionRootCompletionRecordV1 as Record;

impl Record {
    pub fn new(
        last: &Intake,
        termination: Termination,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(b, last.retained_storage(), || {
            if !final_input(last) {
                return Err(Error::Framing("completion requires final selected input"));
            }
            let mut bytes = *last.canonical_bytes();
            bytes[..8].copy_from_slice(MAGIC);
            bytes[8..12].copy_from_slice(&1_u32.to_le_bytes());
            let (kind, value) = match termination {
                Termination::Exited(code) => (1, code),
                Termination::Signaled(signal) => (2, signal),
            };
            bytes[16] = kind;
            bytes[17] = 0;
            bytes[19] = 0;
            bytes[20..24].copy_from_slice(&u32::from(value).to_le_bytes());
            bytes[152..184].copy_from_slice(last.identity());
            validate(&bytes)?;
            let identity = digest(&bytes);
            bytes[DIGEST..].copy_from_slice(&identity);
            Ok((Self { bytes }, Storage(RETAINED)))
        })
    }

    pub fn decode(bytes: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, resources::fixed_input_floor(bytes, N), || {
            if bytes.len() != N {
                return Err(Error::Framing("root completion length"));
            }
            let bytes: [u8; N] = bytes.try_into().expect("checked completion length");
            validate(&bytes)?;
            if bytes[DIGEST..] != digest(&bytes) {
                return Err(Error::Framing("root completion digest"));
            }
            Ok((Self { bytes }, Storage(RETAINED)))
        })
    }

    /// Inert transcript equality only; kernel sender and endpoint checks are separate.
    pub fn matches_intake(&self, last: &Intake, b: &mut Budget<'_>) -> Result<bool> {
        metered(b, RETAINED + last.retained_storage(), || {
            let previous = last.canonical_bytes();
            Ok(final_input(last)
                && self.bytes[18] == previous[18]
                && self.bytes[24..152] == previous[24..152]
                && self.bytes[152..184] == *last.identity()
                && self.bytes[184..DIGEST] == previous[184..DIGEST])
        })
    }

    pub fn termination(&self) -> Termination {
        let value = self.bytes[20];
        match self.bytes[16] {
            1 => Termination::Exited(value),
            2 => Termination::Signaled(value),
            _ => unreachable!("validated root completion"),
        }
    }

    pub const fn canonical_bytes(&self) -> &[u8; N] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

fn final_input(last: &Intake) -> bool {
    last.kind() == IntakeKind::Input && last.role() == last.roles().last()
}

fn validate(bytes: &[u8; N]) -> Result<()> {
    let value = u32::from_le_bytes(bytes[20..24].try_into().expect("fixed status"));
    let length = u64::from_le_bytes(bytes[184..192].try_into().expect("fixed length"));
    if &bytes[..8] != MAGIC
        || bytes[8..12] != 1_u32.to_le_bytes()
        || bytes[12..16] != (N as u32).to_le_bytes()
        || bytes[17] != 0
        || bytes[18] & !7 != 0
        || bytes[19] != 0
        || !(1..=MAX_DESCRIPTOR_BYTES_V3 as u64).contains(&length)
        || !matches!((bytes[16], value), (1, 0..=255) | (2, 1..=64))
    {
        return Err(Error::Framing("root completion header or terminal status"));
    }
    for start in [24, 56, 88, 120, 152] {
        if bytes[start..start + 32] == [0; 32] {
            return Err(Error::Framing("zero root completion association"));
        }
    }
    Ok(())
}

fn digest(bytes: &[u8; N]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(&bytes[..DIGEST]);
    hash.finalize().into()
}

fn metered<T>(
    b: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    b.with_prepaid_scope(floor, resources::ENTRY_WORK, WORK, SCRATCH, |_| operation())
}

#[derive(Debug)]
pub enum CompilerExecutionRootCompletionErrorV1 {
    Framing(&'static str),
    Resource(Resource),
}
use CompilerExecutionRootCompletionErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(reason) => f.write_str(reason),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}
impl fmt::Debug for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionRootCompletionRecordV1")
            .field("termination", &self.termination())
            .field("authority", &"none")
            .finish()
    }
}

#[cfg(test)]
#[path = "root_completion_v1_tests.rs"]
mod tests;
