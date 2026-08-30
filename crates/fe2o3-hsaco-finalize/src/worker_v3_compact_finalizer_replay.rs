//! Compact, canonical restart inputs for native Worker V3 finalization.

use std::{error::Error, fmt, ops::Range, time::Duration};

use fe2o3_artifact_transaction::{
    CompilerModuleHandoffSlotV3, CompilerModuleHandoffTransactionIdentityV3,
    MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3;
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FinalizedProtectedWorkerV3HsacoIdentityV1, LinkOptionV1, MAX_LINK_INPUTS,
    MAX_LINK_OPTION_NAME_BYTES, MAX_LINK_OPTION_VALUE_BYTES, MAX_LINK_OPTIONS,
    MAX_WORKER_EXECUTABLE_BYTES, MAX_WORKER_OUTPUT_BYTES, MAX_WORKER_TOOLCHAIN_ID_BYTES,
    MAX_WORKER_TOTAL_INPUT_BYTES, PreparedFinalizedProtectedWorkerV3HsacoV1,
    ProtectedFirstBuildWorkerV3Error, WorkerExecutionLimitsV1, WorkerInputKindV1,
    WorkerMeasurementV1, WorkerProtocolError,
    first_build_worker_v3::{
        OwnedWorkerV3ProviderReplayPartV1, extract_worker_v3_request_replay_parts_v1,
    },
    worker_protocol_v2::{
        WorkerResponseReplayMetadataV1, validate_worker_response_replay_metadata_bodies_v1,
    },
};

const COMPACT_REPLAY_MAGIC_V1: &[u8; 8] = b"F2V3CFR1";
const COMPACT_REPLAY_VERSION_V1: u16 = 1;
const COMPACT_REPLAY_MAGIC_V2: &[u8; 8] = b"F2V3CFR2";
const COMPACT_REPLAY_VERSION_V2: u16 = 2;
const COMPACT_REPLAY_MAGIC_V3: &[u8; 8] = b"F2V3CFR3";
const COMPACT_REPLAY_VERSION_V3: u16 = 3;
const COMPACT_REPLAY_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3-COMPACT-FINALIZER-REPLAY-CHECKSUM/V1\0";
const COMPACT_REPLAY_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3-COMPACT-FINALIZER-REPLAY-IDENTITY/V1\0";
const COMPACT_REPLAY_CHECKSUM_DOMAIN_V2: &[u8] =
    b"FE2O3/WORKER-V3-COMPACT-FINALIZER-REPLAY-CHECKSUM/V2\0";
const COMPACT_REPLAY_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/WORKER-V3-COMPACT-FINALIZER-REPLAY-IDENTITY/V2\0";
const COMPACT_REPLAY_CHECKSUM_DOMAIN_V3: &[u8] =
    b"FE2O3/WORKER-V3-COMPACT-FINALIZER-REPLAY-CHECKSUM/V3\0";
const COMPACT_REPLAY_IDENTITY_DOMAIN_V3: &[u8] =
    b"FE2O3/WORKER-V3-COMPACT-FINALIZER-REPLAY-IDENTITY/V3\0";

const MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES_V1: usize = 16_644;
const MAX_RESPONSE_PROVIDER_EVIDENCE_BODY_BYTES_V1: usize = 1_067_889;
const MAX_RESPONSE_DERIVATION_EVIDENCE_BODY_BYTES_V1: usize = 5_518;

/// Maximum canonical bytes in one compact native-V3 finalizer replay transcript.
pub const MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1: usize =
    MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1;

const _: () = assert!(MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1 == 2_206_545);

/// Domain-separated identity of one exact compact V3 finalizer replay transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedWorkerV3CompactFinalizerReplayIdentityV1([u8; 32]);

impl ProtectedWorkerV3CompactFinalizerReplayIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Domain-separated identity of one exact transaction-replayable compact V2 transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedWorkerV3CompactFinalizerReplayIdentityV2([u8; 32]);

impl ProtectedWorkerV3CompactFinalizerReplayIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerV3ProviderReplayReferenceV1 {
    pub(crate) kind: WorkerInputKindV1,
    pub(crate) identity: ContentIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompactResponseMetadataRangesV1 {
    diagnostics: Range<usize>,
    provider_evidence: Option<Range<usize>>,
    derivation_evidence: Option<Range<usize>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCompactReplayTailV1 {
    worker: WorkerMeasurementV1,
    execution_limits: WorkerExecutionLimitsV1,
    bootstrap_output_bound: u64,
    external_providers: Vec<WorkerV3ProviderReplayReferenceV1>,
    link_options: Vec<LinkOptionV1>,
    bootstrap_metadata: CompactResponseMetadataRangesV1,
    replay_metadata: CompactResponseMetadataRangesV1,
}

pub(crate) struct ProtectedWorkerV3CompactFinalizerReplayViewV2<'replay> {
    pub(crate) worker: &'replay WorkerMeasurementV1,
    pub(crate) execution_limits: WorkerExecutionLimitsV1,
    pub(crate) bootstrap_output_bound: u64,
    pub(crate) external_providers: &'replay [WorkerV3ProviderReplayReferenceV1],
    pub(crate) link_options: &'replay [LinkOptionV1],
    pub(crate) bootstrap_metadata: WorkerResponseReplayMetadataV1<'replay>,
    pub(crate) replay_metadata: WorkerResponseReplayMetadataV1<'replay>,
}

/// Opaque bounded transcript that reconstructs exact V3 request and response wires.
///
/// Large outer-handoff, provider-payload, raw-HSACO, and finalized-HSACO bytes are not retained in
/// this value. The transcript is inert and grants no compiler, publication, load, or launch
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedWorkerV3CompactFinalizerReplayV1 {
    identity: ProtectedWorkerV3CompactFinalizerReplayIdentityV1,
    expected_finalization_identity: [u8; 32],
    source_evidence_identity: [u8; 32],
    binding_identity: [u8; 32],
    worker: WorkerMeasurementV1,
    execution_limits: WorkerExecutionLimitsV1,
    bootstrap_output_bound: u64,
    external_providers: Vec<WorkerV3ProviderReplayReferenceV1>,
    link_options: Vec<LinkOptionV1>,
    bootstrap_metadata: CompactResponseMetadataRangesV1,
    replay_metadata: CompactResponseMetadataRangesV1,
    canonical_bytes: Vec<u8>,
}

impl ProtectedWorkerV3CompactFinalizerReplayV1 {
    pub const fn identity(&self) -> ProtectedWorkerV3CompactFinalizerReplayIdentityV1 {
        self.identity
    }

    pub const fn expected_finalization_identity(&self) -> &[u8; 32] {
        &self.expected_finalization_identity
    }

    pub const fn source_evidence_identity(&self) -> &[u8; 32] {
        &self.source_evidence_identity
    }

    pub const fn binding_identity(&self) -> &[u8; 32] {
        &self.binding_identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn try_encode_canonical(
        &self,
    ) -> Result<Vec<u8>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        try_copy_bytes(&self.canonical_bytes, "compact transcript encoding")
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        if bytes.len() < COMPACT_REPLAY_MAGIC_V1.len() + 2 + 32
            || bytes.len() > MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1
        {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length);
        }
        Self::decode_owned(try_copy_bytes(bytes, "compact transcript decode")?)
    }

