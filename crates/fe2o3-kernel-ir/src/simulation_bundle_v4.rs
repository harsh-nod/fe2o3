//! Additive, content-bound evidence for one-to-many semantic value materialization.

use std::{error::Error, fmt, ops::Deref};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    MAX_SIMULATION_BUNDLE_BYTES_V3, MAX_SIMULATION_STORAGE_BINDINGS_V3,
    SemanticArgumentOwnershipV1, SemanticStorageUnavailableReasonV1, VerifiedSimulationBundleV3,
};

/// Frozen binary schema version for the aggregate-materialization envelope.
pub const SIMULATION_BUNDLE_VERSION_V4: u16 = 4;
/// Canonical schema tag for the independently versioned component map.
pub const SEMANTIC_STORAGE_MAP_SCHEMA_V2: &str = "fe2o3-semantic-storage-map-v2";
/// The V2 map has the same bounded storage budget as the V1 semantic map.
pub const MAX_SIMULATION_STORAGE_MAP_BYTES_V4: usize = 8 * 1024 * 1024;
/// Maximum total number of source-layout projection steps retained by one map.
pub const MAX_SIMULATION_STORAGE_PROJECTIONS_V4: usize = MAX_SIMULATION_STORAGE_BINDINGS_V3 * 32;
/// Maximum nesting depth of one semantic component path.
pub const MAX_SIMULATION_STORAGE_PROJECTION_DEPTH_V4: usize = 256;
/// Complete V4 envelope bound.
pub const MAX_SIMULATION_BUNDLE_BYTES_V4: usize =
    MAX_SIMULATION_BUNDLE_BYTES_V3 + MAX_SIMULATION_STORAGE_MAP_BYTES_V4 + HEADER_BYTES_V4;

const MAGIC_V4: &[u8; 8] = b"F2SIMB04";
const HEADER_BYTES_V4: usize = 8 + 2 + 2 + 8 + 4 + 32 + 32;
const BUNDLE_IDENTITY_DOMAIN_V4: &[u8] = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V4\0";
const STORAGE_MAP_IDENTITY_DOMAIN_V4: &[u8] = b"FE2O3/SIMULATION-STORAGE-MAP/V4\0";

/// Content identity of a complete canonical V4 envelope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationBundleIdentityV4([u8; 32]);

impl SimulationBundleIdentityV4 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum SemanticStorageMapSchemaV2 {
    #[serde(rename = "fe2o3-semantic-storage-map-v2")]
    V2,
}

/// A source-layout projection. Consumers rederive its semantic byte range for
/// type and validity checks independently of the physical kernarg slot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticStorageProjectionV2 {
    Field { index: u32 },
    ArrayElement { index: u64 },
    EnumVariant { index: u32 },
    EnumDiscriminant,
}

/// Exact KIR representation of one semantic component.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticKirComponentRepresentationV2 {
    ScalarValue,
    RegionPointer,
    RegionSlice,
}

/// One address-free physical slot in the producer-described explicit kernarg
/// image. The V4 identity content-binds these facts but grants no compiler
/// authority; the map never stores host or device addresses.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticKernargSlotV2 {
    byte_offset: u32,
    byte_width: u32,
    byte_alignment: u32,
}

impl SemanticKernargSlotV2 {
    pub const fn new(byte_offset: u32, byte_width: u32, byte_alignment: u32) -> Self {
        Self {
            byte_offset,
            byte_width,
            byte_alignment,
        }
    }

    pub const fn byte_offset(self) -> u32 {
        self.byte_offset
    }

    pub const fn byte_width(self) -> u32 {
        self.byte_width
    }

    pub const fn byte_alignment(self) -> u32 {
        self.byte_alignment
    }

    fn end(self) -> Option<u32> {
        self.byte_offset.checked_add(self.byte_width)
    }

    fn is_structurally_valid(self, total_bytes: u32, allow_empty: bool) -> bool {
        (allow_empty || self.byte_width != 0)
            && self.byte_alignment != 0
            && self.byte_alignment.is_power_of_two()
            && self.byte_offset.is_multiple_of(self.byte_alignment)
            && self.end().is_some_and(|end| end <= total_bytes)
    }
}

/// One leaf of a semantic argument and its exact KIR parameter.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticKirComponentStorageV2 {
    path: Vec<SemanticStorageProjectionV2>,
    kir_parameter_ordinal: u32,
    kir_value_ordinal: u32,
    representation: SemanticKirComponentRepresentationV2,
    value_slot: SemanticKernargSlotV2,
    metadata_slot: Option<SemanticKernargSlotV2>,
}

