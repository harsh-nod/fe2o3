//! Descriptor-backed, fail-closed host-link closure primitives.
//!
//! The closure owns authenticated static-link execution. It retains and validates
//! compiler-produced files, resolves a complete input closure, executes the exact sealed static
//! LLD descriptor with a canonical inherited descriptor table, and admits output only with the
//! exact atomically-created pidfd-backed worker witness. A seccomp profile denies worker process
//! creation, and execution has an internal fixed wall deadline. The crate does not publish
//! artifacts, contact a broker, grant tool approval, or grant runtime authority. Launch requires
//! an explicit [`ApprovedStaticHostLldV1`] minted by an external trusted authority.
//!
//! V1 admits a finite x86-64 ELF subset. Relocatable inputs use explicit section flag/linkage
//! rules, pinned LLVM relocation codes, cross-validated GROUP membership, and inert excluded
//! Rust/LLVM metadata only. Regular archives use GNU 32-bit indexes and GNU long names with
//! canonical producer padding; thin, BSD-name, GNU SYM64, compressed, CREL, active bitcode, and
//! dependent-library forms fail closed. Static outputs use the same bounded structural checks and
//! admit spec-valid merge strings such as LLD's `.comment` while rejecting dynamic execution
//! state and unknown section semantics.
//!
//! Linux is the only supported platform. Non-Linux entry points fail closed instead of reopening
//! diagnostic paths or weakening descriptor transfer requirements.

mod artifact;
mod closure;
mod control;
mod digest;
mod error;
mod model;
mod platform;
#[allow(unsafe_code)]
mod process;
mod result;
mod root;
mod wire;

pub use artifact::{
    HostArtifactCatalogV1, HostLinkHandoffV1, HostLinkPlanV1, PublishedHostArtifactV1,
};
pub use closure::{
    AdmittedHostOutputV1, ApprovedStaticHostLldV1, AuthenticatedHostLinkExecutionV1,
    BrokerReservedHostLinkV1, HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1,
    HOST_LINK_ADMISSION_MAX_MILLIS_PER_POLL_V1, HOST_LINK_ADMISSION_MAX_OPERATIONS_PER_POLL_V1,
    HOST_LINK_BROKER_RESERVATION_NONCE_DOMAIN_V1, HOST_LLD_FIRST_INPUT_CHILD_FD_V1,
    HOST_LLD_INPUT_ARGUMENT_PREFIX_V1, HOST_LLD_PROTOCOL_ARGUMENT_V1,
    HOST_LLD_REQUEST_ARGUMENT_PREFIX_V1, HOST_LLD_RESULT_SOCKET_ARGUMENT_PREFIX_V1,
    HOST_LLD_RESULT_SOCKET_CHILD_FD_V1, HostLinkBrokerReservationV1, HostLinkClosureV1, LldArgvV1,
};
pub use digest::{Sha256Digest, sha256_bytes};
pub use error::{HOST_LINK_REJECTION_CODES_V1, HostLinkError, HostLinkErrorCodeV1};
pub use model::{
    ArtifactIdV1, ArtifactIdentityV1, ArtifactProvenanceV1, DsoBindingV1, ElfClassV1, ElfEndianV1,
    ElfProfileV1, ExecutableToolchainV1, HostArtifactKindV1, HostLinkPlanManifestV1,
    HostLinkPlanSpecV1, LibraryPreferenceV1, LinkerZPolicyV1, OutputTypeV1, PlanArgumentV1,
    ProducerArtifactSpecV1, ReleaseNonceV1, RootInputKindV1, RuntimeDsoClosureV1, TargetTripleV1,
};
pub use process::{
    MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1, authenticated_host_link_available_capacity_v1,
};
pub use result::{
    HOST_LINK_RESULT_COPY_POLICY_V1, HostLinkResultRecordV1, MAX_HOST_LINK_RESULT_RECORD_BYTES_V1,
};
pub use root::{FixedRootSetV1, FixedRootV1};

/// Maximum accepted canonical plan size.
pub const MAX_HOST_LINK_PLAN_BYTES_V1: usize = 4 * 1024 * 1024;
/// Maximum accepted bytes in one retained input.
pub const MAX_HOST_LINK_INPUT_BYTES_V1: u64 = 256 * 1024 * 1024;
/// Maximum aggregate bytes retained by one plan, catalog, or resolved closure.
pub const MAX_HOST_LINK_RETAINED_BYTES_V1: u64 = 2 * 1024 * 1024 * 1024;
/// Maximum accepted bytes in one sealed host-link result.
pub const MAX_HOST_LINK_OUTPUT_BYTES_V1: u64 = 512 * 1024 * 1024;
/// Maximum number of arguments in one plan.
pub const MAX_HOST_LINK_ARGUMENTS_V1: usize = 4_092;
/// Maximum number of unique descriptor-backed inputs accepted by the static tool.
pub const MAX_HOST_LINK_UNIQUE_INPUTS_V1: usize = 2_048;
/// Maximum number of retained producer artifacts in one handoff.
pub const MAX_HOST_LINK_PRODUCERS_V1: usize = MAX_HOST_LINK_UNIQUE_INPUTS_V1;
/// Maximum cumulative archive members across one resolved unique input closure.
pub const MAX_HOST_LINK_ARCHIVE_MEMBERS_V1: u64 = 262_144;
/// Maximum program headers in an admitted static executable.
pub const MAX_HOST_LINK_ELF_PROGRAM_HEADERS_V1: usize = 1_024;
/// Maximum sections in one admitted ELF input or output.
pub const MAX_HOST_LINK_ELF_SECTIONS_V1: usize = 8_192;
/// Maximum entries in one ELF symbol, relocation, group, note, or index table.
pub const MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1: u64 = 1_048_576;
