//! Authority-free simulation bundle carrying exact semantic MIR and storage correspondence.

use std::{error::Error, fmt, ops::Deref};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{DebugSourceMapDocumentV2, MAX_SIMULATION_BUNDLE_BYTES_V2, VerifiedSimulationBundleV2};

pub const SIMULATION_BUNDLE_VERSION_V3: u16 = 3;
pub const SEMANTIC_STORAGE_MAP_SCHEMA_V1: &str = "fe2o3-semantic-storage-map-v1";
pub const MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3: usize = 128 * 1024 * 1024;
pub const MAX_SIMULATION_STORAGE_MAP_BYTES_V3: usize = 8 * 1024 * 1024;
pub const MAX_SIMULATION_STORAGE_BINDINGS_V3: usize = 262_144;
pub const MAX_SIMULATION_BUNDLE_BYTES_V3: usize = MAX_SIMULATION_BUNDLE_BYTES_V2
    + MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3
    + MAX_SIMULATION_STORAGE_MAP_BYTES_V3
    + HEADER_BYTES_V3;

const MAGIC_V3: &[u8; 8] = b"F2SIMB03";
const HEADER_BYTES_V3: usize = 8 + 2 + 2 + 8 + 8 + 4 + 32 + 32 + 32;
const BUNDLE_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V3\0";
const SEMANTIC_MIR_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/SIMULATION-SEMANTIC-MIR/V3\0";
const STORAGE_MAP_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/SIMULATION-STORAGE-MAP/V3\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationBundleIdentityV3([u8; 32]);