    fn decode_owned(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        if canonical_bytes.len() < COMPACT_REPLAY_MAGIC_V1.len() + 2 + 32
            || canonical_bytes.len() > MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1
        {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length);
        }
        let checksum_offset = canonical_bytes
            .len()
            .checked_sub(32)
            .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)?;
        let (body, checksum) = canonical_bytes.split_at(checksum_offset);
        if hash_domain_blob(COMPACT_REPLAY_CHECKSUM_DOMAIN_V1, body) != checksum {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Checksum);
        }

        let mut reader = CompactReplayReaderV1::new(body);
        if reader.take(COMPACT_REPLAY_MAGIC_V1.len())? != COMPACT_REPLAY_MAGIC_V1 {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Magic);
        }
        if reader.u16()? != COMPACT_REPLAY_VERSION_V1 {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Version);
        }
        let expected_finalization_identity = reader.array()?;
        if expected_finalization_identity == [0; 32] {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity);
        }
        let source_evidence_identity = reader.array()?;
        let binding_identity = reader.array()?;
        if source_evidence_identity == [0; 32] || binding_identity == [0; 32] {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity);
        }
        let tail = decode_compact_replay_tail(&mut reader, false)?;
        let identity = ProtectedWorkerV3CompactFinalizerReplayIdentityV1(hash_domain_blob(
            COMPACT_REPLAY_IDENTITY_DOMAIN_V1,
            &canonical_bytes,
        ));
        Ok(Self {
            identity,
            expected_finalization_identity,
            source_evidence_identity,
            binding_identity,
            worker: tail.worker,
            execution_limits: tail.execution_limits,
            bootstrap_output_bound: tail.bootstrap_output_bound,
            external_providers: tail.external_providers,
            link_options: tail.link_options,
            bootstrap_metadata: tail.bootstrap_metadata,
            replay_metadata: tail.replay_metadata,
            canonical_bytes,
        })
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn decode_compact_replay_tail(
    reader: &mut CompactReplayReaderV1<'_>,
    has_derivation_metadata: bool,
) -> Result<DecodedCompactReplayTailV1, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let worker_executable = decode_content_identity(reader, MAX_WORKER_EXECUTABLE_BYTES)?;
    let worker_build_identity = copy_text(
        reader.text_u8(MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
        "worker build identity",
    )?;
    let llvm_build_identity = copy_text(
        reader.text_u8(MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
        "LLVM build identity",
    )?;
    let worker = WorkerMeasurementV1::new(
        worker_executable,
        worker_build_identity,
        llvm_build_identity,
    )
    .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Worker)?;
    let timeout_seconds = reader.u64()?;
    let timeout_nanoseconds = reader.u32()?;
    if timeout_nanoseconds >= 1_000_000_000 {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Limits);
    }
    let timeout = Duration::new(timeout_seconds, timeout_nanoseconds);
    let stdout_bytes = usize::try_from(reader.u64()?)
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Limits)?;
    let stderr_bytes = usize::try_from(reader.u64()?)
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Limits)?;
    let execution_limits = WorkerExecutionLimitsV1::new(timeout, stdout_bytes, stderr_bytes)
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Limits)?;
    let bootstrap_output_bound = reader.u64()?;
    if bootstrap_output_bound == 0 || bootstrap_output_bound > MAX_WORKER_OUTPUT_BYTES as u64 {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::OutputBound);
    }

    let provider_count = reader.u8()? as usize;
    if provider_count > MAX_LINK_INPUTS.saturating_sub(1) {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers);
    }
    let mut external_providers = try_vec(provider_count, "provider references")?;
    let mut provider_payload_bytes = 0_u64;
    let mut previous_provider = None;
    for _ in 0..provider_count {
        let kind = decode_input_kind(reader.u8()?)?;
        let identity = decode_content_identity(reader, MAX_WORKER_TOTAL_INPUT_BYTES as u64)?;
        provider_payload_bytes = provider_payload_bytes
            .checked_add(identity.byte_len())
            .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers)?;
        if provider_payload_bytes > MAX_WORKER_TOTAL_INPUT_BYTES as u64
            || previous_provider.is_some_and(|before: (ContentIdentityV1, WorkerInputKindV1)| {
                before.0 == identity || before >= (identity, kind)
            })
        {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers);
        }
        external_providers.push(WorkerV3ProviderReplayReferenceV1 { kind, identity });
        previous_provider = Some((identity, kind));
    }

    let option_count = reader.u8()? as usize;
    if option_count > MAX_LINK_OPTIONS {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Options);
    }
    let mut link_options = try_vec(option_count, "link options")?;
    for _ in 0..option_count {
        let name = copy_text(
            reader.text_u8(MAX_LINK_OPTION_NAME_BYTES)?,
            "link option name",
        )?;
        let value = copy_text(
            reader.text_u16_allow_empty(MAX_LINK_OPTION_VALUE_BYTES)?,
            "link option value",
        )?;
        let option = LinkOptionV1::new(name, value)
            .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Options)?;
        if link_options
            .last()
            .is_some_and(|before: &LinkOptionV1| before.name() >= option.name())
        {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Options);
        }
        link_options.push(option);
    }

    let bootstrap_metadata = decode_response_metadata(reader, has_derivation_metadata)?;
    let replay_metadata = decode_response_metadata(reader, has_derivation_metadata)?;
    if !reader.finished() {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::TrailingBytes);
    }
    Ok(DecodedCompactReplayTailV1 {
        worker,
        execution_limits,
        bootstrap_output_bound,
        external_providers,
        link_options,
        bootstrap_metadata,
        replay_metadata,
    })
}

/// Compact replay metadata that can independently rederive the strict V3 transaction binding.
///
/// V1 remains decodable for storage compatibility, but carries only an opaque binding digest.
/// V2 instead retains the bounded slot and transaction identity; the build attempt comes from the
/// durable occurrence record and every other binding axis is rederived from the outer handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedWorkerV3CompactFinalizerReplayV2 {
    identity: ProtectedWorkerV3CompactFinalizerReplayIdentityV2,
    expected_finalization_identity: [u8; 32],
    source_evidence_identity: [u8; 32],
    handoff_slot: CompilerModuleHandoffSlotV3,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    worker: WorkerMeasurementV1,
    execution_limits: WorkerExecutionLimitsV1,
    bootstrap_output_bound: u64,
    external_providers: Vec<WorkerV3ProviderReplayReferenceV1>,
    link_options: Vec<LinkOptionV1>,
    bootstrap_metadata: CompactResponseMetadataRangesV1,
    replay_metadata: CompactResponseMetadataRangesV1,
    canonical_bytes: Vec<u8>,
}

impl ProtectedWorkerV3CompactFinalizerReplayV2 {
    pub const fn identity(&self) -> ProtectedWorkerV3CompactFinalizerReplayIdentityV2 {
        self.identity
    }

    pub const fn expected_finalization_identity(&self) -> &[u8; 32] {
        &self.expected_finalization_identity
    }

    pub const fn source_evidence_identity(&self) -> &[u8; 32] {
        &self.source_evidence_identity
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV3 {
        self.handoff_slot
    }

    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV3 {
        self.transaction_identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Reports whether this transcript retains exact LLVM/object/LLD derivation bodies.
    ///
    /// Legacy wire V2 remains decodable for storage inspection but is insufficient for production
    /// finalizer-derivation admission.
    pub fn retains_derivation_metadata(&self) -> bool {
        self.canonical_bytes.starts_with(COMPACT_REPLAY_MAGIC_V3)
    }

    pub fn try_encode_canonical(
        &self,
    ) -> Result<Vec<u8>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        try_copy_bytes(&self.canonical_bytes, "compact V2 transcript encoding")
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    pub(crate) fn replay_view(&self) -> ProtectedWorkerV3CompactFinalizerReplayViewV2<'_> {
        let bootstrap_provider = self
            .bootstrap_metadata
            .provider_evidence
            .as_ref()
            .map(|range| &self.canonical_bytes[range.clone()]);
        let replay_provider = self
            .replay_metadata
            .provider_evidence
            .as_ref()
            .map(|range| &self.canonical_bytes[range.clone()]);
        let bootstrap_derivation = self
            .bootstrap_metadata
            .derivation_evidence
            .as_ref()
            .map(|range| &self.canonical_bytes[range.clone()]);
        let replay_derivation = self
            .replay_metadata
            .derivation_evidence
            .as_ref()
            .map(|range| &self.canonical_bytes[range.clone()]);
        ProtectedWorkerV3CompactFinalizerReplayViewV2 {
            worker: &self.worker,
            execution_limits: self.execution_limits,
            bootstrap_output_bound: self.bootstrap_output_bound,
            external_providers: &self.external_providers,
            link_options: &self.link_options,
            bootstrap_metadata: WorkerResponseReplayMetadataV1::from_bodies(
                &self.canonical_bytes[self.bootstrap_metadata.diagnostics.clone()],
                bootstrap_provider,
                bootstrap_derivation,
            ),
            replay_metadata: WorkerResponseReplayMetadataV1::from_bodies(
                &self.canonical_bytes[self.replay_metadata.diagnostics.clone()],
                replay_provider,
                replay_derivation,
            ),
        }
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        if bytes.len() < COMPACT_REPLAY_MAGIC_V2.len() + 2 + 32
            || bytes.len() > MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1
        {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length);
        }
        Self::decode_owned(try_copy_bytes(bytes, "compact V2 transcript decode")?)
    }

