//! Self-contained authority-free simulation bundle with an exact KIR V10 body.

use std::{error::Error, fmt, str};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV2, MAX_MODULE_BYTES_V1,
    MAX_SIMULATION_DEBUG_MAP_BYTES_V1, MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3,
    MAX_SIMULATION_STORAGE_MAP_BYTES_V3, MAX_SIMULATION_STORAGE_MAP_BYTES_V4, MAX_TEXT_BYTES_V1,
    SemanticKernelStorageV1, SemanticKernelStorageV2, SemanticVariableStorageV1,
    SimulationSourceLineageV1, VerifiedCanonicalKernelIrErrorV10, VerifiedCanonicalKernelIrV8,
    VerifiedCanonicalKernelIrV9, VerifiedCanonicalKernelIrV10,
};

pub const SIMULATION_BUNDLE_VERSION_V5: u16 = 5;
pub const SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5: u16 = 10;
pub const SEMANTIC_STORAGE_MAP_SCHEMA_V5: &str = "fe2o3-semantic-storage-map-v5";
pub const SEMANTIC_AGGREGATE_STORAGE_MAP_SCHEMA_V5: &str =
    "fe2o3-semantic-aggregate-storage-map-v5";
pub const MAX_SIMULATION_BUNDLE_BYTES_V5: usize = HEADER_BYTES_V5
    + MAX_TEXT_BYTES_V1
    + MAX_MODULE_BYTES_V1
    + MAX_SIMULATION_DEBUG_MAP_BYTES_V1
    + MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3
    + MAX_SIMULATION_STORAGE_MAP_BYTES_V3
    + MAX_SIMULATION_STORAGE_MAP_BYTES_V4;

const MAGIC_V5: &[u8; 8] = b"F2SIMB05";
const HEADER_BYTES_V5: usize = 408;
const PRODUCTION_KIR_VERSION_V8: u16 = 8;
const PRODUCTION_KIR_VERSION_V9: u16 = 9;
const SUBJECT_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/SIMULATION-BUNDLE-SUBJECT/V5\0";
const BUNDLE_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V5\0";
const SOURCE_MAP_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/SIMULATION-SOURCE-MAP/V5\0";
const SEMANTIC_MIR_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/SIMULATION-SEMANTIC-MIR/V5\0";
const STORAGE_MAP_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/SIMULATION-STORAGE-MAP/V5\0";
const AGGREGATE_MAP_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/SIMULATION-AGGREGATE-STORAGE-MAP/V5\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationBundleIdentityV5([u8; 32]);

impl SimulationBundleIdentityV5 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact versioned identity of the producer-owned canonical KIR rederived from V10.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationProductionKirIdentityV5 {
    version: u16,
    digest: [u8; 32],
    canonical_length: u64,
}