impl SemanticKirComponentStorageV2 {
    pub fn new(
        path: Vec<SemanticStorageProjectionV2>,
        kir_parameter_ordinal: u32,
        kir_value_ordinal: u32,
        representation: SemanticKirComponentRepresentationV2,
        value_slot: SemanticKernargSlotV2,
        metadata_slot: Option<SemanticKernargSlotV2>,
    ) -> Self {
        Self {
            path,
            kir_parameter_ordinal,
            kir_value_ordinal,
            representation,
            value_slot,
            metadata_slot,
        }
    }

    pub fn path(&self) -> &[SemanticStorageProjectionV2] {
        &self.path
    }

    pub const fn kir_parameter_ordinal(&self) -> u32 {
        self.kir_parameter_ordinal
    }

    pub const fn kir_value_ordinal(&self) -> u32 {
        self.kir_value_ordinal
    }

    pub const fn representation(&self) -> SemanticKirComponentRepresentationV2 {
        self.representation
    }

    pub const fn value_slot(&self) -> SemanticKernargSlotV2 {
        self.value_slot
    }

    pub const fn metadata_slot(&self) -> Option<SemanticKernargSlotV2> {
        self.metadata_slot
    }
}

/// Closed materialization status for one source argument.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticComponentStorageBindingV2 {
    ExactKirComponents {
        components: Vec<SemanticKirComponentStorageV2>,
    },
    Unavailable {
        reason: SemanticStorageUnavailableReasonV1,
    },
    Ambiguous,
}

impl SemanticComponentStorageBindingV2 {
    pub fn exact(components: Vec<SemanticKirComponentStorageV2>) -> Self {
        Self::ExactKirComponents { components }
    }

    pub fn components(&self) -> Option<&[SemanticKirComponentStorageV2]> {
        match self {
            Self::ExactKirComponents { components } => Some(components),
            Self::Unavailable { .. } | Self::Ambiguous => None,
        }
    }
}

/// V2 correspondence for one source argument. The repeated semantic identity
/// is checked against V3 and prevents ordinal-only relabeling.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticArgumentStorageV2 {
    source_ordinal: u32,
    semantic_local: u32,
    semantic_type: u32,
    ownership: SemanticArgumentOwnershipV1,
    storage: SemanticComponentStorageBindingV2,
}

impl SemanticArgumentStorageV2 {
    pub const fn new(
        source_ordinal: u32,
        semantic_local: u32,
        semantic_type: u32,
        ownership: SemanticArgumentOwnershipV1,
        storage: SemanticComponentStorageBindingV2,
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

    pub const fn storage(&self) -> &SemanticComponentStorageBindingV2 {
        &self.storage
    }
}

/// V2 component roster for one semantic/KIR kernel correspondence.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticKernelStorageV2 {
    semantic_root: u32,
    semantic_body: u32,
    kir_function_ordinal: u32,
    explicit_kernarg_bytes: u32,
    explicit_kernarg_alignment: u32,
    arguments: Vec<SemanticArgumentStorageV2>,
}