    fn decode_owned(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        if canonical_bytes.len() < COMPACT_REPLAY_MAGIC_V2.len() + 2 + 32
            || canonical_bytes.len() > MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1
        {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length);
        }
        let checksum_offset = canonical_bytes
            .len()
            .checked_sub(32)
            .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)?;
        let (body, checksum) = canonical_bytes.split_at(checksum_offset);
        let (magic, version, checksum_domain, identity_domain, has_derivation) =
            if body.starts_with(COMPACT_REPLAY_MAGIC_V3) {
                (
                    COMPACT_REPLAY_MAGIC_V3,
                    COMPACT_REPLAY_VERSION_V3,
                    COMPACT_REPLAY_CHECKSUM_DOMAIN_V3,
                    COMPACT_REPLAY_IDENTITY_DOMAIN_V3,
                    true,
                )
            } else {
                (
                    COMPACT_REPLAY_MAGIC_V2,
                    COMPACT_REPLAY_VERSION_V2,
                    COMPACT_REPLAY_CHECKSUM_DOMAIN_V2,
                    COMPACT_REPLAY_IDENTITY_DOMAIN_V2,
                    false,
                )
            };
        if hash_domain_blob(checksum_domain, body) != checksum {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Checksum);
        }

        let mut reader = CompactReplayReaderV1::new(body);
        if reader.take(magic.len())? != magic {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Magic);
        }
        if reader.u16()? != version {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Version);
        }
        let expected_finalization_identity = reader.array()?;
        let source_evidence_identity = reader.array()?;
        if expected_finalization_identity == [0; 32] || source_evidence_identity == [0; 32] {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity);
        }
        let handoff_slot = decode_handoff_slot_v3(reader.u8()?)?;
        let transaction_identity =
            CompilerModuleHandoffTransactionIdentityV3::from_bytes(reader.array()?);
        if transaction_identity.as_bytes() == &[0; 32] {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity);
        }
        let tail = decode_compact_replay_tail(&mut reader, has_derivation)?;
        let identity = ProtectedWorkerV3CompactFinalizerReplayIdentityV2(hash_domain_blob(
            identity_domain,
            &canonical_bytes,
        ));
        Ok(Self {
            identity,
            expected_finalization_identity,
            source_evidence_identity,
            handoff_slot,
            transaction_identity,
            worker: tail.worker,
            execution_limits: tail.execution_limits,
            bootstrap_output_bound: tail.bootstrap_output_bound,
            external_providers: tail.external_providers,
            link_options: tail.link_options,
            bootstrap_metadata: tail.bootstrap_metadata,
            replay_metadata: tail.replay_metadata,
            canonical_bytes,
        })
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Unique large owners and compact transcript prepared for durable V3 restart storage.
pub struct PreparedProtectedWorkerV3CompactFinalizerReplayV1 {
    outer_handoff: Vec<u8>,
    external_provider_payloads: Vec<Vec<u8>>,
    transcript: ProtectedWorkerV3CompactFinalizerReplayV1,
    finalized_hsaco: Vec<u8>,
}

impl fmt::Debug for PreparedProtectedWorkerV3CompactFinalizerReplayV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let provider_bytes = self
            .external_provider_payloads
            .iter()
            .fold(0_usize, |total, payload| {
                total.saturating_add(payload.len())
            });
        formatter
            .debug_struct("PreparedProtectedWorkerV3CompactFinalizerReplayV1")
            .field("outer_handoff_bytes", &self.outer_handoff.len())
            .field(
                "external_provider_count",
                &self.external_provider_payloads.len(),
            )
            .field("external_provider_bytes", &provider_bytes)
            .field("transcript_identity", &self.transcript.identity())
            .field("transcript_bytes", &self.transcript.canonical_bytes().len())
            .field("finalized_hsaco_bytes", &self.finalized_hsaco.len())
            .finish()
    }
}

/// Named inert byte owners ready for the durable Worker V3 storage bridge.
pub struct ProtectedWorkerV3CompactFinalizerReplayPartsV1 {
    /// Exact canonical outer semantic handoff wire.
    pub outer_handoff: Vec<u8>,
    /// Exact external provider payloads in canonical request order.
    pub external_provider_payloads: Vec<Vec<u8>>,
    /// Exact canonical compact finalizer replay transcript.
    pub transcript: Vec<u8>,
    /// Exact finalized canonical HSACO.
    pub finalized_hsaco: Vec<u8>,
}

impl fmt::Debug for ProtectedWorkerV3CompactFinalizerReplayPartsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let provider_bytes = self
            .external_provider_payloads
            .iter()
            .fold(0_usize, |total, payload| {
                total.saturating_add(payload.len())
            });
        formatter
            .debug_struct("ProtectedWorkerV3CompactFinalizerReplayPartsV1")
            .field("outer_handoff_bytes", &self.outer_handoff.len())
            .field(
                "external_provider_count",
                &self.external_provider_payloads.len(),
            )
            .field("external_provider_bytes", &provider_bytes)
            .field("transcript_bytes", &self.transcript.len())
            .field("finalized_hsaco_bytes", &self.finalized_hsaco.len())
            .finish()
    }
}

impl PreparedProtectedWorkerV3CompactFinalizerReplayV1 {
    pub fn outer_handoff(&self) -> &[u8] {
        &self.outer_handoff
    }

    pub fn external_provider_payloads(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        self.external_provider_payloads.iter().map(Vec::as_slice)
    }

    pub const fn transcript(&self) -> &ProtectedWorkerV3CompactFinalizerReplayV1 {
        &self.transcript
    }

    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        &self.finalized_hsaco
    }

    pub fn into_parts(self) -> ProtectedWorkerV3CompactFinalizerReplayPartsV1 {
        ProtectedWorkerV3CompactFinalizerReplayPartsV1 {
            outer_handoff: self.outer_handoff,
            external_provider_payloads: self.external_provider_payloads,
            transcript: self.transcript.into_canonical_bytes(),
            finalized_hsaco: self.finalized_hsaco,
        }
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Unique restart owners using the transaction-replayable compact V2 transcript.
pub struct PreparedProtectedWorkerV3CompactFinalizerReplayV2 {
    outer_handoff: Vec<u8>,
    external_provider_payloads: Vec<Vec<u8>>,
    transcript: ProtectedWorkerV3CompactFinalizerReplayV2,
    finalized_hsaco: Vec<u8>,
}

impl fmt::Debug for PreparedProtectedWorkerV3CompactFinalizerReplayV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let provider_bytes = self
            .external_provider_payloads
            .iter()
            .fold(0_usize, |total, payload| {
                total.saturating_add(payload.len())
            });
        formatter
            .debug_struct("PreparedProtectedWorkerV3CompactFinalizerReplayV2")
            .field("outer_handoff_bytes", &self.outer_handoff.len())
            .field(
                "external_provider_count",
                &self.external_provider_payloads.len(),
            )
            .field("external_provider_bytes", &provider_bytes)
            .field("transcript_identity", &self.transcript.identity())
            .field("transcript_bytes", &self.transcript.canonical_bytes().len())
            .field("finalized_hsaco_bytes", &self.finalized_hsaco.len())
            .finish()
    }
}

pub(crate) struct OwnedProtectedWorkerV3CompactFinalizerReplayPartsV2 {
    pub(crate) outer_handoff: Vec<u8>,
    pub(crate) external_provider_payloads: Vec<Vec<u8>>,
    pub(crate) transcript: Vec<u8>,
    pub(crate) finalized_hsaco: Vec<u8>,
}

