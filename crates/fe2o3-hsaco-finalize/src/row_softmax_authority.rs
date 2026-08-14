use std::{error::Error, fmt};

use fe2o3_artifact_transaction::BuildAttempt;
use sha2::{Digest, Sha256};

pub const ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT: usize = 8;
pub const MAX_ROW_SOFTMAX_V1_AUTHORITY_TRANSCRIPT_BYTES: usize = 4096;
pub const MAX_ROW_SOFTMAX_V1_REVIEWED_SOURCE_BYTES: usize = 1024 * 1024;

const AUTHORITY_DOMAIN: &[u8] = b"fe2o3.row-softmax.collected-authority.v1";
const METADATA_DOMAIN: &[u8] = b"fe2o3.row-softmax.cargo-metadata-observation.v1";
const METADATA_TRANSCRIPT_DOMAIN: &[u8] = b"FE2O3/CARGO-METADATA-BUILD-OBSERVATION/V2\0";
const PROVIDER_DOMAIN: &[u8] = b"FE2O3/ROW-SOFTMAX-PROVIDER-AUTHORITY/V1\0";
const PROVIDER_SOURCE_DOMAIN: &[u8] = b"FE2O3/ROW-SOFTMAX-PROVIDER-SOURCE-IDENTITY/V1\0";
const ABI_DOMAIN: &[u8] = b"fe2o3.row-softmax.abi-binding.v1";
const LAUNCH_DOMAIN: &[u8] = b"fe2o3.row-softmax.launch-binding.v1";
const CORRESPONDENCE_DOMAIN: &[u8] = b"fe2o3.row-softmax.reviewed-correspondence.v1";
const EXPONENTIAL_DOMAIN: &[u8] = b"fe2o3.row-softmax.exponential-boundary.v1";
const REVIEWED_METADATA: &[u8] = b"fe2o3-row-softmax-v1-reviewed";
const ROOT_PREFIX: &[u8] = b"__fe2o3_host_kernel_v1_";
const PROVIDER_CRATE: &[u8] = b"fe2o3_device";

const PORTABLE_MIR_COMMITMENT: [u8; 32] = [
    0xcb, 0x10, 0xb6, 0xfa, 0xc6, 0x47, 0x54, 0x35, 0xe4, 0x5a, 0x6f, 0x91, 0x66, 0x73, 0x9c, 0x9e,
    0x26, 0xba, 0xe1, 0x70, 0x31, 0x10, 0x57, 0x91, 0xab, 0xf3, 0xf4, 0x40, 0xb0, 0x04, 0xd4, 0xdd,
];
const COMPILER_SEMANTICS_COMMITMENT: [u8; 32] = [
    0x31, 0x32, 0xd8, 0x6d, 0x22, 0x9a, 0x39, 0x77, 0xed, 0x9c, 0x52, 0x83, 0xc2, 0x41, 0xc4, 0xf6,
    0xc8, 0x5a, 0xff, 0x23, 0xc1, 0xd1, 0x77, 0xfb, 0x0d, 0x23, 0xc0, 0x74, 0x32, 0x79, 0xf0, 0xa4,
];
const CANONICAL_MODULE_COMMITMENT: [u8; 32] = [
    0x1e, 0x1b, 0x14, 0xc6, 0x84, 0x2f, 0xfd, 0x09, 0x10, 0x3e, 0xb5, 0x5e, 0xb3, 0x9b, 0x1b, 0xca,
    0xe9, 0xc0, 0xda, 0x81, 0x59, 0x7f, 0xed, 0x61, 0x86, 0x76, 0x75, 0x62, 0x33, 0x72, 0x30, 0xe6,
];
const FN_ABI_COMMITMENT: [u8; 32] = [
    0x1f, 0x97, 0x82, 0x38, 0x8c, 0x98, 0x28, 0x56, 0x4b, 0xd6, 0x34, 0xce, 0x21, 0x8a, 0x6f, 0xf1,
    0x18, 0x65, 0xdb, 0xba, 0x8a, 0x52, 0x83, 0xf5, 0xa0, 0x26, 0x7b, 0x2b, 0x7a, 0x97, 0xa4, 0xc6,
];
const ABI_BINDING: &[u8] = b"ptr64;size=32;align=8;input@0:16:8:slice-f32:shared-readonly;output@16:16:8:slice-f32:exclusive-readwrite;lengths=exactly-64-by-host-precondition";
const LAUNCH_BINDING: &[u8] =
    b"rank=1;block=exact(64,1,1);grid=exact(1,1,1);static-shared=0;dynamic-shared=0;wave=64;cov=6";