impl SemanticKernelStorageV2 {
    pub fn new(
        semantic_root: u32,
        semantic_body: u32,
        kir_function_ordinal: u32,
        explicit_kernarg_bytes: u32,
        explicit_kernarg_alignment: u32,
        arguments: Vec<SemanticArgumentStorageV2>,
    ) -> Self {
        Self {
            semantic_root,
            semantic_body,
            kir_function_ordinal,
            explicit_kernarg_bytes,
            explicit_kernarg_alignment,
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

    pub const fn explicit_kernarg_bytes(&self) -> u32 {
        self.explicit_kernarg_bytes
    }

    pub const fn explicit_kernarg_alignment(&self) -> u32 {
        self.explicit_kernarg_alignment
    }

    pub fn arguments(&self) -> &[SemanticArgumentStorageV2] {
        &self.arguments
    }
}

/// Canonical one-to-many storage and physical-kernarg map bound to the exact
/// complete V3 payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticStorageMapV2 {
    schema: SemanticStorageMapSchemaV2,
    #[serde(with = "hex_identity_v4")]
    bundle_v3_identity: [u8; 32],
    kernels: Vec<SemanticKernelStorageV2>,
}

impl SemanticStorageMapV2 {
    pub fn new(
        bundle_v3_identity: [u8; 32],
        kernels: Vec<SemanticKernelStorageV2>,
    ) -> Result<Self, SimulationBundleErrorV4> {
        let mut map = Self {
            schema: SemanticStorageMapSchemaV2::V2,
            bundle_v3_identity,
            kernels,
        };
        map.normalize_and_validate()?;
        Ok(map)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, SimulationBundleErrorV4> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V4 {
            return Err(SimulationBundleErrorV4::InvalidStorageMapLength);
        }
        let mut map: Self = serde_json::from_slice(bytes)
            .map_err(|_| SimulationBundleErrorV4::InvalidStorageMap)?;
        map.normalize_and_validate()?;
        if serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV4::Encoding)? != bytes {
            return Err(SimulationBundleErrorV4::NonCanonicalStorageMap);
        }
        Ok(map)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SimulationBundleErrorV4> {
        let mut map = self.clone();
        map.normalize_and_validate()?;
        let bytes = serde_json::to_vec(&map).map_err(|_| SimulationBundleErrorV4::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_STORAGE_MAP_BYTES_V4 {
            return Err(SimulationBundleErrorV4::InvalidStorageMapLength);
        }
        Ok(bytes)
    }

    pub const fn bundle_v3_identity(&self) -> &[u8; 32] {
        &self.bundle_v3_identity
    }

    pub fn kernels(&self) -> &[SemanticKernelStorageV2] {
        &self.kernels
    }

    fn normalize_and_validate(&mut self) -> Result<(), SimulationBundleErrorV4> {
        if self.bundle_v3_identity == [0; 32] || self.kernels.is_empty() {
            return Err(SimulationBundleErrorV4::InvalidStorageMap);
        }
        if self.kernels.len() > MAX_SIMULATION_STORAGE_BINDINGS_V3 {
            return Err(SimulationBundleErrorV4::ResourceLimit);
        }
        let (bindings, components, projections) = self.kernels.iter().try_fold(
            (0_usize, 0_usize, 0_usize),
            |(bindings, components, projections), kernel| {
                kernel.arguments.iter().try_fold(
                    (bindings, components, projections),
                    |(bindings, components, projections), argument| {
                        let Some(argument_components) = argument.storage.components() else {
                            return Ok((
                                bindings
                                    .checked_add(1)
                                    .ok_or(SimulationBundleErrorV4::ResourceLimit)?,
                                components,
                                projections,
                            ));
                        };
                        let argument_projections =
                            argument_components
                                .iter()
                                .try_fold(0_usize, |total, component| {
                                    if component.path.len()
                                        > MAX_SIMULATION_STORAGE_PROJECTION_DEPTH_V4
                                    {
                                        return Err(SimulationBundleErrorV4::ResourceLimit);
                                    }
                                    total
                                        .checked_add(component.path.len())
                                        .ok_or(SimulationBundleErrorV4::ResourceLimit)
                                })?;
                        Ok((
                            bindings
                                .checked_add(argument_components.len().max(1))
                                .ok_or(SimulationBundleErrorV4::ResourceLimit)?,
                            components
                                .checked_add(argument_components.len())
                                .ok_or(SimulationBundleErrorV4::ResourceLimit)?,
                            projections
                                .checked_add(argument_projections)
                                .ok_or(SimulationBundleErrorV4::ResourceLimit)?,
                        ))
                    },
                )
            },
        )?;
        if bindings > MAX_SIMULATION_STORAGE_BINDINGS_V3
            || projections > MAX_SIMULATION_STORAGE_PROJECTIONS_V4
        {
            return Err(SimulationBundleErrorV4::ResourceLimit);
        }

        let mut parameter_owners = Vec::new();
        parameter_owners
            .try_reserve_exact(components)
            .map_err(|_| SimulationBundleErrorV4::AllocationFailure)?;
        let slot_capacity = components
            .checked_mul(2)
            .ok_or(SimulationBundleErrorV4::ResourceLimit)?;
        let mut physical_slots = Vec::new();
        physical_slots
            .try_reserve_exact(slot_capacity)
            .map_err(|_| SimulationBundleErrorV4::AllocationFailure)?;
        for kernel in &mut self.kernels {
            if kernel.explicit_kernarg_alignment == 0
                || !kernel.explicit_kernarg_alignment.is_power_of_two()
                || !kernel
                    .explicit_kernarg_bytes
                    .is_multiple_of(kernel.explicit_kernarg_alignment)
            {
                return Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout);
            }
            kernel
                .arguments
                .sort_unstable_by_key(|argument| argument.source_ordinal);
            if kernel
                .arguments
                .iter()
                .enumerate()
                .any(|(index, argument)| argument.source_ordinal as usize != index)
            {
                return Err(SimulationBundleErrorV4::InvalidStorageMap);
            }
            for argument in &mut kernel.arguments {
                let SemanticComponentStorageBindingV2::ExactKirComponents { components } =
                    &mut argument.storage
                else {
                    continue;
                };
                for component in components.iter() {
                    let value_end = component.value_slot.end();
                    let metadata_end = component.metadata_slot.and_then(SemanticKernargSlotV2::end);
                    if !component
                        .value_slot
                        .is_structurally_valid(kernel.explicit_kernarg_bytes, false)
                        || component.value_slot.byte_alignment > kernel.explicit_kernarg_alignment
                        || component.metadata_slot.is_some_and(|slot| {
                            !slot.is_structurally_valid(kernel.explicit_kernarg_bytes, false)
                                || slot.byte_alignment > kernel.explicit_kernarg_alignment
                        })
                        || matches!(
                            component.representation,
                            SemanticKirComponentRepresentationV2::RegionSlice
                        ) != component.metadata_slot.is_some()
                        || component.metadata_slot.is_some_and(|metadata| {
                            component.value_slot.byte_offset < metadata_end.unwrap_or(0)
                                && metadata.byte_offset < value_end.unwrap_or(u32::MAX)
                        })
                    {
                        return Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout);
                    }
                    let value_end =
                        value_end.ok_or(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)?;
                    physical_slots.push((
                        kernel.kir_function_ordinal,
                        component.value_slot.byte_offset,
                        value_end,
                    ));
                    if let Some(metadata) = component.metadata_slot {
                        let metadata_end = metadata_end
                            .ok_or(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)?;
                        physical_slots.push((
                            kernel.kir_function_ordinal,
                            metadata.byte_offset,
                            metadata_end,
                        ));
                    }
                }
                components.sort_unstable_by(|left, right| left.path.cmp(&right.path));
                if components
                    .windows(2)
                    .any(|pair| pair[0].path == pair[1].path)
                {
                    return Err(SimulationBundleErrorV4::InvalidStorageMap);
                }
                components.sort_unstable_by_key(|component| component.kir_parameter_ordinal);
                if components
                    .windows(2)
                    .any(|pair| pair[0].kir_parameter_ordinal == pair[1].kir_parameter_ordinal)
                {
                    return Err(SimulationBundleErrorV4::InvalidStorageMap);
                }
                parameter_owners.extend(components.iter().map(|component| {
                    (kernel.kir_function_ordinal, component.kir_parameter_ordinal)
                }));
            }
        }
        parameter_owners.sort_unstable();
        if parameter_owners.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SimulationBundleErrorV4::InvalidStorageMap);
        }
        physical_slots.sort_unstable();
        if physical_slots
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].2 > pair[1].1)
        {
            return Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout);
        }

        self.kernels
            .sort_unstable_by_key(|kernel| kernel.kir_function_ordinal);
        if self
            .kernels
            .windows(2)
            .any(|pair| pair[0].kir_function_ordinal == pair[1].kir_function_ordinal)
        {
            return Err(SimulationBundleErrorV4::InvalidStorageMap);
        }
        self.kernels.sort_unstable_by_key(|kernel| {
            (
                kernel.semantic_root,
                kernel.semantic_body,
                kernel.kir_function_ordinal,
            )
        });
        if self
            .kernels
            .windows(2)
            .any(|pair| pair[0].semantic_root == pair[1].semantic_root)
        {
            return Err(SimulationBundleErrorV4::InvalidStorageMap);
        }
        Ok(())
    }
}