/// Named inert byte owners for one independently replayable V3 load envelope.
///
/// These components retain the same compact V2 transcript used by durable restart storage. They
/// are descriptive bytes only and grant no compiler, publication, load, or launch authority.
pub struct ProtectedWorkerV3CompactFinalizerReplayPartsV2 {
    /// Exact canonical outer semantic handoff wire.
    pub outer_handoff: Vec<u8>,
    /// Exact external provider payloads in canonical request order.
    pub external_provider_payloads: Vec<Vec<u8>>,
    /// Exact canonical compact V2 finalizer replay transcript.
    pub transcript: Vec<u8>,
    /// Exact finalized canonical HSACO.
    pub finalized_hsaco: Vec<u8>,
}

impl fmt::Debug for ProtectedWorkerV3CompactFinalizerReplayPartsV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let provider_bytes = self
            .external_provider_payloads
            .iter()
            .fold(0_usize, |total, payload| {
                total.saturating_add(payload.len())
            });
        formatter
            .debug_struct("ProtectedWorkerV3CompactFinalizerReplayPartsV2")
            .field("outer_handoff_bytes", &self.outer_handoff.len())
            .field(
                "external_provider_count",
                &self.external_provider_payloads.len(),
            )
            .field("external_provider_bytes", &provider_bytes)
            .field("transcript_bytes", &self.transcript.len())
            .field("finalized_hsaco_bytes", &self.finalized_hsaco.len())
            .finish()
    }
}

impl PreparedProtectedWorkerV3CompactFinalizerReplayV2 {
    pub fn outer_handoff(&self) -> &[u8] {
        &self.outer_handoff
    }

    pub fn external_provider_payloads(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        self.external_provider_payloads.iter().map(Vec::as_slice)
    }

