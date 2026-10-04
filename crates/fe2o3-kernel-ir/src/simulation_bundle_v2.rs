//! Versioned simulation-bundle envelope for a canonical source map V2.

use std::{error::Error, fmt, ops::Deref};

use sha2::{Digest, Sha256};

use crate::{
    DebugSourceMapDocumentV2, DebugSourceMapErrorV2, MAX_SIMULATION_BUNDLE_BYTES_V1,
    MAX_SIMULATION_DEBUG_MAP_BYTES_V1, VerifiedSimulationBundleV1,
};

pub const SIMULATION_BUNDLE_VERSION_V2: u16 = 2;
pub const MAX_SIMULATION_BUNDLE_BYTES_V2: usize =
    MAX_SIMULATION_BUNDLE_BYTES_V1 + MAX_SIMULATION_DEBUG_MAP_BYTES_V1 + HEADER_BYTES_V2;

const MAGIC_V2: &[u8; 8] = b"F2SIMB02";
const HEADER_BYTES_V2: usize = 8 + 2 + 2 + 8 + 4 + 32 + 32;
const BUNDLE_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V2\0";
const DEBUG_MAP_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/SIMULATION-DEBUG-MAP/V2\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationBundleIdentityV2([u8; 32]);

impl SimulationBundleIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Strict ownership of one V2 envelope and its independently verified V1 payload.
#[derive(Debug)]
pub struct VerifiedSimulationBundleV2 {
    canonical_bytes: Vec<u8>,
    identity: SimulationBundleIdentityV2,
    inner: VerifiedSimulationBundleV1,
    debug_map_range: std::ops::Range<usize>,
    debug_map_identity: [u8; 32],
}

impl VerifiedSimulationBundleV2 {
    pub fn new(
        inner: VerifiedSimulationBundleV1,
        source_map: DebugSourceMapDocumentV2,
    ) -> Result<Self, SimulationBundleErrorV2> {
        inner
            .revalidate()
            .map_err(|_| SimulationBundleErrorV2::InvalidV1Bundle)?;
        if inner.debug_map().is_some() {
            return Err(SimulationBundleErrorV2::NestedDebugMap);
        }
        if source_map.binding().bundle_subject_identity() != *inner.subject_identity()
            || source_map.binding().canonical_kir().digest()
                != *inner.canonical_kir_v7_identity().digest()
            || source_map.binding().canonical_kir().canonical_bytes()
                != inner.canonical_kir_v7_identity().canonical_length()
        {
            return Err(SimulationBundleErrorV2::DebugMapBindingMismatch);
        }
        let map = source_map
            .to_canonical_json_bytes()
            .map_err(SimulationBundleErrorV2::DebugSourceMap)?;
        let inner_bytes = inner.canonical_bytes();
        let exact_length = HEADER_BYTES_V2
            .checked_add(inner_bytes.len())
            .and_then(|length| length.checked_add(map.len()))
            .ok_or(SimulationBundleErrorV2::BundleTooLarge)?;
        if exact_length > MAX_SIMULATION_BUNDLE_BYTES_V2 {
            return Err(SimulationBundleErrorV2::BundleTooLarge);
        }
        let inner_length = u64::try_from(inner_bytes.len())
            .map_err(|_| SimulationBundleErrorV2::BundleTooLarge)?;
        let map_length =
            u32::try_from(map.len()).map_err(|_| SimulationBundleErrorV2::InvalidDebugMapLength)?;
        let inner_identity = sha256(inner_bytes);
        let map_identity = simulation_debug_map_identity_v2(&map);
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(exact_length)
            .map_err(|_| SimulationBundleErrorV2::AllocationFailure)?;
        bytes.extend_from_slice(MAGIC_V2);
        bytes.extend_from_slice(&SIMULATION_BUNDLE_VERSION_V2.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&inner_length.to_le_bytes());
        bytes.extend_from_slice(&map_length.to_le_bytes());
        bytes.extend_from_slice(&inner_identity);
        bytes.extend_from_slice(&map_identity);
        bytes.extend_from_slice(inner_bytes);
        bytes.extend_from_slice(&map);
        Self::from_canonical_bytes(bytes)
    }