/// Strict custody for an unchanged V3 payload and its V2 component map.
#[derive(Debug)]
pub struct VerifiedSimulationBundleV4 {
    canonical_bytes: Vec<u8>,
    identity: SimulationBundleIdentityV4,
    inner: VerifiedSimulationBundleV3,
    storage_map_range: std::ops::Range<usize>,
    storage_map_identity: [u8; 32],
}

impl VerifiedSimulationBundleV4 {
    /// Identifies only the complete V4 magic prefix. Short and foreign inputs
    /// remain available to earlier-version decoders and their existing errors.
    pub fn has_magic_prefix(bytes: &[u8]) -> bool {
        bytes.get(..MAGIC_V4.len()) == Some(MAGIC_V4)
    }

    pub fn new(
        inner: VerifiedSimulationBundleV3,
        storage_map: SemanticStorageMapV2,
    ) -> Result<Self, SimulationBundleErrorV4> {
        inner
            .revalidate()
            .map_err(|_| SimulationBundleErrorV4::InvalidV3Bundle)?;
        if storage_map.bundle_v3_identity != *inner.identity().as_bytes() {
            return Err(SimulationBundleErrorV4::StorageMapBindingMismatch);
        }
        let map = storage_map.to_canonical_json_bytes()?;
        let exact_length = HEADER_BYTES_V4
            .checked_add(inner.canonical_bytes().len())
            .and_then(|length| length.checked_add(map.len()))
            .ok_or(SimulationBundleErrorV4::BundleTooLarge)?;
        if exact_length > MAX_SIMULATION_BUNDLE_BYTES_V4 {
            return Err(SimulationBundleErrorV4::BundleTooLarge);
        }
        let inner_length = u64::try_from(inner.canonical_bytes().len())
            .map_err(|_| SimulationBundleErrorV4::BundleTooLarge)?;
        let map_length = u32::try_from(map.len())
            .map_err(|_| SimulationBundleErrorV4::InvalidStorageMapLength)?;
        let map_identity = simulation_storage_map_identity_v4(&map);
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(exact_length)
            .map_err(|_| SimulationBundleErrorV4::AllocationFailure)?;
        bytes.extend_from_slice(MAGIC_V4);
        bytes.extend_from_slice(&SIMULATION_BUNDLE_VERSION_V4.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&inner_length.to_le_bytes());
        bytes.extend_from_slice(&map_length.to_le_bytes());
        bytes.extend_from_slice(inner.identity().as_bytes());
        bytes.extend_from_slice(&map_identity);
        bytes.extend_from_slice(inner.canonical_bytes());
        bytes.extend_from_slice(&map);
        Self::from_canonical_bytes(bytes)
    }

    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Result<Self, SimulationBundleErrorV4> {
        if bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V4 {
            return Err(SimulationBundleErrorV4::BundleTooLarge);
        }
        let header = bytes
            .get(..HEADER_BYTES_V4)
            .ok_or(SimulationBundleErrorV4::Truncated)?;
        if header[..8] != *MAGIC_V4 {
            return Err(SimulationBundleErrorV4::InvalidMagic);
        }
        let version = u16::from_le_bytes(header[8..10].try_into().expect("fixed header"));
        if version != SIMULATION_BUNDLE_VERSION_V4 {
            return Err(SimulationBundleErrorV4::UnsupportedVersion(version));
        }
        if header[10..12] != [0; 2] {
            return Err(SimulationBundleErrorV4::InvalidHeader);
        }
        let inner_length = usize::try_from(u64::from_le_bytes(
            header[12..20].try_into().expect("fixed header"),
        ))
        .map_err(|_| SimulationBundleErrorV4::BundleTooLarge)?;
        let map_length = usize::try_from(u32::from_le_bytes(
            header[20..24].try_into().expect("fixed header"),
        ))
        .map_err(|_| SimulationBundleErrorV4::InvalidStorageMapLength)?;
        if inner_length == 0
            || inner_length > MAX_SIMULATION_BUNDLE_BYTES_V3
            || map_length == 0
            || map_length > MAX_SIMULATION_STORAGE_MAP_BYTES_V4
        {
            return Err(SimulationBundleErrorV4::InvalidLength);
        }
        let inner_start = HEADER_BYTES_V4;
        let inner_end = inner_start
            .checked_add(inner_length)
            .ok_or(SimulationBundleErrorV4::BundleTooLarge)?;
        let map_end = inner_end
            .checked_add(map_length)
            .ok_or(SimulationBundleErrorV4::BundleTooLarge)?;
        if map_end != bytes.len() {
            return Err(SimulationBundleErrorV4::TrailingOrMissingBytes);
        }
        let inner_bytes = bytes
            .get(inner_start..inner_end)
            .ok_or(SimulationBundleErrorV4::Truncated)?;
        let map_bytes = bytes
            .get(inner_end..map_end)
            .ok_or(SimulationBundleErrorV4::Truncated)?;
        let claimed_inner: [u8; 32] = header[24..56].try_into().expect("fixed header");
        let claimed_map: [u8; 32] = header[56..88].try_into().expect("fixed header");
        let inner = VerifiedSimulationBundleV3::from_canonical_bytes(copy_bytes_v4(inner_bytes)?)
            .map_err(|_| SimulationBundleErrorV4::InvalidV3Bundle)?;
        if claimed_inner != *inner.identity().as_bytes()
            || claimed_map != simulation_storage_map_identity_v4(map_bytes)
        {
            return Err(SimulationBundleErrorV4::SectionIdentityMismatch);
        }
        let map = SemanticStorageMapV2::from_canonical_json_bytes(map_bytes)?;
        if map.bundle_v3_identity != claimed_inner {
            return Err(SimulationBundleErrorV4::StorageMapBindingMismatch);
        }
        Ok(Self {
            identity: SimulationBundleIdentityV4(domain_hash(BUNDLE_IDENTITY_DOMAIN_V4, &bytes)),
            canonical_bytes: bytes,
            inner,
            storage_map_range: inner_end..map_end,
            storage_map_identity: claimed_map,
        })
    }