    pub const fn transcript(&self) -> &ProtectedWorkerV3CompactFinalizerReplayV2 {
        &self.transcript
    }

    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        &self.finalized_hsaco
    }

    /// Transfers every unique replay owner without copying its large byte attachments.
    pub fn into_parts(self) -> ProtectedWorkerV3CompactFinalizerReplayPartsV2 {
        ProtectedWorkerV3CompactFinalizerReplayPartsV2 {
            outer_handoff: self.outer_handoff,
            external_provider_payloads: self.external_provider_payloads,
            transcript: self.transcript.into_canonical_bytes(),
            finalized_hsaco: self.finalized_hsaco,
        }
    }

    pub(crate) fn into_storage_parts(self) -> OwnedProtectedWorkerV3CompactFinalizerReplayPartsV2 {
        OwnedProtectedWorkerV3CompactFinalizerReplayPartsV2 {
            outer_handoff: self.outer_handoff,
            external_provider_payloads: self.external_provider_payloads,
            transcript: self.transcript.into_canonical_bytes(),
            finalized_hsaco: self.finalized_hsaco,
        }
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Consumes finalized native-V3 evidence into unique durable restart components.
pub fn prepare_protected_worker_v3_compact_finalizer_replay_v1(
    finalized: PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<
    PreparedProtectedWorkerV3CompactFinalizerReplayV1,
    ProtectedWorkerV3CompactFinalizerReplayErrorV1,
> {
    let finalized = finalized.into_compact_replay_parts();
    let source = finalized.source.into_compact_replay_parts();
    let request_parts = extract_worker_v3_request_replay_parts_v1(
        &source.bootstrap_request_bytes,
        &source.replay_request_bytes,
    )?;
    let bootstrap_metadata = source.bootstrap_response.replay_metadata()?;
    let replay_metadata = source.replay_response.replay_metadata()?;
    let transcript_bytes = encode_compact_replay(
        finalized.identity,
        *source.identity.as_bytes(),
        *source.binding.identity().as_bytes(),
        &source.worker,
        source.execution_limits,
        request_parts.bootstrap_output_bound,
        &request_parts.external_providers,
        source.plan.options(),
        bootstrap_metadata,
        replay_metadata,
    )?;
    let transcript = ProtectedWorkerV3CompactFinalizerReplayV1::decode_owned(transcript_bytes)?;

    let mut external_provider_payloads = try_vec(
        request_parts.external_providers.len(),
        "provider payload owners",
    )?;
    external_provider_payloads.extend(
        request_parts
            .external_providers
            .into_iter()
            .map(|provider| provider.bytes),
    );
    let outer_handoff = source.handoff.try_into_canonical_bytes()?;
    Ok(PreparedProtectedWorkerV3CompactFinalizerReplayV1 {
        outer_handoff,
        external_provider_payloads,
        transcript,
        finalized_hsaco: finalized.finalized_bytes,
    })
}

/// Consumes finalized native-V3 evidence into independently replayable durable components.
pub fn prepare_protected_worker_v3_compact_finalizer_replay_v2(
    finalized: PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<
    PreparedProtectedWorkerV3CompactFinalizerReplayV2,
    ProtectedWorkerV3CompactFinalizerReplayErrorV1,
> {
    let finalized = finalized.into_compact_replay_parts();
    let source = finalized.source.into_compact_replay_parts();
    let request_parts = extract_worker_v3_request_replay_parts_v1(
        &source.bootstrap_request_bytes,
        &source.replay_request_bytes,
    )?;
    let bootstrap_metadata = source.bootstrap_response.replay_metadata()?;
    let replay_metadata = source.replay_response.replay_metadata()?;
    let expectation = source.binding.expectation();
    let transcript_bytes = encode_compact_replay_v2(
        finalized.identity,
        *source.identity.as_bytes(),
        expectation.slot(),
        expectation.transaction_identity(),
        &source.worker,
        source.execution_limits,
        request_parts.bootstrap_output_bound,
        &request_parts.external_providers,
        source.plan.options(),
        bootstrap_metadata,
        replay_metadata,
    )?;
    let transcript = ProtectedWorkerV3CompactFinalizerReplayV2::decode_owned(transcript_bytes)?;

    let mut external_provider_payloads = try_vec(
        request_parts.external_providers.len(),
        "provider payload owners",
    )?;
    external_provider_payloads.extend(
        request_parts
            .external_providers
            .into_iter()
            .map(|provider| provider.bytes),
    );
    let outer_handoff = source.handoff.try_into_canonical_bytes()?;
    Ok(PreparedProtectedWorkerV3CompactFinalizerReplayV2 {
        outer_handoff,
        external_provider_payloads,
        transcript,
        finalized_hsaco: finalized.finalized_bytes,
    })
}

#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedWorkerV3CompactFinalizerReplayErrorV1 {
    Length,
    AllocationFailed { component: &'static str },
    Magic,
    Version,
    Checksum,
    Truncated,
    TrailingBytes,
    Identity,
    RetiredHandoffSlot { tag: u8 },
    Worker,
    Limits,
    OutputBound,
    Providers,
    Options,
    ResponseMetadata,
    FirstBuild(ProtectedFirstBuildWorkerV3Error),
    WorkerProtocol(WorkerProtocolError),
    OuterHandoff(InertSemanticCompilerModuleHandoffErrorV3),
}

impl fmt::Display for ProtectedWorkerV3CompactFinalizerReplayErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => formatter.write_str("compact Worker V3 replay length is invalid"),
            Self::AllocationFailed { component } => {
                write!(
                    formatter,
                    "could not allocate compact Worker V3 replay {component}"
                )
            }
            Self::Magic => formatter.write_str("compact Worker V3 replay magic mismatch"),
            Self::Version => formatter.write_str("unsupported compact Worker V3 replay version"),
            Self::Checksum => formatter.write_str("compact Worker V3 replay checksum mismatch"),
            Self::Truncated => formatter.write_str("truncated compact Worker V3 replay"),
            Self::TrailingBytes => formatter.write_str("trailing compact Worker V3 replay bytes"),
            Self::Identity => formatter.write_str("invalid compact Worker V3 replay identity"),
            Self::RetiredHandoffSlot { tag } => {
                write!(formatter, "retired compiler handoff slot tag {tag}")
            }
            Self::Worker => formatter.write_str("invalid compact Worker V3 worker measurement"),
            Self::Limits => formatter.write_str("invalid compact Worker V3 execution limits"),
            Self::OutputBound => formatter.write_str("invalid compact Worker V3 output bound"),
            Self::Providers => formatter.write_str("invalid compact Worker V3 provider references"),
            Self::Options => formatter.write_str("invalid compact Worker V3 link options"),
            Self::ResponseMetadata => {
                formatter.write_str("invalid compact Worker V3 response metadata")
            }
            Self::FirstBuild(error) => error.fmt(formatter),
            Self::WorkerProtocol(error) => error.fmt(formatter),
            Self::OuterHandoff(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProtectedWorkerV3CompactFinalizerReplayErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FirstBuild(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::OuterHandoff(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProtectedFirstBuildWorkerV3Error> for ProtectedWorkerV3CompactFinalizerReplayErrorV1 {
    fn from(error: ProtectedFirstBuildWorkerV3Error) -> Self {
        Self::FirstBuild(error)
    }
}

impl From<WorkerProtocolError> for ProtectedWorkerV3CompactFinalizerReplayErrorV1 {
    fn from(error: WorkerProtocolError) -> Self {
        Self::WorkerProtocol(error)
    }
}

impl From<InertSemanticCompilerModuleHandoffErrorV3>
    for ProtectedWorkerV3CompactFinalizerReplayErrorV1
{
    fn from(error: InertSemanticCompilerModuleHandoffErrorV3) -> Self {
        Self::OuterHandoff(error)
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_compact_replay(
    finalization_identity: FinalizedProtectedWorkerV3HsacoIdentityV1,
    source_evidence_identity: [u8; 32],
    binding_identity: [u8; 32],
    worker: &WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    bootstrap_output_bound: u64,
    external_providers: &[OwnedWorkerV3ProviderReplayPartV1],
    link_options: &[LinkOptionV1],
    bootstrap_metadata: WorkerResponseReplayMetadataV1<'_>,
    replay_metadata: WorkerResponseReplayMetadataV1<'_>,
) -> Result<Vec<u8>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    encode_compact_replay_with_binding(
        finalization_identity,
        source_evidence_identity,
        CompactReplayBindingHeaderV1::OpaqueV1(binding_identity),
        worker,
        limits,
        bootstrap_output_bound,
        external_providers,
        link_options,
        bootstrap_metadata,
        replay_metadata,
    )
}

#[allow(clippy::too_many_arguments)]
fn encode_compact_replay_v2(
    finalization_identity: FinalizedProtectedWorkerV3HsacoIdentityV1,
    source_evidence_identity: [u8; 32],
    handoff_slot: CompilerModuleHandoffSlotV3,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    worker: &WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    bootstrap_output_bound: u64,
    external_providers: &[OwnedWorkerV3ProviderReplayPartV1],
    link_options: &[LinkOptionV1],
    bootstrap_metadata: WorkerResponseReplayMetadataV1<'_>,
    replay_metadata: WorkerResponseReplayMetadataV1<'_>,
) -> Result<Vec<u8>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    encode_compact_replay_with_binding(
        finalization_identity,
        source_evidence_identity,
        CompactReplayBindingHeaderV1::TransactionV2 {
            handoff_slot,
            transaction_identity,
        },
        worker,
        limits,
        bootstrap_output_bound,
        external_providers,
        link_options,
        bootstrap_metadata,
        replay_metadata,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn encode_legacy_compact_replay_v2(
    finalization_identity: FinalizedProtectedWorkerV3HsacoIdentityV1,
    source_evidence_identity: [u8; 32],
    handoff_slot: CompilerModuleHandoffSlotV3,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    worker: &WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    bootstrap_output_bound: u64,
    external_providers: &[OwnedWorkerV3ProviderReplayPartV1],
    link_options: &[LinkOptionV1],
    bootstrap_metadata: WorkerResponseReplayMetadataV1<'_>,
    replay_metadata: WorkerResponseReplayMetadataV1<'_>,
) -> Result<Vec<u8>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    encode_compact_replay_with_binding(
        finalization_identity,
        source_evidence_identity,
        CompactReplayBindingHeaderV1::TransactionLegacyV2 {
            handoff_slot,
            transaction_identity,
        },
        worker,
        limits,
        bootstrap_output_bound,
        external_providers,
        link_options,
        bootstrap_metadata,
        replay_metadata,
    )
}

#[derive(Clone, Copy)]
enum CompactReplayBindingHeaderV1 {
    OpaqueV1([u8; 32]),
    #[cfg(test)]
    TransactionLegacyV2 {
        handoff_slot: CompilerModuleHandoffSlotV3,
        transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    },
    TransactionV2 {
        handoff_slot: CompilerModuleHandoffSlotV3,
        transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    },
}

impl CompactReplayBindingHeaderV1 {
    const fn encoded_len(self) -> usize {
        match self {
            Self::OpaqueV1(_) => 32,
            #[cfg(test)]
            Self::TransactionLegacyV2 { .. } => 33,
            Self::TransactionV2 { .. } => 33,
        }
    }

    const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::OpaqueV1(_) => COMPACT_REPLAY_MAGIC_V1,
            #[cfg(test)]
            Self::TransactionLegacyV2 { .. } => COMPACT_REPLAY_MAGIC_V2,
            Self::TransactionV2 { .. } => COMPACT_REPLAY_MAGIC_V3,
        }
    }

    const fn version(self) -> u16 {
        match self {
            Self::OpaqueV1(_) => COMPACT_REPLAY_VERSION_V1,
            #[cfg(test)]
            Self::TransactionLegacyV2 { .. } => COMPACT_REPLAY_VERSION_V2,
            Self::TransactionV2 { .. } => COMPACT_REPLAY_VERSION_V3,
        }
    }

    const fn checksum_domain(self) -> &'static [u8] {
        match self {
            Self::OpaqueV1(_) => COMPACT_REPLAY_CHECKSUM_DOMAIN_V1,
            #[cfg(test)]
            Self::TransactionLegacyV2 { .. } => COMPACT_REPLAY_CHECKSUM_DOMAIN_V2,
            Self::TransactionV2 { .. } => COMPACT_REPLAY_CHECKSUM_DOMAIN_V3,
        }
    }

    const fn retains_derivation_metadata(self) -> bool {
        matches!(self, Self::TransactionV2 { .. })
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        match self {
            Self::OpaqueV1(binding_identity) => bytes.extend_from_slice(&binding_identity),
            #[cfg(test)]
            Self::TransactionLegacyV2 {
                handoff_slot,
                transaction_identity,
            }
            | Self::TransactionV2 {
                handoff_slot,
                transaction_identity,
            } => {
                bytes.push(handoff_slot as u8);
                bytes.extend_from_slice(transaction_identity.as_bytes());
            }
            #[cfg(not(test))]
            Self::TransactionV2 {
                handoff_slot,
                transaction_identity,
            } => {
                bytes.push(handoff_slot as u8);
                bytes.extend_from_slice(transaction_identity.as_bytes());
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_compact_replay_with_binding(
    finalization_identity: FinalizedProtectedWorkerV3HsacoIdentityV1,
    source_evidence_identity: [u8; 32],
    binding: CompactReplayBindingHeaderV1,
    worker: &WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    bootstrap_output_bound: u64,
    external_providers: &[OwnedWorkerV3ProviderReplayPartV1],
    link_options: &[LinkOptionV1],
    bootstrap_metadata: WorkerResponseReplayMetadataV1<'_>,
    replay_metadata: WorkerResponseReplayMetadataV1<'_>,
) -> Result<Vec<u8>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    validate_construction_parts(
        bootstrap_output_bound,
        external_providers,
        link_options,
        bootstrap_metadata,
        replay_metadata,
    )?;
    let exact_length = compact_replay_encoded_length(
        binding.encoded_len(),
        binding.retains_derivation_metadata(),
        worker,
        external_providers,
        link_options,
        bootstrap_metadata,
        replay_metadata,
    )?;
    if exact_length > MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1 {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length);
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(exact_length).map_err(|_| {
        ProtectedWorkerV3CompactFinalizerReplayErrorV1::AllocationFailed {
            component: "canonical bytes",
        }
    })?;
    bytes.extend_from_slice(binding.magic());
    bytes.extend_from_slice(&binding.version().to_le_bytes());
    bytes.extend_from_slice(finalization_identity.as_bytes());
    bytes.extend_from_slice(&source_evidence_identity);
    binding.encode(&mut bytes);
    encode_content_identity(&mut bytes, worker.executable());
    push_u8_text(&mut bytes, worker.worker_build_identity())?;
    push_u8_text(&mut bytes, worker.llvm_build_identity())?;
    bytes.extend_from_slice(&limits.timeout().as_secs().to_le_bytes());
    bytes.extend_from_slice(&limits.timeout().subsec_nanos().to_le_bytes());
    let stdout_bytes = u64::try_from(limits.stdout_bytes())
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Limits)?;
    let stderr_bytes = u64::try_from(limits.stderr_bytes())
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Limits)?;
    bytes.extend_from_slice(&stdout_bytes.to_le_bytes());
    bytes.extend_from_slice(&stderr_bytes.to_le_bytes());
    bytes.extend_from_slice(&bootstrap_output_bound.to_le_bytes());
    bytes.push(
        u8::try_from(external_providers.len())
            .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers)?,
    );
    for provider in external_providers {
        bytes.push(provider.kind as u8);
        encode_content_identity(&mut bytes, provider.identity);
    }
    bytes.push(
        u8::try_from(link_options.len())
            .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Options)?,
    );
    for option in link_options {
        push_u8_text(&mut bytes, option.name())?;
        push_u16_text(&mut bytes, option.value())?;
    }
    encode_response_metadata(
        &mut bytes,
        bootstrap_metadata,
        binding.retains_derivation_metadata(),
    )?;
    encode_response_metadata(
        &mut bytes,
        replay_metadata,
        binding.retains_derivation_metadata(),
    )?;
    let checksum = hash_domain_blob(binding.checksum_domain(), &bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), exact_length);
    Ok(bytes)
}

fn validate_construction_parts(
    bootstrap_output_bound: u64,
    external_providers: &[OwnedWorkerV3ProviderReplayPartV1],
    link_options: &[LinkOptionV1],
    bootstrap_metadata: WorkerResponseReplayMetadataV1<'_>,
    replay_metadata: WorkerResponseReplayMetadataV1<'_>,
) -> Result<(), ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    if bootstrap_output_bound == 0 || bootstrap_output_bound > MAX_WORKER_OUTPUT_BYTES as u64 {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::OutputBound);
    }
    if external_providers.len() > MAX_LINK_INPUTS.saturating_sub(1) {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers);
    }
    let mut payload_bytes = 0_usize;
    let mut previous = None;
    for provider in external_providers {
        if !provider.identity.matches(&provider.bytes)
            || previous.is_some_and(|before: (ContentIdentityV1, WorkerInputKindV1)| {
                before.0 == provider.identity || before >= (provider.identity, provider.kind)
            })
        {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers);
        }
        payload_bytes = payload_bytes
            .checked_add(provider.bytes.len())
            .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers)?;
        if payload_bytes > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers);
        }
        previous = Some((provider.identity, provider.kind));
    }
    if link_options.len() > MAX_LINK_OPTIONS
        || link_options.windows(2).any(|pair| {
            pair[0].name() >= pair[1].name()
                || pair[0].name().len() > MAX_LINK_OPTION_NAME_BYTES
                || pair[0].value().len() > MAX_LINK_OPTION_VALUE_BYTES
        })
        || link_options.last().is_some_and(|option| {
            option.name().len() > MAX_LINK_OPTION_NAME_BYTES
                || option.value().len() > MAX_LINK_OPTION_VALUE_BYTES
        })
    {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Options);
    }
    for metadata in [bootstrap_metadata, replay_metadata] {
        validate_worker_response_replay_metadata_bodies_v1(
            metadata.diagnostics_body(),
            metadata.provider_evidence_body(),
            metadata.derivation_evidence_body(),
        )?;
    }
    Ok(())
}

