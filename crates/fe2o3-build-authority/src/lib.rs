#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = r#"
Canonical formats shared by the protected fe2o3 build-authority boundary.

This crate defines inert policy data and content identities. Parsing a policy
does not establish process identity, isolation, freshness, or publication
authority. The only profile accepted by Policy V1 is the non-authoritative
standalone foundation profile, and it carries no publication rights.
"#]

mod compiler_closure;
mod policy_v1;
mod protocol_v1;

pub use compiler_closure::{
    COMPILER_CLOSURE_IDENTITY_DOMAIN_V1, CompilerClosureDigestFieldV1, CompilerClosureErrorV1,
    CompilerClosureV1, RUSTC_EXECUTABLE_RUNTIME_IDENTITY_DOMAIN_V1,
    derive_compiler_closure_identity_v1, derive_rustc_executable_runtime_identity_v1,
};
pub use policy_v1::{
    AuthorityProfileV1, POLICY_IDENTITY_DOMAIN_V1, POLICY_V1_ENCODED_LEN, POLICY_V1_FIELD_COUNT,
    POLICY_V1_HEADER_LEN, POLICY_V1_MAGIC, POLICY_V1_TARGET, POLICY_V1_VERSION,
    PipelineAllowlistV1, PipelineV1, PolicyDigestFieldV1, PolicyErrorV1, PolicyV1,
    PublicationRightsV1, decode_policy_v1, encode_policy_v1, policy_identity_sha256_v1,
};
pub use protocol_v1::{
    ACCEPT_V1_PAYLOAD_LEN, ADMISSION_IDENTITY_DOMAIN_V1, ARGUMENT_VECTOR_IDENTITY_DOMAIN_V1,
    ATTEST_V1_PAYLOAD_LEN, ATTESTATION_IDENTITY_DOMAIN_V1, AcceptV1, ArgvIdentityErrorV1, AttestV1,
    CHALLENGE_V1_PAYLOAD_LEN, ChallengeV1, DENY_V1_PAYLOAD_LEN, DenyReasonV1, DenyV1, FrameKindV1,
    GRANT_V1_PAYLOAD_LEN, GrantV1, IDENTITY_V1_LEN, NONCE_V1_LEN, PROTECTED_AUTHORITY_ARGV0_V1,
    PROTOCOL_V1_HEADER_LEN, PROTOCOL_V1_MAGIC, PROTOCOL_V1_MAX_ARGUMENT_BYTES,
    PROTOCOL_V1_MAX_ARGUMENTS, PROTOCOL_V1_MAX_TOTAL_ARGUMENT_BYTES, PROTOCOL_V1_VERSION,
    ProtocolErrorV1, ProtocolFrameV1, ProtocolIdentityFieldV1, ProtocolTargetV1,
    argv_identity_sha256_v1, decode_protocol_frame_v1, encode_protocol_frame_v1,
};