    pub fn from_canonical_bytes(canonical_bytes: Vec<u8>) -> Result<Self, SimulationBundleErrorV2> {
        if canonical_bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V2 {
            return Err(SimulationBundleErrorV2::BundleTooLarge);
        }
        let header = canonical_bytes
            .get(..HEADER_BYTES_V2)
            .ok_or(SimulationBundleErrorV2::Truncated)?;
        if header.get(..8) != Some(MAGIC_V2.as_slice()) {
            return Err(SimulationBundleErrorV2::InvalidMagic);
        }
        let version = u16::from_le_bytes(header[8..10].try_into().expect("fixed header"));
        if version != SIMULATION_BUNDLE_VERSION_V2 {
            return Err(SimulationBundleErrorV2::UnsupportedVersion(version));
        }
        if header[10..12] != [0; 2] {
            return Err(SimulationBundleErrorV2::InvalidHeader);
        }
        let inner_length = usize::try_from(u64::from_le_bytes(
            header[12..20].try_into().expect("fixed header"),
        ))
        .map_err(|_| SimulationBundleErrorV2::BundleTooLarge)?;
        let map_length = usize::try_from(u32::from_le_bytes(
            header[20..24].try_into().expect("fixed header"),
        ))
        .map_err(|_| SimulationBundleErrorV2::InvalidDebugMapLength)?;
        if inner_length == 0
            || inner_length > MAX_SIMULATION_BUNDLE_BYTES_V1
            || map_length == 0
            || map_length > MAX_SIMULATION_DEBUG_MAP_BYTES_V1
        {
            return Err(SimulationBundleErrorV2::InvalidLength);
        }
        let claimed_inner_identity: [u8; 32] = header[24..56].try_into().expect("fixed header");
        let claimed_map_identity: [u8; 32] = header[56..88].try_into().expect("fixed header");
        let inner_start = HEADER_BYTES_V2;
        let inner_end = inner_start
            .checked_add(inner_length)
            .ok_or(SimulationBundleErrorV2::BundleTooLarge)?;
        let map_end = inner_end
            .checked_add(map_length)
            .ok_or(SimulationBundleErrorV2::BundleTooLarge)?;
        if map_end != canonical_bytes.len() {
            return Err(SimulationBundleErrorV2::TrailingOrMissingBytes);
        }
        let inner_bytes = canonical_bytes
            .get(inner_start..inner_end)
            .ok_or(SimulationBundleErrorV2::Truncated)?;
        let map_bytes = canonical_bytes
            .get(inner_end..map_end)
            .ok_or(SimulationBundleErrorV2::Truncated)?;
        if sha256(inner_bytes) != claimed_inner_identity {
            return Err(SimulationBundleErrorV2::V1BundleIdentityMismatch);
        }
        if simulation_debug_map_identity_v2(map_bytes) != claimed_map_identity {
            return Err(SimulationBundleErrorV2::DebugMapIdentityMismatch);
        }
        let inner = VerifiedSimulationBundleV1::from_canonical_bytes(inner_bytes.to_vec())
            .map_err(|_| SimulationBundleErrorV2::InvalidV1Bundle)?;
        if inner.debug_map().is_some() {
            return Err(SimulationBundleErrorV2::NestedDebugMap);
        }
        let map = DebugSourceMapDocumentV2::from_canonical_json_bytes(map_bytes)
            .map_err(SimulationBundleErrorV2::DebugSourceMap)?;
        if map.binding().bundle_subject_identity() != *inner.subject_identity()
            || map.binding().canonical_kir().digest() != *inner.canonical_kir_v7_identity().digest()
            || map.binding().canonical_kir().canonical_bytes()
                != inner.canonical_kir_v7_identity().canonical_length()
        {
            return Err(SimulationBundleErrorV2::DebugMapBindingMismatch);
        }
        let identity =
            SimulationBundleIdentityV2(domain_hash(BUNDLE_IDENTITY_DOMAIN_V2, &canonical_bytes));
        Ok(Self {
            canonical_bytes,
            identity,
            inner,
            debug_map_range: inner_end..map_end,
            debug_map_identity: claimed_map_identity,
        })
    }

