//! Shared fixed framing; each public codec admits only its own schema and header.
use crate::{
    COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1 as ENFORCEMENT,
    CompilerApprovalPolicyErrorV1 as Error, CompilerClosureV2,
};
use sha2::{Digest, Sha256};

pub(crate) const LENGTH: usize = 352;
const HEADER: usize = 32;
const CLOSURE_END: usize = HEADER + 7 * 32;
const PROFILE_END: usize = CLOSURE_END + 32;
const PAYLOAD_END: usize = PROFILE_END + 32;

#[derive(Clone, Copy)]
pub(crate) struct Schema {
    pub magic: [u8; 8],
    pub version: u16,
    pub domain: &'static [u8],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub bytes: [u8; LENGTH],
    pub closure: CompilerClosureV2,
}

pub(crate) fn encode<E>(
    closure: CompilerClosureV2,
    profile: [u8; 32],
    runtime: [u8; 32],
    enforcement: u16,
    extension: [u8; 12],
    schema: Schema,
) -> Result<Record, Error<E>> {
    validate_requirements(&profile, &runtime, enforcement)?;
    let mut bytes = [0; LENGTH];
    bytes[..8].copy_from_slice(&schema.magic);
    bytes[8..10].copy_from_slice(&schema.version.to_le_bytes());
    bytes[10..12].copy_from_slice(&(HEADER as u16).to_le_bytes());
    bytes[12..16].copy_from_slice(&(LENGTH as u32).to_le_bytes());
    bytes[16..18].copy_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes[18..20].copy_from_slice(&enforcement.to_le_bytes());
    bytes[20..HEADER].copy_from_slice(&extension);
    let digests = [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
        closure.identity_sha256(),
    ];
    for (slot, digest) in bytes[HEADER..CLOSURE_END].chunks_exact_mut(32).zip(digests) {
        slot.copy_from_slice(&digest);
    }
    bytes[CLOSURE_END..PROFILE_END].copy_from_slice(&profile);
    bytes[PROFILE_END..PAYLOAD_END].copy_from_slice(&runtime);
    let identity = hash(&bytes[..PAYLOAD_END], schema);
    bytes[PAYLOAD_END..].copy_from_slice(&identity);
    Ok(Record { bytes, closure })
}

pub(crate) fn require_length<E>(bytes: &[u8]) -> Result<(), Error<E>> {
    if bytes.len() != LENGTH {
        return Err(Error::Length);
    }
    Ok(())
}

// Called only after exact-length validation and the public codec's work charge.
pub(crate) fn require_header<E>(bytes: &[u8], schema: Schema) -> Result<(), Error<E>> {
    if bytes[..8] != schema.magic
        || bytes[8..10] != schema.version.to_le_bytes()
        || bytes[10..12] != (HEADER as u16).to_le_bytes()
    {
        return Err(Error::Header);
    }
    if bytes[12..16] != (LENGTH as u32).to_le_bytes() {
        return Err(Error::Length);
    }
    Ok(())
}

// The family-specific reserved/credential header checks precede payload decoding.
pub(crate) fn decode_payload<E>(bytes: &[u8], schema: Schema) -> Result<Record, Error<E>> {
    let version = u16::from_le_bytes(bytes[18..20].try_into().expect("fixed header"));
    validate_requirements(
        &bytes[CLOSURE_END..PROFILE_END],
        &bytes[PROFILE_END..PAYLOAD_END],
        version,
    )?;
    if bytes[PAYLOAD_END..] != hash(&bytes[..PAYLOAD_END], schema) {
        return Err(Error::Identity);
    }
    let digests: [[u8; 32]; 7] = bytes[HEADER..CLOSURE_END]
        .as_chunks::<32>()
        .0
        .try_into()
        .expect("fixed closure");
    let closure = CompilerClosureV2::from_pins_and_identity(
        digests[0],
        digests[1],
        digests[2],
        digests[3],
        digests[4],
        digests[5],
        u16::from_le_bytes(bytes[16..18].try_into().expect("fixed header")),
        digests[6],
    )
    .map_err(Error::CompilerClosure)?;
    Ok(Record {
        bytes: bytes.try_into().expect("checked fixed record"),
        closure,
    })
}

fn validate_requirements<E>(profile: &[u8], runtime: &[u8], version: u16) -> Result<(), Error<E>> {
    if version != ENFORCEMENT {
        return Err(Error::UnsupportedRuntimeEnforcementVersion { version });
    }
    if profile == [0; 32] {
        return Err(Error::ZeroClientProfileIdentity);
    }
    if runtime == [0; 32] {
        return Err(Error::ZeroRuntimeManifestIdentity);
    }
    Ok(())
}
fn hash(bytes: &[u8], schema: Schema) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(schema.domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

macro_rules! accessors {
    () => {
        /// Complete canonical inert record, including its terminal digest.
        pub const fn canonical_bytes(&self) -> &[u8; codec::LENGTH] {
            &self.bytes
        }
        /// Validated six-pin compiler closure, not an approved executable owner.
        pub const fn compiler_closure(&self) -> CompilerClosureV2 {
            self.closure
        }
        /// Exact V3 client-profile identity; its generation is already identity-bound.
        pub fn client_profile_identity(&self) -> &[u8; 32] {
            self.bytes[256..288]
                .try_into()
                .expect("fixed profile identity")
        }
        /// Claimed runtime manifest identity, not authenticated runtime custody.
        pub fn runtime_manifest_identity(&self) -> &[u8; 32] {
            self.bytes[288..320]
                .try_into()
                .expect("fixed manifest identity")
        }
        /// Requested closed rule set, not evidence of runtime support or enforcement.
        pub const fn required_runtime_enforcement_version(&self) -> u16 {
            crate::COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1
        }
        /// Domain-separated exact record identity, not a signature or approval.
        pub fn identity(&self) -> &[u8; 32] {
            self.bytes[320..].try_into().expect("fixed record identity")
        }
        /// Public policy bytes grant no authority.
        pub const fn grants_authority(&self) -> bool {
            false
        }
    };
}
pub(crate) use accessors;