impl SimulationProductionKirIdentityV5 {
    pub fn new(
        version: u16,
        digest: [u8; 32],
        canonical_length: u64,
    ) -> Result<Self, SimulationBundleErrorV5> {
        if !matches!(
            version,
            PRODUCTION_KIR_VERSION_V8 | PRODUCTION_KIR_VERSION_V9
        ) || digest == [0; 32]
            || canonical_length == 0
            || canonical_length > MAX_MODULE_BYTES_V1 as u64
        {
            return Err(SimulationBundleErrorV5::InvalidProductionKirIdentity);
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
enum SemanticStorageMapSchemaV5 {
    #[serde(rename = "fe2o3-semantic-storage-map-v5")]
    V5,
}

/// Version-neutral semantic/source storage correspondence for a V5 subject.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticStorageMapV5 {
    schema: SemanticStorageMapSchemaV5,
    #[serde(with = "hex_identity_v5")]
    bundle_subject_identity: [u8; 32],
    semantic_mir_version: u16,
    #[serde(with = "hex_identity_v5")]
    semantic_mir_sha256: [u8; 32],
    semantic_mir_bytes: u64,
    #[serde(with = "hex_identity_v5")]
    target_layout_identity: [u8; 32],
    canonical_kir_version: u16,
    #[serde(with = "hex_identity_v5")]
    canonical_kir_sha256: [u8; 32],
    canonical_kir_bytes: u64,
    kernels: Vec<SemanticKernelStorageV1>,
    variables: Vec<SemanticVariableStorageV1>,
}

impl SemanticStorageMapV5 {
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
    ) -> Result<Self, SimulationBundleErrorV5> {
        let map = Self {
            schema: SemanticStorageMapSchemaV5::V5,
            bundle_subject_identity,
            semantic_mir_version,
            semantic_mir_sha256,
            semantic_mir_bytes,
            target_layout_identity,
            canonical_kir_version: SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5,
            canonical_kir_sha256,
            canonical_kir_bytes,
            kernels,
            variables,
        };
        map.validate()?;
        Ok(map)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, SimulationBundleErrorV5> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V3 {
            return Err(SimulationBundleErrorV5::InvalidStorageMapLength);
        }
        let map: Self = serde_json::from_slice(bytes)
            .map_err(|_| SimulationBundleErrorV5::InvalidStorageMap)?;
        map.validate()?;
        if serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV5::Encoding)? != bytes {
            return Err(SimulationBundleErrorV5::NonCanonicalStorageMap);
        }
        Ok(map)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SimulationBundleErrorV5> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| SimulationBundleErrorV5::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V3 {
            return Err(SimulationBundleErrorV5::InvalidStorageMapLength);
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

    fn validate(&self) -> Result<(), SimulationBundleErrorV5> {
        if self.bundle_subject_identity == [0; 32]
            || self.semantic_mir_version == 0
            || self.semantic_mir_sha256 == [0; 32]
            || self.semantic_mir_bytes == 0
            || self.semantic_mir_bytes > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3 as u64
            || self.target_layout_identity == [0; 32]
            || self.canonical_kir_version != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5
            || self.canonical_kir_sha256 == [0; 32]
            || self.canonical_kir_bytes == 0
            || self.kernels.is_empty()
        {
            return Err(SimulationBundleErrorV5::InvalidStorageMap);
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
        .map_err(|_| SimulationBundleErrorV5::InvalidStorageMap)?;
        if normalized.kernels() != self.kernels || normalized.variables() != self.variables {
            return Err(SimulationBundleErrorV5::InvalidStorageMap);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum SemanticAggregateStorageMapSchemaV5 {
    #[serde(rename = "fe2o3-semantic-aggregate-storage-map-v5")]
    V5,
}

/// Aggregate component and address-free simulator-kernarg correspondence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAggregateStorageMapV5 {
    schema: SemanticAggregateStorageMapSchemaV5,
    #[serde(with = "hex_identity_v5")]
    bundle_subject_identity: [u8; 32],
    canonical_kir_version: u16,
    #[serde(with = "hex_identity_v5")]
    canonical_kir_sha256: [u8; 32],
    canonical_kir_bytes: u64,
    kernels: Vec<SemanticKernelStorageV2>,
}

impl SemanticAggregateStorageMapV5 {
    pub fn new(
        bundle_subject_identity: [u8; 32],
        canonical_kir_sha256: [u8; 32],
        canonical_kir_bytes: u64,
        kernels: Vec<SemanticKernelStorageV2>,
    ) -> Result<Self, SimulationBundleErrorV5> {
        let map = Self {
            schema: SemanticAggregateStorageMapSchemaV5::V5,
            bundle_subject_identity,
            canonical_kir_version: SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5,
            canonical_kir_sha256,
            canonical_kir_bytes,
            kernels,
        };
        map.validate()?;
        Ok(map)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, SimulationBundleErrorV5> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V4 {
            return Err(SimulationBundleErrorV5::InvalidAggregateStorageMapLength);
        }
        let map: Self = serde_json::from_slice(bytes)
            .map_err(|_| SimulationBundleErrorV5::InvalidAggregateStorageMap)?;
        map.validate()?;
        if serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV5::Encoding)? != bytes {
            return Err(SimulationBundleErrorV5::NonCanonicalAggregateStorageMap);
        }
        Ok(map)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SimulationBundleErrorV5> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| SimulationBundleErrorV5::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V4 {
            return Err(SimulationBundleErrorV5::InvalidAggregateStorageMapLength);
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

    fn validate(&self) -> Result<(), SimulationBundleErrorV5> {
        if self.bundle_subject_identity == [0; 32]
            || self.canonical_kir_version != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5
            || self.canonical_kir_sha256 == [0; 32]
            || self.canonical_kir_bytes == 0
            || self.kernels.is_empty()
        {
            return Err(SimulationBundleErrorV5::InvalidAggregateStorageMap);
        }
        let normalized =
            crate::SemanticStorageMapV2::new(self.bundle_subject_identity, self.kernels.clone())
                .map_err(|_| SimulationBundleErrorV5::InvalidAggregateStorageMap)?;
        if normalized.kernels() != self.kernels {
            return Err(SimulationBundleErrorV5::InvalidAggregateStorageMap);
        }
        Ok(())
    }
}

/// Map-independent V5 metadata awaiting compiler source/storage sections.
#[must_use = "dropping the prepared owner abandons simulation bundle finalization"]
pub struct PreparedSimulationBundleV5 {
    source_lineage: SimulationSourceLineageV1,
    production_kir_identity: SimulationProductionKirIdentityV5,
    target: String,
    canonical_kir_v10: Vec<u8>,
    canonical_kir_v10_digest: [u8; 32],
    canonical_kir_v10_length: u64,
    kernel_abi_identity: [u8; 32],
    kernel_count: u32,
    subject_identity: [u8; 32],
}

impl PreparedSimulationBundleV5 {
    pub fn new(
        source_lineage: SimulationSourceLineageV1,
        production_kir_identity: SimulationProductionKirIdentityV5,
        target: &str,
        canonical_kir_v10: VerifiedCanonicalKernelIrV10,
    ) -> Result<Self, SimulationBundleErrorV5> {
        validate_target_v5(target)?;
        canonical_kir_v10.revalidate()?;
        let canonical_kir_v10_digest = *canonical_kir_v10.identity().digest();
        let canonical_kir_v10_length = canonical_kir_v10.identity().canonical_length();
        let (_, module) = VerifiedCanonicalKernelIrV10::from_canonical_bytes_with_module(
            copy_bytes_v5(canonical_kir_v10.canonical_bytes())?,
        )?;
        validate_production_bridge_v5(&module, production_kir_identity)?;
        let kernel_count = u32::try_from(module.kernels.len())
            .map_err(|_| SimulationBundleErrorV5::KernelCountOverflow)?;
        let kernel_abi_identity = kernel_abi_identity_v5(&module)?;
        let subject_identity = subject_identity_v5(
            source_lineage,
            production_kir_identity,
            target,
            canonical_kir_v10_digest,
            canonical_kir_v10_length,
            kernel_abi_identity,
            kernel_count,
        );
        Ok(Self {
            source_lineage,
            production_kir_identity,
            target: target.to_owned(),
            canonical_kir_v10: canonical_kir_v10.into_canonical_bytes(),
            canonical_kir_v10_digest,
            canonical_kir_v10_length,
            kernel_abi_identity,
            kernel_count,
            subject_identity,
        })
    }

    pub const fn subject_identity(&self) -> &[u8; 32] {
        &self.subject_identity
    }
    pub const fn canonical_kir_v10_digest(&self) -> &[u8; 32] {
        &self.canonical_kir_v10_digest
    }
    pub const fn canonical_kir_v10_length(&self) -> u64 {
        self.canonical_kir_v10_length
    }
    pub fn debug_source_map_binding(&self) -> DebugSourceMapBindingV1 {
        DebugSourceMapBindingV1::new(
            self.subject_identity,
            self.canonical_kir_v10_digest,
            self.canonical_kir_v10_length,
        )
        .expect("verified V5 identities form a valid source-map binding")
    }

    pub fn finalize(
        self,
        source_map: DebugSourceMapDocumentV2,
        semantic_mir: Vec<u8>,
        storage_map: SemanticStorageMapV5,
        aggregate_storage_map: SemanticAggregateStorageMapV5,
    ) -> Result<VerifiedSimulationBundleV5, SimulationBundleErrorV5> {
        if source_map.binding() != self.debug_source_map_binding() {
            return Err(SimulationBundleErrorV5::SourceMapBindingMismatch);
        }
        if semantic_mir.is_empty() || semantic_mir.len() > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3 {
            return Err(SimulationBundleErrorV5::InvalidSemanticMirLength);
        }
        let semantic_identity = sha256(&semantic_mir);
        if storage_map.bundle_subject_identity != self.subject_identity
            || storage_map.semantic_mir_sha256 != semantic_identity
            || storage_map.semantic_mir_bytes != semantic_mir.len() as u64
            || storage_map.canonical_kir_sha256 != self.canonical_kir_v10_digest
            || storage_map.canonical_kir_bytes != self.canonical_kir_v10_length
            || aggregate_storage_map.bundle_subject_identity != self.subject_identity
            || aggregate_storage_map.canonical_kir_sha256 != self.canonical_kir_v10_digest
            || aggregate_storage_map.canonical_kir_bytes != self.canonical_kir_v10_length
        {
            return Err(SimulationBundleErrorV5::StorageMapBindingMismatch);
        }
        validate_source_variable_storage_v5(&storage_map, &source_map)?;
        validate_aggregate_correspondence_v5(&storage_map, &aggregate_storage_map)?;
        let source_map = source_map
            .to_canonical_json_bytes()
            .map_err(SimulationBundleErrorV5::DebugSourceMap)?;
        let storage_map = storage_map.to_canonical_json_bytes()?;
        let aggregate_storage_map = aggregate_storage_map.to_canonical_json_bytes()?;
        encode_bundle_v5(
            self,
            source_map,
            semantic_mir,
            storage_map,
            aggregate_storage_map,
        )
    }
}

/// Strict self-contained V5 custody. It grants no compiler or execution authority.
#[derive(Debug)]
pub struct VerifiedSimulationBundleV5 {
    canonical_bytes: Vec<u8>,
    identity: SimulationBundleIdentityV5,
    subject_identity: [u8; 32],
    source_lineage: SimulationSourceLineageV1,
    production_kir_identity: SimulationProductionKirIdentityV5,
    canonical_kir_v10_digest: [u8; 32],
    canonical_kir_v10_length: u64,
    kernel_abi_identity: [u8; 32],
    kernel_count: u32,
    target_range: std::ops::Range<usize>,
    kir_range: std::ops::Range<usize>,
    source_map_range: std::ops::Range<usize>,
    semantic_mir_range: std::ops::Range<usize>,
    storage_map_range: std::ops::Range<usize>,
    aggregate_storage_map_range: std::ops::Range<usize>,
}

impl VerifiedSimulationBundleV5 {
    pub fn has_magic_prefix(bytes: &[u8]) -> bool {
        bytes.get(..MAGIC_V5.len()) == Some(MAGIC_V5)
    }

    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Result<Self, SimulationBundleErrorV5> {
        if bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V5 {
            return Err(SimulationBundleErrorV5::BundleTooLarge);
        }
        let header = bytes
            .get(..HEADER_BYTES_V5)
            .ok_or(SimulationBundleErrorV5::Truncated)?;
        let mut decoder = HeaderDecoderV5::new(header);
        if decoder.array::<8>()? != *MAGIC_V5 {
            return Err(SimulationBundleErrorV5::InvalidMagic);
        }
        let version = decoder.u16()?;
        if version != SIMULATION_BUNDLE_VERSION_V5 {
            return Err(SimulationBundleErrorV5::UnsupportedVersion(version));
        }
        if decoder.u16()? != 0 {
            return Err(SimulationBundleErrorV5::InvalidHeader);
        }
        let production_version = decoder.u16()?;
        let canonical_version = decoder.u16()?;
        if canonical_version != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5 {
            return Err(SimulationBundleErrorV5::UnsupportedCanonicalKirVersion(
                canonical_version,
            ));
        }
        let claimed_kernel_count = decoder.u32()?;
        let target_length = usize::from(decoder.u16()?);
        if decoder.array::<6>()? != [0; 6] {
            return Err(SimulationBundleErrorV5::InvalidHeader);
        }
        let kir_length =
            usize::try_from(decoder.u64()?).map_err(|_| SimulationBundleErrorV5::BundleTooLarge)?;
        let source_length = usize::try_from(decoder.u32()?)
            .map_err(|_| SimulationBundleErrorV5::InvalidSourceMapLength)?;
        let semantic_length = usize::try_from(decoder.u64()?)
            .map_err(|_| SimulationBundleErrorV5::InvalidSemanticMirLength)?;
        let storage_length = usize::try_from(decoder.u32()?)
            .map_err(|_| SimulationBundleErrorV5::InvalidStorageMapLength)?;
        let aggregate_length = usize::try_from(decoder.u32()?)
            .map_err(|_| SimulationBundleErrorV5::InvalidAggregateStorageMapLength)?;
        let source_lineage = SimulationSourceLineageV1::new(
            decoder.array::<32>()?,
            decoder.u64()?,
            decoder.array::<32>()?,
            decoder.u64()?,
        )
        .map_err(|_| SimulationBundleErrorV5::InvalidSourceLineage)?;
        let production_kir_identity = SimulationProductionKirIdentityV5::new(
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
            return Err(SimulationBundleErrorV5::InvalidLength);
        }
        let target_start = HEADER_BYTES_V5;
        let target_end = checked_end_v5(target_start, target_length)?;
        let kir_end = checked_end_v5(target_end, kir_length)?;
        let source_end = checked_end_v5(kir_end, source_length)?;
        let semantic_end = checked_end_v5(source_end, semantic_length)?;
        let storage_end = checked_end_v5(semantic_end, storage_length)?;
        let aggregate_end = checked_end_v5(storage_end, aggregate_length)?;
        if aggregate_end != bytes.len() {
            return Err(SimulationBundleErrorV5::TrailingOrMissingBytes);
        }
        let target = str::from_utf8(&bytes[target_start..target_end])
            .map_err(|_| SimulationBundleErrorV5::InvalidTarget)?;
        validate_target_v5(target)?;
        let (canonical, module) = VerifiedCanonicalKernelIrV10::from_canonical_bytes_with_module(
            copy_bytes_v5(&bytes[target_end..kir_end])?,
        )?;
        if *canonical.identity().digest() != claimed_kir_digest
            || canonical.identity().canonical_length() != claimed_kir_length
        {
            return Err(SimulationBundleErrorV5::CanonicalKirIdentityMismatch);
        }
        validate_production_bridge_v5(&module, production_kir_identity)?;
        let kernel_count = u32::try_from(module.kernels.len())
            .map_err(|_| SimulationBundleErrorV5::KernelCountOverflow)?;
        let kernel_abi_identity = kernel_abi_identity_v5(&module)?;
        if kernel_count != claimed_kernel_count || kernel_abi_identity != claimed_abi {
            return Err(SimulationBundleErrorV5::KernelAbiIdentityMismatch);
        }
        let subject_identity = subject_identity_v5(
            source_lineage,
            production_kir_identity,
            target,
            claimed_kir_digest,
            claimed_kir_length,
            kernel_abi_identity,
            kernel_count,
        );
        if subject_identity != claimed_subject {
            return Err(SimulationBundleErrorV5::SubjectIdentityMismatch);
        }
        let source_bytes = &bytes[kir_end..source_end];
        let semantic_bytes = &bytes[source_end..semantic_end];
        let storage_bytes = &bytes[semantic_end..storage_end];
        let aggregate_bytes = &bytes[storage_end..aggregate_end];
        if domain_hash(SOURCE_MAP_IDENTITY_DOMAIN_V5, source_bytes) != claimed_source
            || domain_hash(SEMANTIC_MIR_IDENTITY_DOMAIN_V5, semantic_bytes) != claimed_semantic
            || domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V5, storage_bytes) != claimed_storage
            || domain_hash(AGGREGATE_MAP_IDENTITY_DOMAIN_V5, aggregate_bytes) != claimed_aggregate
        {
            return Err(SimulationBundleErrorV5::SectionIdentityMismatch);
        }
        let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(source_bytes)
            .map_err(SimulationBundleErrorV5::DebugSourceMap)?;
        if source_map.binding()
            != DebugSourceMapBindingV1::new(
                subject_identity,
                claimed_kir_digest,
                claimed_kir_length,
            )
            .map_err(|_| SimulationBundleErrorV5::SourceMapBindingMismatch)?
        {
            return Err(SimulationBundleErrorV5::SourceMapBindingMismatch);
        }
        let storage_map = SemanticStorageMapV5::from_canonical_json_bytes(storage_bytes)?;
        let aggregate_map =
            SemanticAggregateStorageMapV5::from_canonical_json_bytes(aggregate_bytes)?;
        if storage_map.bundle_subject_identity != subject_identity
            || storage_map.semantic_mir_sha256 != sha256(semantic_bytes)
            || storage_map.semantic_mir_bytes != semantic_length as u64
            || storage_map.canonical_kir_sha256 != claimed_kir_digest
            || storage_map.canonical_kir_bytes != claimed_kir_length
            || aggregate_map.bundle_subject_identity != subject_identity
            || aggregate_map.canonical_kir_sha256 != claimed_kir_digest
            || aggregate_map.canonical_kir_bytes != claimed_kir_length
        {
            return Err(SimulationBundleErrorV5::StorageMapBindingMismatch);
        }
        validate_source_variable_storage_v5(&storage_map, &source_map)?;
        validate_aggregate_correspondence_v5(&storage_map, &aggregate_map)?;
        Ok(Self {
            identity: SimulationBundleIdentityV5(domain_hash(BUNDLE_IDENTITY_DOMAIN_V5, &bytes)),
            canonical_bytes: bytes,
            subject_identity,
            source_lineage,
            production_kir_identity,
            canonical_kir_v10_digest: claimed_kir_digest,
            canonical_kir_v10_length: claimed_kir_length,
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

    pub fn revalidate(&self) -> Result<(), SimulationBundleErrorV5> {
        let decoded = Self::from_canonical_bytes(copy_bytes_v5(&self.canonical_bytes)?)?;
        if decoded.identity != self.identity
            || decoded.subject_identity != self.subject_identity
            || decoded.production_kir_identity != self.production_kir_identity
            || decoded.canonical_kir_v10_digest != self.canonical_kir_v10_digest
            || decoded.canonical_kir_v10_length != self.canonical_kir_v10_length
            || decoded.kernel_abi_identity != self.kernel_abi_identity
            || decoded.kernel_count != self.kernel_count
        {
            return Err(SimulationBundleErrorV5::IdentityMismatch);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }
    pub const fn identity(&self) -> SimulationBundleIdentityV5 {
        self.identity
    }
    pub const fn subject_identity(&self) -> &[u8; 32] {
        &self.subject_identity
    }
    pub const fn source_lineage(&self) -> SimulationSourceLineageV1 {
        self.source_lineage
    }
    pub const fn production_kir_identity(&self) -> SimulationProductionKirIdentityV5 {
        self.production_kir_identity
    }
    pub const fn canonical_kir_v10_digest(&self) -> &[u8; 32] {
        &self.canonical_kir_v10_digest
    }
    pub const fn canonical_kir_v10_length(&self) -> u64 {
        self.canonical_kir_v10_length
    }
    pub const fn kernel_abi_identity(&self) -> &[u8; 32] {
        &self.kernel_abi_identity
    }
    pub const fn kernel_count(&self) -> u32 {
        self.kernel_count
    }
    pub fn target(&self) -> &str {
        str::from_utf8(&self.canonical_bytes[self.target_range.clone()])
            .expect("validated V5 target")
    }
    pub fn canonical_kir_v10(&self) -> &[u8] {
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
    /// Returns the V5 section-custody identity committed in this bundle header.
    pub fn debug_map_section_identity(&self) -> [u8; 32] {
        domain_hash(SOURCE_MAP_IDENTITY_DOMAIN_V5, self.debug_map())
    }
    pub fn semantic_mir_identity(&self) -> [u8; 32] {
        domain_hash(SEMANTIC_MIR_IDENTITY_DOMAIN_V5, self.semantic_mir())
    }
    pub fn storage_map_identity(&self) -> [u8; 32] {
        domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V5, self.storage_map())
    }
    pub fn aggregate_storage_map_identity(&self) -> [u8; 32] {
        domain_hash(
            AGGREGATE_MAP_IDENTITY_DOMAIN_V5,
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

fn encode_bundle_v5(
    prepared: PreparedSimulationBundleV5,
    source_map: Vec<u8>,
    semantic_mir: Vec<u8>,
    storage_map: Vec<u8>,
    aggregate_map: Vec<u8>,
) -> Result<VerifiedSimulationBundleV5, SimulationBundleErrorV5> {
    let exact_length = HEADER_BYTES_V5
        .checked_add(prepared.target.len())
        .and_then(|n| n.checked_add(prepared.canonical_kir_v10.len()))
        .and_then(|n| n.checked_add(source_map.len()))
        .and_then(|n| n.checked_add(semantic_mir.len()))
        .and_then(|n| n.checked_add(storage_map.len()))
        .and_then(|n| n.checked_add(aggregate_map.len()))
        .ok_or(SimulationBundleErrorV5::BundleTooLarge)?;
    if exact_length > MAX_SIMULATION_BUNDLE_BYTES_V5 {
        return Err(SimulationBundleErrorV5::BundleTooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(exact_length)
        .map_err(|_| SimulationBundleErrorV5::AllocationFailure)?;
    bytes.extend_from_slice(MAGIC_V5);
    bytes.extend_from_slice(&SIMULATION_BUNDLE_VERSION_V5.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&prepared.production_kir_identity.version.to_le_bytes());
    bytes.extend_from_slice(&SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5.to_le_bytes());
    bytes.extend_from_slice(&prepared.kernel_count.to_le_bytes());
    bytes.extend_from_slice(
        &u16::try_from(prepared.target.len())
            .map_err(|_| SimulationBundleErrorV5::InvalidTarget)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&[0; 6]);
    bytes.extend_from_slice(&prepared.canonical_kir_v10_length.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(source_map.len())
            .map_err(|_| SimulationBundleErrorV5::InvalidSourceMapLength)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(semantic_mir.len())
            .map_err(|_| SimulationBundleErrorV5::InvalidSemanticMirLength)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u32::try_from(storage_map.len())
            .map_err(|_| SimulationBundleErrorV5::InvalidStorageMapLength)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u32::try_from(aggregate_map.len())
            .map_err(|_| SimulationBundleErrorV5::InvalidAggregateStorageMapLength)?
            .to_le_bytes(),
    );
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
    bytes.extend_from_slice(&prepared.canonical_kir_v10_digest);
    bytes.extend_from_slice(&prepared.canonical_kir_v10_length.to_le_bytes());
    bytes.extend_from_slice(&prepared.kernel_abi_identity);
    bytes.extend_from_slice(&domain_hash(SOURCE_MAP_IDENTITY_DOMAIN_V5, &source_map));
    bytes.extend_from_slice(&domain_hash(SEMANTIC_MIR_IDENTITY_DOMAIN_V5, &semantic_mir));
    bytes.extend_from_slice(&domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V5, &storage_map));
    bytes.extend_from_slice(&domain_hash(
        AGGREGATE_MAP_IDENTITY_DOMAIN_V5,
        &aggregate_map,
    ));
    bytes.extend_from_slice(&prepared.subject_identity);
    debug_assert_eq!(bytes.len(), HEADER_BYTES_V5);
    bytes.extend_from_slice(prepared.target.as_bytes());
    bytes.extend_from_slice(&prepared.canonical_kir_v10);
    bytes.extend_from_slice(&source_map);
    bytes.extend_from_slice(&semantic_mir);
    bytes.extend_from_slice(&storage_map);
    bytes.extend_from_slice(&aggregate_map);
    VerifiedSimulationBundleV5::from_canonical_bytes(bytes)
}

fn validate_production_bridge_v5(
    module: &crate::Module,
    claimed: SimulationProductionKirIdentityV5,
) -> Result<(), SimulationBundleErrorV5> {
    let (digest, length) = match claimed.version {
        PRODUCTION_KIR_VERSION_V8 => {
            let owner = VerifiedCanonicalKernelIrV8::from_module(module.clone())
                .map_err(|_| SimulationBundleErrorV5::ProductionBridgeMismatch)?;
            (
                *owner.identity().digest(),
                owner.identity().canonical_length(),
            )
        }
        PRODUCTION_KIR_VERSION_V9 => {
            let owner = VerifiedCanonicalKernelIrV9::from_module(module.clone())
                .map_err(|_| SimulationBundleErrorV5::ProductionBridgeMismatch)?;
            (
                *owner.identity().digest(),
                owner.identity().canonical_length(),
            )
        }
        _ => return Err(SimulationBundleErrorV5::InvalidProductionKirIdentity),
    };
    if digest != claimed.digest || length != claimed.canonical_length {
        return Err(SimulationBundleErrorV5::ProductionBridgeMismatch);
    }
    Ok(())
}

fn validate_source_variable_storage_v5(
    map: &SemanticStorageMapV5,
    source_map: &DebugSourceMapDocumentV2,
) -> Result<(), SimulationBundleErrorV5> {
    if source_map.variables().len() != map.variables.len() {
        return Err(SimulationBundleErrorV5::StorageMapBindingMismatch);
    }
    for binding in &map.variables {
        let source = source_map
            .variables()
            .iter()
            .find(|source| source.identity() == binding.variable_identity())
            .ok_or(SimulationBundleErrorV5::StorageMapBindingMismatch)?;
        let function_ordinal = map
            .kernels
            .iter()
            .find(|kernel| kernel.semantic_body() == binding.semantic_function())
            .map(|kernel| u64::from(kernel.kir_function_ordinal()))
            .ok_or(SimulationBundleErrorV5::StorageMapBindingMismatch)?;
        if source.function_ordinal() != function_ordinal {
            return Err(SimulationBundleErrorV5::StorageMapBindingMismatch);
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
                return Err(SimulationBundleErrorV5::StorageMapBindingMismatch);
            }
            (_, None) => {}
        }
    }
    Ok(())
}

fn validate_aggregate_correspondence_v5(
    storage: &SemanticStorageMapV5,
    aggregate: &SemanticAggregateStorageMapV5,
) -> Result<(), SimulationBundleErrorV5> {
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
        return Err(SimulationBundleErrorV5::AggregateStorageMapBindingMismatch);
    }
    Ok(())
}

fn kernel_abi_identity_v5(module: &crate::Module) -> Result<[u8; 32], SimulationBundleErrorV5> {
    crate::simulation_bundle_v1::kernel_abi_identity(module)
        .map_err(|_| SimulationBundleErrorV5::InvalidKernelAbi)
}

#[allow(clippy::too_many_arguments)]
fn subject_identity_v5(
    lineage: SimulationSourceLineageV1,
    production: SimulationProductionKirIdentityV5,
    target: &str,
    kir_digest: [u8; 32],
    kir_length: u64,
    abi: [u8; 32],
    kernel_count: u32,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SUBJECT_IDENTITY_DOMAIN_V5);
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
    hasher.update(SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V5.to_le_bytes());
    hasher.update(kir_digest);
    hasher.update(kir_length.to_le_bytes());
    hasher.update((target.len() as u64).to_le_bytes());
    hasher.update(target.as_bytes());
    hasher.update(abi);
    hasher.update(kernel_count.to_le_bytes());
    hasher.finalize().into()
}

fn validate_target_v5(target: &str) -> Result<(), SimulationBundleErrorV5> {
    if target.is_empty()
        || target.len() > MAX_TEXT_BYTES_V1
        || !target.is_ascii()
        || target.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(SimulationBundleErrorV5::InvalidTarget);
    }
    Ok(())
}

fn checked_end_v5(start: usize, length: usize) -> Result<usize, SimulationBundleErrorV5> {
    start
        .checked_add(length)
        .ok_or(SimulationBundleErrorV5::BundleTooLarge)
}

fn copy_bytes_v5(bytes: &[u8]) -> Result<Vec<u8>, SimulationBundleErrorV5> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| SimulationBundleErrorV5::AllocationFailure)?;
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

struct HeaderDecoderV5<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> HeaderDecoderV5<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], SimulationBundleErrorV5> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(SimulationBundleErrorV5::Truncated)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(SimulationBundleErrorV5::Truncated)?
            .try_into()
            .map_err(|_| SimulationBundleErrorV5::Truncated)?;
        self.cursor = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, SimulationBundleErrorV5> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, SimulationBundleErrorV5> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, SimulationBundleErrorV5> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn is_done(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}

#[derive(Debug)]
pub enum SimulationBundleErrorV5 {
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
    ProductionBridgeMismatch,
    CanonicalKir(VerifiedCanonicalKernelIrErrorV10),
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

impl fmt::Display for SimulationBundleErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported simulation bundle version {version}")
            }
            Self::UnsupportedCanonicalKirVersion(version) => write!(
                formatter,
                "unsupported simulation bundle canonical KIR version {version}"
            ),
            Self::CanonicalKir(error) => write!(formatter, "invalid canonical KIR V10: {error}"),
            other => write!(formatter, "invalid simulation bundle V5: {other:?}"),
        }
    }
}

impl Error for SimulationBundleErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalKir(error) => Some(error),
            Self::DebugSourceMap(error) => Some(error),
            _ => None,
        }
    }
}

impl From<VerifiedCanonicalKernelIrErrorV10> for SimulationBundleErrorV5 {
    fn from(error: VerifiedCanonicalKernelIrErrorV10) -> Self {
        Self::CanonicalKir(error)
    }
}

mod hex_identity_v5 {
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
mod tests {
    use super::*;
    use crate::{
        AccessMode, AddressSpace, BasicBlock, BlockId, DebugSourceMapFileV1, DebugSourceMapSpanV1,
        Function, Kernel, LaunchDomain, LaunchExtent, SemanticArgumentOwnershipV1,
        SemanticArgumentStorageV1, SemanticArgumentStorageV2, SemanticComponentStorageBindingV2,
        SemanticKernargSlotV2, SemanticKirComponentRepresentationV2, SemanticKirComponentStorageV2,
        SemanticKirStorageRepresentationV1, SemanticStorageBindingV1, Signature, Terminator, Type,
        ValueId, VerifiedCanonicalKernelIrV8, WorkgroupSize, decode_module_v10,
    };