    pub fn revalidate(&self) -> Result<(), SimulationBundleErrorV2> {
        let decoded = Self::from_canonical_bytes(self.canonical_bytes.clone())?;
        if decoded.identity != self.identity
            || decoded.debug_map_identity != self.debug_map_identity
            || decoded.inner.identity() != self.inner.identity()
        {
            return Err(SimulationBundleErrorV2::IdentityMismatch);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    pub const fn identity(&self) -> SimulationBundleIdentityV2 {
        self.identity
    }

    pub const fn inner_v1(&self) -> &VerifiedSimulationBundleV1 {
        &self.inner
    }

    pub fn into_inner_v1(self) -> VerifiedSimulationBundleV1 {
        self.inner
    }

    pub fn debug_map(&self) -> &[u8] {
        &self.canonical_bytes[self.debug_map_range.clone()]
    }

    pub const fn debug_map_identity(&self) -> &[u8; 32] {
        &self.debug_map_identity
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

impl Deref for VerifiedSimulationBundleV2 {
    type Target = VerifiedSimulationBundleV1;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub fn simulation_debug_map_identity_v2(bytes: &[u8]) -> [u8; 32] {
    domain_hash(DEBUG_MAP_IDENTITY_DOMAIN_V2, bytes)
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

#[derive(Debug)]
pub enum SimulationBundleErrorV2 {
    BundleTooLarge,
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidHeader,
    InvalidDebugMapLength,
    Truncated,
    TrailingOrMissingBytes,
    V1BundleIdentityMismatch,
    DebugMapIdentityMismatch,
    DebugMapBindingMismatch,
    NestedDebugMap,
    InvalidV1Bundle,
    DebugSourceMap(DebugSourceMapErrorV2),
    AllocationFailure,
    IdentityMismatch,
}

impl fmt::Display for SimulationBundleErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid fe2o3 simulation bundle V2: {self:?}")
    }
}

impl Error for SimulationBundleErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DebugSourceMap(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BasicBlock, BlockId, DebugSourceMapBindingV1, DebugSourceMapFileV1, DebugSourceMapSpanV1,
        Function, Kernel, LaunchDomain, LaunchExtent, Module, PreparedSimulationBundleV1,
        Signature, SimulationCompilerExecutionBindingV1, SimulationProductionKirIdentityV1,
        SimulationSourceLineageV1, Terminator, Type, VerifiedCanonicalKernelIrV7,
        VerifiedCanonicalKernelIrV8, WorkgroupSize,
    };

    fn v1_bundle() -> VerifiedSimulationBundleV1 {
        let mut module = Module::new("bundle_v2_test");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        module.functions.push(Function::kernel_entry(
            "fill",
            Signature::new(vec![Type::F32], vec![]),
            vec![crate::ValueId(0)],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "fill",
            "fill",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize { x: 64, y: 1, z: 1 });
        module.kernels.push(kernel);
        let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
        PreparedSimulationBundleV1::new(
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
        .unwrap()
    }

    fn map(bundle: &VerifiedSimulationBundleV1) -> DebugSourceMapDocumentV2 {
        DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                *bundle.subject_identity(),
                *bundle.canonical_kir_v7_identity().digest(),
                bundle.canonical_kir_v7_identity().canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new([4; 32], 16, "/src/kernel.rs".into()).unwrap()],
            Vec::new(),
            vec![DebugSourceMapSpanV1::new([4; 32], 1, 2, 1, 2).unwrap()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn v2_envelope_round_trips_without_broadening_v1() {
        let inner = v1_bundle();
        let bundle = VerifiedSimulationBundleV2::new(inner, map(&v1_bundle())).unwrap();
        let decoded =
            VerifiedSimulationBundleV2::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .unwrap();
        assert_eq!(decoded.canonical_bytes(), bundle.canonical_bytes());
        assert_eq!(decoded.identity(), bundle.identity());
        assert!(
            VerifiedSimulationBundleV1::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .is_err()
        );
        assert!(!decoded.authenticates_compiler_execution());
        assert!(!decoded.grants_hardware_authority());
    }

    #[test]
    fn substitution_and_noncanonical_map_fail_closed() {
        let inner = v1_bundle();
        let bundle = VerifiedSimulationBundleV2::new(inner, map(&v1_bundle())).unwrap();
        for offset in [
            24_usize,
            56,
            HEADER_BYTES_V2,
            bundle.canonical_bytes().len() - 1,
        ] {
            let mut bytes = bundle.canonical_bytes().to_vec();
            bytes[offset] ^= 1;
            assert!(VerifiedSimulationBundleV2::from_canonical_bytes(bytes).is_err());
        }
        let mut trailing = bundle.canonical_bytes().to_vec();
        trailing.push(0);
        assert!(matches!(
            VerifiedSimulationBundleV2::from_canonical_bytes(trailing),
            Err(SimulationBundleErrorV2::TrailingOrMissingBytes)
        ));
    }
}
