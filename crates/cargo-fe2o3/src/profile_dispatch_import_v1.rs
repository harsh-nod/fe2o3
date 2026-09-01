//! Inert, canonical custody receipt for one in-process dispatch import.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use fe2o3_semantic_import::{
    CaptureIdentityV1, CaptureUnavailableReasonV1, ContentIdentityRecordV1, ContentSchemeV1,
    ProfilerDeviceBindingV4, ProfilerDispatchBindingV4, ProfilerEnvironmentBindingV4,
    ProfilerSourceKindV4, ProfilerUnavailableFactV4, SemanticProfilerBundleV4, TruthOriginV1,
    capture_content_identity_v1, decode_capture_v1, encode_capture_v1,
    encode_profiler_bundle_v4, import_rocprofv3_csv_profiler_bundle_v4,
    import_rocprofv3_json_profiler_bundle_v4, profiler_bundle_content_identity_v4,
};
use fe2o3_semantic_trace::{KernelIrIdentityClaimV1, OpaqueIdentityV1, WaveWidthV1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const PROFILE_DISPATCH_BUNDLE_FILE_V1: &str =
    "fe2o3-semantic-profiler-bundle-v4.json";
pub(crate) const PROFILE_DISPATCH_RECEIPT_FILE_V1: &str =
    "fe2o3-profile-dispatch-import-receipt-v1.json";
pub(crate) const MAX_PROFILE_DISPATCH_RECEIPT_BYTES_V1: u64 = 2 * 1024 * 1024;

const PROFILE_DISPATCH_RECEIPT_SCHEMA_VERSION_V1: u16 = 1;
const PROFILE_DISPATCH_IMPORT_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.profile-dispatch-import.v1\0";
const PROFILE_DISPATCH_RECEIPT_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.profile-dispatch-import-receipt.v1\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DispatchImportSourceKindV1 {
    Rocprofv3KernelDispatchJson,
    Rocprofv3KernelDispatchCsv,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservedTargetFamilyV1 {
    Gfx942,
    Gfx950,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DispatchImportTargetBindingV1 {
    pub(crate) source_agent_id: u64,
    pub(crate) kfd_node: u32,
    pub(crate) stable_identity: ContentIdentityRecordV1,
    pub(crate) target_profile_record: ContentIdentityRecordV1,
    pub(crate) family: ObservedTargetFamilyV1,
    pub(crate) gfx_target_version: u64,
    pub(crate) wave_width: u16,
}

#[derive(Clone, Debug)]
pub(crate) struct DispatchImportBindingV1 {
    pub(crate) collection_authorization: CaptureIdentityV1,
    pub(crate) source_relative: String,
    pub(crate) source_artifact: ContentIdentityRecordV1,
    pub(crate) kernel_ir: ContentIdentityRecordV1,
    pub(crate) environment: ContentIdentityRecordV1,
    pub(crate) collector_tool: ContentIdentityRecordV1,
    pub(crate) collector_configuration: ContentIdentityRecordV1,
    pub(crate) targets: Vec<DispatchImportTargetBindingV1>,
    pub(crate) wave_width: WaveWidthV1,
}

#[derive(Debug)]
pub(crate) struct DispatchImportProductV1 {
    pub(crate) bundle: SemanticProfilerBundleV4,
    pub(crate) bundle_bytes: Vec<u8>,
    pub(crate) bundle_identity: ContentIdentityRecordV1,
    pub(crate) capture_bytes: Vec<u8>,
    pub(crate) capture_identity: ContentIdentityRecordV1,
    pub(crate) receipt_bytes: Vec<u8>,
    pub(crate) receipt_identity: ContentIdentityRecordV1,
}

pub(crate) fn import_dispatch_v1(
    source_kind: DispatchImportSourceKindV1,
    source: &[u8],
    binding: DispatchImportBindingV1,
) -> Result<DispatchImportProductV1, ProfileDispatchImportErrorV1> {
    validate_raw_source(source, binding.source_artifact)?;
    validate_binding(&binding)?;
    let kernel_ir_claim = KernelIrIdentityClaimV1::canonical_v7_claim(
        OpaqueIdentityV1::new(binding.kernel_ir.digest.as_bytes())
            .map_err(|_| ProfileDispatchImportErrorV1::InvalidBinding)?,
        binding.kernel_ir.canonical_len,
    )
    .map_err(|_| ProfileDispatchImportErrorV1::InvalidBinding)?;
    let environment = ProfilerEnvironmentBindingV4 {
        environment: binding.environment,
        collector_tool: binding.collector_tool,
        collector_configuration: binding.collector_configuration,
        stable_device_bindings: binding
            .targets
            .iter()
            .map(|target| ProfilerDeviceBindingV4 {
                source_agent_id: target.source_agent_id,
                stable_identity: target.stable_identity,
            })
            .collect(),
    };
    let bundle_binding = ProfilerDispatchBindingV4 {
        environment,
        kernel_ir_claim,
        artifact: None,
        source_map: None,
        wave_width: binding.wave_width,
    };
    let bundle = match source_kind {
        DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson => {
            import_rocprofv3_json_profiler_bundle_v4(source, bundle_binding)
        }
        DispatchImportSourceKindV1::Rocprofv3KernelDispatchCsv => {
            import_rocprofv3_csv_profiler_bundle_v4(source, bundle_binding)
        }
    }
    .map_err(|_| ProfileDispatchImportErrorV1::ImportRejected)?;
    validate_nonclaims(&bundle, binding.kernel_ir)?;

    let capture = bundle
        .dispatch_capture
        .as_ref()
        .ok_or(ProfileDispatchImportErrorV1::MissingCapture)?;
    let capture_bytes = encode_capture_v1(capture)
        .map_err(|_| ProfileDispatchImportErrorV1::CaptureEncode)?;
    let capture_identity = capture_content_identity_v1(&capture_bytes)
        .map_err(|_| ProfileDispatchImportErrorV1::CaptureEncode)?;
    let bundle_bytes = encode_profiler_bundle_v4(&bundle)
        .map_err(|_| ProfileDispatchImportErrorV1::BundleEncode)?;
    let bundle_identity = profiler_bundle_content_identity_v4(&bundle_bytes)
        .map_err(|_| ProfileDispatchImportErrorV1::BundleEncode)?;
    let receipt = ProfileDispatchImportReceiptV1::new(
        source_kind,
        &binding,
        &bundle,
        bundle_identity,
        capture_identity,
    )?;
    let receipt_bytes = encode_profile_dispatch_import_receipt_v1(&receipt)?;
    let receipt_identity = profile_dispatch_import_receipt_identity_v1(&receipt_bytes)?;
    Ok(DispatchImportProductV1 {
        bundle,
        bundle_bytes,
        bundle_identity,
        capture_bytes,
        capture_identity,
        receipt_bytes,
        receipt_identity,
    })
}

fn validate_raw_source(
    source: &[u8],
    identity: ContentIdentityRecordV1,
) -> Result<(), ProfileDispatchImportErrorV1> {
    let length = u64::try_from(source.len())
        .map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?;
    if source.is_empty()
        || identity.scheme != ContentSchemeV1::RawCanonicalSha256
        || identity.format_version != 1
        || identity.canonical_len != length
        || identity.digest.as_bytes() != <[u8; 32]>::from(Sha256::digest(source))
    {
        return Err(ProfileDispatchImportErrorV1::InvalidSourceIdentity);
    }
    Ok(())
}

fn validate_binding(
    binding: &DispatchImportBindingV1,
) -> Result<(), ProfileDispatchImportErrorV1> {
    if binding.kernel_ir.scheme != ContentSchemeV1::DomainSeparatedSha256
        || binding.kernel_ir.format_version != 1
        || binding.kernel_ir.canonical_len == 0
        || binding.targets.is_empty()
        || binding.targets.len() > 64
        || binding.wave_width != WaveWidthV1::Wave64
    {
        return Err(ProfileDispatchImportErrorV1::InvalidBinding);
    }
    let mut agents = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    let mut stable = BTreeSet::new();
    for target in &binding.targets {
        if target.wave_width != binding.wave_width.lanes()
            || !agents.insert(target.source_agent_id)
            || !nodes.insert(target.kfd_node)
            || !stable.insert(target.stable_identity.digest)
            || target.stable_identity.scheme != ContentSchemeV1::RawCanonicalSha256
            || target.stable_identity.format_version != 1
            || target.stable_identity.canonical_len == 0
            || target.target_profile_record.scheme
                != ContentSchemeV1::DomainSeparatedSha256
            || target.target_profile_record.format_version != 1
            || target.target_profile_record.canonical_len == 0
            || !matches!(
                (target.family, target.gfx_target_version),
                (ObservedTargetFamilyV1::Gfx942, 90_402)
                    | (ObservedTargetFamilyV1::Gfx950, 90_500)
            )
        {
            return Err(ProfileDispatchImportErrorV1::InvalidBinding);
        }
    }
    Ok(())
}

fn validate_nonclaims(
    bundle: &SemanticProfilerBundleV4,
    kernel_ir: ContentIdentityRecordV1,
) -> Result<(), ProfileDispatchImportErrorV1> {
    if bundle.devices.iter().any(|device| {
        device.stable_identity.origin != TruthOriginV1::Declared
            || device.source_bound_origin != TruthOriginV1::Observed
    }) || !bundle
        .unavailable
        .contains(&ProfilerUnavailableFactV4::SourceIrIsaCorrelation)
    {
        return Err(ProfileDispatchImportErrorV1::AuthorityElevation);
    }
    let capture = bundle
        .dispatch_capture
        .as_ref()
        .ok_or(ProfileDispatchImportErrorV1::MissingCapture)?;
    for dispatch in &capture.dispatches {
        if dispatch.kernel_ir.origin != TruthOriginV1::Declared
            || dispatch.kernel_ir.digest.as_bytes() != kernel_ir.digest.as_bytes()
            || dispatch.kernel_ir.canonical_len != kernel_ir.canonical_len
            || dispatch.artifact.origin != TruthOriginV1::Unavailable
            || dispatch.artifact.value.is_some()
            || dispatch.artifact.unavailable_reason
                != Some(CaptureUnavailableReasonV1::NotProvided)
            || dispatch.source_map.origin != TruthOriginV1::Unavailable
            || dispatch.source_map.value.is_some()
            || dispatch.source_map.unavailable_reason
                != Some(CaptureUnavailableReasonV1::NotProvided)
        {
            return Err(ProfileDispatchImportErrorV1::AuthorityElevation);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportAuthorityNonclaimsV1 {
    compiler_authority: bool,
    runtime_authority: bool,
    executed_artifact_identity: bool,
    source_map_identity: bool,
    kernel_symbol_association: bool,
    source_ir_isa_correlation: bool,
    exact_target_xnack_observation: bool,
    decoded_att_events: bool,
    performance_authority: bool,
}

impl ImportAuthorityNonclaimsV1 {
    const fn none() -> Self {
        Self {
            compiler_authority: false,
            runtime_authority: false,
            executed_artifact_identity: false,
            source_map_identity: false,
            kernel_symbol_association: false,
            source_ir_isa_correlation: false,
            exact_target_xnack_observation: false,
            decoded_att_events: false,
            performance_authority: false,
        }
    }

    const fn is_none(self) -> bool {
        !self.compiler_authority
            && !self.runtime_authority
            && !self.executed_artifact_identity
            && !self.source_map_identity
            && !self.kernel_symbol_association
            && !self.source_ir_isa_correlation
            && !self.exact_target_xnack_observation
            && !self.decoded_att_events
            && !self.performance_authority
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UnavailableIdentityV1 {
    origin: TruthOriginV1,
    reason: CaptureUnavailableReasonV1,
}

impl UnavailableIdentityV1 {
    const fn not_provided() -> Self {
        Self {
            origin: TruthOriginV1::Unavailable,
            reason: CaptureUnavailableReasonV1::NotProvided,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReceiptTargetBindingV1 {
    source_agent_id: u64,
    kfd_node: u32,
    stable_identity: ContentIdentityRecordV1,
    target_profile_record: ContentIdentityRecordV1,
    family: ObservedTargetFamilyV1,
    gfx_target_version: u64,
    wave_width: u16,
    exact_xnack_origin: TruthOriginV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProfileDispatchImportReceiptV1 {
    schema_version: u16,
    authority: ImportAuthorityNonclaimsV1,
    source_kind: DispatchImportSourceKindV1,
    collection_authorization: CaptureIdentityV1,
    source_relative: String,
    source_artifact: ContentIdentityRecordV1,
    source_evidence: ContentIdentityRecordV1,
    normalized_projection: ContentIdentityRecordV1,
    import_identity: CaptureIdentityV1,
    kernel_ir: ContentIdentityRecordV1,
    environment: ContentIdentityRecordV1,
    collector_tool: ContentIdentityRecordV1,
    collector_configuration: ContentIdentityRecordV1,
    bundle: ContentIdentityRecordV1,
    capture: ContentIdentityRecordV1,
    run_identity: CaptureIdentityV1,
    run_count: u64,
    device_count: u64,
    dispatch_count: u64,
    targets: Vec<ReceiptTargetBindingV1>,
    dispatch_identities: Vec<CaptureIdentityV1>,
    artifact: UnavailableIdentityV1,
    source_map: UnavailableIdentityV1,
    characteristic_correlation: UnavailableIdentityV1,
}

impl ProfileDispatchImportReceiptV1 {
    fn new(
        source_kind: DispatchImportSourceKindV1,
        binding: &DispatchImportBindingV1,
        bundle: &SemanticProfilerBundleV4,
        bundle_identity: ContentIdentityRecordV1,
        capture_identity: ContentIdentityRecordV1,
    ) -> Result<Self, ProfileDispatchImportErrorV1> {
        let capture = bundle
            .dispatch_capture
            .as_ref()
            .ok_or(ProfileDispatchImportErrorV1::MissingCapture)?;
        let source_evidence = bundle
            .source
            .value
            .ok_or(ProfileDispatchImportErrorV1::InvalidBundle)?;
        let normalized_projection = bundle
            .normalized_projection
            .value
            .ok_or(ProfileDispatchImportErrorV1::InvalidBundle)?;
        let mut targets = Vec::new();
        targets
            .try_reserve(bundle.devices.len())
            .map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?;
        for device in &bundle.devices {
            let stable = device
                .stable_identity
                .value
                .ok_or(ProfileDispatchImportErrorV1::InvalidBundle)?;
            let target = binding
                .targets
                .iter()
                .find(|target| target.stable_identity == stable)
                .ok_or(ProfileDispatchImportErrorV1::InvalidBundle)?;
            targets.push(ReceiptTargetBindingV1 {
                source_agent_id: target.source_agent_id,
                kfd_node: target.kfd_node,
                stable_identity: target.stable_identity,
                target_profile_record: target.target_profile_record,
                family: target.family,
                gfx_target_version: target.gfx_target_version,
                wave_width: target.wave_width,
                exact_xnack_origin: TruthOriginV1::Unavailable,
            });
        }
        let run_count = u64::try_from(capture.runs.len())
            .map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?;
        let device_count = u64::try_from(capture.devices.len())
            .map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?;
        let dispatch_count = u64::try_from(capture.dispatches.len())
            .map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?;
        let mut receipt = Self {
            schema_version: PROFILE_DISPATCH_RECEIPT_SCHEMA_VERSION_V1,
            authority: ImportAuthorityNonclaimsV1::none(),
            source_kind,
            collection_authorization: binding.collection_authorization,
            source_relative: binding.source_relative.clone(),
            source_artifact: binding.source_artifact,
            source_evidence,
            normalized_projection,
            import_identity: binding.collection_authorization,
            kernel_ir: binding.kernel_ir,
            environment: binding.environment,
            collector_tool: binding.collector_tool,
            collector_configuration: binding.collector_configuration,
            bundle: bundle_identity,
            capture: capture_identity,
            run_identity: bundle.run_identity,
            run_count,
            device_count,
            dispatch_count,
            targets,
            dispatch_identities: capture
                .dispatches
                .iter()
                .map(|dispatch| dispatch.identity)
                .collect(),
            artifact: UnavailableIdentityV1::not_provided(),
            source_map: UnavailableIdentityV1::not_provided(),
            characteristic_correlation: UnavailableIdentityV1 {
                origin: TruthOriginV1::Unavailable,
                reason: CaptureUnavailableReasonV1::NotRepresented,
            },
        };
        receipt.import_identity = receipt.derive_import_identity()?;
        receipt.validate()?;
        Ok(receipt)
    }

    fn validate(&self) -> Result<(), ProfileDispatchImportErrorV1> {
        if self.schema_version != PROFILE_DISPATCH_RECEIPT_SCHEMA_VERSION_V1
            || !self.authority.is_none()
            || !valid_relative_source(&self.source_relative)
            || self.targets.is_empty()
            || self.targets.len() > 64
            || self.dispatch_identities.is_empty()
            || self.dispatch_identities.len() > 16_384
            || self.run_count != 1
            || self.device_count != self.targets.len() as u64
            || self.dispatch_count != self.dispatch_identities.len() as u64
            || self.artifact != UnavailableIdentityV1::not_provided()
            || self.source_map != UnavailableIdentityV1::not_provided()
            || self.characteristic_correlation.origin != TruthOriginV1::Unavailable
            || self.characteristic_correlation.reason
                != CaptureUnavailableReasonV1::NotRepresented
            || self.import_identity != self.derive_import_identity()?
        {
            return Err(ProfileDispatchImportErrorV1::InvalidReceipt);
        }
        let mut agents = BTreeSet::new();
        let mut nodes = BTreeSet::new();
        let mut stable = BTreeSet::new();
        let mut dispatches = BTreeSet::new();
        for target in &self.targets {
            if target.wave_width != 64
                || target.exact_xnack_origin != TruthOriginV1::Unavailable
                || !agents.insert(target.source_agent_id)
                || !nodes.insert(target.kfd_node)
                || !stable.insert(target.stable_identity.digest)
            {
                return Err(ProfileDispatchImportErrorV1::InvalidReceipt);
            }
        }
        if self
            .dispatch_identities
            .iter()
            .any(|identity| !dispatches.insert(*identity))
        {
            return Err(ProfileDispatchImportErrorV1::InvalidReceipt);
        }
        Ok(())
    }

    fn derive_import_identity(&self) -> Result<CaptureIdentityV1, ProfileDispatchImportErrorV1> {
        let mut digest = Sha256::new();
        digest.update(PROFILE_DISPATCH_IMPORT_IDENTITY_DOMAIN_V1);
        digest.update([match self.source_kind {
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson => 1,
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchCsv => 2,
        }]);
        digest.update(self.collection_authorization.as_bytes());
        digest.update((self.source_relative.len() as u64).to_le_bytes());
        digest.update(self.source_relative.as_bytes());
        for identity in [
            self.source_artifact,
            self.source_evidence,
            self.normalized_projection,
            self.kernel_ir,
            self.environment,
            self.collector_tool,
            self.collector_configuration,
        ] {
            update_content_identity(&mut digest, identity);
        }
        for target in &self.targets {
            digest.update(target.source_agent_id.to_le_bytes());
            digest.update(target.kfd_node.to_le_bytes());
            update_content_identity(&mut digest, target.stable_identity);
            update_content_identity(&mut digest, target.target_profile_record);
            digest.update(target.gfx_target_version.to_le_bytes());
            digest.update(target.wave_width.to_le_bytes());
        }
        CaptureIdentityV1::new(digest.finalize().into())
            .map_err(|_| ProfileDispatchImportErrorV1::InvalidReceipt)
    }
}

fn valid_relative_source(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4_096
        && !path.contains("//")
        && !std::path::Path::new(path).is_absolute()
        && std::path::Path::new(path)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn update_content_identity(digest: &mut Sha256, identity: ContentIdentityRecordV1) {
    digest.update([match identity.scheme {
        ContentSchemeV1::RawCanonicalSha256 => 1,
        ContentSchemeV1::DomainSeparatedSha256 => 2,
    }]);
    digest.update(identity.format_version.to_le_bytes());
    digest.update(identity.digest.as_bytes());
    digest.update(identity.canonical_len.to_le_bytes());
}

fn encode_profile_dispatch_import_receipt_v1(
    receipt: &ProfileDispatchImportReceiptV1,
) -> Result<Vec<u8>, ProfileDispatchImportErrorV1> {
    receipt.validate()?;
    let bytes = serde_json::to_vec(receipt)
        .map_err(|_| ProfileDispatchImportErrorV1::ReceiptEncode)?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?
            > MAX_PROFILE_DISPATCH_RECEIPT_BYTES_V1
    {
        return Err(ProfileDispatchImportErrorV1::ReceiptTooLarge);
    }
    Ok(bytes)
}

fn decode_profile_dispatch_import_receipt_v1(
    bytes: &[u8],
) -> Result<ProfileDispatchImportReceiptV1, ProfileDispatchImportErrorV1> {
    if bytes.is_empty()
        || u64::try_from(bytes.len()).map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?
            > MAX_PROFILE_DISPATCH_RECEIPT_BYTES_V1
    {
        return Err(ProfileDispatchImportErrorV1::ReceiptTooLarge);
    }
    let receipt: ProfileDispatchImportReceiptV1 = serde_json::from_slice(bytes)
        .map_err(|_| ProfileDispatchImportErrorV1::ReceiptDecode)?;
    receipt.validate()?;
    if serde_json::to_vec(&receipt).map_err(|_| ProfileDispatchImportErrorV1::ReceiptEncode)?
        != bytes
    {
        return Err(ProfileDispatchImportErrorV1::NonCanonicalReceipt);
    }
    Ok(receipt)
}

fn profile_dispatch_import_receipt_identity_v1(
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, ProfileDispatchImportErrorV1> {
    let _ = decode_profile_dispatch_import_receipt_v1(bytes)?;
    let mut digest = Sha256::new();
    digest.update(PROFILE_DISPATCH_RECEIPT_IDENTITY_DOMAIN_V1);
    digest.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PROFILE_DISPATCH_RECEIPT_SCHEMA_VERSION_V1,
        digest: CaptureIdentityV1::new(digest.finalize().into())
            .map_err(|_| ProfileDispatchImportErrorV1::InvalidReceipt)?,
        canonical_len: u64::try_from(bytes.len())
            .map_err(|_| ProfileDispatchImportErrorV1::SizeOverflow)?,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProfileDispatchImportErrorV1 {
    InvalidSourceIdentity,
    InvalidBinding,
    ImportRejected,
    MissingCapture,
    InvalidBundle,
    AuthorityElevation,
    CaptureEncode,
    BundleEncode,
    InvalidReceipt,
    ReceiptEncode,
    ReceiptDecode,
    ReceiptTooLarge,
    NonCanonicalReceipt,
    SizeOverflow,
}

impl fmt::Display for ProfileDispatchImportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "profile dispatch import rejected: {self:?}")
    }
}

impl Error for ProfileDispatchImportErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_semantic_import::{decode_profiler_bundle_v4, MAX_PROFILER_SOURCE_BYTES_V4};

    fn content(byte: u8, len: u64, scheme: ContentSchemeV1) -> ContentIdentityRecordV1 {
        ContentIdentityRecordV1 {
            scheme,
            format_version: 1,
            digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
            canonical_len: len,
        }
    }

    fn source_identity(source: &[u8]) -> ContentIdentityRecordV1 {
        ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::RawCanonicalSha256,
            format_version: 1,
            digest: CaptureIdentityV1::new(Sha256::digest(source).into()).unwrap(),
            canonical_len: source.len() as u64,
        }
    }

    fn source() -> &'static [u8] {
        br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[{"start_timestamp":100,"end_timestamp":180,"dispatch_info":{"agent_id":{"handle":7},"workgroup_size":{"x":64,"y":1,"z":1},"grid_size":{"x":256,"y":1,"z":1}}}]}}]}"#
    }

    fn binding(source: &[u8]) -> DispatchImportBindingV1 {
        DispatchImportBindingV1 {
            collection_authorization: CaptureIdentityV1::new([9; 32]).unwrap(),
            source_relative: "collector_results/dispatch.json".to_owned(),
            source_artifact: source_identity(source),
            kernel_ir: content(1, 97, ContentSchemeV1::DomainSeparatedSha256),
            environment: content(2, 80, ContentSchemeV1::RawCanonicalSha256),
            collector_tool: content(3, 90, ContentSchemeV1::RawCanonicalSha256),
            collector_configuration: content(4, 100, ContentSchemeV1::RawCanonicalSha256),
            targets: vec![DispatchImportTargetBindingV1 {
                source_agent_id: 7,
                kfd_node: 7,
                stable_identity: content(5, 110, ContentSchemeV1::RawCanonicalSha256),
                target_profile_record: content(
                    6,
                    120,
                    ContentSchemeV1::DomainSeparatedSha256,
                ),
                family: ObservedTargetFamilyV1::Gfx942,
                gfx_target_version: 90_402,
                wave_width: 64,
            }],
            wave_width: WaveWidthV1::Wave64,
        }
    }

    #[test]
    fn exact_import_codec_preserves_declared_and_unavailable_boundaries() {
        let product = import_dispatch_v1(
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson,
            source(),
            binding(source()),
        )
        .unwrap();
        let decoded = decode_profiler_bundle_v4(&product.bundle_bytes).unwrap();
        let capture = decode_capture_v1(&product.capture_bytes).unwrap();
        assert_eq!(decoded, product.bundle);
        assert_eq!(capture, *decoded.dispatch_capture.as_ref().unwrap());
        assert_eq!(capture.dispatches[0].kernel_ir.origin, TruthOriginV1::Declared);
        assert_eq!(capture.dispatches[0].artifact.origin, TruthOriginV1::Unavailable);
        assert_eq!(capture.dispatches[0].source_map.origin, TruthOriginV1::Unavailable);
        let receipt = decode_profile_dispatch_import_receipt_v1(&product.receipt_bytes).unwrap();
        assert!(receipt.authority.is_none());
        assert_eq!(
            profile_dispatch_import_receipt_identity_v1(&product.receipt_bytes).unwrap(),
            product.receipt_identity
        );
    }

    #[test]
    fn source_and_binding_substitution_fail_closed() {
        let mut wrong_source = binding(source());
        wrong_source.source_artifact.digest = CaptureIdentityV1::new([44; 32]).unwrap();
        assert_eq!(
            import_dispatch_v1(
                DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson,
                source(),
                wrong_source,
            )
            .unwrap_err(),
            ProfileDispatchImportErrorV1::InvalidSourceIdentity
        );

        let remapped_source = String::from_utf8(source().to_vec())
            .unwrap()
            .replace("\"handle\":7", "\"handle\":8")
            .into_bytes();
        let mut remapped = binding(&remapped_source);
        remapped.targets[0].source_agent_id = 8;
        let product = import_dispatch_v1(
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson,
            &remapped_source,
            remapped,
        )
        .unwrap();
        let receipt = decode_profile_dispatch_import_receipt_v1(&product.receipt_bytes).unwrap();
        assert_eq!(receipt.targets[0].source_agent_id, 8);
        assert_eq!(receipt.targets[0].kfd_node, 7);

        let mut duplicate_node = binding(source());
        let mut second = duplicate_node.targets[0];
        second.source_agent_id = 8;
        second.stable_identity.digest = CaptureIdentityV1::new([8; 32]).unwrap();
        duplicate_node.targets.push(second);
        assert_eq!(
            import_dispatch_v1(
                DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson,
                source(),
                duplicate_node,
            )
            .unwrap_err(),
            ProfileDispatchImportErrorV1::InvalidBinding
        );
    }

    #[test]
    fn receipt_rejects_resealed_authority_and_identity_substitution() {
        let product = import_dispatch_v1(
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson,
            source(),
            binding(source()),
        )
        .unwrap();
        let mut receipt = decode_profile_dispatch_import_receipt_v1(&product.receipt_bytes).unwrap();
        receipt.authority.runtime_authority = true;
        assert_eq!(
            decode_profile_dispatch_import_receipt_v1(&serde_json::to_vec(&receipt).unwrap())
                .unwrap_err(),
            ProfileDispatchImportErrorV1::InvalidReceipt
        );

        let mut receipt = decode_profile_dispatch_import_receipt_v1(&product.receipt_bytes).unwrap();
        receipt.source_artifact.digest = CaptureIdentityV1::new([45; 32]).unwrap();
        assert_eq!(
            decode_profile_dispatch_import_receipt_v1(&serde_json::to_vec(&receipt).unwrap())
                .unwrap_err(),
            ProfileDispatchImportErrorV1::InvalidReceipt
        );
    }

    #[test]
    fn malformed_noncanonical_and_oversized_receipts_are_rejected() {
        let product = import_dispatch_v1(
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson,
            source(),
            binding(source()),
        )
        .unwrap();
        let mut whitespace = product.receipt_bytes.clone();
        whitespace.push(b'\n');
        assert_eq!(
            decode_profile_dispatch_import_receipt_v1(&whitespace).unwrap_err(),
            ProfileDispatchImportErrorV1::NonCanonicalReceipt
        );
        assert_eq!(
            decode_profile_dispatch_import_receipt_v1(b"{}").unwrap_err(),
            ProfileDispatchImportErrorV1::ReceiptDecode
        );
        let oversized = vec![b'x'; MAX_PROFILE_DISPATCH_RECEIPT_BYTES_V1 as usize + 1];
        assert_eq!(
            decode_profile_dispatch_import_receipt_v1(&oversized).unwrap_err(),
            ProfileDispatchImportErrorV1::ReceiptTooLarge
        );
        assert!(MAX_PROFILE_DISPATCH_RECEIPT_BYTES_V1 < MAX_PROFILER_SOURCE_BYTES_V4);
    }
}