    fn bundle_for_production_version(production_version: u16) -> VerifiedSimulationBundleV5 {
        let mut module = crate::Module::new("bundle_v5_test");
        let access = match production_version {
            8 => AccessMode::ReadWrite,
            9 => AccessMode::WriteOnly,
            other => panic!("unsupported test production version {other}"),
        };
        let slice = Type::slice(Type::F32, AddressSpace::Global, access);
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        module.functions.push(Function::kernel_entry(
            "kernel",
            Signature::new(vec![slice], vec![]),
            vec![ValueId(7)],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "kernel",
            "kernel",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize { x: 64, y: 1, z: 1 });
        module.kernels.push(kernel);
        let (production_digest, production_length) = match production_version {
            8 => {
                let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
                (
                    *production.identity().digest(),
                    production.identity().canonical_length(),
                )
            }
            9 => {
                let production = VerifiedCanonicalKernelIrV9::from_module(module.clone()).unwrap();
                (
                    *production.identity().digest(),
                    production.identity().canonical_length(),
                )
            }
            _ => unreachable!(),
        };
        let prepared = PreparedSimulationBundleV5::new(
            SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
            SimulationProductionKirIdentityV5::new(
                production_version,
                production_digest,
                production_length,
            )
            .unwrap(),
            "gfx950:xnack-",
            VerifiedCanonicalKernelIrV10::from_module(module).unwrap(),
        )
        .unwrap();
        let source_map = DebugSourceMapDocumentV2::new(
            prepared.debug_source_map_binding(),
            vec![DebugSourceMapFileV1::new([4; 32], 16, "/src/kernel.rs".into()).unwrap()],
            Vec::new(),
            vec![DebugSourceMapSpanV1::new([4; 32], 1, 2, 1, 2).unwrap()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let semantic = b"exact-production-semantic-mir-v5-fixture".to_vec();
        let storage = SemanticStorageMapV5::new(
            *prepared.subject_identity(),
            production_version,
            sha256(&semantic),
            semantic.len() as u64,
            [9; 32],
            *prepared.canonical_kir_v10_digest(),
            prepared.canonical_kir_v10_length(),
            vec![SemanticKernelStorageV1::new(
                0,
                0,
                0,
                vec![SemanticArgumentStorageV1::new(
                    0,
                    0,
                    0,
                    SemanticArgumentOwnershipV1::UniqueBorrow,
                    SemanticStorageBindingV1::ExactKirParameter {
                        kir_parameter_ordinal: 0,
                        kir_value_ordinal: 7,
                        representation: SemanticKirStorageRepresentationV1::RegionSlice,
                    },
                )],
            )],
            Vec::new(),
        )
        .unwrap();
        let aggregate = SemanticAggregateStorageMapV5::new(
            *prepared.subject_identity(),
            *prepared.canonical_kir_v10_digest(),
            prepared.canonical_kir_v10_length(),
            vec![SemanticKernelStorageV2::new(
                0,
                0,
                0,
                16,
                8,
                vec![SemanticArgumentStorageV2::new(
                    0,
                    0,
                    0,
                    SemanticArgumentOwnershipV1::UniqueBorrow,
                    SemanticComponentStorageBindingV2::exact(vec![
                        SemanticKirComponentStorageV2::new(
                            Vec::new(),
                            0,
                            7,
                            SemanticKirComponentRepresentationV2::RegionSlice,
                            SemanticKernargSlotV2::new(0, 8, 8),
                            Some(SemanticKernargSlotV2::new(8, 8, 8)),
                        ),
                    ]),
                )],
            )],
        )
        .unwrap();
        prepared
            .finalize(source_map, semantic, storage, aggregate)
            .unwrap()
    }

    fn bundle() -> VerifiedSimulationBundleV5 {
        bundle_for_production_version(9)
    }

    #[test]
    fn v5_round_trips_exact_v9_production_as_v10_without_authority() {
        let bundle = bundle();
        let decoded =
            VerifiedSimulationBundleV5::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .unwrap();
        assert_eq!(decoded.identity(), bundle.identity());
        assert_eq!(decoded.production_kir_identity().version(), 9);
        assert_eq!(decoded.canonical_kir_v10()[8..10], 10_u16.to_le_bytes());
        assert!(!decoded.authenticates_compiler_execution());
        assert!(!decoded.grants_compiler_authority());
        assert!(!decoded.grants_hardware_authority());
        assert!(!decoded.grants_load_authority());
        assert!(!decoded.grants_launch_authority());
        assert!(
            crate::VerifiedSimulationBundleV4::from_canonical_bytes(
                bundle.canonical_bytes().to_vec(),
            )
            .is_err()
        );
    }

    #[test]
    fn v5_rejects_an_embedded_exact_kir_v11_body() {
        let bundle = bundle();
        let mut bytes = bundle.canonical_bytes().to_vec();
        bytes[bundle.kir_range.start + 8..bundle.kir_range.start + 10]
            .copy_from_slice(&11_u16.to_le_bytes());
        assert!(matches!(
            VerifiedSimulationBundleV5::from_canonical_bytes(bytes),
            Err(SimulationBundleErrorV5::CanonicalKir(
                crate::VerifiedCanonicalKernelIrErrorV10::NotExactV10 { version: 11 }
            ))
        ));
    }

    #[test]
    fn v5_round_trips_exact_v8_production_as_v10_without_semantic_drift() {
        let bundle = bundle_for_production_version(8);
        let decoded =
            VerifiedSimulationBundleV5::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .unwrap();
        assert_eq!(decoded.production_kir_identity().version(), 8);
        let module = decode_module_v10(decoded.canonical_kir_v10()).unwrap();
        let production = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
        assert_eq!(
            decoded.production_kir_identity().digest(),
            *production.identity().digest()
        );
        assert_eq!(
            decoded.production_kir_identity().canonical_length(),
            production.identity().canonical_length()
        );
        assert!(!decoded.authenticates_compiler_execution());
        assert!(!decoded.grants_hardware_authority());
    }

    #[test]
    fn v5_rejects_version_bridge_section_and_trailing_substitution() {
        let bundle = bundle();
        let mut wrong_production_version = bundle.canonical_bytes().to_vec();
        wrong_production_version[12..14].copy_from_slice(&8_u16.to_le_bytes());
        assert!(matches!(
            VerifiedSimulationBundleV5::from_canonical_bytes(wrong_production_version),
            Err(SimulationBundleErrorV5::ProductionBridgeMismatch)
        ));

        let mut wrong_canonical_version = bundle.canonical_bytes().to_vec();
        wrong_canonical_version[14..16].copy_from_slice(&9_u16.to_le_bytes());
        assert!(matches!(
            VerifiedSimulationBundleV5::from_canonical_bytes(wrong_canonical_version),
            Err(SimulationBundleErrorV5::UnsupportedCanonicalKirVersion(9))
        ));

        for offset in [
            176_usize,
            248,
            HEADER_BYTES_V5,
            bundle.canonical_bytes().len() - 1,
        ] {
            let mut hostile = bundle.canonical_bytes().to_vec();
            hostile[offset] ^= 1;
            assert!(VerifiedSimulationBundleV5::from_canonical_bytes(hostile).is_err());
        }
        let mut trailing = bundle.canonical_bytes().to_vec();
        trailing.push(0);
        assert!(matches!(
            VerifiedSimulationBundleV5::from_canonical_bytes(trailing),
            Err(SimulationBundleErrorV5::TrailingOrMissingBytes)
        ));
    }

    #[test]
    fn v5_map_decoders_reject_unknown_fields_and_subject_substitution() {
        let bundle = bundle();
        let mut storage: serde_json::Value = serde_json::from_slice(bundle.storage_map()).unwrap();
        storage
            .as_object_mut()
            .unwrap()
            .insert("forged".into(), true.into());
        assert!(matches!(
            SemanticStorageMapV5::from_canonical_json_bytes(&serde_json::to_vec(&storage).unwrap()),
            Err(SimulationBundleErrorV5::InvalidStorageMap)
        ));

        let mut hostile: SemanticAggregateStorageMapV5 =
            serde_json::from_slice(bundle.aggregate_storage_map()).unwrap();
        hostile.bundle_subject_identity = [0x11; 32];
        let hostile = SemanticAggregateStorageMapV5::from_canonical_json_bytes(
            &hostile.to_canonical_json_bytes().unwrap(),
        )
        .unwrap();
        assert_ne!(hostile.bundle_subject_identity(), bundle.subject_identity());

        let hostile_map = hostile.to_canonical_json_bytes().unwrap();
        assert_eq!(hostile_map.len(), bundle.aggregate_storage_map().len());
        let mut hostile_bundle = bundle.canonical_bytes().to_vec();
        let aggregate_start = hostile_bundle.len() - hostile_map.len();
        hostile_bundle[aggregate_start..].copy_from_slice(&hostile_map);
        hostile_bundle[344..376]
            .copy_from_slice(&domain_hash(AGGREGATE_MAP_IDENTITY_DOMAIN_V5, &hostile_map));
        assert!(matches!(
            VerifiedSimulationBundleV5::from_canonical_bytes(hostile_bundle),
            Err(SimulationBundleErrorV5::StorageMapBindingMismatch)
        ));
    }

    #[test]
    fn v5_aggregate_map_reuses_the_exact_v2_hostile_layout_boundary() {
        use crate::SemanticStorageProjectionV2::Field;

        let component = |field, ordinal, slot| {
            SemanticKirComponentStorageV2::new(
                vec![Field { index: field }],
                ordinal,
                ordinal,
                SemanticKirComponentRepresentationV2::ScalarValue,
                slot,
                None,
            )
        };
        let map = |components| {
            SemanticAggregateStorageMapV5::new(
                [0x31; 32],
                [0x32; 32],
                123,
                vec![SemanticKernelStorageV2::new(
                    0,
                    0,
                    0,
                    16,
                    8,
                    vec![SemanticArgumentStorageV2::new(
                        0,
                        0,
                        0,
                        SemanticArgumentOwnershipV1::ByValue,
                        SemanticComponentStorageBindingV2::exact(components),
                    )],
                )],
            )
        };
        let first = component(0, 0, SemanticKernargSlotV2::new(0, 8, 8));
        let second = component(1, 1, SemanticKernargSlotV2::new(8, 8, 8));
        let exact = map(vec![first.clone(), second.clone()]).unwrap();
        SemanticAggregateStorageMapV5::from_canonical_json_bytes(
            &exact.to_canonical_json_bytes().unwrap(),
        )
        .unwrap();

        let duplicate_path = component(0, 1, SemanticKernargSlotV2::new(8, 8, 8));
        let overlapping_slot = component(1, 1, SemanticKernargSlotV2::new(4, 4, 4));
        let unaligned_offset = component(1, 1, SemanticKernargSlotV2::new(4, 8, 8));
        let out_of_range_slot = component(1, 1, SemanticKernargSlotV2::new(16, 8, 8));
        for hostile in [
            vec![first.clone(), duplicate_path],
            vec![first.clone(), overlapping_slot],
            vec![first.clone(), unaligned_offset],
            vec![first.clone(), out_of_range_slot],
            vec![second, first],
        ] {
            assert!(matches!(
                map(hostile),
                Err(SimulationBundleErrorV5::InvalidAggregateStorageMap)
            ));
        }
        assert!(matches!(
            SemanticAggregateStorageMapV5::new([0; 32], [0x32; 32], 123, exact.kernels().to_vec(),),
            Err(SimulationBundleErrorV5::InvalidAggregateStorageMap)
        ));
        assert!(matches!(
            SemanticAggregateStorageMapV5::new([0x31; 32], [0; 32], 123, exact.kernels().to_vec(),),
            Err(SimulationBundleErrorV5::InvalidAggregateStorageMap)
        ));
        assert!(matches!(
            SemanticAggregateStorageMapV5::new([0x31; 32], [0x32; 32], 0, exact.kernels().to_vec(),),
            Err(SimulationBundleErrorV5::InvalidAggregateStorageMap)
        ));
    }
}