fn compact_replay_encoded_length(
    binding_header_bytes: usize,
    retains_derivation_metadata: bool,
    worker: &WorkerMeasurementV1,
    external_providers: &[OwnedWorkerV3ProviderReplayPartV1],
    link_options: &[LinkOptionV1],
    bootstrap_metadata: WorkerResponseReplayMetadataV1<'_>,
    replay_metadata: WorkerResponseReplayMetadataV1<'_>,
) -> Result<usize, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let fixed = COMPACT_REPLAY_MAGIC_V1.len()
        + 2
        + 32
        + 32
        + binding_header_bytes
        + 40
        + 1
        + 1
        + 8
        + 4
        + 8
        + 8
        + 8
        + 1
        + 1
        + 32;
    let provider_references = external_providers.len().checked_mul(41);
    let options = link_options.iter().try_fold(0_usize, |sum, option| {
        sum.checked_add(1 + option.name().len() + 2 + option.value().len())
    });
    let response_metadata =
        [bootstrap_metadata, replay_metadata]
            .into_iter()
            .try_fold(0_usize, |sum, metadata| {
                sum.checked_add(2 + metadata.diagnostics_body().len() + 4)
                    .and_then(|value| {
                        value.checked_add(metadata.provider_evidence_body().map_or(0, <[u8]>::len))
                    })
                    .and_then(|value| {
                        if retains_derivation_metadata {
                            value.checked_add(2).and_then(|value| {
                                value.checked_add(
                                    metadata.derivation_evidence_body().map_or(0, <[u8]>::len),
                                )
                            })
                        } else {
                            Some(value)
                        }
                    })
            });
    fixed
        .checked_add(worker.worker_build_identity().len())
        .and_then(|value| value.checked_add(worker.llvm_build_identity().len()))
        .and_then(|value| value.checked_add(provider_references?))
        .and_then(|value| value.checked_add(options?))
        .and_then(|value| value.checked_add(response_metadata?))
        .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)
}

fn encode_response_metadata(
    bytes: &mut Vec<u8>,
    metadata: WorkerResponseReplayMetadataV1<'_>,
    retains_derivation_metadata: bool,
) -> Result<(), ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let diagnostics_len = u16::try_from(metadata.diagnostics_body().len())
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata)?;
    bytes.extend_from_slice(&diagnostics_len.to_le_bytes());
    bytes.extend_from_slice(metadata.diagnostics_body());
    let provider_len = u32::try_from(metadata.provider_evidence_body().map_or(0, <[u8]>::len))
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata)?;
    bytes.extend_from_slice(&provider_len.to_le_bytes());
    if let Some(provider) = metadata.provider_evidence_body() {
        bytes.extend_from_slice(provider);
    }
    if retains_derivation_metadata {
        let derivation_len =
            u16::try_from(metadata.derivation_evidence_body().map_or(0, <[u8]>::len))
                .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata)?;
        bytes.extend_from_slice(&derivation_len.to_le_bytes());
        if let Some(derivation) = metadata.derivation_evidence_body() {
            bytes.extend_from_slice(derivation);
        }
    }
    Ok(())
}

fn decode_response_metadata(
    reader: &mut CompactReplayReaderV1<'_>,
    has_derivation_metadata: bool,
) -> Result<CompactResponseMetadataRangesV1, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let diagnostics_len = reader.u16()? as usize;
    if diagnostics_len > MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES_V1 {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata);
    }
    let diagnostics = reader.range(diagnostics_len)?;
    let provider_len = usize::try_from(reader.u32()?)
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata)?;
    if provider_len > MAX_RESPONSE_PROVIDER_EVIDENCE_BODY_BYTES_V1 {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata);
    }
    let provider_evidence = if provider_len == 0 {
        None
    } else {
        Some(reader.range(provider_len)?)
    };
    let derivation_evidence = if has_derivation_metadata {
        let derivation_len = reader.u16()? as usize;
        if derivation_len > MAX_RESPONSE_DERIVATION_EVIDENCE_BODY_BYTES_V1 {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata);
        }
        if derivation_len == 0 {
            None
        } else {
            Some(reader.range(derivation_len)?)
        }
    } else {
        None
    };
    validate_worker_response_replay_metadata_bodies_v1(
        reader.bytes(&diagnostics)?,
        provider_evidence
            .as_ref()
            .map(|range| reader.bytes(range))
            .transpose()?,
        derivation_evidence
            .as_ref()
            .map(|range| reader.bytes(range))
            .transpose()?,
    )
    .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::ResponseMetadata)?;
    Ok(CompactResponseMetadataRangesV1 {
        diagnostics,
        provider_evidence,
        derivation_evidence,
    })
}

