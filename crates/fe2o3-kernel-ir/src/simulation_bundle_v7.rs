//! Self-contained authority-free simulation bundle with an exact KIR V12 body.

use std::{error::Error, fmt, str};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV2, MAX_MODULE_BYTES_V1,
    MAX_SIMULATION_DEBUG_MAP_BYTES_V1, MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3,
    MAX_SIMULATION_STORAGE_MAP_BYTES_V3, MAX_SIMULATION_STORAGE_MAP_BYTES_V4, MAX_TEXT_BYTES_V1,
    SemanticKernelStorageV1, SemanticKernelStorageV2, SemanticVariableStorageV1,
    SimulationSourceLineageV1, VerifiedCanonicalKernelIrErrorV12, VerifiedCanonicalKernelIrV12,
};

pub const SIMULATION_BUNDLE_VERSION_V7: u16 = 7;
pub const SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7: u16 = 12;
pub const SEMANTIC_STORAGE_MAP_SCHEMA_V7: &str = "fe2o3-semantic-storage-map-v7";
pub const SEMANTIC_AGGREGATE_STORAGE_MAP_SCHEMA_V7: &str =
    "fe2o3-semantic-aggregate-storage-map-v7";
pub const MAX_SIMULATION_BUNDLE_BYTES_V7: usize = HEADER_BYTES_V7
    + MAX_TEXT_BYTES_V1
    + MAX_MODULE_BYTES_V1
    + MAX_SIMULATION_DEBUG_MAP_BYTES_V1
    + MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3
    + MAX_SIMULATION_STORAGE_MAP_BYTES_V3
    + MAX_SIMULATION_STORAGE_MAP_BYTES_V4;

const MAGIC_V7: &[u8; 8] = b"F2SIMB07";
const HEADER_BYTES_V7: usize = 416;
const PRODUCTION_KIR_VERSION_V12: u16 = 12;
const SUBJECT_IDENTITY_DOMAIN_V7: &[u8] = b"FE2O3/SIMULATION-BUNDLE-SUBJECT/V7\0";
const BUNDLE_IDENTITY_DOMAIN_V7: &[u8] = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V7\0";
const SOURCE_MAP_IDENTITY_DOMAIN_V7: &[u8] = b"FE2O3/SIMULATION-SOURCE-MAP/V7\0";
const SEMANTIC_MIR_IDENTITY_DOMAIN_V7: &[u8] = b"FE2O3/SIMULATION-SEMANTIC-MIR/V7\0";
const STORAGE_MAP_IDENTITY_DOMAIN_V7: &[u8] = b"FE2O3/SIMULATION-STORAGE-MAP/V7\0";
const AGGREGATE_MAP_IDENTITY_DOMAIN_V7: &[u8] = b"FE2O3/SIMULATION-AGGREGATE-STORAGE-MAP/V7\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationBundleIdentityV7([u8; 32]);

impl SimulationBundleIdentityV7 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact versioned identity of the producer-owned canonical KIR rederived from V12.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationProductionKirIdentityV7 {
    version: u16,
    digest: [u8; 32],
    canonical_length: u64,
}

impl SimulationProductionKirIdentityV7 {
    pub fn new(
        version: u16,
        digest: [u8; 32],
        canonical_length: u64,
    ) -> Result<Self, SimulationBundleErrorV7> {
        if version != PRODUCTION_KIR_VERSION_V12
            || digest == [0; 32]
            || canonical_length == 0
            || canonical_length > MAX_MODULE_BYTES_V1 as u64
        {
            return Err(SimulationBundleErrorV7::InvalidProductionKirIdentity);
        }
        Ok(Self {
            version,
            digest,
            canonical_length,
        })
    }

