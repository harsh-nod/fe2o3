#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = r#"
Canonical formats shared by the protected fe2o3 build-authority boundary.

This crate defines inert policy data, content identities, wire frames, and
transcript validators. Parsing or validating them does not establish process
identity, isolation, freshness, replay exclusion, execution authority, or
publication authority. Broker V4 is explicitly `AUTHORITY=none`; production
authority requires a broker-owned durable replay registry and session capability.
The only profile accepted by Policy V1 is the non-authoritative standalone
foundation profile, and it carries no publication rights.
"#]

mod broker_v3;
mod broker_v4;
mod cargo_environment_v1;
mod compiler_closure;
mod policy_v1;
mod protocol_v1;

pub use broker_v3::{
    BOOTSTRAP_V3_PAYLOAD_LEN, BROKER_V3_BINDING_IDENTITY_DOMAIN, BROKER_V3_BINDING_WIRE_LEN,
    BROKER_V3_HEADER_LEN, BROKER_V3_MAGIC, BROKER_V3_VERSION, BootstrapV3, BrokerDescriptorKindV3,
    BrokerDescriptorManifestV3, BrokerFrameKindV3, BrokerFrameV3, BrokerIdentityFieldV3,
    BrokerPhaseV3, BrokerProtocolErrorV3, BrokerStateErrorV3, BrokerStateV3, BrokerTargetV3,
    BrokerTranscriptFieldV3, CAPABILITIES_V3_PAYLOAD_LEN, CONSUME_V3_PAYLOAD_LEN, CapabilitiesV3,
    CapabilityBindingV3, ConsumeV3, DESCRIPTOR_MANIFEST_V3_WIRE_LEN, HELLO_V3_PAYLOAD_LEN, HelloV3,
    POST_EXEC_V3_PAYLOAD_LEN, PREPARE_V3_PAYLOAD_LEN, PROCESS_IDENTITY_V3_WIRE_LEN, PostExecV3,
    PrepareV3, ProcessIdentityV3, decode_broker_frame_v3, decode_capability_binding_v3,
    encode_broker_frame_v3,
};
pub use broker_v4::{
    BROKER_V4_AUTHORITY, BROKER_V4_BINDING_IDENTITY_DOMAIN, BROKER_V4_BINDING_OFFSET,
    BROKER_V4_BINDING_WIRE_LEN, BROKER_V4_HEADER_LEN, BROKER_V4_MAGIC, BROKER_V4_PROCESS_OFFSET,
    BROKER_V4_VERSION, BrokerAuthorityV4, BrokerFrameKindV4, BrokerFrameV4, BrokerIdentityFieldV4,
    BrokerProtocolErrorV4, BrokerReplayRegistryV4, BrokerSessionClaimV4, BrokerStateErrorV4,
    BrokerTargetV4, BrokerTranscriptFieldV4, BrokerTranscriptValidatorV4,
    BrokerValidationRejectedV4, CapabilityBindingV4, CompletedBrokerTranscriptV4,
    GrantedHostLinkTranscriptV4, HOST_LINK_CLOSURE_OFFSET_V4,
    HOST_LINK_COMMIT_DURABLE_PLAN_OFFSET_V4, HOST_LINK_COMMIT_OUTPUT_LENGTH_OFFSET_V4,
    HOST_LINK_COMMIT_OUTPUT_MODE_OFFSET_V4, HOST_LINK_COMMIT_OUTPUT_SHA256_OFFSET_V4,
    HOST_LINK_COMMIT_RESERVED_OFFSET_V4, HOST_LINK_COMMIT_V4_PAYLOAD_LEN,
    HOST_LINK_GRANT_OFFSET_V4, HOST_LINK_GRANT_V4_PAYLOAD_LEN, HOST_LINK_OUTPUT_MODE_V4,
    HOST_LINK_PLAN_OFFSET_V4, HOST_LINK_PREPARE_V4_PAYLOAD_LEN, HOST_LINK_REQUEST_OFFSET_V4,
    HostLinkCommitV4, HostLinkGrantV4, HostLinkPrepareV4, PROCESS_IDENTITY_V4_WIRE_LEN,
    PreparedHostLinkTranscriptV4, ProcessIdentityV4, decode_broker_frame_v4,
    decode_capability_binding_v4, encode_broker_frame_v4,
};
pub use cargo_environment_v1::{
    AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1, AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1,
    AUTHORITY_CARGO_ENVIRONMENT_IDENTITY_DOMAIN_V1, AUTHORITY_CARGO_ENVIRONMENT_MAGIC_V1,
    AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1, AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1,
    AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1, AUTHORITY_CARGO_ENVIRONMENT_TARGET_V1,
    AUTHORITY_CARGO_ENVIRONMENT_VERSION_V1, AUTHORITY_CARGO_MODE_ARGV_V1,
    AuthorityCargoEnvironmentErrorV1, AuthorityCargoEnvironmentPathErrorV1,
    AuthorityCargoEnvironmentV1, AuthorityCargoEnvironmentVariableV1,
    ForbiddenCargoEnvironmentChannelV1, authority_cargo_environment_identity_sha256_v1,
    decode_authority_cargo_environment_v1, encode_authority_cargo_environment_v1,
};
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
    ProtocolErrorV1, ProtocolFrameV1, ProtocolIdentityFieldV1, ProtocolPhaseV1,
    ProtocolStateErrorV1, ProtocolStateV1, ProtocolTargetV1, TranscriptFieldV1,
    argv_identity_sha256_v1, decode_protocol_frame_v1, encode_protocol_frame_v1,
};