fn decode_input_kind(
    value: u8,
) -> Result<WorkerInputKindV1, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    match value {
        1 => Ok(WorkerInputKindV1::LlvmBitcode),
        2 => Ok(WorkerInputKindV1::AmdGpuRelocatable),
        3 => Ok(WorkerInputKindV1::LlvmTextIr),
        _ => Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers),
    }
}

fn decode_handoff_slot_v3(
    value: u8,
) -> Result<CompilerModuleHandoffSlotV3, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    match value {
        0 => Ok(CompilerModuleHandoffSlotV3::Production),
        tag @ (1 | 2) => {
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::RetiredHandoffSlot { tag })
        }
        _ => Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity),
    }
}

fn encode_content_identity(bytes: &mut Vec<u8>, identity: ContentIdentityV1) {
    bytes.extend_from_slice(identity.sha256());
    bytes.extend_from_slice(&identity.byte_len().to_le_bytes());
}

fn decode_content_identity(
    reader: &mut CompactReplayReaderV1<'_>,
    maximum: u64,
) -> Result<ContentIdentityV1, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let digest = reader.array()?;
    let byte_len = reader.u64()?;
    if byte_len == 0 || byte_len > maximum {
        return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity);
    }
    Ok(ContentIdentityV1::from_parts(digest, byte_len))
}

fn push_u16_text(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let length = u16::try_from(value.len())
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)?;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_u8_text(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let length = u8::try_from(value.len())
        .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)?;
    bytes.push(length);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn hash_domain_blob(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn try_copy_bytes(
    bytes: &[u8],
    component: &'static str,
) -> Result<Vec<u8>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let mut value = Vec::new();
    value.try_reserve_exact(bytes.len()).map_err(|_| {
        ProtectedWorkerV3CompactFinalizerReplayErrorV1::AllocationFailed { component }
    })?;
    value.extend_from_slice(bytes);
    Ok(value)
}

fn copy_text(
    value: &str,
    component: &'static str,
) -> Result<String, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let mut copied = String::new();
    copied.try_reserve_exact(value.len()).map_err(|_| {
        ProtectedWorkerV3CompactFinalizerReplayErrorV1::AllocationFailed { component }
    })?;
    copied.push_str(value);
    Ok(copied)
}

fn try_vec<T>(
    capacity: usize,
    component: &'static str,
) -> Result<Vec<T>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ProtectedWorkerV3CompactFinalizerReplayErrorV1::AllocationFailed { component }
    })?;
    Ok(values)
}