    pub const fn version(self) -> u16 {
        self.version
    }
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
    pub const fn canonical_length(self) -> u64 {
        self.canonical_length
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum SemanticStorageMapSchemaV7 {
    #[serde(rename = "fe2o3-semantic-storage-map-v7")]
    V7,
}

/// Version-neutral semantic/source storage correspondence for a V7 subject.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticStorageMapV7 {
    schema: SemanticStorageMapSchemaV7,
    #[serde(with = "hex_identity_v7")]
    bundle_subject_identity: [u8; 32],
    semantic_mir_version: u16,
    #[serde(with = "hex_identity_v7")]
    semantic_mir_sha256: [u8; 32],
    semantic_mir_bytes: u64,
    #[serde(with = "hex_identity_v7")]
    target_layout_identity: [u8; 32],
    canonical_kir_version: u16,
    #[serde(with = "hex_identity_v7")]
    canonical_kir_sha256: [u8; 32],
    canonical_kir_bytes: u64,
    kernels: Vec<SemanticKernelStorageV1>,
    variables: Vec<SemanticVariableStorageV1>,
}

impl SemanticStorageMapV7 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bundle_subject_identity: [u8; 32],
        semantic_mir_version: u16,
        semantic_mir_sha256: [u8; 32],
        semantic_mir_bytes: u64,
        target_layout_identity: [u8; 32],
        canonical_kir_sha256: [u8; 32],
        canonical_kir_bytes: u64,
        kernels: Vec<SemanticKernelStorageV1>,
        variables: Vec<SemanticVariableStorageV1>,
    ) -> Result<Self, SimulationBundleErrorV7> {
        let map = Self {
            schema: SemanticStorageMapSchemaV7::V7,
            bundle_subject_identity,
            semantic_mir_version,
            semantic_mir_sha256,
            semantic_mir_bytes,
            target_layout_identity,
            canonical_kir_version: SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7,
            canonical_kir_sha256,
            canonical_kir_bytes,
            kernels,
            variables,
        };
        map.validate()?;
        Ok(map)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, SimulationBundleErrorV7> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V3 {
            return Err(SimulationBundleErrorV7::InvalidStorageMapLength);
        }
        let map: Self = serde_json::from_slice(bytes)
            .map_err(|_| SimulationBundleErrorV7::InvalidStorageMap)?;
        map.validate()?;
        if serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV7::Encoding)? != bytes {
            return Err(SimulationBundleErrorV7::NonCanonicalStorageMap);
        }
        Ok(map)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SimulationBundleErrorV7> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| SimulationBundleErrorV7::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V3 {
            return Err(SimulationBundleErrorV7::InvalidStorageMapLength);
        }
        Ok(bytes)
    }

    pub const fn bundle_subject_identity(&self) -> &[u8; 32] {
        &self.bundle_subject_identity
    }
    pub const fn semantic_mir_version(&self) -> u16 {
        self.semantic_mir_version
    }
    pub const fn semantic_mir_sha256(&self) -> &[u8; 32] {
        &self.semantic_mir_sha256
    }
    pub const fn semantic_mir_bytes(&self) -> u64 {
        self.semantic_mir_bytes
    }
    pub const fn target_layout_identity(&self) -> &[u8; 32] {
        &self.target_layout_identity
    }
    pub const fn canonical_kir_version(&self) -> u16 {
        self.canonical_kir_version
    }
    pub const fn canonical_kir_sha256(&self) -> &[u8; 32] {
        &self.canonical_kir_sha256
    }
    pub const fn canonical_kir_bytes(&self) -> u64 {
        self.canonical_kir_bytes
    }
    pub fn kernels(&self) -> &[SemanticKernelStorageV1] {
        &self.kernels
    }
    pub fn variables(&self) -> &[SemanticVariableStorageV1] {
        &self.variables
    }

    fn validate(&self) -> Result<(), SimulationBundleErrorV7> {
        if self.bundle_subject_identity == [0; 32]
            || self.semantic_mir_version == 0
            || self.semantic_mir_sha256 == [0; 32]
            || self.semantic_mir_bytes == 0
            || self.semantic_mir_bytes > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3 as u64
            || self.target_layout_identity == [0; 32]
            || self.canonical_kir_version != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7
            || self.canonical_kir_sha256 == [0; 32]
            || self.canonical_kir_bytes == 0
            || self.kernels.is_empty()
        {
            return Err(SimulationBundleErrorV7::InvalidStorageMap);
        }
        let normalized = crate::SemanticStorageMapV1::new(
            self.bundle_subject_identity,
            self.bundle_subject_identity,
            self.semantic_mir_version,
            self.semantic_mir_sha256,
            self.semantic_mir_bytes,
            self.target_layout_identity,
            self.canonical_kir_sha256,
            self.canonical_kir_bytes,
            self.kernels.clone(),
            self.variables.clone(),
        )
        .map_err(|_| SimulationBundleErrorV7::InvalidStorageMap)?;
        if normalized.kernels() != self.kernels || normalized.variables() != self.variables {
            return Err(SimulationBundleErrorV7::InvalidStorageMap);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum SemanticAggregateStorageMapSchemaV7 {
    #[serde(rename = "fe2o3-semantic-aggregate-storage-map-v7")]
    V7,
}

/// Aggregate component and address-free simulator-kernarg correspondence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAggregateStorageMapV7 {
    schema: SemanticAggregateStorageMapSchemaV7,
    #[serde(with = "hex_identity_v7")]
    bundle_subject_identity: [u8; 32],
    canonical_kir_version: u16,
    #[serde(with = "hex_identity_v7")]
    canonical_kir_sha256: [u8; 32],
    canonical_kir_bytes: u64,
    kernels: Vec<SemanticKernelStorageV2>,
}

impl SemanticAggregateStorageMapV7 {
    pub fn new(
        bundle_subject_identity: [u8; 32],
        canonical_kir_sha256: [u8; 32],
        canonical_kir_bytes: u64,
        kernels: Vec<SemanticKernelStorageV2>,
    ) -> Result<Self, SimulationBundleErrorV7> {
        let map = Self {
            schema: SemanticAggregateStorageMapSchemaV7::V7,
            bundle_subject_identity,
            canonical_kir_version: SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7,
            canonical_kir_sha256,
            canonical_kir_bytes,
            kernels,
        };
        map.validate()?;
        Ok(map)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, SimulationBundleErrorV7> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V4 {
            return Err(SimulationBundleErrorV7::InvalidAggregateStorageMapLength);
        }
        let map: Self = serde_json::from_slice(bytes)
            .map_err(|_| SimulationBundleErrorV7::InvalidAggregateStorageMap)?;
        map.validate()?;
        if serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV7::Encoding)? != bytes {
            return Err(SimulationBundleErrorV7::NonCanonicalAggregateStorageMap);
        }
        Ok(map)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SimulationBundleErrorV7> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| SimulationBundleErrorV7::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V4 {
            return Err(SimulationBundleErrorV7::InvalidAggregateStorageMapLength);
        }
        Ok(bytes)
    }

    pub const fn bundle_subject_identity(&self) -> &[u8; 32] {
        &self.bundle_subject_identity
    }
    pub const fn canonical_kir_version(&self) -> u16 {
        self.canonical_kir_version
    }
    pub const fn canonical_kir_sha256(&self) -> &[u8; 32] {
        &self.canonical_kir_sha256
    }
    pub const fn canonical_kir_bytes(&self) -> u64 {
        self.canonical_kir_bytes
    }
    pub fn kernels(&self) -> &[SemanticKernelStorageV2] {
        &self.kernels
    }

    fn validate(&self) -> Result<(), SimulationBundleErrorV7> {
        if self.bundle_subject_identity == [0; 32]
            || self.canonical_kir_version != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7
            || self.canonical_kir_sha256 == [0; 32]
            || self.canonical_kir_bytes == 0
            || self.kernels.is_empty()
        {
            return Err(SimulationBundleErrorV7::InvalidAggregateStorageMap);
        }
        let normalized =
            crate::SemanticStorageMapV2::new(self.bundle_subject_identity, self.kernels.clone())
                .map_err(|_| SimulationBundleErrorV7::InvalidAggregateStorageMap)?;
        if normalized.kernels() != self.kernels {
            return Err(SimulationBundleErrorV7::InvalidAggregateStorageMap);
        }
        Ok(())
    }
}