const CORRESPONDENCE: &[u8] = b"exact reviewed Rust portable-MIR identity selects the private fe2o3::row_softmax_v1 canonical module;one lane performs three ordered 64-element loops;bounded reviewed correspondence only;not a compiler-refinement proof";
const EXPONENTIAL_BOUNDARY: &[u8] = b"canonical Kernel IR names its abstract f32 exp operation;no authenticated implementation, approximation/error contract, OCML bitcode, link request, LLVM lowering, or real-number softmax equivalence";
const FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

/// Independently reviewed provider item and its exact source-file assignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1ProviderItemV1 {
    canonical_path: &'static str,
    source_path: &'static str,
}

impl RowSoftmaxV1ProviderItemV1 {
    pub const fn canonical_path(self) -> &'static str {
        self.canonical_path
    }

    pub const fn source_path(self) -> &'static str {
        self.source_path
    }
}

pub const ROW_SOFTMAX_V1_PROVIDER_ITEMS: [RowSoftmaxV1ProviderItemV1;
    ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT] = [
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::DisjointSlice",
        source_path: "lib.rs",
    },
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::ThreadIndex",
        source_path: "thread.rs",
    },
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::thread::index_1d",
        source_path: "thread.rs",
    },
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::ThreadIndex::get",
        source_path: "thread.rs",
    },
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::DisjointSlice::<T>::get_mut_at",
        source_path: "lib.rs",
    },
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::DeviceMath",
        source_path: "math.rs",
    },
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::DeviceMath::from_compiler",
        source_path: "math.rs",
    },
    RowSoftmaxV1ProviderItemV1 {
        canonical_path: "fe2o3_device::DeviceMath::exp_f32",
        source_path: "math.rs",
    },
];

/// Exact rustc/provider identities provisioned independently from a compiler handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1ProviderManifestV1 {
    stable_crate_id: u64,
    crate_hash: [u8; 16],
    definition_identities: [[u8; 16]; ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT],
    source_identities: [[u8; 32]; ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT],
}

impl RowSoftmaxV1ProviderManifestV1 {
    pub fn new(
        stable_crate_id: u64,
        crate_hash: [u8; 16],
        definition_identities: [[u8; 16]; ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT],
        source_identities: [[u8; 32]; ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT],
    ) -> Result<Self, RowSoftmaxV1AuthorityPolicyErrorV1> {
        if stable_crate_id == 0 || crate_hash == [0; 16] {
            return Err(invalid_policy("provider crate identity"));
        }
        if definition_identities
            .iter()
            .any(|identity| identity == &[0; 16])
            || source_identities
                .iter()
                .any(|identity| identity == &[0; 32])
        {
            return Err(invalid_policy("provider item identity"));
        }
        for (index, identity) in definition_identities.iter().enumerate() {
            if definition_identities[..index].contains(identity) {
                return Err(invalid_policy("duplicate provider definition identity"));
            }
        }
        if source_identities[0] != source_identities[4]
            || source_identities[1] != source_identities[2]
            || source_identities[1] != source_identities[3]
            || source_identities[5] != source_identities[6]
            || source_identities[5] != source_identities[7]
            || source_identities[0] == source_identities[1]
            || source_identities[0] == source_identities[5]
            || source_identities[1] == source_identities[5]
        {
            return Err(invalid_policy("provider item-to-source mapping"));
        }
        Ok(Self {
            stable_crate_id,
            crate_hash,
            definition_identities,
            source_identities,
        })
    }
}

/// Independent build and provider policy required to interpret a row authority transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1AuthorityPolicyV1 {
    provider: RowSoftmaxV1ProviderManifestV1,
    attempt: BuildAttempt,
    broker_executable_sha256: [u8; 32],
}

impl RowSoftmaxV1AuthorityPolicyV1 {
    pub fn new(
        provider: RowSoftmaxV1ProviderManifestV1,
        attempt: BuildAttempt,
        broker_executable_sha256: [u8; 32],
    ) -> Result<Self, RowSoftmaxV1AuthorityPolicyErrorV1> {
        if attempt.session().as_bytes() == &[0; 16]
            || attempt.invocation().as_bytes() == &[0; 32]
            || broker_executable_sha256 == [0; 32]
        {
            return Err(invalid_policy("managed build authority"));
        }
        Ok(Self {
            provider,
            attempt,
            broker_executable_sha256,
        })
    }
}

