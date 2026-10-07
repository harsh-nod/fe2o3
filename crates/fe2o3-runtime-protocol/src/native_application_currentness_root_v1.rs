//! Inert framing for a distinct root/application-currentness service gate.
use crate::NativeApplicationRegistrationBindingV1 as Registration;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionServiceLaunchManifestV3 as Manifest,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

const MAGIC: &[u8; 8] = b"F3NACR1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION-CURRENTNESS-ROOT/V1\0";
const HEADER: usize = 16;
const FIELDS: usize = HEADER + 6 * 32;
pub const NATIVE_APPLICATION_CURRENTNESS_ROOT_BYTES_V1: usize = FIELDS + 32;
/// Exact existing root-channel datagram width, with a distinct native payload
/// and canonical zero tail. This is not the compiler root-control codec.
pub const NATIVE_APPLICATION_CURRENTNESS_ROOT_GATE_BYTES_V1: usize = 4096;
const BYTES: usize = NATIVE_APPLICATION_CURRENTNESS_ROOT_BYTES_V1;
const ENTRY: usize = 8;
const WORK: usize = ENTRY + 8 * BYTES;
const SCRATCH: usize =
    4 * size_of::<NativeApplicationCurrentnessRootRecordV1>() + 2 * size_of::<Sha256>() + 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeApplicationCurrentnessRootKindV1 {
    Request,
    Reply,
}
use NativeApplicationCurrentnessRootKindV1 as Kind;

/// A descriptive packet, never root admission or currentness authority.
/// The reply digest is public anti-confusion framing, not authentication. The
/// service must separately retain the original root/issuer process and channel.
/// This wire is deliberately incompatible with the compiler occurrence gate.
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationCurrentnessRootRecordV1 {
    bytes: [u8; BYTES],
}
use NativeApplicationCurrentnessRootRecordV1 as Record;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationCurrentnessRootStorageV1(usize);
impl NativeApplicationCurrentnessRootStorageV1 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use NativeApplicationCurrentnessRootStorageV1 as Storage;

#[derive(Debug)]
pub enum NativeApplicationCurrentnessRootErrorV1 {
    Resource(Resource),
    Length,
    Header,
    Identity,
    Binding,
    Kind,
}
use NativeApplicationCurrentnessRootErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native application currentness root: {self:?}")
    }
}
impl std::error::Error for Error {}

impl Record {
    pub const WORK: usize = WORK;
    pub const SCRATCH: usize = SCRATCH;
    pub const STORAGE: usize = size_of::<(Self, Storage)>();

    pub fn has_gate_magic(bytes: &[u8]) -> bool {
        bytes.starts_with(MAGIC)
    }

    pub fn decode_gate(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        let n = NATIVE_APPLICATION_CURRENTNESS_ROOT_GATE_BYTES_V1;
        budget.charge_work(ENTRY)?;
        budget.with_prepaid_scope(bytes.len().min(n), 0, 2 * n, n + SCRATCH, |budget| {
            if bytes.len() != n || bytes[BYTES..].iter().any(|byte| *byte != 0) {
                return Err(Error::Length);
            }
            Self::decode(&bytes[..BYTES], budget)
        })
    }