/// Map-independent V7 metadata awaiting compiler source/storage sections.
#[must_use = "dropping the prepared owner abandons simulation bundle finalization"]
pub struct PreparedSimulationBundleV7 {
    source_lineage: SimulationSourceLineageV1,
    production_kir_identity: SimulationProductionKirIdentityV7,
    final_graph_epoch: u64,
    target: String,
    canonical_kir_v12: Vec<u8>,
    canonical_kir_v12_digest: [u8; 32],
    canonical_kir_v12_length: u64,
    kernel_abi_identity: [u8; 32],
    kernel_count: u32,
    subject_identity: [u8; 32],
}

impl PreparedSimulationBundleV7 {
    pub fn new(
        source_lineage: SimulationSourceLineageV1,
        production_kir_identity: SimulationProductionKirIdentityV7,
        final_graph_epoch: u64,
        target: &str,
        canonical_kir_v12: VerifiedCanonicalKernelIrV12,
    ) -> Result<Self, SimulationBundleErrorV7> {
        if final_graph_epoch == 0 {
            return Err(SimulationBundleErrorV7::InvalidFinalGraphEpoch);
        }
        validate_target_v7(target)?;
        canonical_kir_v12.revalidate()?;
        let canonical_kir_v12_digest = *canonical_kir_v12.identity().digest();
        let canonical_kir_v12_length = canonical_kir_v12.identity().canonical_length();
        let (_, module) = VerifiedCanonicalKernelIrV12::from_canonical_bytes_with_module(
            copy_bytes_v7(canonical_kir_v12.canonical_bytes())?,
        )?;
        validate_production_bridge_v7(&module, production_kir_identity)?;
        let kernel_count = u32::try_from(module.kernels.len())
            .map_err(|_| SimulationBundleErrorV7::KernelCountOverflow)?;
        let kernel_abi_identity = kernel_abi_identity_v7(&module)?;
        let subject_identity = subject_identity_v7(
            source_lineage,
            production_kir_identity,
            final_graph_epoch,
            target,
            canonical_kir_v12_digest,
            canonical_kir_v12_length,
            kernel_abi_identity,
            kernel_count,
        );
        Ok(Self {
            source_lineage,
            production_kir_identity,
            final_graph_epoch,
            target: target.to_owned(),
            canonical_kir_v12: canonical_kir_v12.into_canonical_bytes(),
            canonical_kir_v12_digest,
            canonical_kir_v12_length,
            kernel_abi_identity,
            kernel_count,
            subject_identity,
        })
    }

    pub const fn subject_identity(&self) -> &[u8; 32] {
        &self.subject_identity
    }
    pub const fn canonical_kir_v12_digest(&self) -> &[u8; 32] {
        &self.canonical_kir_v12_digest
    }
    pub const fn canonical_kir_v12_length(&self) -> u64 {
        self.canonical_kir_v12_length
    }
    pub const fn final_graph_epoch(&self) -> u64 {
        self.final_graph_epoch
    }
    pub fn debug_source_map_binding(&self) -> DebugSourceMapBindingV1 {
        DebugSourceMapBindingV1::new(
            self.subject_identity,
            self.canonical_kir_v12_digest,
            self.canonical_kir_v12_length,
        )
        .expect("verified V7 identities form a valid source-map binding")
    }

    pub fn finalize(
        self,
        source_map: DebugSourceMapDocumentV2,
        semantic_mir: Vec<u8>,
        storage_map: SemanticStorageMapV7,
        aggregate_storage_map: SemanticAggregateStorageMapV7,
    ) -> Result<VerifiedSimulationBundleV7, SimulationBundleErrorV7> {
        if source_map.binding() != self.debug_source_map_binding() {
            return Err(SimulationBundleErrorV7::SourceMapBindingMismatch);
        }
        if semantic_mir.is_empty() || semantic_mir.len() > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3 {
            return Err(SimulationBundleErrorV7::InvalidSemanticMirLength);
        }
        let semantic_identity = sha256(&semantic_mir);
        if storage_map.bundle_subject_identity != self.subject_identity
            || storage_map.semantic_mir_sha256 != semantic_identity
            || storage_map.semantic_mir_bytes != semantic_mir.len() as u64
            || storage_map.canonical_kir_sha256 != self.canonical_kir_v12_digest
            || storage_map.canonical_kir_bytes != self.canonical_kir_v12_length
            || aggregate_storage_map.bundle_subject_identity != self.subject_identity
            || aggregate_storage_map.canonical_kir_sha256 != self.canonical_kir_v12_digest
            || aggregate_storage_map.canonical_kir_bytes != self.canonical_kir_v12_length
        {
            return Err(SimulationBundleErrorV7::StorageMapBindingMismatch);
        }
        validate_source_variable_storage_v7(&storage_map, &source_map)?;
        validate_aggregate_correspondence_v7(&storage_map, &aggregate_storage_map)?;
        let source_map = source_map
            .to_canonical_json_bytes()
            .map_err(SimulationBundleErrorV7::DebugSourceMap)?;
        let storage_map = storage_map.to_canonical_json_bytes()?;
        let aggregate_storage_map = aggregate_storage_map.to_canonical_json_bytes()?;
        encode_bundle_v7(
            self,
            source_map,
            semantic_mir,
            storage_map,
            aggregate_storage_map,
        )
    }
}