/// Failure to construct or match an independently provisioned row authority policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RowSoftmaxV1AuthorityPolicyErrorV1 {
    InvalidPolicy(&'static str),
    MalformedTranscript(&'static str),
    TranscriptMismatch(&'static str),
}

impl fmt::Display for RowSoftmaxV1AuthorityPolicyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(field) => {
                write!(formatter, "invalid row authority policy: {field}")
            }
            Self::MalformedTranscript(field) => {
                write!(formatter, "malformed row authority transcript: {field}")
            }
            Self::TranscriptMismatch(field) => {
                write!(formatter, "row authority transcript differs: {field}")
            }
        }
    }
}

impl Error for RowSoftmaxV1AuthorityPolicyErrorV1 {}

/// Derives the identity rustc records for one reviewed provider source file.
pub fn derive_row_softmax_v1_provider_source_identity_v1(
    relative_path: &str,
    source: &[u8],
) -> Result<[u8; 32], RowSoftmaxV1AuthorityPolicyErrorV1> {
    if !matches!(relative_path, "lib.rs" | "thread.rs" | "math.rs")
        || source.is_empty()
        || source.len() > MAX_ROW_SOFTMAX_V1_REVIEWED_SOURCE_BYTES
    {
        return Err(invalid_policy("reviewed provider source"));
    }
    let mut digest = Sha256::new();
    digest.update(PROVIDER_SOURCE_DOMAIN);
    digest.update((relative_path.len() as u64).to_le_bytes());
    digest.update(relative_path.as_bytes());
    digest.update((source.len() as u64).to_le_bytes());
    digest.update(source);
    Ok(digest.finalize().into())
}

pub(crate) fn validate_row_softmax_v1_authority_transcript(
    transcript: &[u8],
    descriptor_source_commitment: [u8; 32],
    exponential_boundary_commitment: [u8; 32],
    policy: RowSoftmaxV1AuthorityPolicyV1,
) -> Result<(), RowSoftmaxV1AuthorityPolicyErrorV1> {
    let mut decoder = TranscriptDecoder::new(transcript)?;
    for (field, expected) in [
        ("authority domain", AUTHORITY_DOMAIN),
        (
            "portable MIR commitment",
            PORTABLE_MIR_COMMITMENT.as_slice(),
        ),
        (
            "compiler semantics commitment",
            COMPILER_SEMANTICS_COMMITMENT.as_slice(),
        ),
        (
            "canonical module commitment",
            CANONICAL_MODULE_COMMITMENT.as_slice(),
        ),
        (
            "descriptor source commitment",
            descriptor_source_commitment.as_slice(),
        ),
    ] {
        decoder.expect(field, expected)?;
    }
    validate_root_identity(decoder.field("root instance identity")?)?;
    for (field, expected) in [
        ("kernel export", b"row_softmax_v1".as_slice()),
        ("target", b"gfx942:xnack-".as_slice()),
        ("code-object version", 6_u16.to_le_bytes().as_slice()),
        ("explicit kernarg bytes", 32_u64.to_le_bytes().as_slice()),
        ("complete kernarg bytes", 288_u64.to_le_bytes().as_slice()),
        ("row elements", 64_u32.to_le_bytes().as_slice()),
        (
            "ABI binding commitment",
            domain_commitment(ABI_DOMAIN, ABI_BINDING).as_slice(),
        ),
        (
            "rustc function ABI commitment",
            FN_ABI_COMMITMENT.as_slice(),
        ),
        (
            "launch binding commitment",
            domain_commitment(LAUNCH_DOMAIN, LAUNCH_BINDING).as_slice(),
        ),
        (
            "correspondence commitment",
            domain_commitment(CORRESPONDENCE_DOMAIN, CORRESPONDENCE).as_slice(),
        ),
        (
            "exponential boundary commitment",
            exponential_boundary_commitment.as_slice(),
        ),
        (
            "frontend contract commitment",
            Sha256::digest(FRONTEND_CONTRACT).as_slice(),
        ),
    ] {
        decoder.expect(field, expected)?;
    }
    if exponential_boundary_commitment
        != domain_commitment(EXPONENTIAL_DOMAIN, EXPONENTIAL_BOUNDARY)
    {
        return Err(transcript_mismatch("reviewed exponential boundary"));
    }

    let generated_metadata = decoder.field("Cargo generated metadata")?;
    if generated_metadata.len() != 16 || !generated_metadata.iter().copied().all(is_lower_hex) {
        return Err(malformed_transcript("Cargo generated metadata"));
    }
    decoder.expect("reviewed Cargo metadata", REVIEWED_METADATA)?;
    let metadata_commitment = cargo_metadata_commitment(generated_metadata);
    decoder.expect("Cargo metadata commitment", &metadata_commitment)?;

    decoder.expect("provider crate name", PROVIDER_CRATE)?;
    decoder.expect(
        "provider stable crate ID",
        &policy.provider.stable_crate_id.to_le_bytes(),
    )?;
    decoder.expect("provider crate hash", &policy.provider.crate_hash)?;
    let metadata_transcript = cargo_metadata_transcript(generated_metadata);
    decoder.expect("provider Cargo metadata observation", &metadata_transcript)?;
    decoder.expect(
        "provider definition source",
        &policy.provider.source_identities[0],
    )?;
    for (item, identity) in ROW_SOFTMAX_V1_PROVIDER_ITEMS
        .iter()
        .zip(policy.provider.definition_identities.iter())
    {
        decoder.expect(item.canonical_path(), identity)?;
    }
    for (item, identity) in ROW_SOFTMAX_V1_PROVIDER_ITEMS
        .iter()
        .zip(policy.provider.source_identities.iter())
    {
        decoder.expect(item.source_path(), identity)?;
    }
    let provider_commitment = provider_commitment(&policy.provider, metadata_transcript);
    decoder.expect("provider commitment", &provider_commitment)?;

    decoder.expect(
        "build generation",
        &policy.attempt.generation().to_le_bytes(),
    )?;
    decoder.expect("build session", policy.attempt.session().as_bytes())?;
    decoder.expect("build invocation", policy.attempt.invocation().as_bytes())?;
    decoder.expect("managed Cargo metadata transcript", &metadata_transcript)?;
    decoder.expect("broker executable", &policy.broker_executable_sha256)?;
    if !decoder.finished() {
        return Err(malformed_transcript("trailing fields"));
    }
    Ok(())
}