impl SimulationBundleIdentityV3 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum SemanticStorageMapSchemaV1 {
    #[serde(rename = "fe2o3-semantic-storage-map-v1")]
    V1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticStorageMapV1 {
    schema: SemanticStorageMapSchemaV1,
    #[serde(with = "hex_identity_v3")]
    bundle_v2_identity: [u8; 32],
    #[serde(with = "hex_identity_v3")]
    bundle_subject_identity: [u8; 32],
    semantic_mir_version: u16,
    #[serde(with = "hex_identity_v3")]
    semantic_mir_sha256: [u8; 32],
    semantic_mir_bytes: u64,
    #[serde(with = "hex_identity_v3")]
    target_layout_identity: [u8; 32],
    #[serde(with = "hex_identity_v3")]
    canonical_kir_v7_sha256: [u8; 32],
    canonical_kir_v7_bytes: u64,
    kernels: Vec<SemanticKernelStorageV1>,
    variables: Vec<SemanticVariableStorageV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticKernelStorageV1 {
    semantic_root: u32,
    semantic_body: u32,
    kir_function_ordinal: u32,
    arguments: Vec<SemanticArgumentStorageV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticArgumentStorageV1 {
    source_ordinal: u32,
    semantic_local: u32,
    semantic_type: u32,
    ownership: SemanticArgumentOwnershipV1,
    storage: SemanticStorageBindingV1,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticVariableStorageV1 {
    #[serde(with = "hex_identity_v3")]
    variable_identity: [u8; 32],
    semantic_function: u32,
    semantic_local: Option<u32>,
    semantic_type: Option<u32>,
    storage: SemanticStorageBindingV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticArgumentOwnershipV1 {
    ByValue,
    SharedBorrow,
    UniqueBorrow,
    ExclusiveOwner,
    RawPointer,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticKirStorageRepresentationV1 {
    Scalar,
    RegionPointer,
    RegionSlice,
    OpaqueFlattened,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStorageUnavailableReasonV1 {
    AbiIgnored,
    NoRetainedKirStorage,
    OptimizedOut,
    AmbiguousCorrespondence,
    UnrepresentedSourceVariable,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticStorageBindingV1 {
    ExactKirParameter {
        kir_parameter_ordinal: u32,
        kir_value_ordinal: u32,
        representation: SemanticKirStorageRepresentationV1,
    },
    Unavailable {
        reason: SemanticStorageUnavailableReasonV1,
    },
    Ambiguous,
}

impl SemanticKernelStorageV1 {
    pub fn new(
        semantic_root: u32,
        semantic_body: u32,
        kir_function_ordinal: u32,
        arguments: Vec<SemanticArgumentStorageV1>,
    ) -> Self {
        Self {
            semantic_root,
            semantic_body,
            kir_function_ordinal,
            arguments,
        }
    }
    pub const fn semantic_root(&self) -> u32 {
        self.semantic_root
    }
    pub const fn semantic_body(&self) -> u32 {
        self.semantic_body
    }
    pub const fn kir_function_ordinal(&self) -> u32 {
        self.kir_function_ordinal
    }
    pub fn arguments(&self) -> &[SemanticArgumentStorageV1] {
        &self.arguments
    }
}

impl SemanticArgumentStorageV1 {
    pub const fn new(
        source_ordinal: u32,
        semantic_local: u32,
        semantic_type: u32,
        ownership: SemanticArgumentOwnershipV1,
        storage: SemanticStorageBindingV1,
    ) -> Self {
        Self {
            source_ordinal,
            semantic_local,
            semantic_type,
            ownership,
            storage,
        }
    }
    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }
    pub const fn semantic_local(&self) -> u32 {
        self.semantic_local
    }
    pub const fn semantic_type(&self) -> u32 {
        self.semantic_type
    }
    pub const fn ownership(&self) -> SemanticArgumentOwnershipV1 {
        self.ownership
    }
    pub const fn storage(&self) -> &SemanticStorageBindingV1 {
        &self.storage
    }
}

impl SemanticVariableStorageV1 {
    pub const fn new(
        variable_identity: [u8; 32],
        semantic_function: u32,
        semantic_local: Option<u32>,
        semantic_type: Option<u32>,
        storage: SemanticStorageBindingV1,
    ) -> Self {
        Self {
            variable_identity,
            semantic_function,
            semantic_local,
            semantic_type,
            storage,
        }
    }
    pub const fn variable_identity(&self) -> [u8; 32] {
        self.variable_identity
    }
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }
    pub const fn semantic_local(&self) -> Option<u32> {
        self.semantic_local
    }
    pub const fn semantic_type(&self) -> Option<u32> {
        self.semantic_type
    }
    pub const fn storage(&self) -> &SemanticStorageBindingV1 {
        &self.storage
    }
}

impl SemanticStorageMapV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bundle_v2_identity: [u8; 32],
        bundle_subject_identity: [u8; 32],
        semantic_mir_version: u16,
        semantic_mir_sha256: [u8; 32],
        semantic_mir_bytes: u64,
        target_layout_identity: [u8; 32],
        canonical_kir_v7_sha256: [u8; 32],
        canonical_kir_v7_bytes: u64,
        kernels: Vec<SemanticKernelStorageV1>,
        variables: Vec<SemanticVariableStorageV1>,
    ) -> Result<Self, SimulationBundleErrorV3> {
        let mut map = Self {
            schema: SemanticStorageMapSchemaV1::V1,
            bundle_v2_identity,
            bundle_subject_identity,
            semantic_mir_version,
            semantic_mir_sha256,
            semantic_mir_bytes,
            target_layout_identity,
            canonical_kir_v7_sha256,
            canonical_kir_v7_bytes,
            kernels,
            variables,
        };
        map.normalize_and_validate()?;
        Ok(map)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, SimulationBundleErrorV3> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V3 {
            return Err(SimulationBundleErrorV3::InvalidStorageMapLength);
        }
        let mut map: Self = serde_json::from_slice(bytes)
            .map_err(|_| SimulationBundleErrorV3::InvalidStorageMap)?;
        map.normalize_and_validate()?;
        if serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV3::Encoding)? != bytes {
            return Err(SimulationBundleErrorV3::NonCanonicalStorageMap);
        }
        Ok(map)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SimulationBundleErrorV3> {
        let mut map = self.clone();
        map.normalize_and_validate()?;
        let bytes = serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV3::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V3 {
            return Err(SimulationBundleErrorV3::InvalidStorageMapLength);
        }
        Ok(bytes)
    }

    pub const fn bundle_v2_identity(&self) -> &[u8; 32] {
        &self.bundle_v2_identity
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
    pub const fn canonical_kir_v7_sha256(&self) -> &[u8; 32] {
        &self.canonical_kir_v7_sha256
    }
    pub const fn canonical_kir_v7_bytes(&self) -> u64 {
        self.canonical_kir_v7_bytes
    }
    pub fn kernels(&self) -> &[SemanticKernelStorageV1] {
        &self.kernels
    }
    pub fn variables(&self) -> &[SemanticVariableStorageV1] {
        &self.variables
    }

    fn normalize_and_validate(&mut self) -> Result<(), SimulationBundleErrorV3> {
        if self.semantic_mir_version == 0
            || self.semantic_mir_bytes == 0
            || self.semantic_mir_bytes > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3 as u64
            || self.canonical_kir_v7_bytes == 0
            || [
                self.bundle_v2_identity,
                self.bundle_subject_identity,
                self.semantic_mir_sha256,
                self.target_layout_identity,
                self.canonical_kir_v7_sha256,
            ]
            .contains(&[0; 32])
            || self.kernels.is_empty()
        {
            return Err(SimulationBundleErrorV3::InvalidStorageMap);
        }
        let count = self
            .kernels
            .iter()
            .try_fold(self.variables.len(), |count, kernel| {
                count.checked_add(kernel.arguments.len())
            })
            .ok_or(SimulationBundleErrorV3::ResourceLimit)?;
        if count > MAX_SIMULATION_STORAGE_BINDINGS_V3 {
            return Err(SimulationBundleErrorV3::ResourceLimit);
        }
        for kernel in &mut self.kernels {
            kernel
                .arguments
                .sort_unstable_by_key(|argument| argument.source_ordinal);
            if kernel
                .arguments
                .windows(2)
                .any(|pair| pair[0].source_ordinal == pair[1].source_ordinal)
                || kernel
                    .arguments
                    .iter()
                    .enumerate()
                    .any(|(index, argument)| argument.source_ordinal as usize != index)
            {
                return Err(SimulationBundleErrorV3::InvalidStorageMap);
            }
        }
        self.kernels
            .sort_unstable_by_key(|kernel| (kernel.semantic_root, kernel.semantic_body));
        if self
            .kernels
            .windows(2)
            .any(|pair| pair[0].semantic_root == pair[1].semantic_root)
            || self
                .kernels
                .windows(2)
                .any(|pair| pair[0].kir_function_ordinal == pair[1].kir_function_ordinal)
        {
            return Err(SimulationBundleErrorV3::InvalidStorageMap);
        }
        self.variables
            .sort_unstable_by_key(|variable| variable.variable_identity);
        if self
            .variables
            .windows(2)
            .any(|pair| pair[0].variable_identity == pair[1].variable_identity)
            || self.variables.iter().any(|variable| {
                variable.variable_identity == [0; 32]
                    || variable.semantic_local.is_some() != variable.semantic_type.is_some()
            })
        {
            return Err(SimulationBundleErrorV3::InvalidStorageMap);
        }
        Ok(())
    }
}

/// Strict custody for V2 debug/source data plus exact admitted semantic MIR.
#[derive(Debug)]
pub struct VerifiedSimulationBundleV3 {
    canonical_bytes: Vec<u8>,
    identity: SimulationBundleIdentityV3,
    inner: VerifiedSimulationBundleV2,
    semantic_mir_range: std::ops::Range<usize>,
    semantic_mir_identity: [u8; 32],
    storage_map_range: std::ops::Range<usize>,
    storage_map_identity: [u8; 32],
}

impl VerifiedSimulationBundleV3 {
    pub fn new(
        inner: VerifiedSimulationBundleV2,
        semantic_mir: Vec<u8>,
        storage_map: SemanticStorageMapV1,
    ) -> Result<Self, SimulationBundleErrorV3> {
        inner
            .revalidate()
            .map_err(|_| SimulationBundleErrorV3::InvalidV2Bundle)?;
        if semantic_mir.is_empty() || semantic_mir.len() > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3 {
            return Err(SimulationBundleErrorV3::InvalidSemanticMirLength);
        }
        let semantic_identity = sha256(&semantic_mir);
        if storage_map.bundle_v2_identity != *inner.identity().as_bytes()
            || storage_map.bundle_subject_identity != *inner.subject_identity()
            || storage_map.semantic_mir_sha256 != semantic_identity
            || storage_map.semantic_mir_bytes != semantic_mir.len() as u64
            || storage_map.canonical_kir_v7_sha256 != *inner.canonical_kir_v7_identity().digest()
            || storage_map.canonical_kir_v7_bytes
                != inner.canonical_kir_v7_identity().canonical_length()
        {
            return Err(SimulationBundleErrorV3::StorageMapBindingMismatch);
        }
        let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(inner.debug_map())
            .map_err(|_| SimulationBundleErrorV3::InvalidV2Bundle)?;
        validate_source_variable_storage(&storage_map, &source_map)?;
        let map = storage_map.to_canonical_json_bytes()?;
        let total = HEADER_BYTES_V3
            .checked_add(inner.canonical_bytes().len())
            .and_then(|value| value.checked_add(semantic_mir.len()))
            .and_then(|value| value.checked_add(map.len()))
            .ok_or(SimulationBundleErrorV3::BundleTooLarge)?;
        if total > MAX_SIMULATION_BUNDLE_BYTES_V3 {
            return Err(SimulationBundleErrorV3::BundleTooLarge);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| SimulationBundleErrorV3::AllocationFailure)?;
        bytes.extend_from_slice(MAGIC_V3);
        bytes.extend_from_slice(&SIMULATION_BUNDLE_VERSION_V3.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(inner.canonical_bytes().len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(semantic_mir.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(map.len() as u32).to_le_bytes());
        bytes.extend_from_slice(inner.identity().as_bytes());
        bytes.extend_from_slice(&simulation_semantic_mir_identity_v3(&semantic_mir));
        bytes.extend_from_slice(&simulation_storage_map_identity_v3(&map));
        bytes.extend_from_slice(inner.canonical_bytes());
        bytes.extend_from_slice(&semantic_mir);
        bytes.extend_from_slice(&map);
        Self::from_canonical_bytes(bytes)
    }

    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Result<Self, SimulationBundleErrorV3> {
        if bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V3 {
            return Err(SimulationBundleErrorV3::BundleTooLarge);
        }
        let header = bytes
            .get(..HEADER_BYTES_V3)
            .ok_or(SimulationBundleErrorV3::Truncated)?;
        if header[..8] != *MAGIC_V3 {
            return Err(SimulationBundleErrorV3::InvalidMagic);
        }
        let version = u16::from_le_bytes(header[8..10].try_into().expect("fixed header"));
        if version != SIMULATION_BUNDLE_VERSION_V3 {
            return Err(SimulationBundleErrorV3::UnsupportedVersion(version));
        }
        if header[10..12] != [0; 2] {
            return Err(SimulationBundleErrorV3::InvalidHeader);
        }
        let inner_len = usize::try_from(u64::from_le_bytes(
            header[12..20].try_into().expect("fixed header"),
        ))
        .map_err(|_| SimulationBundleErrorV3::BundleTooLarge)?;
        let semantic_len = usize::try_from(u64::from_le_bytes(
            header[20..28].try_into().expect("fixed header"),
        ))
        .map_err(|_| SimulationBundleErrorV3::BundleTooLarge)?;
        let map_len = u32::from_le_bytes(header[28..32].try_into().expect("fixed header")) as usize;
        if inner_len == 0
            || inner_len > MAX_SIMULATION_BUNDLE_BYTES_V2
            || semantic_len == 0
            || semantic_len > MAX_SIMULATION_SEMANTIC_MIR_BYTES_V3
            || map_len == 0
            || map_len > MAX_SIMULATION_STORAGE_MAP_BYTES_V3
        {
            return Err(SimulationBundleErrorV3::InvalidLength);
        }
        let inner_start = HEADER_BYTES_V3;
        let inner_end = inner_start
            .checked_add(inner_len)
            .ok_or(SimulationBundleErrorV3::BundleTooLarge)?;
        let semantic_end = inner_end
            .checked_add(semantic_len)
            .ok_or(SimulationBundleErrorV3::BundleTooLarge)?;
        let map_end = semantic_end
            .checked_add(map_len)
            .ok_or(SimulationBundleErrorV3::BundleTooLarge)?;
        if map_end != bytes.len() {
            return Err(SimulationBundleErrorV3::TrailingOrMissingBytes);
        }
        let inner_bytes = &bytes[inner_start..inner_end];
        let semantic_bytes = &bytes[inner_end..semantic_end];
        let map_bytes = &bytes[semantic_end..map_end];
        let claimed_inner: [u8; 32] = header[32..64].try_into().expect("fixed header");
        let claimed_semantic: [u8; 32] = header[64..96].try_into().expect("fixed header");
        let claimed_map: [u8; 32] = header[96..128].try_into().expect("fixed header");
        let inner = VerifiedSimulationBundleV2::from_canonical_bytes(inner_bytes.to_vec())
            .map_err(|_| SimulationBundleErrorV3::InvalidV2Bundle)?;
        if claimed_inner != *inner.identity().as_bytes()
            || claimed_semantic != simulation_semantic_mir_identity_v3(semantic_bytes)
            || claimed_map != simulation_storage_map_identity_v3(map_bytes)
        {
            return Err(SimulationBundleErrorV3::SectionIdentityMismatch);
        }
        let map = SemanticStorageMapV1::from_canonical_json_bytes(map_bytes)?;
        if map.bundle_v2_identity != claimed_inner
            || map.bundle_subject_identity != *inner.subject_identity()
            || map.semantic_mir_sha256 != sha256(semantic_bytes)
            || map.semantic_mir_bytes != semantic_len as u64
            || map.canonical_kir_v7_sha256 != *inner.canonical_kir_v7_identity().digest()
            || map.canonical_kir_v7_bytes != inner.canonical_kir_v7_identity().canonical_length()
        {
            return Err(SimulationBundleErrorV3::StorageMapBindingMismatch);
        }
        let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(inner.debug_map())
            .map_err(|_| SimulationBundleErrorV3::InvalidV2Bundle)?;
        validate_source_variable_storage(&map, &source_map)?;
        Ok(Self {
            identity: SimulationBundleIdentityV3(domain_hash(BUNDLE_IDENTITY_DOMAIN_V3, &bytes)),
            canonical_bytes: bytes,
            inner,
            semantic_mir_range: inner_end..semantic_end,
            semantic_mir_identity: claimed_semantic,
            storage_map_range: semantic_end..map_end,
            storage_map_identity: claimed_map,
        })
    }

    pub fn revalidate(&self) -> Result<(), SimulationBundleErrorV3> {
        let decoded = Self::from_canonical_bytes(self.canonical_bytes.clone())?;
        if decoded.identity != self.identity
            || decoded.semantic_mir_identity != self.semantic_mir_identity
            || decoded.storage_map_identity != self.storage_map_identity
        {
            return Err(SimulationBundleErrorV3::IdentityMismatch);
        }
        Ok(())
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub const fn identity(&self) -> SimulationBundleIdentityV3 {
        self.identity
    }
    pub const fn inner_v2(&self) -> &VerifiedSimulationBundleV2 {
        &self.inner
    }
    pub fn into_inner_v2(self) -> VerifiedSimulationBundleV2 {
        self.inner
    }
    pub fn semantic_mir(&self) -> &[u8] {
        &self.canonical_bytes[self.semantic_mir_range.clone()]
    }
    pub const fn semantic_mir_identity(&self) -> &[u8; 32] {
        &self.semantic_mir_identity
    }
    pub fn storage_map(&self) -> &[u8] {
        &self.canonical_bytes[self.storage_map_range.clone()]
    }
    pub const fn storage_map_identity(&self) -> &[u8; 32] {
        &self.storage_map_identity
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

impl Deref for VerifiedSimulationBundleV3 {
    type Target = VerifiedSimulationBundleV2;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

trait DebugSourceMapDocumentVariableKey {
    fn identity(&self) -> [u8; 32];
}
impl DebugSourceMapDocumentVariableKey for crate::DebugSourceVariableV2 {
    fn identity(&self) -> [u8; 32] {
        self.identity()
    }
}

fn validate_source_variable_storage(
    map: &SemanticStorageMapV1,
    source_map: &DebugSourceMapDocumentV2,
) -> Result<(), SimulationBundleErrorV3> {
    if source_map.variables().len() != map.variables.len() {
        return Err(SimulationBundleErrorV3::StorageMapBindingMismatch);
    }
    for binding in &map.variables {
        let source_index = source_map
            .variables()
            .binary_search_by_key(
                &binding.variable_identity,
                DebugSourceMapDocumentVariableKey::identity,
            )
            .map_err(|_| SimulationBundleErrorV3::StorageMapBindingMismatch)?;
        let source = &source_map.variables()[source_index];
        let function_ordinal = map
            .kernels
            .iter()
            .find(|kernel| kernel.semantic_body == binding.semantic_function)
            .map(|kernel| u64::from(kernel.kir_function_ordinal))
            .ok_or(SimulationBundleErrorV3::StorageMapBindingMismatch)?;
        if source.function_ordinal() != function_ordinal {
            return Err(SimulationBundleErrorV3::StorageMapBindingMismatch);
        }
        match (&binding.storage, source.function_binding()) {
            (
                SemanticStorageBindingV1::ExactKirParameter {
                    kir_value_ordinal, ..
                },
                Some(source_binding),
            ) if source_binding.generation() == 1
                && source_binding.value_ordinal() == u64::from(*kir_value_ordinal) => {}
            (SemanticStorageBindingV1::ExactKirParameter { .. }, _) | (_, Some(_)) => {
                return Err(SimulationBundleErrorV3::StorageMapBindingMismatch);
            }
            (_, None) => {}
        }
    }
    Ok(())
}

pub fn simulation_semantic_mir_identity_v3(bytes: &[u8]) -> [u8; 32] {
    domain_hash(SEMANTIC_MIR_IDENTITY_DOMAIN_V3, bytes)
}
pub fn simulation_storage_map_identity_v3(bytes: &[u8]) -> [u8; 32] {
    domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V3, bytes)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationBundleErrorV3 {
    BundleTooLarge,
    InvalidLength,
    InvalidSemanticMirLength,
    InvalidStorageMapLength,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidHeader,
    Truncated,
    TrailingOrMissingBytes,
    InvalidV2Bundle,
    InvalidStorageMap,
    NonCanonicalStorageMap,
    StorageMapBindingMismatch,
    SectionIdentityMismatch,
    ResourceLimit,
    AllocationFailure,
    Encoding,
    IdentityMismatch,
}
impl fmt::Display for SimulationBundleErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid fe2o3 simulation bundle V3: {self:?}")
    }
}
impl Error for SimulationBundleErrorV3 {}

mod hex_identity_v3 {
    use serde::{Deserialize, Deserializer, Serializer, de};
    pub fn serialize<S>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut encoded = [0_u8; 64];
        for (index, byte) in value.iter().copied().enumerate() {
            encoded[index * 2] = hex(byte >> 4);
            encoded[index * 2 + 1] = hex(byte & 0x0f);
        }
        serializer.serialize_str(std::str::from_utf8(&encoded).expect("hex is ASCII"))
    }
    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        if text.len() != 64 || !text.is_ascii() {
            return Err(de::Error::custom(
                "identity must be 64 lowercase hex digits",
            ));
        }
        let mut decoded = [0_u8; 32];
        for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
            let high = nibble(pair[0])
                .ok_or_else(|| de::Error::custom("identity must use lowercase hex"))?;
            let low = nibble(pair[1])
                .ok_or_else(|| de::Error::custom("identity must use lowercase hex"))?;
            decoded[index] = (high << 4) | low;
        }
        Ok(decoded)
    }
    const fn hex(value: u8) -> u8 {
        if value < 10 {
            b'0' + value
        } else {
            b'a' + value - 10
        }
    }
    const fn nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BasicBlock, BlockId, DebugSourceMapBindingV1, DebugSourceMapDocumentV2,
        DebugSourceMapFileV1, DebugSourceMapSpanV1, Function, Kernel, LaunchDomain, LaunchExtent,
        Module, PreparedSimulationBundleV1, Signature, SimulationCompilerExecutionBindingV1,
        SimulationProductionKirIdentityV1, SimulationSourceLineageV1, Terminator, Type, ValueId,
        VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV8, WorkgroupSize,
    };

    fn v2_bundle() -> VerifiedSimulationBundleV2 {
        let mut module = Module::new("bundle_v3_test");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        module.functions.push(Function::kernel_entry(
            "kernel",
            Signature::new(vec![Type::F32], vec![]),
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
        let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
        let inner = PreparedSimulationBundleV1::new(
            SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
            SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
            SimulationProductionKirIdentityV1::v8(
                *production.identity().digest(),
                production.identity().canonical_length(),
            )
            .unwrap(),
            "gfx942:xnack-",
            VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
        )
        .unwrap()
        .finalize_without_source_map()
        .unwrap();
        let map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                *inner.subject_identity(),
                *inner.canonical_kir_v7_identity().digest(),
                inner.canonical_kir_v7_identity().canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new([4; 32], 16, "/src/kernel.rs".into()).unwrap()],
            Vec::new(),
            vec![DebugSourceMapSpanV1::new([4; 32], 1, 2, 1, 2).unwrap()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        VerifiedSimulationBundleV2::new(inner, map).unwrap()
    }

    fn bundle() -> VerifiedSimulationBundleV3 {
        let inner = v2_bundle();
        let semantic = b"exact-production-semantic-mir-fixture".to_vec();
        let storage = SemanticStorageMapV1::new(
            *inner.identity().as_bytes(),
            *inner.subject_identity(),
            9,
            sha256(&semantic),
            semantic.len() as u64,
            [9; 32],
            *inner.canonical_kir_v7_identity().digest(),
            inner.canonical_kir_v7_identity().canonical_length(),
            vec![SemanticKernelStorageV1::new(
                0,
                0,
                0,
                vec![SemanticArgumentStorageV1::new(
                    0,
                    1,
                    0,
                    SemanticArgumentOwnershipV1::ByValue,
                    SemanticStorageBindingV1::ExactKirParameter {
                        kir_parameter_ordinal: 0,
                        kir_value_ordinal: 7,
                        representation: SemanticKirStorageRepresentationV1::Scalar,
                    },
                )],
            )],
            Vec::new(),
        )
        .unwrap();
        VerifiedSimulationBundleV3::new(inner, semantic, storage).unwrap()
    }

    #[test]
    fn v3_round_trips_without_broadening_v2() {
        let bundle = bundle();
        let decoded =
            VerifiedSimulationBundleV3::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .unwrap();
        assert_eq!(decoded.canonical_bytes(), bundle.canonical_bytes());
        assert_eq!(decoded.identity(), bundle.identity());
        assert_eq!(
            decoded.semantic_mir(),
            b"exact-production-semantic-mir-fixture"
        );
        assert!(
            VerifiedSimulationBundleV2::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .is_err()
        );
        assert!(!decoded.authenticates_compiler_execution());
        assert!(!decoded.grants_hardware_authority());
    }

    #[test]
    fn every_section_and_trailing_bytes_are_fail_closed() {
        let bundle = bundle();
        let bytes = bundle.canonical_bytes();
        let inner_len = u64::from_le_bytes(bytes[12..20].try_into().unwrap()) as usize;
        let semantic_len = u64::from_le_bytes(bytes[20..28].try_into().unwrap()) as usize;
        for offset in [
            32_usize,
            64,
            96,
            HEADER_BYTES_V3,
            HEADER_BYTES_V3 + inner_len,
            HEADER_BYTES_V3 + inner_len + semantic_len,
            bytes.len() - 1,
        ] {
            let mut hostile = bytes.to_vec();
            hostile[offset] ^= 1;
            assert!(VerifiedSimulationBundleV3::from_canonical_bytes(hostile).is_err());
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(matches!(
            VerifiedSimulationBundleV3::from_canonical_bytes(trailing),
            Err(SimulationBundleErrorV3::TrailingOrMissingBytes)
        ));
    }

    #[test]
    fn storage_map_rejects_ambiguous_rosters_and_noncanonical_json() {
        let bundle = bundle();
        let map = SemanticStorageMapV1::from_canonical_json_bytes(bundle.storage_map()).unwrap();
        let mut noncanonical = map.to_canonical_json_bytes().unwrap();
        noncanonical.push(b' ');
        assert!(matches!(
            SemanticStorageMapV1::from_canonical_json_bytes(&noncanonical),
            Err(SimulationBundleErrorV3::NonCanonicalStorageMap)
        ));
        assert!(matches!(
            SemanticStorageMapV1::new(
                *map.bundle_v2_identity(),
                *map.bundle_subject_identity(),
                map.semantic_mir_version(),
                *map.semantic_mir_sha256(),
                map.semantic_mir_bytes(),
                *map.target_layout_identity(),
                *map.canonical_kir_v7_sha256(),
                map.canonical_kir_v7_bytes(),
                vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
                vec![
                    SemanticVariableStorageV1::new(
                        [1; 32],
                        0,
                        None,
                        None,
                        SemanticStorageBindingV1::Ambiguous,
                    ),
                    SemanticVariableStorageV1::new(
                        [1; 32],
                        0,
                        None,
                        None,
                        SemanticStorageBindingV1::Ambiguous,
                    ),
                ],
            ),
            Err(SimulationBundleErrorV3::InvalidStorageMap)
        ));
    }
}