/// Strict self-contained V7 custody. It grants no compiler or execution authority.
#[derive(Debug)]
pub struct VerifiedSimulationBundleV7 {
    canonical_bytes: Vec<u8>,
    identity: SimulationBundleIdentityV7,
    subject_identity: [u8; 32],
    source_lineage: SimulationSourceLineageV1,
    production_kir_identity: SimulationProductionKirIdentityV7,
    final_graph_epoch: u64,
    canonical_kir_v12_digest: [u8; 32],
    canonical_kir_v12_length: u64,
    kernel_abi_identity: [u8; 32],
    kernel_count: u32,
    target_range: std::ops::Range<usize>,
    kir_range: std::ops::Range<usize>,
    source_map_range: std::ops::Range<usize>,
    semantic_mir_range: std::ops::Range<usize>,
    storage_map_range: std::ops::Range<usize>,
    aggregate_storage_map_range: std::ops::Range<usize>,
}

impl VerifiedSimulationBundleV7 {
    pub fn has_magic_prefix(bytes: &[u8]) -> bool {
        bytes.get(..MAGIC_V7.len()) == Some(MAGIC_V7)
    }

    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Result<Self, SimulationBundleErrorV7> {
        if bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V7 {
            return Err(SimulationBundleErrorV7::BundleTooLarge);
        }
        let header = bytes
            .get(..HEADER_BYTES_V7)
            .ok_or(SimulationBundleErrorV7::Truncated)?;
        let mut decoder = HeaderDecoderV7::new(header);
        if decoder.array::<8>()? != *MAGIC_V7 {
            return Err(SimulationBundleErrorV7::InvalidMagic);
        }
        let version = decoder.u16()?;
        if version != SIMULATION_BUNDLE_VERSION_V7 {
            return Err(SimulationBundleErrorV7::UnsupportedVersion(version));
        }
        if decoder.u16()? != 0 {
            return Err(SimulationBundleErrorV7::InvalidHeader);
        }
        let production_version = decoder.u16()?;
        let canonical_version = decoder.u16()?;
        if canonical_version != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7 {
            return Err(SimulationBundleErrorV7::UnsupportedCanonicalKirVersion(
                canonical_version,
            ));
        }
        let claimed_kernel_count = decoder.u32()?;
        let target_length = usize::from(decoder.u16()?);
        if decoder.array::<6>()? != [0; 6] {
            return Err(SimulationBundleErrorV7::InvalidHeader);
        }
        let kir_length =
            usize::try_from(decoder.u64()?).map_err(|_| SimulationBundleErrorV7::BundleTooLarge)?;
        let source_length = usize::try_from(decoder.u32()?)
            .map_err(|_| SimulationBundleErrorV7::InvalidSourceMapLength)?;
        let semantic_length = usize::try_from(decoder.u64()?)
            .map_err(|_| SimulationBundleErrorV7::InvalidSemanticMirLength)?;
        let storage_length = usize::try_from(decoder.u32()?)
            .map_err(|_| SimulationBundleErrorV7::InvalidStorageMapLength)?;
        let aggregate_length = usize::try_from(decoder.u32()?)
            .map_err(|_| SimulationBundleErrorV7::InvalidAggregateStorageMapLength)?;
        let final_graph_epoch = decoder.u64()?;
        if final_graph_epoch == 0 {
            return Err(SimulationBundleErrorV7::InvalidFinalGraphEpoch);
        }
        let source_lineage = SimulationSourceLineageV1::new(
            decoder.array::<32>()?,
            decoder.u64()?,
            decoder.array::<32>()?,
            decoder.u64()?,
        )
        .map_err(|_| SimulationBundleErrorV7::InvalidSourceLineage)?;
        let production_kir_identity = SimulationProductionKirIdentityV7::new(
            production_version,
            decoder.array::<32>()?,
            decoder.u64()?,
        )?;
        let claimed_kir_digest = decoder.array::<32>()?;
        let claimed_kir_length = decoder.u64()?;
        let claimed_abi = decoder.array::<32>()?;
        let claimed_source = decoder.array::<32>()?;
        let claimed_semantic = decoder.array::<32>()?;
        let claimed_storage = decoder.array::<32>()?;
        let claimed_aggregate = decoder.array::<32>()?;
        let claimed_subject = decoder.array::<32>()?;
        if !decoder.is_done()
            || target_length == 0
            || target_length > MAX_TEXT_BYTES_V1
            || kir_length == 0
            || kir_length > MAX_MODULE_BYTES_V1
            || source_length == 0
            || source_length > MAX_SIMULATION_DEBUG_MAP_BYTES_V1
            || semantic_length == 0
            || semantic_length > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3
            || storage_length == 0
            || storage_length > MAX_SIMULATION_STORAGE_MAP_BYTES_V3
            || aggregate_length == 0
            || aggregate_length > MAX_SIMULATION_STORAGE_MAP_BYTES_V4
            || claimed_kir_length != kir_length as u64
        {
            return Err(SimulationBundleErrorV7::InvalidLength);
        }
        let target_start = HEADER_BYTES_V7;
        let target_end = checked_end_v7(target_start, target_length)?;
        let kir_end = checked_end_v7(target_end, kir_length)?;
        let source_end = checked_end_v7(kir_end, source_length)?;
        let semantic_end = checked_end_v7(source_end, semantic_length)?;
        let storage_end = checked_end_v7(semantic_end, storage_length)?;
        let aggregate_end = checked_end_v7(storage_end, aggregate_length)?;
        if aggregate_end != bytes.len() {
            return Err(SimulationBundleErrorV7::TrailingOrMissingBytes);
        }
        let target = str::from_utf8(&bytes[target_start..target_end])
            .map_err(|_| SimulationBundleErrorV7::InvalidTarget)?;
        validate_target_v7(target)?;
        let (canonical, module) = VerifiedCanonicalKernelIrV12::from_canonical_bytes_with_module(
            copy_bytes_v7(&bytes[target_end..kir_end])?,
        )?;
        if *canonical.identity().digest() != claimed_kir_digest
            || canonical.identity().canonical_length() != claimed_kir_length
        {
            return Err(SimulationBundleErrorV7::CanonicalKirIdentityMismatch);
        }
        validate_production_bridge_v7(&module, production_kir_identity)?;
        let kernel_count = u32::try_from(module.kernels.len())
            .map_err(|_| SimulationBundleErrorV7::KernelCountOverflow)?;
        let kernel_abi_identity = kernel_abi_identity_v7(&module)?;
        if kernel_count != claimed_kernel_count || kernel_abi_identity != claimed_abi {
            return Err(SimulationBundleErrorV7::KernelAbiIdentityMismatch);
        }
        let subject_identity = subject_identity_v7(
            source_lineage,
            production_kir_identity,
            final_graph_epoch,
            target,
            claimed_kir_digest,
            claimed_kir_length,
            kernel_abi_identity,
            kernel_count,
        );
        if subject_identity != claimed_subject {
            return Err(SimulationBundleErrorV7::SubjectIdentityMismatch);
        }
        let source_bytes = &bytes[kir_end..source_end];
        let semantic_bytes = &bytes[source_end..semantic_end];
        let storage_bytes = &bytes[semantic_end..storage_end];
        let aggregate_bytes = &bytes[storage_end..aggregate_end];
        if domain_hash(SOURCE_MAP_IDENTITY_DOMAIN_V7, source_bytes) != claimed_source
            || domain_hash(SEMANTIC_MIR_IDENTITY_DOMAIN_V7, semantic_bytes) != claimed_semantic
            || domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V7, storage_bytes) != claimed_storage
            || domain_hash(AGGREGATE_MAP_IDENTITY_DOMAIN_V7, aggregate_bytes) != claimed_aggregate
        {
            return Err(SimulationBundleErrorV7::SectionIdentityMismatch);
        }
        let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(source_bytes)
            .map_err(SimulationBundleErrorV7::DebugSourceMap)?;
        if source_map.binding()
            != DebugSourceMapBindingV1::new(
                subject_identity,
                claimed_kir_digest,
                claimed_kir_length,
            )
            .map_err(|_| SimulationBundleErrorV7::SourceMapBindingMismatch)?
        {
            return Err(SimulationBundleErrorV7::SourceMapBindingMismatch);
        }
        let storage_map = SemanticStorageMapV7::from_canonical_json_bytes(storage_bytes)?;
        let aggregate_map =
            SemanticAggregateStorageMapV7::from_canonical_json_bytes(aggregate_bytes)?;
        if storage_map.bundle_subject_identity != subject_identity
            || storage_map.semantic_mir_sha256 != sha256(semantic_bytes)
            || storage_map.semantic_mir_bytes != semantic_length as u64
            || storage_map.canonical_kir_sha256 != claimed_kir_digest
            || storage_map.canonical_kir_bytes != claimed_kir_length
            || aggregate_map.bundle_subject_identity != subject_identity
            || aggregate_map.canonical_kir_sha256 != claimed_kir_digest
            || aggregate_map.canonical_kir_bytes != claimed_kir_length
        {
            return Err(SimulationBundleErrorV7::StorageMapBindingMismatch);
        }
        validate_source_variable_storage_v7(&storage_map, &source_map)?;
        validate_aggregate_correspondence_v7(&storage_map, &aggregate_map)?;
        Ok(Self {
            identity: SimulationBundleIdentityV7(domain_hash(BUNDLE_IDENTITY_DOMAIN_V7, &bytes)),
            canonical_bytes: bytes,
            subject_identity,
            source_lineage,
            production_kir_identity,
            final_graph_epoch,
            canonical_kir_v12_digest: claimed_kir_digest,
            canonical_kir_v12_length: claimed_kir_length,
            kernel_abi_identity,
            kernel_count,
            target_range: target_start..target_end,
            kir_range: target_end..kir_end,
            source_map_range: kir_end..source_end,
            semantic_mir_range: source_end..semantic_end,
            storage_map_range: semantic_end..storage_end,
            aggregate_storage_map_range: storage_end..aggregate_end,
        })
    }

    pub fn revalidate(&self) -> Result<(), SimulationBundleErrorV7> {
        let decoded = Self::from_canonical_bytes(copy_bytes_v7(&self.canonical_bytes)?)?;
        if decoded.identity != self.identity
            || decoded.subject_identity != self.subject_identity
            || decoded.production_kir_identity != self.production_kir_identity
            || decoded.final_graph_epoch != self.final_graph_epoch
            || decoded.canonical_kir_v12_digest != self.canonical_kir_v12_digest
            || decoded.canonical_kir_v12_length != self.canonical_kir_v12_length
            || decoded.kernel_abi_identity != self.kernel_abi_identity
            || decoded.kernel_count != self.kernel_count
        {
            return Err(SimulationBundleErrorV7::IdentityMismatch);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }
    pub const fn identity(&self) -> SimulationBundleIdentityV7 {
        self.identity
    }
    pub const fn subject_identity(&self) -> &[u8; 32] {
        &self.subject_identity
    }
    pub const fn source_lineage(&self) -> SimulationSourceLineageV1 {
        self.source_lineage
    }
    pub const fn production_kir_identity(&self) -> SimulationProductionKirIdentityV7 {
        self.production_kir_identity
    }
    pub const fn final_graph_epoch(&self) -> u64 {
        self.final_graph_epoch
    }
    pub const fn canonical_kir_v12_digest(&self) -> &[u8; 32] {
        &self.canonical_kir_v12_digest
    }
    pub const fn canonical_kir_v12_length(&self) -> u64 {
        self.canonical_kir_v12_length
    }
    pub const fn kernel_abi_identity(&self) -> &[u8; 32] {
        &self.kernel_abi_identity
    }
    pub const fn kernel_count(&self) -> u32 {
        self.kernel_count
    }
    pub fn target(&self) -> &str {
        str::from_utf8(&self.canonical_bytes[self.target_range.clone()])
            .expect("validated V7 target")
    }
    pub fn canonical_kir_v12(&self) -> &[u8] {
        &self.canonical_bytes[self.kir_range.clone()]
    }
    pub fn debug_map(&self) -> &[u8] {
        &self.canonical_bytes[self.source_map_range.clone()]
    }
    pub fn semantic_mir(&self) -> &[u8] {
        &self.canonical_bytes[self.semantic_mir_range.clone()]
    }
    pub fn storage_map(&self) -> &[u8] {
        &self.canonical_bytes[self.storage_map_range.clone()]
    }
    pub fn aggregate_storage_map(&self) -> &[u8] {
        &self.canonical_bytes[self.aggregate_storage_map_range.clone()]
    }
    /// Returns the stable V2 document identity used by debugger source-map APIs.
    pub fn debug_map_identity(&self) -> [u8; 32] {
        crate::simulation_debug_map_identity_v2(self.debug_map())
    }
    /// Returns the V7 section-custody identity committed in this bundle header.
    pub fn debug_map_section_identity(&self) -> [u8; 32] {
        domain_hash(SOURCE_MAP_IDENTITY_DOMAIN_V7, self.debug_map())
    }
    pub fn semantic_mir_identity(&self) -> [u8; 32] {
        domain_hash(SEMANTIC_MIR_IDENTITY_DOMAIN_V7, self.semantic_mir())
    }
    pub fn storage_map_identity(&self) -> [u8; 32] {
        domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V7, self.storage_map())
    }
    pub fn aggregate_storage_map_identity(&self) -> [u8; 32] {
        domain_hash(
            AGGREGATE_MAP_IDENTITY_DOMAIN_V7,
            self.aggregate_storage_map(),
        )
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
    pub const fn grants_hardware_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_compiler_execution(&self) -> bool {
        false
    }
}