fn validate_root_identity(value: &[u8]) -> Result<(), RowSoftmaxV1AuthorityPolicyErrorV1> {
    let Some(suffix) = value.strip_prefix(ROOT_PREFIX) else {
        return Err(transcript_mismatch("root instance identity"));
    };
    if suffix.len() != 64 || !suffix.iter().copied().all(is_lower_hex) {
        return Err(malformed_transcript("root instance identity"));
    }
    Ok(())
}

fn provider_commitment(
    provider: &RowSoftmaxV1ProviderManifestV1,
    metadata_transcript: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, PROVIDER_DOMAIN);
    hash_field(&mut digest, PROVIDER_CRATE);
    hash_field(&mut digest, &provider.stable_crate_id.to_le_bytes());
    hash_field(&mut digest, &provider.crate_hash);
    hash_field(&mut digest, &metadata_transcript);
    for ((item, definition), source) in ROW_SOFTMAX_V1_PROVIDER_ITEMS
        .iter()
        .zip(provider.definition_identities.iter())
        .zip(provider.source_identities.iter())
    {
        hash_field(&mut digest, item.canonical_path().as_bytes());
        hash_field(&mut digest, definition);
        hash_field(&mut digest, source);
    }
    digest.finalize().into()
}

fn cargo_metadata_commitment(generated: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, METADATA_DOMAIN);
    hash_field(&mut digest, generated);
    hash_field(&mut digest, REVIEWED_METADATA);
    digest.finalize().into()
}

fn cargo_metadata_transcript(generated: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(METADATA_TRANSCRIPT_DOMAIN);
    digest.update(2_u64.to_le_bytes());
    for token in [generated, REVIEWED_METADATA] {
        digest.update((token.len() as u64).to_le_bytes());
        digest.update(token);
    }
    digest.finalize().into()
}

fn domain_commitment(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, domain);
    hash_field(&mut digest, value);
    digest.finalize().into()
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

const fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (byte >= b'a' && byte <= b'f')
}

fn invalid_policy(field: &'static str) -> RowSoftmaxV1AuthorityPolicyErrorV1 {
    RowSoftmaxV1AuthorityPolicyErrorV1::InvalidPolicy(field)
}

fn malformed_transcript(field: &'static str) -> RowSoftmaxV1AuthorityPolicyErrorV1 {
    RowSoftmaxV1AuthorityPolicyErrorV1::MalformedTranscript(field)
}