    pub fn revalidate(&self) -> Result<(), SimulationBundleErrorV4> {
        let decoded = Self::from_canonical_bytes(copy_bytes_v4(&self.canonical_bytes)?)?;
        if decoded.identity != self.identity
            || decoded.storage_map_identity != self.storage_map_identity
            || decoded.inner.identity() != self.inner.identity()
        {
            return Err(SimulationBundleErrorV4::IdentityMismatch);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    pub const fn identity(&self) -> SimulationBundleIdentityV4 {
        self.identity
    }

    pub const fn inner_v3(&self) -> &VerifiedSimulationBundleV3 {
        &self.inner
    }

    pub fn into_inner_v3(self) -> VerifiedSimulationBundleV3 {
        self.inner
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

impl Deref for VerifiedSimulationBundleV4 {
    type Target = VerifiedSimulationBundleV3;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Domain-separated identity of canonical V2 component-map bytes.
pub fn simulation_storage_map_identity_v4(bytes: &[u8]) -> [u8; 32] {
    domain_hash(STORAGE_MAP_IDENTITY_DOMAIN_V4, bytes)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn copy_bytes_v4(bytes: &[u8]) -> Result<Vec<u8>, SimulationBundleErrorV4> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| SimulationBundleErrorV4::AllocationFailure)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

/// Fail-closed V4 custody or component-map error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationBundleErrorV4 {
    BundleTooLarge,
    InvalidLength,
    InvalidStorageMapLength,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidHeader,
    Truncated,
    TrailingOrMissingBytes,
    InvalidV3Bundle,
    InvalidStorageMap,
    InvalidPhysicalKernargLayout,
    NonCanonicalStorageMap,
    StorageMapBindingMismatch,
    SectionIdentityMismatch,
    ResourceLimit,
    AllocationFailure,
    Encoding,
    IdentityMismatch,
}

impl fmt::Display for SimulationBundleErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid fe2o3 simulation bundle V4: {self:?}")
    }
}

impl Error for SimulationBundleErrorV4 {}

mod hex_identity_v4 {
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
        Module, PreparedSimulationBundleV1, SemanticArgumentStorageV1, SemanticKernelStorageV1,
        SemanticKirStorageRepresentationV1, SemanticStorageBindingV1, SemanticStorageMapV1,
        Signature, SimulationCompilerExecutionBindingV1, SimulationProductionKirIdentityV1,
        SimulationSourceLineageV1, Terminator, Type, ValueId, VerifiedCanonicalKernelIrV7,
        VerifiedCanonicalKernelIrV8, VerifiedSimulationBundleV2, WorkgroupSize,
    };

    fn v3_bundle() -> VerifiedSimulationBundleV3 {
        let mut module = Module::new("bundle_v4_test");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        module.functions.push(Function::kernel_entry(
            "kernel",
            Signature::new(
                vec![
                    Type::Scalar(crate::ScalarType::U16),
                    Type::Scalar(crate::ScalarType::U64),
                ],
                vec![],
            ),
            vec![ValueId(7), ValueId(8)],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "kernel",
            "kernel",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize { x: 1, y: 1, z: 1 });
        module.kernels.push(kernel);
        let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
        let v1 = PreparedSimulationBundleV1::new(
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
        let source_map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                *v1.subject_identity(),
                *v1.canonical_kir_v7_identity().digest(),
                v1.canonical_kir_v7_identity().canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new([4; 32], 16, "/src/kernel.rs".into()).unwrap()],
            Vec::new(),
            vec![DebugSourceMapSpanV1::new([4; 32], 1, 2, 1, 2).unwrap()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let v2 = VerifiedSimulationBundleV2::new(v1, source_map).unwrap();
        let semantic = b"exact-production-semantic-mir-fixture".to_vec();
        let v1_map = SemanticStorageMapV1::new(
            *v2.identity().as_bytes(),
            *v2.subject_identity(),
            9,
            Sha256::digest(&semantic).into(),
            semantic.len() as u64,
            [9; 32],
            *v2.canonical_kir_v7_identity().digest(),
            v2.canonical_kir_v7_identity().canonical_length(),
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
                        representation: SemanticKirStorageRepresentationV1::OpaqueFlattened,
                    },
                )],
            )],
            Vec::new(),
        )
        .unwrap();
        VerifiedSimulationBundleV3::new(v2, semantic, v1_map).unwrap()
    }

    fn map(bundle: &VerifiedSimulationBundleV3) -> SemanticStorageMapV2 {
        SemanticStorageMapV2::new(
            *bundle.identity().as_bytes(),
            vec![SemanticKernelStorageV2::new(
                0,
                0,
                0,
                16,
                8,
                vec![SemanticArgumentStorageV2::new(
                    0,
                    1,
                    0,
                    SemanticArgumentOwnershipV1::ByValue,
                    SemanticComponentStorageBindingV2::exact(vec![
                        SemanticKirComponentStorageV2::new(
                            vec![SemanticStorageProjectionV2::Field { index: 0 }],
                            0,
                            7,
                            SemanticKirComponentRepresentationV2::ScalarValue,
                            SemanticKernargSlotV2::new(0, 2, 2),
                            None,
                        ),
                        SemanticKirComponentStorageV2::new(
                            vec![SemanticStorageProjectionV2::Field { index: 1 }],
                            1,
                            8,
                            SemanticKirComponentRepresentationV2::ScalarValue,
                            SemanticKernargSlotV2::new(8, 8, 8),
                            None,
                        ),
                    ]),
                )],
            )],
        )
        .unwrap()
    }

    #[test]
    fn v4_round_trips_without_broadening_v3() {
        let inner = v3_bundle();
        let bundle = VerifiedSimulationBundleV4::new(inner, map(&v3_bundle())).unwrap();
        let decoded =
            VerifiedSimulationBundleV4::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .unwrap();
        assert_eq!(decoded.canonical_bytes(), bundle.canonical_bytes());
        assert_eq!(decoded.identity(), bundle.identity());
        assert!(
            VerifiedSimulationBundleV3::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .is_err()
        );
        assert!(!decoded.grants_hardware_authority());
        assert!(!decoded.authenticates_compiler_execution());
    }

    #[test]
    fn discriminator_rejects_short_and_foreign_prefixes() {
        assert!(VerifiedSimulationBundleV4::has_magic_prefix(
            b"F2SIMB04payload"
        ));
        assert!(!VerifiedSimulationBundleV4::has_magic_prefix(b"F2SIMB0"));
        assert!(!VerifiedSimulationBundleV4::has_magic_prefix(
            b"F2SIMB03payload"
        ));
        assert!(!VerifiedSimulationBundleV4::has_magic_prefix(&[]));
    }

    #[test]
    fn substitution_noncanonical_and_duplicate_components_fail_closed() {
        let inner = v3_bundle();
        let bundle = VerifiedSimulationBundleV4::new(inner, map(&v3_bundle())).unwrap();
        for offset in [
            24_usize,
            56,
            HEADER_BYTES_V4,
            bundle.canonical_bytes().len() - 1,
        ] {
            let mut bytes = bundle.canonical_bytes().to_vec();
            bytes[offset] ^= 1;
            assert!(VerifiedSimulationBundleV4::from_canonical_bytes(bytes).is_err());
        }
        let mut trailing = bundle.canonical_bytes().to_vec();
        trailing.push(0);
        assert!(matches!(
            VerifiedSimulationBundleV4::from_canonical_bytes(trailing),
            Err(SimulationBundleErrorV4::TrailingOrMissingBytes)
        ));

        let duplicate = SemanticKirComponentStorageV2::new(
            Vec::new(),
            0,
            7,
            SemanticKirComponentRepresentationV2::ScalarValue,
            SemanticKernargSlotV2::new(0, 2, 2),
            None,
        );
        assert!(matches!(
            SemanticStorageMapV2::new(
                *v3_bundle().identity().as_bytes(),
                vec![SemanticKernelStorageV2::new(
                    0,
                    0,
                    0,
                    2,
                    2,
                    vec![SemanticArgumentStorageV2::new(
                        0,
                        1,
                        0,
                        SemanticArgumentOwnershipV1::ByValue,
                        SemanticComponentStorageBindingV2::exact(vec![
                            duplicate.clone(),
                            duplicate,
                        ]),
                    )],
                )],
            ),
            Err(SimulationBundleErrorV4::InvalidStorageMap)
        ));

        let cross_argument = SemanticKirComponentStorageV2::new(
            Vec::new(),
            0,
            7,
            SemanticKirComponentRepresentationV2::ScalarValue,
            SemanticKernargSlotV2::new(0, 2, 2),
            None,
        );
        let mut cross_argument_second = cross_argument.clone();
        cross_argument_second.value_slot = SemanticKernargSlotV2::new(2, 2, 2);
        assert!(matches!(
            SemanticStorageMapV2::new(
                *v3_bundle().identity().as_bytes(),
                vec![SemanticKernelStorageV2::new(
                    0,
                    0,
                    0,
                    4,
                    2,
                    vec![
                        SemanticArgumentStorageV2::new(
                            0,
                            1,
                            0,
                            SemanticArgumentOwnershipV1::ByValue,
                            SemanticComponentStorageBindingV2::exact(vec![cross_argument.clone(),]),
                        ),
                        SemanticArgumentStorageV2::new(
                            1,
                            2,
                            0,
                            SemanticArgumentOwnershipV1::ByValue,
                            SemanticComponentStorageBindingV2::exact(vec![cross_argument_second]),
                        ),
                    ],
                )],
            ),
            Err(SimulationBundleErrorV4::InvalidStorageMap)
        ));

        let oversized_path = vec![
            SemanticStorageProjectionV2::Field { index: 0 };
            MAX_SIMULATION_STORAGE_PROJECTION_DEPTH_V4 + 1
        ];
        assert!(matches!(
            SemanticStorageMapV2::new(
                *v3_bundle().identity().as_bytes(),
                vec![SemanticKernelStorageV2::new(
                    0,
                    0,
                    0,
                    2,
                    2,
                    vec![SemanticArgumentStorageV2::new(
                        0,
                        1,
                        0,
                        SemanticArgumentOwnershipV1::ByValue,
                        SemanticComponentStorageBindingV2::exact(vec![
                            SemanticKirComponentStorageV2::new(
                                oversized_path,
                                0,
                                7,
                                SemanticKirComponentRepresentationV2::ScalarValue,
                                SemanticKernargSlotV2::new(0, 2, 2),
                                None,
                            ),
                        ]),
                    )],
                )],
            ),
            Err(SimulationBundleErrorV4::ResourceLimit)
        ));

        let excessive_components = (0..=MAX_SIMULATION_STORAGE_BINDINGS_V3)
            .map(|ordinal| {
                SemanticKirComponentStorageV2::new(
                    Vec::new(),
                    ordinal as u32,
                    ordinal as u32,
                    SemanticKirComponentRepresentationV2::ScalarValue,
                    SemanticKernargSlotV2::new(0, 1, 1),
                    None,
                )
            })
            .collect();
        assert!(matches!(
            SemanticStorageMapV2::new(
                *v3_bundle().identity().as_bytes(),
                vec![SemanticKernelStorageV2::new(
                    0,
                    0,
                    0,
                    1,
                    1,
                    vec![SemanticArgumentStorageV2::new(
                        0,
                        1,
                        0,
                        SemanticArgumentOwnershipV1::ByValue,
                        SemanticComponentStorageBindingV2::exact(excessive_components),
                    )],
                )],
            ),
            Err(SimulationBundleErrorV4::ResourceLimit)
        ));

        let mut invalid_total = map(&v3_bundle());
        invalid_total.kernels[0].explicit_kernarg_bytes = 8;
        assert!(matches!(
            invalid_total.to_canonical_json_bytes(),
            Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)
        ));

        let mut invalid_alignment = map(&v3_bundle());
        invalid_alignment.kernels[0].explicit_kernarg_alignment = 3;
        assert!(matches!(
            invalid_alignment.to_canonical_json_bytes(),
            Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)
        ));

        let mut insufficient_alignment = map(&v3_bundle());
        insufficient_alignment.kernels[0].explicit_kernarg_alignment = 2;
        assert!(matches!(
            insufficient_alignment.to_canonical_json_bytes(),
            Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)
        ));

        let mut misaligned_component = map(&v3_bundle());
        let SemanticComponentStorageBindingV2::ExactKirComponents { components } =
            &mut misaligned_component.kernels[0].arguments[0].storage
        else {
            unreachable!()
        };
        components[1].value_slot = SemanticKernargSlotV2::new(4, 8, 8);
        assert!(matches!(
            misaligned_component.to_canonical_json_bytes(),
            Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)
        ));

        let mut out_of_bounds_component = map(&v3_bundle());
        let SemanticComponentStorageBindingV2::ExactKirComponents { components } =
            &mut out_of_bounds_component.kernels[0].arguments[0].storage
        else {
            unreachable!()
        };
        components[1].value_slot = SemanticKernargSlotV2::new(16, 8, 8);
        assert!(matches!(
            out_of_bounds_component.to_canonical_json_bytes(),
            Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)
        ));

        let mut overlapping_components = map(&v3_bundle());
        let SemanticComponentStorageBindingV2::ExactKirComponents { components } =
            &mut overlapping_components.kernels[0].arguments[0].storage
        else {
            unreachable!()
        };
        components[1].value_slot = SemanticKernargSlotV2::new(0, 8, 8);
        assert!(matches!(
            overlapping_components.to_canonical_json_bytes(),
            Err(SimulationBundleErrorV4::InvalidPhysicalKernargLayout)
        ));
    }
}