fn encode_bundle_v7(
    prepared: PreparedSimulationBundleV7,
    source_map: Vec<u8>,
    semantic_mir: Vec<u8>,
    storage_map: Vec<u8>,
    aggregate_map: Vec<u8>,
) -> Result<VerifiedSimulationBundleV7, SimulationBundleErrorV7> {
    let exact_length = HEADER_BYTES_V7
        .checked_add(prepared.target.len())
        .and_then(|n| n.checked_add(prepared.canonical_kir_v12.len()))
        .and_then(|n| n.checked_add(source_map.len()))
        .and_then(|n| n.checked_add(semantic_mir.len()))
        .and_then(|n| n.checked_add(storage_map.len()))
        .and_then(|n| n.checked_add(aggregate_map.len()))
        .ok_or(SimulationBundleErrorV7::BundleTooLarge)?;
    if exact_length > MAX_SIMULATION_BUNDLE_BYTES_V7 {
        return Err(SimulationBundleErrorV7::BundleTooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(exact_length)
        .map_err(|_| SimulationBundleErrorV7::AllocationFailure)?;
    bytes.extend_from_slice(MAGIC_V7);
    bytes.extend_from_slice(&SIMULATION_BUNDLE_VERSION_V7.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&prepared.production_kir_identity.version.to_le_bytes());
    bytes.extend_from_slice(&SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7.to_le_bytes());
    bytes.extend_from_slice(&prepared.kernel_count.to_le_bytes());
    bytes.extend_from_slice(
        &u16::try_from(prepared.target.len())
            .map_err(|_| SimulationBundleErrorV7::InvalidTarget)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&[0; 6]);
    bytes.extend_from_slice(&prepared.canonical_kir_v12_length.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(source_map.len())
            .map_err(|_| SimulationBundleErrorV7::InvalidSourceMapLength)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(semantic_mir.len())
            .map_err(|_| SimulationBundleErrorV7::InvalidSemanticMirLength)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u32::try_from(storage_map.len())
            .map_err(|_| SimulationBundleErrorV7::InvalidStorageMapLength)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u32::try_from(aggregate_map.len())
            .map_err(|_| SimulationBundleErrorV7::InvalidAggregateStorageMapLength)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&prepared.final_graph_epoch.to_le_bytes());
    bytes.extend_from_slice(
        &prepared
            .source_lineage
            .rustc_identity_inventory_receipt_sha256(),
    );
    bytes.extend_from_slice(
        &prepared
            .source_lineage
            .rustc_identity_inventory_receipt_bytes()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &prepared
            .source_lineage
            .rustc_preflight_plan_receipt_sha256(),
    );
    bytes.extend_from_slice(
        &prepared
            .source_lineage
            .rustc_preflight_plan_receipt_bytes()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&prepared.production_kir_identity.digest);
    bytes.extend_from_slice(
        &prepared
            .production_kir_identity
            .canonical_length
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&prepared.canonical_kir_v12_digest);
    bytes.extend_from_slice(&prepared.canonical_kir_v12_length.to_le_bytes());
    bytes.extend_from_slice(&prepared.kernel_abi_identity);
    bytes.extend_from_slice(&domain_hash(SOURCE_MAP_IDENTITY_DOMAIN_V7, &source_map));
    bytes.extend_from_slice(&domain_hash(SEMANTIC_MIR_IDENTITY_DOMAIN_V7, &semantic_mir));
    bytes.extend_from_slice(&domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V7, &storage_map));
    bytes.extend_from_slice(&domain_hash(
        AGGREGATE_MAP_IDENTITY_DOMAIN_V7,
        &aggregate_map,
    ));
    bytes.extend_from_slice(&prepared.subject_identity);
    debug_assert_eq!(bytes.len(), HEADER_BYTES_V7);
    bytes.extend_from_slice(prepared.target.as_bytes());
    bytes.extend_from_slice(&prepared.canonical_kir_v12);
    bytes.extend_from_slice(&source_map);
    bytes.extend_from_slice(&semantic_mir);
    bytes.extend_from_slice(&storage_map);
    bytes.extend_from_slice(&aggregate_map);
    VerifiedSimulationBundleV7::from_canonical_bytes(bytes)
}

fn validate_production_bridge_v7(
    module: &crate::Module,
    claimed: SimulationProductionKirIdentityV7,
) -> Result<(), SimulationBundleErrorV7> {
    if claimed.version != PRODUCTION_KIR_VERSION_V12 {
        return Err(SimulationBundleErrorV7::InvalidProductionKirIdentity);
    }
    let owner = VerifiedCanonicalKernelIrV12::from_module(module.clone())
        .map_err(|_| SimulationBundleErrorV7::ProductionBridgeMismatch)?;
    let (digest, length) = (
        *owner.identity().digest(),
        owner.identity().canonical_length(),
    );
    if digest != claimed.digest || length != claimed.canonical_length {
        return Err(SimulationBundleErrorV7::ProductionBridgeMismatch);
    }
    Ok(())
}

fn validate_source_variable_storage_v7(
    map: &SemanticStorageMapV7,
    source_map: &DebugSourceMapDocumentV2,
) -> Result<(), SimulationBundleErrorV7> {
    if source_map.variables().len() != map.variables.len() {
        return Err(SimulationBundleErrorV7::StorageMapBindingMismatch);
    }
    for binding in &map.variables {
        let source = source_map
            .variables()
            .iter()
            .find(|source| source.identity() == binding.variable_identity())
            .ok_or(SimulationBundleErrorV7::StorageMapBindingMismatch)?;
        let function_ordinal = map
            .kernels
            .iter()
            .find(|kernel| kernel.semantic_body() == binding.semantic_function())
            .map(|kernel| u64::from(kernel.kir_function_ordinal()))
            .ok_or(SimulationBundleErrorV7::StorageMapBindingMismatch)?;
        if source.function_ordinal() != function_ordinal {
            return Err(SimulationBundleErrorV7::StorageMapBindingMismatch);
        }
        match (binding.storage(), source.function_binding()) {
            (
                crate::SemanticStorageBindingV1::ExactKirParameter {
                    kir_value_ordinal, ..
                },
                Some(source_binding),
            ) if source_binding.generation() == 1
                && source_binding.value_ordinal() == u64::from(*kir_value_ordinal) => {}
            (crate::SemanticStorageBindingV1::ExactKirParameter { .. }, _) | (_, Some(_)) => {
                return Err(SimulationBundleErrorV7::StorageMapBindingMismatch);
            }
            (_, None) => {}
        }
    }
    Ok(())
}

fn validate_aggregate_correspondence_v7(
    storage: &SemanticStorageMapV7,
    aggregate: &SemanticAggregateStorageMapV7,
) -> Result<(), SimulationBundleErrorV7> {
    if storage.kernels.len() != aggregate.kernels.len()
        || storage
            .kernels
            .iter()
            .zip(&aggregate.kernels)
            .any(|(left, right)| {
                left.semantic_root() != right.semantic_root()
                    || left.semantic_body() != right.semantic_body()
                    || left.kir_function_ordinal() != right.kir_function_ordinal()
                    || left.arguments().len() != right.arguments().len()
                    || left
                        .arguments()
                        .iter()
                        .zip(right.arguments())
                        .any(|(a, b)| {
                            a.source_ordinal() != b.source_ordinal()
                                || a.semantic_local() != b.semantic_local()
                                || a.semantic_type() != b.semantic_type()
                                || a.ownership() != b.ownership()
                        })
            })
    {
        return Err(SimulationBundleErrorV7::AggregateStorageMapBindingMismatch);
    }
    Ok(())
}

fn kernel_abi_identity_v7(module: &crate::Module) -> Result<[u8; 32], SimulationBundleErrorV7> {
    crate::simulation_bundle_v1::kernel_abi_identity(module)
        .map_err(|_| SimulationBundleErrorV7::InvalidKernelAbi)
}

#[allow(clippy::too_many_arguments)]
fn subject_identity_v7(
    lineage: SimulationSourceLineageV1,
    production: SimulationProductionKirIdentityV7,
    final_graph_epoch: u64,
    target: &str,
    kir_digest: [u8; 32],
    kir_length: u64,
    abi: [u8; 32],
    kernel_count: u32,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SUBJECT_IDENTITY_DOMAIN_V7);
    hasher.update(lineage.rustc_identity_inventory_receipt_sha256());
    hasher.update(
        lineage
            .rustc_identity_inventory_receipt_bytes()
            .to_le_bytes(),
    );
    hasher.update(lineage.rustc_preflight_plan_receipt_sha256());
    hasher.update(lineage.rustc_preflight_plan_receipt_bytes().to_le_bytes());
    hasher.update(production.version.to_le_bytes());
    hasher.update(production.digest);
    hasher.update(production.canonical_length.to_le_bytes());
    hasher.update(final_graph_epoch.to_le_bytes());
    hasher.update(SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V7.to_le_bytes());
    hasher.update(kir_digest);
    hasher.update(kir_length.to_le_bytes());
    hasher.update((target.len() as u64).to_le_bytes());
    hasher.update(target.as_bytes());
    hasher.update(abi);
    hasher.update(kernel_count.to_le_bytes());
    hasher.finalize().into()
}