fn transcript_mismatch(field: &'static str) -> RowSoftmaxV1AuthorityPolicyErrorV1 {
    RowSoftmaxV1AuthorityPolicyErrorV1::TranscriptMismatch(field)
}

struct TranscriptDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> TranscriptDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, RowSoftmaxV1AuthorityPolicyErrorV1> {
        if bytes.is_empty() || bytes.len() > MAX_ROW_SOFTMAX_V1_AUTHORITY_TRANSCRIPT_BYTES {
            return Err(malformed_transcript("length"));
        }
        Ok(Self { bytes, offset: 0 })
    }

    fn field(
        &mut self,
        name: &'static str,
    ) -> Result<&'a [u8], RowSoftmaxV1AuthorityPolicyErrorV1> {
        let length_end = self
            .offset
            .checked_add(8)
            .ok_or_else(|| malformed_transcript(name))?;
        let encoded: [u8; 8] = self
            .bytes
            .get(self.offset..length_end)
            .ok_or_else(|| malformed_transcript(name))?
            .try_into()
            .expect("field length has exact width");
        let length =
            usize::try_from(u64::from_le_bytes(encoded)).map_err(|_| malformed_transcript(name))?;
        let end = length_end
            .checked_add(length)
            .ok_or_else(|| malformed_transcript(name))?;
        let field = self
            .bytes
            .get(length_end..end)
            .ok_or_else(|| malformed_transcript(name))?;
        self.offset = end;
        Ok(field)
    }

    fn expect(
        &mut self,
        name: &'static str,
        expected: &[u8],
    ) -> Result<(), RowSoftmaxV1AuthorityPolicyErrorV1> {
        if self.field(name)? != expected {
            return Err(transcript_mismatch(name));
        }
        Ok(())
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATED: &[u8] = b"0123456789abcdef";
    const DESCRIPTOR: [u8; 32] = [0x51; 32];
    const BROKER: [u8; 32] = [0x52; 32];

    fn attempt() -> BuildAttempt {
        BuildAttempt::from_env_value(
            "7:01010101010101010101010101010101:0202020202020202020202020202020202020202020202020202020202020202",
        )
        .unwrap()
    }

    fn source_identities() -> [[u8; 32]; ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT] {
        let lib =
            derive_row_softmax_v1_provider_source_identity_v1("lib.rs", b"lib source").unwrap();
        let thread =
            derive_row_softmax_v1_provider_source_identity_v1("thread.rs", b"thread source")
                .unwrap();
        let math =
            derive_row_softmax_v1_provider_source_identity_v1("math.rs", b"math source").unwrap();
        [lib, thread, thread, thread, lib, math, math, math]
    }

    fn manifest() -> RowSoftmaxV1ProviderManifestV1 {
        let definitions = std::array::from_fn(|index| [u8::try_from(index + 1).unwrap(); 16]);
        RowSoftmaxV1ProviderManifestV1::new(9, [0x33; 16], definitions, source_identities())
            .unwrap()
    }

    fn policy() -> RowSoftmaxV1AuthorityPolicyV1 {
        RowSoftmaxV1AuthorityPolicyV1::new(manifest(), attempt(), BROKER).unwrap()
    }

    fn canonical_fields() -> Vec<Vec<u8>> {
        let provider = manifest();
        let metadata_transcript = cargo_metadata_transcript(GENERATED);
        let mut fields = vec![
            AUTHORITY_DOMAIN.to_vec(),
            PORTABLE_MIR_COMMITMENT.to_vec(),
            COMPILER_SEMANTICS_COMMITMENT.to_vec(),
            CANONICAL_MODULE_COMMITMENT.to_vec(),
            DESCRIPTOR.to_vec(),
            [ROOT_PREFIX, b"0".repeat(64).as_slice()].concat(),
            b"row_softmax_v1".to_vec(),
            b"gfx942:xnack-".to_vec(),
            6_u16.to_le_bytes().to_vec(),
            32_u64.to_le_bytes().to_vec(),
            288_u64.to_le_bytes().to_vec(),
            64_u32.to_le_bytes().to_vec(),
            domain_commitment(ABI_DOMAIN, ABI_BINDING).to_vec(),
            FN_ABI_COMMITMENT.to_vec(),
            domain_commitment(LAUNCH_DOMAIN, LAUNCH_BINDING).to_vec(),
            domain_commitment(CORRESPONDENCE_DOMAIN, CORRESPONDENCE).to_vec(),
            domain_commitment(EXPONENTIAL_DOMAIN, EXPONENTIAL_BOUNDARY).to_vec(),
            Sha256::digest(FRONTEND_CONTRACT).to_vec(),
            GENERATED.to_vec(),
            REVIEWED_METADATA.to_vec(),
            cargo_metadata_commitment(GENERATED).to_vec(),
            PROVIDER_CRATE.to_vec(),
            provider.stable_crate_id.to_le_bytes().to_vec(),
            provider.crate_hash.to_vec(),
            metadata_transcript.to_vec(),
            provider.source_identities[0].to_vec(),
        ];
        fields.extend(
            provider
                .definition_identities
                .iter()
                .map(|identity| identity.to_vec()),
        );
        fields.extend(
            provider
                .source_identities
                .iter()
                .map(|identity| identity.to_vec()),
        );
        fields.extend([
            provider_commitment(&provider, metadata_transcript).to_vec(),
            attempt().generation().to_le_bytes().to_vec(),
            attempt().session().as_bytes().to_vec(),
            attempt().invocation().as_bytes().to_vec(),
            metadata_transcript.to_vec(),
            BROKER.to_vec(),
        ]);
        fields
    }

    fn encode(fields: &[Vec<u8>]) -> Vec<u8> {
        let mut transcript = Vec::new();
        for field in fields {
            transcript.extend_from_slice(&(field.len() as u64).to_le_bytes());
            transcript.extend_from_slice(field);
        }
        transcript
    }

    fn validate(transcript: &[u8]) -> Result<(), RowSoftmaxV1AuthorityPolicyErrorV1> {
        validate_row_softmax_v1_authority_transcript(
            transcript,
            DESCRIPTOR,
            domain_commitment(EXPONENTIAL_DOMAIN, EXPONENTIAL_BOUNDARY),
            policy(),
        )
    }

    #[test]
    fn exact_independent_policy_admits_the_canonical_transcript() {
        validate(&encode(&canonical_fields())).unwrap();
    }

    #[test]
    fn provider_and_managed_build_substitutions_fail_closed() {
        let fields = canonical_fields();
        for index in 22..fields.len() {
            let mut changed = fields.clone();
            changed[index][0] ^= 1;
            assert!(validate(&encode(&changed)).is_err(), "field {index}");
        }
    }

    #[test]
    fn framing_truncation_trailing_and_semantic_substitution_fail_closed() {
        let transcript = encode(&canonical_fields());
        for end in 0..transcript.len() {
            assert!(validate(&transcript[..end]).is_err(), "prefix {end}");
        }
        let mut trailing = transcript.clone();
        trailing.extend_from_slice(&0_u64.to_le_bytes());
        assert!(validate(&trailing).is_err());

        let mut fields = canonical_fields();
        for index in 0..22 {
            let mut changed = fields.clone();
            changed[index][0] ^= 1;
            assert!(validate(&encode(&changed)).is_err(), "field {index}");
        }
        fields[0] = vec![0; MAX_ROW_SOFTMAX_V1_AUTHORITY_TRANSCRIPT_BYTES];
        assert!(validate(&encode(&fields)).is_err());
    }

    #[test]
    fn manifest_enforces_unique_definitions_and_exact_item_source_mapping() {
        let valid = manifest();
        let mut duplicate_definitions = valid.definition_identities;
        duplicate_definitions[1] = duplicate_definitions[0];
        assert!(
            RowSoftmaxV1ProviderManifestV1::new(
                valid.stable_crate_id,
                valid.crate_hash,
                duplicate_definitions,
                valid.source_identities,
            )
            .is_err()
        );

        let mut swapped_sources = valid.source_identities;
        swapped_sources.swap(0, 1);
        assert!(
            RowSoftmaxV1ProviderManifestV1::new(
                valid.stable_crate_id,
                valid.crate_hash,
                valid.definition_identities,
                swapped_sources,
            )
            .is_err()
        );
    }

    #[test]
    fn source_identity_binds_exact_relative_path_and_bytes() {
        let baseline =
            derive_row_softmax_v1_provider_source_identity_v1("lib.rs", b"source").unwrap();
        assert_ne!(
            baseline,
            derive_row_softmax_v1_provider_source_identity_v1("thread.rs", b"source").unwrap()
        );
        assert_ne!(
            baseline,
            derive_row_softmax_v1_provider_source_identity_v1("lib.rs", b"changed").unwrap()
        );
        assert!(derive_row_softmax_v1_provider_source_identity_v1("other.rs", b"source").is_err());
    }
}