struct CompactReplayReaderV1<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> CompactReplayReaderV1<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'bytes [u8], ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        let range = self.range(length)?;
        self.bytes(&range)
    }

    fn range(
        &mut self,
        length: usize,
    ) -> Result<Range<usize>, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated)?;
        if end > self.bytes.len() {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated);
        }
        let range = self.offset..end;
        self.offset = end;
        Ok(range)
    }

    fn bytes(
        &self,
        range: &Range<usize>,
    ) -> Result<&'bytes [u8], ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        self.bytes
            .get(range.clone())
            .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(
            |_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated,
        )?))
    }

    fn u32(&mut self) -> Result<u32, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(
            |_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated,
        )?))
    }

    fn u64(&mut self) -> Result<u64, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated,
        )?))
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated)
    }

    fn text_u8(
        &mut self,
        maximum: usize,
    ) -> Result<&'bytes str, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        let length = self.u8()? as usize;
        if length == 0 || length > maximum {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length);
        }
        std::str::from_utf8(self.take(length)?)
            .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)
    }

    fn text_u16_allow_empty(
        &mut self,
        maximum: usize,
    ) -> Result<&'bytes str, ProtectedWorkerV3CompactFinalizerReplayErrorV1> {
        let length = self.u16()? as usize;
        if length > maximum {
            return Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length);
        }
        std::str::from_utf8(self.take(length)?)
            .map_err(|_| ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKER_BUILD_ID: &str = "worker-v1";
    const LLVM_BUILD_ID: &str = "llvm-v1";

    fn valid_parts() -> (WorkerMeasurementV1, WorkerExecutionLimitsV1) {
        let worker = WorkerMeasurementV1::new(
            ContentIdentityV1::from_parts([4; 32], 1),
            WORKER_BUILD_ID,
            LLVM_BUILD_ID,
        )
        .unwrap();
        let limits = WorkerExecutionLimitsV1::new(Duration::from_secs(2), 4096, 1024).unwrap();
        (worker, limits)
    }

    fn valid_bytes() -> Vec<u8> {
        let diagnostics = 0_u32.to_le_bytes();
        let metadata = WorkerResponseReplayMetadataV1::from_test_bodies(&diagnostics, None);
        let (worker, limits) = valid_parts();
        encode_compact_replay(
            FinalizedProtectedWorkerV3HsacoIdentityV1::from_test_bytes([1; 32]),
            [2; 32],
            [3; 32],
            &worker,
            limits,
            4096,
            &[],
            &[],
            metadata,
            metadata,
        )
        .unwrap()
    }

    fn valid_v2_bytes() -> Vec<u8> {
        let diagnostics = 0_u32.to_le_bytes();
        let metadata = WorkerResponseReplayMetadataV1::from_test_bodies(&diagnostics, None);
        let (worker, limits) = valid_parts();
        encode_compact_replay_v2(
            FinalizedProtectedWorkerV3HsacoIdentityV1::from_test_bytes([1; 32]),
            [2; 32],
            CompilerModuleHandoffSlotV3::Production,
            CompilerModuleHandoffTransactionIdentityV3::from_bytes([3; 32]),
            &worker,
            limits,
            4096,
            &[],
            &[],
            metadata,
            metadata,
        )
        .unwrap()
    }

    fn valid_legacy_v2_bytes() -> Vec<u8> {
        let diagnostics = 0_u32.to_le_bytes();
        let metadata = WorkerResponseReplayMetadataV1::from_test_bodies(&diagnostics, None);
        let (worker, limits) = valid_parts();
        encode_legacy_compact_replay_v2(
            FinalizedProtectedWorkerV3HsacoIdentityV1::from_test_bytes([1; 32]),
            [2; 32],
            CompilerModuleHandoffSlotV3::Production,
            CompilerModuleHandoffTransactionIdentityV3::from_bytes([3; 32]),
            &worker,
            limits,
            4096,
            &[],
            &[],
            metadata,
            metadata,
        )
        .unwrap()
    }

    fn reseal(bytes: &mut [u8]) {
        let body_end = bytes.len() - 32;
        let checksum = hash_domain_blob(COMPACT_REPLAY_CHECKSUM_DOMAIN_V1, &bytes[..body_end]);
        bytes[body_end..].copy_from_slice(&checksum);
    }

    fn reseal_v2(bytes: &mut [u8]) {
        let body_end = bytes.len() - 32;
        let checksum = hash_domain_blob(COMPACT_REPLAY_CHECKSUM_DOMAIN_V3, &bytes[..body_end]);
        bytes[body_end..].copy_from_slice(&checksum);
    }

    #[test]
    fn compact_replay_round_trips_exactly_and_remains_inert() {
        let bytes = valid_bytes();
        let replay = ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&bytes).unwrap();

        assert_eq!(replay.canonical_bytes(), bytes);
        assert_eq!(replay.try_encode_canonical().unwrap(), bytes);
        assert_eq!(replay.expected_finalization_identity(), &[1; 32]);
        assert_eq!(replay.source_evidence_identity(), &[2; 32]);
        assert_eq!(replay.binding_identity(), &[3; 32]);
        assert!(!replay.authenticates_compiler_origin());
        assert!(!replay.grants_publication_authority());
        assert!(!replay.grants_load_authority());
        assert!(!replay.grants_launch_authority());

        let decoded_again =
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded_again.identity(), replay.identity());
        assert_eq!(decoded_again.into_canonical_bytes(), bytes);
    }

    #[test]
    fn compact_v2_round_trips_transaction_axes_and_rejects_v1_cross_use() {
        let bytes = valid_v2_bytes();
        let replay = ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&bytes).unwrap();

        assert_eq!(replay.canonical_bytes(), bytes);
        assert_eq!(replay.try_encode_canonical().unwrap(), bytes);
        assert_eq!(replay.expected_finalization_identity(), &[1; 32]);
        assert_eq!(replay.source_evidence_identity(), &[2; 32]);
        assert_eq!(
            replay.handoff_slot(),
            CompilerModuleHandoffSlotV3::Production
        );
        assert_eq!(replay.transaction_identity().as_bytes(), &[3; 32]);
        assert!(replay.retains_derivation_metadata());
        assert!(!replay.authenticates_compiler_origin());
        assert!(!replay.grants_publication_authority());
        assert!(!replay.grants_load_authority());
        assert!(!replay.grants_launch_authority());
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&bytes),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Checksum)
        );
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&valid_bytes()),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Checksum)
        );
    }

    #[test]
    fn legacy_compact_v2_remains_decodable_but_identifies_missing_derivation_custody() {
        let bytes = valid_legacy_v2_bytes();
        let replay = ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&bytes).unwrap();
        assert_eq!(replay.canonical_bytes(), bytes);
        assert!(!replay.retains_derivation_metadata());
    }

    #[test]
    fn compact_v2_rejects_retired_unknown_slots_and_zero_transaction_identity() {
        let bytes = valid_v2_bytes();
        let slot_offset = COMPACT_REPLAY_MAGIC_V2.len() + 2 + 2 * 32;

        for tag in [1, 2] {
            let mut retired_slot = bytes.clone();
            retired_slot[slot_offset] = tag;
            reseal_v2(&mut retired_slot);
            assert_eq!(
                ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&retired_slot),
                Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::RetiredHandoffSlot { tag })
            );
        }

        let mut unknown_slot = bytes.clone();
        unknown_slot[slot_offset] = u8::MAX;
        reseal_v2(&mut unknown_slot);
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&unknown_slot),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity)
        );

        let mut zero_transaction = bytes;
        zero_transaction[slot_offset + 1..slot_offset + 33].fill(0);
        reseal_v2(&mut zero_transaction);
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&zero_transaction),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity)
        );
    }

    #[test]
    fn compact_replay_rejects_corruption_and_cross_version_bytes() {
        let bytes = valid_bytes();

        let oversized = vec![0; MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1 + 1];
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&oversized),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Length)
        );

        let mut corrupt_checksum = bytes.clone();
        *corrupt_checksum.last_mut().unwrap() ^= 1;
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&corrupt_checksum),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Checksum)
        );

        let mut wrong_magic = bytes.clone();
        wrong_magic[0] ^= 1;
        reseal(&mut wrong_magic);
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&wrong_magic),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Magic)
        );

        let mut wrong_version = bytes.clone();
        wrong_version[COMPACT_REPLAY_MAGIC_V1.len()..COMPACT_REPLAY_MAGIC_V1.len() + 2]
            .copy_from_slice(&2_u16.to_le_bytes());
        reseal(&mut wrong_version);
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&wrong_version),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Version)
        );

        let mut zero_identity = bytes.clone();
        zero_identity[COMPACT_REPLAY_MAGIC_V1.len() + 2..COMPACT_REPLAY_MAGIC_V1.len() + 2 + 32]
            .fill(0);
        reseal(&mut zero_identity);
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&zero_identity),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Identity)
        );

        let body_end = bytes.len() - 32;
        let mut trailing = bytes[..body_end].to_vec();
        trailing.push(0);
        trailing.extend_from_slice(&[0; 32]);
        reseal(&mut trailing);
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&trailing),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::TrailingBytes)
        );

        let mut truncated = bytes[..body_end - 1].to_vec();
        truncated.extend_from_slice(&[0; 32]);
        reseal(&mut truncated);
        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&truncated),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Truncated)
        );
    }

    #[test]
    fn compact_replay_rejects_noncanonical_duration_without_panicking() {
        let mut bytes = valid_bytes();
        let timeout_nanoseconds_offset = COMPACT_REPLAY_MAGIC_V1.len()
            + 2
            + 3 * 32
            + 40
            + 1
            + WORKER_BUILD_ID.len()
            + 1
            + LLVM_BUILD_ID.len()
            + 8;
        bytes[timeout_nanoseconds_offset..timeout_nanoseconds_offset + 4]
            .copy_from_slice(&1_000_000_000_u32.to_le_bytes());
        reseal(&mut bytes);

        assert_eq!(
            ProtectedWorkerV3CompactFinalizerReplayV1::decode_canonical(&bytes),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Limits)
        );
    }

    #[test]
    fn compact_replay_construction_rejects_unbound_and_unsorted_parts() {
        let diagnostics = 0_u32.to_le_bytes();
        let metadata = WorkerResponseReplayMetadataV1::from_test_bodies(&diagnostics, None);
        let (worker, limits) = valid_parts();
        let mismatched_provider = OwnedWorkerV3ProviderReplayPartV1 {
            kind: WorkerInputKindV1::LlvmBitcode,
            identity: ContentIdentityV1::calculate(b"different"),
            bytes: b"provider".to_vec(),
        };
        assert_eq!(
            encode_compact_replay(
                FinalizedProtectedWorkerV3HsacoIdentityV1::from_test_bytes([1; 32]),
                [2; 32],
                [3; 32],
                &worker,
                limits,
                4096,
                &[mismatched_provider],
                &[],
                metadata,
                metadata,
            ),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Providers)
        );

        let unsorted_options = [
            LinkOptionV1::new("z-option", "1").unwrap(),
            LinkOptionV1::new("a-option", "2").unwrap(),
        ];
        assert_eq!(
            encode_compact_replay(
                FinalizedProtectedWorkerV3HsacoIdentityV1::from_test_bytes([1; 32]),
                [2; 32],
                [3; 32],
                &worker,
                limits,
                4096,
                &[],
                &unsorted_options,
                metadata,
                metadata,
            ),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::Options)
        );

        assert_eq!(
            encode_compact_replay(
                FinalizedProtectedWorkerV3HsacoIdentityV1::from_test_bytes([1; 32]),
                [2; 32],
                [3; 32],
                &worker,
                limits,
                0,
                &[],
                &[],
                metadata,
                metadata,
            ),
            Err(ProtectedWorkerV3CompactFinalizerReplayErrorV1::OutputBound)
        );
    }

    #[test]
    fn compact_replay_maximum_encoding_fits_the_storage_contract() {
        const V1_FIXED_BYTES: usize = 218;
        const V2_FIXED_BYTES: usize = V1_FIXED_BYTES + 1;
        const MAX_BUILD_ID_BYTES: usize = 2 * MAX_WORKER_TOOLCHAIN_ID_BYTES;
        const MAX_PROVIDER_REFERENCE_BYTES: usize = (MAX_LINK_INPUTS - 1) * 41;
        const MAX_OPTION_BYTES: usize =
            MAX_LINK_OPTIONS * (1 + MAX_LINK_OPTION_NAME_BYTES + 2 + MAX_LINK_OPTION_VALUE_BYTES);
        const MAX_RESPONSE_BYTES: usize = 2
            * (2 + MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES_V1
                + 4
                + MAX_RESPONSE_PROVIDER_EVIDENCE_BODY_BYTES_V1);
        const MAX_DERIVATION_RESPONSE_BYTES: usize =
            2 * (2 + MAX_RESPONSE_DERIVATION_EVIDENCE_BODY_BYTES_V1);
        const V1_CODEC_MAXIMUM: usize = V1_FIXED_BYTES
            + MAX_BUILD_ID_BYTES
            + MAX_PROVIDER_REFERENCE_BYTES
            + MAX_OPTION_BYTES
            + MAX_RESPONSE_BYTES;
        const V2_CODEC_MAXIMUM: usize = V2_FIXED_BYTES
            + MAX_BUILD_ID_BYTES
            + MAX_PROVIDER_REFERENCE_BYTES
            + MAX_OPTION_BYTES
            + MAX_RESPONSE_BYTES
            + MAX_DERIVATION_RESPONSE_BYTES;

        assert_eq!(V1_CODEC_MAXIMUM, 2_195_495);
        assert_eq!(V2_CODEC_MAXIMUM, 2_206_536);
        assert_eq!(
            MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1 - V2_CODEC_MAXIMUM,
            9
        );
    }
}