fn validate_target_v7(target: &str) -> Result<(), SimulationBundleErrorV7> {
    if target.is_empty()
        || target.len() > MAX_TEXT_BYTES_V1
        || !target.is_ascii()
        || target.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(SimulationBundleErrorV7::InvalidTarget);
    }
    Ok(())
}

fn checked_end_v7(start: usize, length: usize) -> Result<usize, SimulationBundleErrorV7> {
    start
        .checked_add(length)
        .ok_or(SimulationBundleErrorV7::BundleTooLarge)
}

fn copy_bytes_v7(bytes: &[u8]) -> Result<Vec<u8>, SimulationBundleErrorV7> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| SimulationBundleErrorV7::AllocationFailure)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

struct HeaderDecoderV7<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> HeaderDecoderV7<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], SimulationBundleErrorV7> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(SimulationBundleErrorV7::Truncated)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(SimulationBundleErrorV7::Truncated)?
            .try_into()
            .map_err(|_| SimulationBundleErrorV7::Truncated)?;
        self.cursor = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, SimulationBundleErrorV7> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, SimulationBundleErrorV7> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, SimulationBundleErrorV7> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn is_done(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}

#[derive(Debug)]
pub enum SimulationBundleErrorV7 {
    BundleTooLarge,
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u16),
    UnsupportedCanonicalKirVersion(u16),
    InvalidHeader,
    InvalidLength,
    InvalidTarget,
    InvalidSourceLineage,
    InvalidProductionKirIdentity,
    InvalidFinalGraphEpoch,
    ProductionBridgeMismatch,
    CanonicalKir(VerifiedCanonicalKernelIrErrorV12),
    CanonicalKirIdentityMismatch,
    KernelCountOverflow,
    InvalidKernelAbi,
    KernelAbiIdentityMismatch,
    SubjectIdentityMismatch,
    InvalidSourceMapLength,
    SourceMapBindingMismatch,
    DebugSourceMap(crate::DebugSourceMapErrorV2),
    InvalidSemanticMirLength,
    InvalidStorageMapLength,
    InvalidStorageMap,
    NonCanonicalStorageMap,
    InvalidAggregateStorageMapLength,
    InvalidAggregateStorageMap,
    NonCanonicalAggregateStorageMap,
    StorageMapBindingMismatch,
    AggregateStorageMapBindingMismatch,
    SectionIdentityMismatch,
    TrailingOrMissingBytes,
    ResourceLimit,
    AllocationFailure,
    Encoding,
    IdentityMismatch,
}