    pub fn gate_bytes(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<[u8; NATIVE_APPLICATION_CURRENTNESS_ROOT_GATE_BYTES_V1]> {
        let n = NATIVE_APPLICATION_CURRENTNESS_ROOT_GATE_BYTES_V1;
        budget.with_prepaid_scope(Self::STORAGE, ENTRY, 2 * n, n + SCRATCH, |_| {
            let mut bytes = [0; NATIVE_APPLICATION_CURRENTNESS_ROOT_GATE_BYTES_V1];
            bytes[..BYTES].copy_from_slice(&self.bytes);
            Ok(bytes)
        })
    }

    /// Inputs stay prepaid. The returned full owner charge is unreserved.
    pub fn request(
        policy: &Policy,
        manifest: &Manifest,
        registration: &Registration,
        challenge: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        budget.charge_work(ENTRY)?;
        let floor = policy
            .retained_storage()
            .checked_add(manifest.retained_storage())
            .and_then(|n| n.checked_add(registration.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, WORK - ENTRY, SCRATCH, |_| {
            if manifest.policy_identity() != policy.identity()
                || registration
                    .compiler_handoff()
                    .launch_manifest()
                    .canonical_bytes()
                    != manifest.canonical_bytes()
                || challenge == [0; 32]
            {
                return Err(Error::Binding);
            }
            Self::from_fields(
                Kind::Request,
                [
                    *policy.identity().as_bytes(),
                    *manifest.identity().as_bytes(),
                    *registration.identity().as_bytes(),
                    *registration.association().identity().as_bytes(),
                    challenge,
                    registration.association().carriage_identity(),
                ],
            )
        })
    }

    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        budget.with_prepaid_scope(bytes.len().min(BYTES), ENTRY, WORK, SCRATCH, |_| {
            let bytes: &[u8; BYTES] = bytes.try_into().map_err(|_| Error::Length)?;
            if &bytes[..8] != MAGIC
                || bytes[8..10] != 1u16.to_le_bytes()
                || !matches!(bytes[10], 1 | 2)
                || bytes[11] != 0
                || bytes[12..16] != (BYTES as u32).to_le_bytes()
            {
                return Err(Error::Header);
            }
            if bytes[HEADER..FIELDS].chunks_exact(32).any(|v| v == [0; 32])
                || bytes[FIELDS..] != checksum(&bytes[..FIELDS])
            {
                return Err(Error::Identity);
            }
            Ok((Self { bytes: *bytes }, Storage(Self::STORAGE)))
        })
    }

    pub fn reply(&self, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        budget.with_prepaid_scope(Self::STORAGE, ENTRY, WORK, SCRATCH, |_| {
            if self.kind() != Kind::Request {
                return Err(Error::Kind);
            }
            Self::from_fields(Kind::Reply, self.fields())
        })
    }

    pub fn matches_reply(&self, reply: &Self, budget: &mut Budget<'_>) -> Result<bool> {
        budget.with_prepaid_scope(2 * Self::STORAGE, ENTRY, WORK, SCRATCH, |_| {
            Ok(self.kind() == Kind::Request
                && reply.kind() == Kind::Reply
                && self.bytes[HEADER..FIELDS] == reply.bytes[HEADER..FIELDS])
        })
    }

    /// Only compares inert launch identities; it cannot authenticate the peer.
    pub fn matches_launch(
        &self,
        policy: &Policy,
        manifest: &Manifest,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        budget.charge_work(ENTRY)?;
        let floor = Self::STORAGE
            .checked_add(policy.retained_storage())
            .and_then(|n| n.checked_add(manifest.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, WORK - ENTRY, SCRATCH, |_| {
            Ok(manifest.policy_identity() == policy.identity()
                && self.field(0) == *policy.identity().as_bytes()
                && self.field(1) == *manifest.identity().as_bytes())
        })
    }
    pub fn kind(&self) -> Kind {
        match self.bytes[10] {
            1 => Kind::Request,
            2 => Kind::Reply,
            _ => unreachable!(),
        }
    }
    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }
    pub fn registration_identity(&self) -> [u8; 32] {
        self.field(2)
    }
    pub fn association_identity(&self) -> [u8; 32] {
        self.field(3)
    }
    pub fn challenge(&self) -> [u8; 32] {
        self.field(4)
    }
    pub fn carriage_identity(&self) -> [u8; 32] {
        self.field(5)
    }
    /// Inert framing identity; only original authenticated channel custody can
    /// establish what issued this packet or whether it is still current.
    pub fn identity(&self) -> [u8; 32] {
        self.bytes[FIELDS..].try_into().unwrap()
    }
    pub const fn authenticates_currentness(&self) -> bool {
        false
    }
    fn field(&self, index: usize) -> [u8; 32] {
        self.bytes[HEADER + 32 * index..HEADER + 32 * (index + 1)]
            .try_into()
            .unwrap()
    }
    fn fields(&self) -> [[u8; 32]; 6] {
        std::array::from_fn(|i| self.field(i))
    }
    fn from_fields(kind: Kind, fields: [[u8; 32]; 6]) -> Result<(Self, Storage)> {
        if fields.iter().any(|v| *v == [0; 32]) {
            return Err(Error::Identity);
        }
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10] = match kind {
            Kind::Request => 1,
            Kind::Reply => 2,
        };
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        for (slot, value) in bytes[HEADER..FIELDS].chunks_exact_mut(32).zip(fields) {
            slot.copy_from_slice(&value);
        }
        let digest = checksum(&bytes[..FIELDS]);
        bytes[FIELDS..].copy_from_slice(&digest);
        Ok((Self { bytes }, Storage(Self::STORAGE)))
    }
}
fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    fn budget<T>(f: impl FnOnce(&mut Budget<'_>) -> T) -> T {
        let mut work = Work::new(1_000_000);
        let mut b = Budget::new(&mut work, 1_000_000);
        b.reserve_storage(100_000).unwrap();
        f(&mut b)
    }
    #[test]
    fn role_separated_roundtrip_never_authenticates_currentness() {
        budget(|b| {
            let (request, _) = Record::from_fields(Kind::Request, [[1; 32]; 6]).unwrap();
            let (reply, _) = request.reply(b).unwrap();
            assert!(request.matches_reply(&reply, b).unwrap());
            assert!(!request.matches_reply(&request, b).unwrap());
            assert!(reply.reply(b).is_err());
            assert_ne!(request.canonical_bytes(), reply.canonical_bytes());
            assert_eq!(Record::decode(reply.canonical_bytes(), b).unwrap().0, reply);
            assert!(!reply.authenticates_currentness());
        });
    }
    #[test]
    fn every_field_mutation_reserved_byte_and_legacy_magic_fail_closed() {
        let (request, _) = Record::from_fields(Kind::Request, [[1; 32]; 6]).unwrap();
        for index in 0..BYTES {
            let mut bytes = *request.canonical_bytes();
            bytes[index] ^= 1;
            assert!(
                budget(|b| Record::decode(&bytes, b)).is_err(),
                "byte {index}"
            );
        }
        assert!(budget(|b| Record::decode(&request.canonical_bytes()[..BYTES - 1], b)).is_err());
    }
    #[test]
    fn exact_challenge_and_both_native_associations_are_bound() {
        budget(|b| {
            let (request, _) = Record::from_fields(Kind::Request, [[1; 32]; 6]).unwrap();
            for index in 0..6 {
                let mut fields = request.fields();
                fields[index] = [2; 32];
                let (changed, _) = Record::from_fields(Kind::Reply, fields).unwrap();
                assert!(!request.matches_reply(&changed, b).unwrap());
            }
            assert!(Record::from_fields(Kind::Request, [[0; 32]; 6]).is_err());
        });
    }
    #[test]
    fn compiler_root_codec_cannot_admit_the_distinct_currentness_role() {
        use fe2o3_compiler_execution_protocol::{
            CompilerExecutionRootControlErrorV3 as CompilerError,
            CompilerExecutionRootControlRecordV3 as Compiler,
        };
        budget(|b| {
            let (request, _) = Record::from_fields(Kind::Request, [[1; 32]; 6]).unwrap();
            let gate = request.gate_bytes(b).unwrap();
            assert!(matches!(
                Compiler::decode(&gate, b),
                Err(CompilerError::Framing(_))
            ));
            let mut wrong = gate;
            wrong[8] = 2;
            assert!(matches!(Record::decode_gate(&wrong, b), Err(Error::Header)));
        });
    }
}
