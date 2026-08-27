#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = r#"
Canonical formats shared by the protected fe2o3 build-authority boundary.

This crate defines inert environment data, content identities, wire frames,
and transcript validators. Parsing or validating them does not establish process
identity, isolation, freshness, replay exclusion, execution authority, or
publication authority. Broker V4 is explicitly `AUTHORITY=none`; production
authority requires a broker-owned durable replay registry and session capability.
"#]

mod broker_v4;
mod cargo_environment_v1;
mod compiler_closure;

/// Canonical `argv[0]` for the protected authority release executable.
pub const PROTECTED_AUTHORITY_ARGV0: &[u8] = b"/usr/libexec/fe2o3/cargo-fe2o3";
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
    CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1, COMPILER_CLOSURE_IDENTITY_DOMAIN_V2,
    CompilerClosureDigestFieldV2, CompilerClosureErrorV2, CompilerClosureV2,
    derive_compiler_closure_identity_v1, derive_compiler_closure_identity_v2,
    derive_rustc_executable_runtime_identity_v1,
};