impl fmt::Display for SimulationBundleErrorV7 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported simulation bundle version {version}")
            }
            Self::UnsupportedCanonicalKirVersion(version) => write!(
                formatter,
                "unsupported simulation bundle canonical KIR version {version}"
            ),
            Self::CanonicalKir(error) => write!(formatter, "invalid canonical KIR V12: {error}"),
            other => write!(formatter, "invalid simulation bundle V7: {other:?}"),
        }
    }
}

impl Error for SimulationBundleErrorV7 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalKir(error) => Some(error),
            Self::DebugSourceMap(error) => Some(error),
            _ => None,
        }
    }
}

impl From<VerifiedCanonicalKernelIrErrorV12> for SimulationBundleErrorV7 {
    fn from(error: VerifiedCanonicalKernelIrErrorV12) -> Self {
        Self::CanonicalKir(error)
    }
}

mod hex_identity_v7 {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        let mut text = String::with_capacity(64);
        for byte in value {
            use std::fmt::Write;
            write!(&mut text, "{byte:02x}").expect("string write");
        }
        serializer.serialize_str(&text)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != 64 {
            return Err(D::Error::custom(
                "identity must have 64 lowercase hex digits",
            ));
        }
        let mut bytes = [0; 32];
        for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
            let high = nibble(pair[0])
                .ok_or_else(|| D::Error::custom("identity must be lowercase hex"))?;
            let low = nibble(pair[1])
                .ok_or_else(|| D::Error::custom("identity must be lowercase hex"))?;
            bytes[index] = (high << 4) | low;
        }
        Ok(bytes)
    }
    const fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
