//! Bounded, authority-free custody for exact CPU-simulation inputs.

use std::{error::Error, fmt, str};

use sha2::{Digest, Sha256};

use crate::{
    AccessMode, AddressSpace, DebugSourceMapBindingV1, DebugSourceMapDocumentV1,
    DebugSourceMapErrorV1, LaunchDomain, LaunchExtent, MAX_MODULE_BYTES_V1, MAX_TEXT_BYTES_V1,
    Module, ScalarType, Type, VerifiedCanonicalKernelIrErrorV7,
    VerifiedCanonicalKernelIrIdentityV7, VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV8,
    decode_module_v7,
};

/// Frozen binary schema version for an authority-free simulation bundle.
pub const SIMULATION_BUNDLE_VERSION_V1: u16 = 1;
/// Exact byte length of the canonical compiler-execution subject bound by V1.
pub const COMPILER_EXECUTION_SUBJECT_BYTES_V1: usize = 690;
/// Maximum byte length of each diagnostic source-lineage content binding.
pub const MAX_SIMULATION_SOURCE_LINEAGE_BYTES_V1: u64 = 4 * 1024 * 1024;
/// Maximum canonical compiler-owned source-map payload.
pub const MAX_SIMULATION_DEBUG_MAP_BYTES_V1: usize = 4 * 1024 * 1024;
/// Maximum complete bundle size, including the exact KIR and debug map.
pub const MAX_SIMULATION_BUNDLE_BYTES_V1: usize = MAX_MODULE_BYTES_V1
    + MAX_SIMULATION_DEBUG_MAP_BYTES_V1
    + COMPILER_EXECUTION_SUBJECT_BYTES_V1
    + MAX_TEXT_BYTES_V1
    + HEADER_BYTES_V1;
/// Exact schema named by a present compiler-owned source-map payload.
pub const SIMULATION_DEBUG_MAP_SCHEMA_V1: &str = "fe2o3-debug-source-map-v1";

const MAGIC_V1: &[u8; 8] = b"F2SIMB01";
const FLAGS_DEBUG_MAP_PRESENT: u16 = 1;
const KNOWN_FLAGS: u16 = FLAGS_DEBUG_MAP_PRESENT;
const COMPILER_EXECUTION_UNAVAILABLE_EXTRACTION_ONLY_TAG: u8 = 0;
const COMPILER_EXECUTION_CANONICAL_ASSOCIATION_TAG: u8 = 1;
const PRODUCTION_KIR_VERSION_V8: u16 = 8;
const HEADER_BYTES_V1: usize =
    8 + 2 + 2 + 1 + 7 + 32 + 8 + 32 + 8 + 32 + 8 + 2 + 32 + 8 + 32 + 8 + 32 + 4 + 2 + 32 + 4 + 32;
const SUBJECT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SIMULATION-BUNDLE-SUBJECT/V1\0";
const BUNDLE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V1\0";
const DEBUG_MAP_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SIMULATION-DEBUG-MAP/V1\0";
const KERNEL_ABI_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SIMULATION-KERNEL-ABI/V1\0";

/// Unverified wire association with an inert compiler-execution subject V1.
///
/// The canonical variant retains exact bytes so a higher owner can decode an
/// `InertCompilerExecutionSubjectV1`, independently reconstruct that subject
/// from an already-retained or consumed exact strict V3 handoff, and then
/// cross-check every lineage/KIR/target binding. Bundle decoding alone verifies
/// only bundle content; it neither validates the nested subject nor creates
/// that higher owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationCompilerExecutionBindingV1 {
    UnavailableExtractionOnly,
    Canonical(SimulationCanonicalCompilerExecutionAssociationV1),
}

/// Exact bytes plus the claimed terminal identity of an inert subject.
///
/// This type is deliberately an untrusted association. Callers can construct
/// it, so a higher-layer join must strictly decode and cross-check the bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationCanonicalCompilerExecutionAssociationV1 {
    claimed_identity: [u8; 32],
    canonical_bytes: Box<[u8; COMPILER_EXECUTION_SUBJECT_BYTES_V1]>,
}

impl SimulationCanonicalCompilerExecutionAssociationV1 {
    pub fn from_unverified_wire_association(
        claimed_identity: [u8; 32],
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, SimulationBundleErrorV1> {
        require_nonzero("claimed compiler-execution subject", &claimed_identity)?;
        let canonical_bytes: Box<[u8; COMPILER_EXECUTION_SUBJECT_BYTES_V1]> = canonical_bytes
            .into_boxed_slice()
            .try_into()
            .map_err(|_| SimulationBundleErrorV1::InvalidCompilerExecutionSubject)?;
        Ok(Self {
            claimed_identity,
            canonical_bytes,
        })
    }

    pub const fn claimed_identity(&self) -> &[u8; 32] {
        &self.claimed_identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        self.canonical_bytes.as_slice()
    }
}

impl SimulationCompilerExecutionBindingV1 {
    fn wire_fields(&self) -> (u8, [u8; 32], u64, &[u8]) {
        match self {
            Self::UnavailableExtractionOnly => (
                COMPILER_EXECUTION_UNAVAILABLE_EXTRACTION_ONLY_TAG,
                [0; 32],
                0,
                &[],
            ),
            Self::Canonical(subject) => (
                COMPILER_EXECUTION_CANONICAL_ASSOCIATION_TAG,
                *subject.claimed_identity(),
                COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64,
                subject.canonical_bytes(),
            ),
        }
    }

    fn from_wire_fields(
        tag: u8,
        claimed_identity: [u8; 32],
        canonical_length: u64,
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, SimulationBundleErrorV1> {
        if tag == COMPILER_EXECUTION_UNAVAILABLE_EXTRACTION_ONLY_TAG {
            if claimed_identity != [0; 32] || canonical_length != 0 || !canonical_bytes.is_empty() {
                return Err(SimulationBundleErrorV1::InvalidCompilerExecutionSubject);
            }
            return Ok(Self::UnavailableExtractionOnly);
        }
        if tag != COMPILER_EXECUTION_CANONICAL_ASSOCIATION_TAG
            || canonical_length != COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64
        {
            return Err(SimulationBundleErrorV1::InvalidCompilerExecutionSubject);
        }
        Ok(Self::Canonical(
            SimulationCanonicalCompilerExecutionAssociationV1::from_unverified_wire_association(
                claimed_identity,
                canonical_bytes,
            )?,
        ))
    }
}

/// Exact inert V3 receipt identities for compiler-owned source transcripts.
///
/// These are diagnostic content bindings, not compiler-execution provenance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationSourceLineageV1 {
    rustc_identity_inventory_receipt_sha256: [u8; 32],
    rustc_identity_inventory_receipt_bytes: u64,
    rustc_preflight_plan_receipt_sha256: [u8; 32],
    rustc_preflight_plan_receipt_bytes: u64,
}

impl SimulationSourceLineageV1 {
    pub fn new(
        rustc_identity_inventory_receipt_sha256: [u8; 32],
        rustc_identity_inventory_receipt_bytes: u64,
        rustc_preflight_plan_receipt_sha256: [u8; 32],
        rustc_preflight_plan_receipt_bytes: u64,
    ) -> Result<Self, SimulationBundleErrorV1> {
        require_nonzero(
            "rustc identity inventory V3 receipt",
            &rustc_identity_inventory_receipt_sha256,
        )?;
        require_nonzero(
            "rustc preflight plan V3 receipt",
            &rustc_preflight_plan_receipt_sha256,
        )?;
        if rustc_identity_inventory_receipt_bytes == 0
            || rustc_identity_inventory_receipt_bytes > MAX_SIMULATION_SOURCE_LINEAGE_BYTES_V1
            || rustc_preflight_plan_receipt_bytes == 0
            || rustc_preflight_plan_receipt_bytes > MAX_SIMULATION_SOURCE_LINEAGE_BYTES_V1
        {
            return Err(SimulationBundleErrorV1::InvalidSourceLineageLength);
        }
        Ok(Self {
            rustc_identity_inventory_receipt_sha256,
            rustc_identity_inventory_receipt_bytes,
            rustc_preflight_plan_receipt_sha256,
            rustc_preflight_plan_receipt_bytes,
        })
    }

    pub const fn rustc_identity_inventory_receipt_sha256(self) -> [u8; 32] {
        self.rustc_identity_inventory_receipt_sha256
    }

    pub const fn rustc_identity_inventory_receipt_bytes(self) -> u64 {
        self.rustc_identity_inventory_receipt_bytes
    }

    pub const fn rustc_preflight_plan_receipt_sha256(self) -> [u8; 32] {
        self.rustc_preflight_plan_receipt_sha256
    }

    pub const fn rustc_preflight_plan_receipt_bytes(self) -> u64 {
        self.rustc_preflight_plan_receipt_bytes
    }
}

/// Identity of the production-owned canonical KIR from which V7 was projected.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationProductionKirIdentityV1 {
    version: u16,
    digest: [u8; 32],
    canonical_length: u64,
}

impl SimulationProductionKirIdentityV1 {
    /// Constructs the only production identity representable by bundle V1.
    ///
    /// Production V9 operations are not silently downgraded to simulator V7.
    pub fn v8(digest: [u8; 32], canonical_length: u64) -> Result<Self, SimulationBundleErrorV1> {
        require_nonzero("production KIR", &digest)?;
        if canonical_length == 0 || canonical_length > MAX_MODULE_BYTES_V1 as u64 {
            return Err(SimulationBundleErrorV1::InvalidProductionKirLength);
        }
        Ok(Self {
            version: PRODUCTION_KIR_VERSION_V8,
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

/// Bounded exact bytes for a claimed compiler-produced debug map.
///
/// Bundle custody commits these bytes but does not authenticate their producer.
/// A future sole-transaction owner may bind the payload to compiler output;
/// protected-execution authentication still requires the issuer/Worker join.
#[derive(Debug, Eq, PartialEq)]
pub struct SimulationDebugMapV1 {
    canonical_bytes: Vec<u8>,
    identity: [u8; 32],
}

impl SimulationDebugMapV1 {
    pub fn from_unverified_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, SimulationBundleErrorV1> {
        if canonical_bytes.is_empty() || canonical_bytes.len() > MAX_SIMULATION_DEBUG_MAP_BYTES_V1 {
            return Err(SimulationBundleErrorV1::InvalidDebugMapLength);
        }
        let identity = simulation_debug_map_identity_v1(&canonical_bytes);
        require_nonzero("debug map", &identity)?;
        Ok(Self {
            canonical_bytes,
            identity,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

/// Typed content identity of one complete canonical simulation bundle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationBundleIdentityV1([u8; 32]);

impl SimulationBundleIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A strict, content-addressed bundle containing one exact verified KIR V7.
///
/// This owner grants no proof, artifact, load, hardware, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedSimulationBundleV1 {
    canonical_bytes: Vec<u8>,
    identity: SimulationBundleIdentityV1,
    subject_identity: [u8; 32],
    compiler_execution_binding: SimulationCompilerExecutionBindingV1,
    source_lineage: SimulationSourceLineageV1,
    production_kir_identity: SimulationProductionKirIdentityV1,
    canonical_kir_v7_identity: VerifiedCanonicalKernelIrIdentityV7,
    kernel_abi_identity: [u8; 32],
    kernel_count: u32,
    target_range: std::ops::Range<usize>,
    kir_range: std::ops::Range<usize>,
    debug_map_range: Option<std::ops::Range<usize>>,
}

/// Verified map-independent simulation metadata awaiting one finalization.
///
/// The subject identity deliberately excludes the optional debug map. This
/// move-only owner lets the compiler bind a map to that exact subject without
/// constructing and decoding an intermediate bundle or accepting loose hashes.
#[must_use = "dropping the prepared owner abandons simulation bundle finalization"]
pub struct PreparedSimulationBundleV1 {
    compiler_execution_binding: SimulationCompilerExecutionBindingV1,
    source_lineage: SimulationSourceLineageV1,
    production_kir_identity: SimulationProductionKirIdentityV1,
    target: String,
    canonical_kir_v7_identity: VerifiedCanonicalKernelIrIdentityV7,
    canonical_kir_v7: Vec<u8>,
    kernel_abi_identity: [u8; 32],
    kernel_count: u32,
    subject_identity: [u8; 32],
}

impl PreparedSimulationBundleV1 {
    pub fn new(
        compiler_execution_binding: SimulationCompilerExecutionBindingV1,
        source_lineage: SimulationSourceLineageV1,
        production_kir_identity: SimulationProductionKirIdentityV1,
        target: &str,
        canonical_kir_v7: VerifiedCanonicalKernelIrV7,
    ) -> Result<Self, SimulationBundleErrorV1> {
        validate_target(target)?;
        let canonical_kir_v7_identity = *canonical_kir_v7.identity();
        let canonical_kir_v7 = canonical_kir_v7.into_canonical_bytes();
        let module =
            decode_module_v7(&canonical_kir_v7).map_err(SimulationBundleErrorV1::KernelIrDecode)?;
        validate_production_identity(&module, production_kir_identity)?;
        let kernel_count = u32::try_from(module.kernels.len())
            .map_err(|_| SimulationBundleErrorV1::KernelCountOverflow)?;
        let kernel_abi_identity = kernel_abi_identity(&module)?;
        let subject_identity = subject_identity(
            &compiler_execution_binding,
            source_lineage,
            production_kir_identity,
            target,
            &canonical_kir_v7_identity,
            &kernel_abi_identity,
            kernel_count,
        );
        Ok(Self {
            compiler_execution_binding,
            source_lineage,
            production_kir_identity,
            target: target.to_owned(),
            canonical_kir_v7_identity,
            canonical_kir_v7,
            kernel_abi_identity,
            kernel_count,
            subject_identity,
        })
    }

    pub const fn subject_identity(&self) -> &[u8; 32] {
        &self.subject_identity
    }

    pub const fn canonical_kir_v7_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV7 {
        &self.canonical_kir_v7_identity
    }

    pub fn debug_source_map_binding(&self) -> DebugSourceMapBindingV1 {
        DebugSourceMapBindingV1::new(
            self.subject_identity,
            *self.canonical_kir_v7_identity.digest(),
            self.canonical_kir_v7_identity.canonical_length(),
        )
        .expect("verified bundle identities form a valid source-map binding")
    }

    pub fn finalize_with_source_map(
        self,
        document: DebugSourceMapDocumentV1,
    ) -> Result<VerifiedSimulationBundleV1, SimulationBundleErrorV1> {
        if document.binding() != self.debug_source_map_binding() {
            return Err(SimulationBundleErrorV1::DebugMapBindingMismatch);
        }
        let bytes = document
            .to_canonical_json_bytes()
            .map_err(SimulationBundleErrorV1::DebugSourceMap)?;
        let debug_map = SimulationDebugMapV1::from_unverified_canonical_bytes(bytes)?;
        self.finalize(Some(debug_map))
    }

    pub fn finalize_without_source_map(
        self,
    ) -> Result<VerifiedSimulationBundleV1, SimulationBundleErrorV1> {
        self.finalize(None)
    }

    fn finalize(
        self,
        debug_map: Option<SimulationDebugMapV1>,
    ) -> Result<VerifiedSimulationBundleV1, SimulationBundleErrorV1> {
        if let Some(debug_map) = &debug_map {
            let document =
                DebugSourceMapDocumentV1::from_canonical_json_bytes(debug_map.canonical_bytes())
                    .map_err(SimulationBundleErrorV1::DebugSourceMap)?;
            if document.binding() != self.debug_source_map_binding() {
                return Err(SimulationBundleErrorV1::DebugMapBindingMismatch);
            }
        }
        let debug_length = debug_map
            .as_ref()
            .map_or(0, |map| map.canonical_bytes.len());
        let (
            compiler_execution_tag,
            compiler_subject_identity,
            compiler_subject_length,
            compiler_subject_bytes,
        ) = self.compiler_execution_binding.wire_fields();
        let exact_length = HEADER_BYTES_V1
            .checked_add(self.target.len())
            .and_then(|length| length.checked_add(self.canonical_kir_v7.len()))
            .and_then(|length| length.checked_add(compiler_subject_bytes.len()))
            .and_then(|length| length.checked_add(debug_length))
            .ok_or(SimulationBundleErrorV1::BundleTooLarge)?;
        if exact_length > MAX_SIMULATION_BUNDLE_BYTES_V1 {
            return Err(SimulationBundleErrorV1::BundleTooLarge);
        }
        let target_length =
            u16::try_from(self.target.len()).map_err(|_| SimulationBundleErrorV1::InvalidTarget)?;
        let debug_length_u32 = u32::try_from(debug_length)
            .map_err(|_| SimulationBundleErrorV1::InvalidDebugMapLength)?;
        let flags = u16::from(debug_map.is_some()) * FLAGS_DEBUG_MAP_PRESENT;
        let debug_identity = debug_map.as_ref().map_or([0; 32], |map| *map.identity());

        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(exact_length)
            .map_err(|_| SimulationBundleErrorV1::AllocationFailure)?;
        bytes.extend_from_slice(MAGIC_V1);
        bytes.extend_from_slice(&SIMULATION_BUNDLE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.push(compiler_execution_tag);
        bytes.extend_from_slice(&[0; 7]);
        bytes.extend_from_slice(&compiler_subject_identity);
        bytes.extend_from_slice(&compiler_subject_length.to_le_bytes());
        bytes.extend_from_slice(&self.source_lineage.rustc_identity_inventory_receipt_sha256);
        bytes.extend_from_slice(
            &self
                .source_lineage
                .rustc_identity_inventory_receipt_bytes
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&self.source_lineage.rustc_preflight_plan_receipt_sha256);
        bytes.extend_from_slice(
            &self
                .source_lineage
                .rustc_preflight_plan_receipt_bytes
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&self.production_kir_identity.version.to_le_bytes());
        bytes.extend_from_slice(&self.production_kir_identity.digest);
        bytes.extend_from_slice(&self.production_kir_identity.canonical_length.to_le_bytes());
        bytes.extend_from_slice(self.canonical_kir_v7_identity.digest());
        bytes.extend_from_slice(
            &self
                .canonical_kir_v7_identity
                .canonical_length()
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&self.kernel_abi_identity);
        bytes.extend_from_slice(&self.kernel_count.to_le_bytes());
        bytes.extend_from_slice(&target_length.to_le_bytes());
        bytes.extend_from_slice(&debug_identity);
        bytes.extend_from_slice(&debug_length_u32.to_le_bytes());
        bytes.extend_from_slice(&self.subject_identity);
        debug_assert_eq!(bytes.len(), HEADER_BYTES_V1);
        bytes.extend_from_slice(self.target.as_bytes());
        bytes.extend_from_slice(&self.canonical_kir_v7);
        bytes.extend_from_slice(compiler_subject_bytes);
        if let Some(debug_map) = debug_map {
            bytes.extend_from_slice(&debug_map.canonical_bytes);
        }
        debug_assert_eq!(bytes.len(), exact_length);
        VerifiedSimulationBundleV1::from_canonical_bytes(bytes)
    }
}

impl VerifiedSimulationBundleV1 {
    /// Builds a canonical bundle from a V7 projection of the already-lowered
    /// production module. The supplied V8 identity is independently rederived
    /// from V7 semantics before admission.
    pub fn new(
        compiler_execution_binding: SimulationCompilerExecutionBindingV1,
        source_lineage: SimulationSourceLineageV1,
        production_kir_identity: SimulationProductionKirIdentityV1,
        target: &str,
        canonical_kir_v7: VerifiedCanonicalKernelIrV7,
        debug_map: Option<SimulationDebugMapV1>,
    ) -> Result<Self, SimulationBundleErrorV1> {
        PreparedSimulationBundleV1::new(
            compiler_execution_binding,
            source_lineage,
            production_kir_identity,
            target,
            canonical_kir_v7,
        )?
        .finalize(debug_map)
    }

    /// Strictly decodes, rederives, and retains one complete bundle allocation.
    pub fn from_canonical_bytes(canonical_bytes: Vec<u8>) -> Result<Self, SimulationBundleErrorV1> {
        if canonical_bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V1 {
            return Err(SimulationBundleErrorV1::BundleTooLarge);
        }
        let header = canonical_bytes
            .get(..HEADER_BYTES_V1)
            .ok_or(SimulationBundleErrorV1::Truncated)?;
        let mut decoder = HeaderDecoder::new(header);
        if decoder.array::<8>()? != *MAGIC_V1 {
            return Err(SimulationBundleErrorV1::InvalidMagic);
        }
        let version = decoder.u16()?;
        if version != SIMULATION_BUNDLE_VERSION_V1 {
            return Err(SimulationBundleErrorV1::UnsupportedVersion(version));
        }
        let flags = decoder.u16()?;
        if flags & !KNOWN_FLAGS != 0 {
            return Err(SimulationBundleErrorV1::InvalidFlags(flags));
        }
        let compiler_execution_tag = decoder.u8()?;
        if decoder.array::<7>()? != [0; 7] {
            return Err(SimulationBundleErrorV1::InvalidHeader);
        }
        let claimed_compiler_subject_identity = decoder.array::<32>()?;
        let claimed_compiler_subject_length = decoder.u64()?;
        let compiler_subject_length = match compiler_execution_tag {
            COMPILER_EXECUTION_UNAVAILABLE_EXTRACTION_ONLY_TAG
                if claimed_compiler_subject_identity == [0; 32]
                    && claimed_compiler_subject_length == 0 =>
            {
                0
            }
            COMPILER_EXECUTION_CANONICAL_ASSOCIATION_TAG => {
                if claimed_compiler_subject_length != COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64 {
                    return Err(SimulationBundleErrorV1::InvalidCompilerExecutionSubject);
                }
                COMPILER_EXECUTION_SUBJECT_BYTES_V1
            }
            _ => return Err(SimulationBundleErrorV1::InvalidCompilerExecutionSubject),
        };
        let source_lineage = SimulationSourceLineageV1::new(
            decoder.array()?,
            decoder.u64()?,
            decoder.array()?,
            decoder.u64()?,
        )?;
        let production_version = decoder.u16()?;
        if production_version != PRODUCTION_KIR_VERSION_V8 {
            return Err(SimulationBundleErrorV1::UnsupportedProductionKirVersion(
                production_version,
            ));
        }
        let production_kir_identity =
            SimulationProductionKirIdentityV1::v8(decoder.array()?, decoder.u64()?)?;
        let claimed_kir_digest = decoder.array::<32>()?;
        let claimed_kir_length = decoder.u64()?;
        let claimed_abi_identity = decoder.array::<32>()?;
        require_nonzero("kernel ABI", &claimed_abi_identity)?;
        let claimed_kernel_count = decoder.u32()?;
        let target_length = usize::from(decoder.u16()?);
        let claimed_debug_identity = decoder.array::<32>()?;
        let debug_length = usize::try_from(decoder.u32()?)
            .map_err(|_| SimulationBundleErrorV1::InvalidDebugMapLength)?;
        let claimed_subject_identity = decoder.array::<32>()?;
        require_nonzero("bundle subject", &claimed_subject_identity)?;
        if !decoder.is_finished() {
            return Err(SimulationBundleErrorV1::InvalidHeader);
        }
        if debug_length > MAX_SIMULATION_DEBUG_MAP_BYTES_V1
            || (flags & FLAGS_DEBUG_MAP_PRESENT == 0) != (debug_length == 0)
        {
            return Err(SimulationBundleErrorV1::InvalidDebugMapLength);
        }
        if debug_length == 0 {
            if claimed_debug_identity != [0; 32] {
                return Err(SimulationBundleErrorV1::InvalidDebugMapLength);
            }
        } else {
            require_nonzero("debug map", &claimed_debug_identity)?;
        }
        let kir_length = usize::try_from(claimed_kir_length)
            .map_err(|_| SimulationBundleErrorV1::InvalidKernelIrLength)?;
        if kir_length == 0 || kir_length > MAX_MODULE_BYTES_V1 {
            return Err(SimulationBundleErrorV1::InvalidKernelIrLength);
        }
        let target_start = HEADER_BYTES_V1;
        let target_end = target_start
            .checked_add(target_length)
            .ok_or(SimulationBundleErrorV1::BundleTooLarge)?;
        let kir_end = target_end
            .checked_add(kir_length)
            .ok_or(SimulationBundleErrorV1::BundleTooLarge)?;
        let compiler_subject_end = kir_end
            .checked_add(compiler_subject_length)
            .ok_or(SimulationBundleErrorV1::BundleTooLarge)?;
        let debug_end = compiler_subject_end
            .checked_add(debug_length)
            .ok_or(SimulationBundleErrorV1::BundleTooLarge)?;
        if debug_end != canonical_bytes.len() {
            return Err(SimulationBundleErrorV1::TrailingOrMissingBytes);
        }
        let target = str::from_utf8(
            canonical_bytes
                .get(target_start..target_end)
                .ok_or(SimulationBundleErrorV1::Truncated)?,
        )
        .map_err(|_| SimulationBundleErrorV1::InvalidTarget)?;
        validate_target(target)?;
        let kir_bytes = canonical_bytes
            .get(target_end..kir_end)
            .ok_or(SimulationBundleErrorV1::Truncated)?;
        let kir_owner = VerifiedCanonicalKernelIrV7::from_canonical_bytes(kir_bytes.to_vec())
            .map_err(SimulationBundleErrorV1::KernelIr)?;
        if kir_owner.identity().digest() != &claimed_kir_digest
            || kir_owner.identity().canonical_length() != claimed_kir_length
        {
            return Err(SimulationBundleErrorV1::KernelIrIdentityMismatch);
        }
        let canonical_kir_v7_identity = *kir_owner.identity();
        let compiler_execution_binding = SimulationCompilerExecutionBindingV1::from_wire_fields(
            compiler_execution_tag,
            claimed_compiler_subject_identity,
            claimed_compiler_subject_length,
            canonical_bytes
                .get(kir_end..compiler_subject_end)
                .ok_or(SimulationBundleErrorV1::Truncated)?
                .to_vec(),
        )?;
        let module =
            decode_module_v7(kir_bytes).map_err(SimulationBundleErrorV1::KernelIrDecode)?;
        validate_production_identity(&module, production_kir_identity)?;
        let kernel_count = u32::try_from(module.kernels.len())
            .map_err(|_| SimulationBundleErrorV1::KernelCountOverflow)?;
        let kernel_abi_identity = kernel_abi_identity(&module)?;
        if kernel_count != claimed_kernel_count || kernel_abi_identity != claimed_abi_identity {
            return Err(SimulationBundleErrorV1::KernelAbiIdentityMismatch);
        }
        if debug_length != 0 {
            let debug_bytes = canonical_bytes
                .get(compiler_subject_end..debug_end)
                .ok_or(SimulationBundleErrorV1::Truncated)?;
            if simulation_debug_map_identity_v1(debug_bytes) != claimed_debug_identity {
                return Err(SimulationBundleErrorV1::DebugMapIdentityMismatch);
            }
        }
        let subject_identity = subject_identity(
            &compiler_execution_binding,
            source_lineage,
            production_kir_identity,
            target,
            &canonical_kir_v7_identity,
            &kernel_abi_identity,
            kernel_count,
        );
        if subject_identity != claimed_subject_identity {
            return Err(SimulationBundleErrorV1::SubjectIdentityMismatch);
        }
        if debug_length != 0 {
            let debug_bytes = canonical_bytes
                .get(compiler_subject_end..debug_end)
                .ok_or(SimulationBundleErrorV1::Truncated)?;
            let document = DebugSourceMapDocumentV1::from_canonical_json_bytes(debug_bytes)
                .map_err(SimulationBundleErrorV1::DebugSourceMap)?;
            let expected = DebugSourceMapBindingV1::new(
                subject_identity,
                *canonical_kir_v7_identity.digest(),
                canonical_kir_v7_identity.canonical_length(),
            )
            .map_err(SimulationBundleErrorV1::DebugSourceMap)?;
            if document.binding() != expected {
                return Err(SimulationBundleErrorV1::DebugMapBindingMismatch);
            }
        }
        let identity =
            SimulationBundleIdentityV1(domain_hash(BUNDLE_IDENTITY_DOMAIN_V1, &canonical_bytes));
        Ok(Self {
            canonical_bytes,
            identity,
            subject_identity,
            compiler_execution_binding,
            source_lineage,
            production_kir_identity,
            canonical_kir_v7_identity,
            kernel_abi_identity,
            kernel_count,
            target_range: target_start..target_end,
            kir_range: target_end..kir_end,
            debug_map_range: (debug_length != 0).then_some(compiler_subject_end..debug_end),
        })
    }

    pub fn revalidate(&self) -> Result<(), SimulationBundleErrorV1> {
        let decoded = Self::from_canonical_bytes(self.canonical_bytes.clone())?;
        if decoded.identity != self.identity
            || decoded.subject_identity != self.subject_identity
            || decoded.compiler_execution_binding != self.compiler_execution_binding
            || decoded.source_lineage != self.source_lineage
            || decoded.production_kir_identity != self.production_kir_identity
            || decoded.canonical_kir_v7_identity != self.canonical_kir_v7_identity
            || decoded.kernel_abi_identity != self.kernel_abi_identity
            || decoded.kernel_count != self.kernel_count
        {
            return Err(SimulationBundleErrorV1::IdentityMismatch);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    pub const fn identity(&self) -> SimulationBundleIdentityV1 {
        self.identity
    }

    /// Identity a compiler-produced debug map must name. It excludes the map
    /// payload, avoiding a circular full-bundle identity.
    pub const fn subject_identity(&self) -> &[u8; 32] {
        &self.subject_identity
    }

    /// Returns the explicit extraction-only or canonical wire association.
    pub const fn compiler_execution_binding(&self) -> &SimulationCompilerExecutionBindingV1 {
        &self.compiler_execution_binding
    }

    /// Requires exact inert subject bytes for a higher-layer strict join.
    ///
    /// Success remains an unverified wire association. It does not authenticate
    /// compiler execution and must be decoded and cross-checked above this
    /// crate against the exact strict V3 handoff and production transaction.
    pub fn require_canonical_compiler_execution_association(
        &self,
    ) -> Result<&SimulationCanonicalCompilerExecutionAssociationV1, SimulationBundleErrorV1> {
        match &self.compiler_execution_binding {
            SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly => {
                Err(SimulationBundleErrorV1::MissingCompilerExecutionSubject)
            }
            SimulationCompilerExecutionBindingV1::Canonical(subject) => Ok(subject),
        }
    }

    pub const fn source_lineage(&self) -> SimulationSourceLineageV1 {
        self.source_lineage
    }

    pub const fn production_kir_identity(&self) -> SimulationProductionKirIdentityV1 {
        self.production_kir_identity
    }

    pub const fn canonical_kir_v7_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV7 {
        &self.canonical_kir_v7_identity
    }

    pub const fn kernel_abi_identity(&self) -> &[u8; 32] {
        &self.kernel_abi_identity
    }

    pub const fn kernel_count(&self) -> u32 {
        self.kernel_count
    }

    pub fn target(&self) -> &str {
        str::from_utf8(&self.canonical_bytes[self.target_range.clone()])
            .expect("validated target remains UTF-8")
    }

    pub fn canonical_kir_v7(&self) -> &[u8] {
        &self.canonical_bytes[self.kir_range.clone()]
    }

    pub fn debug_map(&self) -> Option<&[u8]> {
        self.debug_map_range
            .as_ref()
            .map(|range| &self.canonical_bytes[range.clone()])
    }

    /// Returns the identity committed for the exact embedded debug-map bytes.
    ///
    /// Bundle decoding has already compared this rederived identity with the
    /// header commitment. The identity authenticates bundle content
    /// association only, not compiler execution or source authorship.
    pub fn debug_map_identity(&self) -> Option<[u8; 32]> {
        self.debug_map().map(simulation_debug_map_identity_v1)
    }

    pub const fn debug_map_schema(&self) -> Option<&'static str> {
        if self.debug_map_range.is_some() {
            Some(SIMULATION_DEBUG_MAP_SCHEMA_V1)
        } else {
            None
        }
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
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

    /// A canonical subject or signed attestation is still inert pending the
    /// protected issuer and durable Worker V3 consumer join.
    pub const fn authenticates_compiler_execution(&self) -> bool {
        false
    }
}

fn validate_production_identity(
    module: &Module,
    claimed: SimulationProductionKirIdentityV1,
) -> Result<(), SimulationBundleErrorV1> {
    if claimed.version != PRODUCTION_KIR_VERSION_V8 {
        return Err(SimulationBundleErrorV1::UnsupportedProductionKirVersion(
            claimed.version,
        ));
    }
    let production = VerifiedCanonicalKernelIrV8::from_module(module.clone())
        .map_err(SimulationBundleErrorV1::ProductionKernelIr)?;
    if production.identity().digest() != &claimed.digest
        || production.identity().canonical_length() != claimed.canonical_length
    {
        return Err(SimulationBundleErrorV1::ProductionKirIdentityMismatch);
    }
    Ok(())
}

fn validate_target(target: &str) -> Result<(), SimulationBundleErrorV1> {
    if target.is_empty()
        || target.len() > MAX_TEXT_BYTES_V1
        || target
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(SimulationBundleErrorV1::InvalidTarget);
    }
    Ok(())
}

fn subject_identity(
    compiler_execution_binding: &SimulationCompilerExecutionBindingV1,
    lineage: SimulationSourceLineageV1,
    production: SimulationProductionKirIdentityV1,
    target: &str,
    kir: &VerifiedCanonicalKernelIrIdentityV7,
    abi: &[u8; 32],
    kernel_count: u32,
) -> [u8; 32] {
    let mut digest = domain_hasher(SUBJECT_IDENTITY_DOMAIN_V1);
    let (tag, claimed_identity, canonical_length, canonical_bytes) =
        compiler_execution_binding.wire_fields();
    digest.update([tag]);
    digest.update(claimed_identity);
    digest.update(canonical_length.to_le_bytes());
    digest.update(canonical_bytes);
    digest.update(lineage.rustc_identity_inventory_receipt_sha256);
    digest.update(lineage.rustc_identity_inventory_receipt_bytes.to_le_bytes());
    digest.update(lineage.rustc_preflight_plan_receipt_sha256);
    digest.update(lineage.rustc_preflight_plan_receipt_bytes.to_le_bytes());
    digest.update(production.version.to_le_bytes());
    digest.update(production.digest);
    digest.update(production.canonical_length.to_le_bytes());
    digest.update(kir.digest());
    digest.update(kir.canonical_length().to_le_bytes());
    digest.update((target.len() as u64).to_le_bytes());
    digest.update(target.as_bytes());
    digest.update(abi);
    digest.update(kernel_count.to_le_bytes());
    digest.finalize().into()
}

fn kernel_abi_identity(module: &Module) -> Result<[u8; 32], SimulationBundleErrorV1> {
    let mut digest = domain_hasher(KERNEL_ABI_IDENTITY_DOMAIN_V1);
    let count = u32::try_from(module.kernels.len())
        .map_err(|_| SimulationBundleErrorV1::KernelCountOverflow)?;
    digest.update(count.to_le_bytes());
    for kernel in &module.kernels {
        hash_text(&mut digest, kernel.id.as_str())?;
        hash_text(&mut digest, kernel.entry.as_str())?;
        hash_launch_domain(&mut digest, &kernel.domain);
        match kernel.workgroup_size {
            Some(size) => {
                digest.update([1]);
                digest.update(size.x.to_le_bytes());
                digest.update(size.y.to_le_bytes());
                digest.update(size.z.to_le_bytes());
            }
            None => digest.update([0]),
        }
        let entry = module
            .function(&kernel.entry)
            .ok_or(SimulationBundleErrorV1::MissingKernelEntry)?;
        hash_types(&mut digest, &entry.signature.parameters)?;
        hash_types(&mut digest, &entry.signature.results)?;
    }
    Ok(digest.finalize().into())
}

fn hash_text(digest: &mut Sha256, text: &str) -> Result<(), SimulationBundleErrorV1> {
    let length =
        u32::try_from(text.len()).map_err(|_| SimulationBundleErrorV1::InvalidKernelAbi)?;
    digest.update(length.to_le_bytes());
    digest.update(text.as_bytes());
    Ok(())
}

fn hash_launch_domain(digest: &mut Sha256, domain: &LaunchDomain) {
    match domain {
        LaunchDomain::D1 { x } => {
            digest.update([1]);
            hash_launch_extent(digest, *x);
        }
        LaunchDomain::D2 { x, y } => {
            digest.update([2]);
            hash_launch_extent(digest, *x);
            hash_launch_extent(digest, *y);
        }
        LaunchDomain::D3 { x, y, z } => {
            digest.update([3]);
            hash_launch_extent(digest, *x);
            hash_launch_extent(digest, *y);
            hash_launch_extent(digest, *z);
        }
    }
}

fn hash_launch_extent(digest: &mut Sha256, extent: LaunchExtent) {
    match extent {
        LaunchExtent::Dynamic => digest.update([0]),
        LaunchExtent::Static(value) => {
            digest.update([1]);
            digest.update(value.to_le_bytes());
        }
    }
}

fn hash_types(digest: &mut Sha256, types: &[Type]) -> Result<(), SimulationBundleErrorV1> {
    let length =
        u32::try_from(types.len()).map_err(|_| SimulationBundleErrorV1::InvalidKernelAbi)?;
    digest.update(length.to_le_bytes());
    for ty in types {
        hash_type(digest, ty, 0)?;
    }
    Ok(())
}

fn hash_type(digest: &mut Sha256, ty: &Type, depth: usize) -> Result<(), SimulationBundleErrorV1> {
    if depth > 64 {
        return Err(SimulationBundleErrorV1::InvalidKernelAbi);
    }
    match ty {
        Type::Unit => digest.update([0]),
        Type::Scalar(scalar) => digest.update([1, scalar_tag(*scalar)]),
        Type::Pointer(pointer) => {
            digest.update([
                2,
                address_space_tag(pointer.address_space),
                access_mode_tag(pointer.access),
            ]);
            hash_type(digest, &pointer.pointee, depth + 1)?;
        }
        Type::Slice(slice) => {
            digest.update([
                3,
                address_space_tag(slice.address_space),
                access_mode_tag(slice.access),
            ]);
            hash_type(digest, &slice.element, depth + 1)?;
        }
    }
    Ok(())
}

const fn scalar_tag(scalar: ScalarType) -> u8 {
    match scalar {
        ScalarType::Bool => 0,
        ScalarType::I8 => 1,
        ScalarType::I16 => 2,
        ScalarType::I32 => 3,
        ScalarType::I64 => 4,
        ScalarType::I128 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::U128 => 10,
        ScalarType::Index => 11,
        ScalarType::F16 => 12,
        ScalarType::Bf16 => 13,
        ScalarType::F32 => 14,
        ScalarType::F64 => 15,
    }
}

const fn address_space_tag(space: AddressSpace) -> u8 {
    match space {
        AddressSpace::Private => 0,
        AddressSpace::Workgroup => 1,
        AddressSpace::Global => 2,
        AddressSpace::Constant => 3,
        AddressSpace::Generic => 4,
    }
}

const fn access_mode_tag(access: AccessMode) -> u8 {
    match access {
        AccessMode::ReadOnly => 0,
        AccessMode::ReadWrite => 1,
    }
}

pub fn simulation_debug_map_identity_v1(bytes: &[u8]) -> [u8; 32] {
    domain_hash(DEBUG_MAP_IDENTITY_DOMAIN_V1, bytes)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = domain_hasher(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn domain_hasher(domain: &[u8]) -> Sha256 {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u32).to_le_bytes());
    digest.update(domain);
    digest
}

fn require_nonzero(
    field: &'static str,
    identity: &[u8; 32],
) -> Result<(), SimulationBundleErrorV1> {
    if *identity == [0; 32] {
        Err(SimulationBundleErrorV1::ZeroIdentity(field))
    } else {
        Ok(())
    }
}

struct HeaderDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> HeaderDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SimulationBundleErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(SimulationBundleErrorV1::Truncated)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(SimulationBundleErrorV1::Truncated)?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| SimulationBundleErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, SimulationBundleErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u8(&mut self) -> Result<u8, SimulationBundleErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, SimulationBundleErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, SimulationBundleErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationBundleErrorV1 {
    AllocationFailure,
    BundleTooLarge,
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidFlags(u16),
    InvalidHeader,
    ZeroIdentity(&'static str),
    InvalidCompilerExecutionSubject,
    MissingCompilerExecutionSubject,
    InvalidSourceLineageLength,
    UnsupportedProductionKirVersion(u16),
    InvalidProductionKirLength,
    InvalidKernelIrLength,
    InvalidDebugMapLength,
    InvalidTarget,
    KernelCountOverflow,
    MissingKernelEntry,
    InvalidKernelAbi,
    TrailingOrMissingBytes,
    KernelIr(VerifiedCanonicalKernelIrErrorV7),
    KernelIrDecode(crate::KernelIrDecodeError),
    ProductionKernelIr(crate::VerifiedCanonicalKernelIrErrorV8),
    ProductionKirIdentityMismatch,
    KernelIrIdentityMismatch,
    KernelAbiIdentityMismatch,
    DebugMapIdentityMismatch,
    DebugMapBindingMismatch,
    DebugSourceMap(DebugSourceMapErrorV1),
    SubjectIdentityMismatch,
    IdentityMismatch,
}

impl fmt::Display for SimulationBundleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailure => {
                formatter.write_str("cannot reserve bounded simulation bundle")
            }
            Self::BundleTooLarge => formatter.write_str("simulation bundle exceeds its byte bound"),
            Self::Truncated => formatter.write_str("simulation bundle is truncated"),
            Self::InvalidMagic => formatter.write_str("simulation bundle magic is invalid"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported simulation bundle version {version}")
            }
            Self::InvalidFlags(flags) => {
                write!(formatter, "simulation bundle has unknown flags {flags:#x}")
            }
            Self::InvalidHeader => formatter.write_str("simulation bundle header is invalid"),
            Self::ZeroIdentity(field) => {
                write!(formatter, "simulation bundle has zero {field} identity")
            }
            Self::InvalidCompilerExecutionSubject => formatter.write_str(
                "simulation compiler-execution subject presence, identity, or length is invalid",
            ),
            Self::MissingCompilerExecutionSubject => formatter.write_str(
                "simulation bundle has no canonical inert compiler-execution subject association",
            ),
            Self::InvalidSourceLineageLength => {
                formatter.write_str("simulation source-lineage V3 receipt length is invalid")
            }
            Self::UnsupportedProductionKirVersion(version) => write!(
                formatter,
                "production Kernel IR V{version} cannot be represented by simulation bundle V1; no downgrade was attempted"
            ),
            Self::InvalidProductionKirLength => {
                formatter.write_str("production Kernel IR length is invalid")
            }
            Self::InvalidKernelIrLength => {
                formatter.write_str("canonical Kernel IR V7 length is invalid")
            }
            Self::InvalidDebugMapLength => {
                formatter.write_str("simulation debug-map presence or length is invalid")
            }
            Self::DebugMapBindingMismatch => formatter.write_str(
                "simulation debug map does not name the exact bundle subject and canonical KIR",
            ),
            Self::DebugSourceMap(error) => write!(formatter, "simulation {error}"),
            Self::InvalidTarget => formatter.write_str("simulation bundle target is invalid"),
            Self::KernelCountOverflow => {
                formatter.write_str("simulation kernel count does not fit the bundle wire")
            }
            Self::MissingKernelEntry => {
                formatter.write_str("simulation kernel ABI references a missing entry")
            }
            Self::InvalidKernelAbi => {
                formatter.write_str("simulation kernel ABI exceeds canonical bounds")
            }
            Self::TrailingOrMissingBytes => formatter
                .write_str("simulation bundle length fields do not consume the exact input"),
            Self::KernelIr(error) => error.fmt(formatter),
            Self::KernelIrDecode(error) => error.fmt(formatter),
            Self::ProductionKernelIr(error) => error.fmt(formatter),
            Self::ProductionKirIdentityMismatch => formatter
                .write_str("production Kernel IR identity does not match the exact V7 semantics"),
            Self::KernelIrIdentityMismatch => {
                formatter.write_str("canonical Kernel IR V7 identity mismatch")
            }
            Self::KernelAbiIdentityMismatch => {
                formatter.write_str("simulation kernel ABI identity mismatch")
            }
            Self::DebugMapIdentityMismatch => {
                formatter.write_str("simulation debug-map identity mismatch")
            }
            Self::SubjectIdentityMismatch => {
                formatter.write_str("simulation bundle subject identity mismatch")
            }
            Self::IdentityMismatch => {
                formatter.write_str("retained simulation bundle identity mismatch")
            }
        }
    }
}

impl Error for SimulationBundleErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::KernelIr(error) => Some(error),
            Self::KernelIrDecode(error) => Some(error),
            Self::ProductionKernelIr(error) => Some(error),
            Self::DebugSourceMap(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        BasicBlock, BlockId, DebugSourceMapFileV1, DebugSourceMapSpanV1, Function, Kernel,
        LaunchDomain, LaunchExtent, Signature, Terminator, WorkgroupSize,
    };

    fn module() -> Module {
        let mut module = Module::new("bundle_test");
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
        module
    }

    fn bundle(with_debug_map: bool) -> VerifiedSimulationBundleV1 {
        let module = module();
        let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
        let prepared = PreparedSimulationBundleV1::new(
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
        .unwrap();
        if with_debug_map {
            let binding = prepared.debug_source_map_binding();
            prepared
                .finalize_with_source_map(
                    DebugSourceMapDocumentV1::new(
                        binding,
                        vec![
                            DebugSourceMapFileV1::new([4; 32], 16, "/src/kernel.rs".into())
                                .unwrap(),
                        ],
                        Vec::new(),
                        vec![DebugSourceMapSpanV1::new([4; 32], 1, 2, 1, 2).unwrap()],
                    )
                    .unwrap(),
                )
                .unwrap()
        } else {
            prepared.finalize_without_source_map().unwrap()
        }
    }

    #[test]
    fn exact_bundle_round_trips_and_grants_no_authority() {
        let bundle = bundle(true);
        let decoded =
            VerifiedSimulationBundleV1::from_canonical_bytes(bundle.canonical_bytes().to_vec())
                .unwrap();
        assert_eq!(decoded.canonical_bytes(), bundle.canonical_bytes());
        assert_eq!(decoded.identity(), bundle.identity());
        assert_eq!(decoded.target(), "gfx942:xnack-");
        assert_eq!(decoded.kernel_count(), 1);
        assert_eq!(
            decoded.compiler_execution_binding(),
            &SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly
        );
        assert!(matches!(
            decoded.require_canonical_compiler_execution_association(),
            Err(SimulationBundleErrorV1::MissingCompilerExecutionSubject)
        ));
        assert_eq!(
            decoded.debug_map_schema(),
            Some(SIMULATION_DEBUG_MAP_SCHEMA_V1)
        );
        assert_eq!(decoded.debug_map_identity(), bundle.debug_map_identity());
        assert!(decoded.debug_map_identity().is_some());
        assert!(!decoded.canonical_kir_v7().is_empty());
        assert!(!decoded.grants_proof_authority());
        assert!(!decoded.grants_artifact_authority());
        assert!(!decoded.grants_compiler_authority());
        assert!(!decoded.grants_hardware_authority());
        assert!(!decoded.grants_load_authority());
        assert!(!decoded.grants_launch_authority());
        assert!(!decoded.authenticates_compiler_execution());
        decoded.revalidate().unwrap();
    }

    #[test]
    fn unverified_subject_association_is_exact_and_never_promotes() {
        let module = module();
        let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
        let compiler_subject =
            SimulationCanonicalCompilerExecutionAssociationV1::from_unverified_wire_association(
                [9; 32],
                vec![7; COMPILER_EXECUTION_SUBJECT_BYTES_V1],
            )
            .unwrap();
        let bundle = VerifiedSimulationBundleV1::new(
            SimulationCompilerExecutionBindingV1::Canonical(compiler_subject.clone()),
            SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
            SimulationProductionKirIdentityV1::v8(
                *production.identity().digest(),
                production.identity().canonical_length(),
            )
            .unwrap(),
            "gfx942:xnack-",
            VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(
            bundle
                .require_canonical_compiler_execution_association()
                .unwrap(),
            &compiler_subject
        );
        assert!(!bundle.authenticates_compiler_execution());

        let subject_start =
            HEADER_BYTES_V1 + bundle.target().len() + bundle.canonical_kir_v7().len();
        let mut substituted_subject = bundle.canonical_bytes().to_vec();
        substituted_subject[subject_start] ^= 1;
        assert!(matches!(
            VerifiedSimulationBundleV1::from_canonical_bytes(substituted_subject),
            Err(SimulationBundleErrorV1::SubjectIdentityMismatch)
        ));

        let mut absent_tag = bundle.canonical_bytes().to_vec();
        absent_tag[12] = COMPILER_EXECUTION_UNAVAILABLE_EXTRACTION_ONLY_TAG;
        assert!(matches!(
            VerifiedSimulationBundleV1::from_canonical_bytes(absent_tag),
            Err(SimulationBundleErrorV1::InvalidCompilerExecutionSubject)
        ));
        let mut wrong_length = bundle.into_canonical_bytes();
        wrong_length[52..60]
            .copy_from_slice(&((COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64) - 1).to_le_bytes());
        assert!(matches!(
            VerifiedSimulationBundleV1::from_canonical_bytes(wrong_length),
            Err(SimulationBundleErrorV1::InvalidCompilerExecutionSubject)
        ));
    }

    #[test]
    fn hostile_substitution_of_each_identity_domain_fails_closed() {
        let baseline = bundle(false).into_canonical_bytes();
        for offset in [20, 60, 92, 100, 132, 142, 182, 222, 296] {
            let mut substituted = baseline.clone();
            substituted[offset] ^= 1;
            assert!(
                VerifiedSimulationBundleV1::from_canonical_bytes(substituted).is_err(),
                "substitution at byte {offset} was admitted"
            );
        }
        let mut target = baseline.clone();
        target[HEADER_BYTES_V1] ^= 1;
        assert!(VerifiedSimulationBundleV1::from_canonical_bytes(target).is_err());
        let mut kir = baseline;
        kir[HEADER_BYTES_V1 + "gfx942:xnack-".len() + 12] ^= 1;
        assert!(VerifiedSimulationBundleV1::from_canonical_bytes(kir).is_err());
    }

    #[test]
    fn bounds_and_presence_bits_are_strict() {
        assert!(
            SimulationCanonicalCompilerExecutionAssociationV1::from_unverified_wire_association(
                [0; 32],
                vec![1; COMPILER_EXECUTION_SUBJECT_BYTES_V1]
            )
            .is_err()
        );
        assert!(
            SimulationCanonicalCompilerExecutionAssociationV1::from_unverified_wire_association(
                [1; 32],
                vec![1; COMPILER_EXECUTION_SUBJECT_BYTES_V1 - 1]
            )
            .is_err()
        );
        assert!(SimulationSourceLineageV1::new([0; 32], 1, [1; 32], 1).is_err());
        assert!(SimulationSourceLineageV1::new([1; 32], 0, [2; 32], 1).is_err());
        assert!(SimulationProductionKirIdentityV1::v8([1; 32], 0).is_err());
        assert!(
            SimulationDebugMapV1::from_unverified_canonical_bytes(vec![
                0;
                MAX_SIMULATION_DEBUG_MAP_BYTES_V1
                    + 1
            ])
            .is_err()
        );
        let mut zero_debug_identity = bundle(true).into_canonical_bytes();
        zero_debug_identity[260..292].fill(0);
        assert!(matches!(
            VerifiedSimulationBundleV1::from_canonical_bytes(zero_debug_identity),
            Err(SimulationBundleErrorV1::ZeroIdentity("debug map"))
        ));
        let baseline = bundle(false).into_canonical_bytes();
        assert!(
            VerifiedSimulationBundleV1::from_canonical_bytes(
                baseline[..HEADER_BYTES_V1 - 1].to_vec()
            )
            .is_err()
        );
        let mut unknown_flags = baseline.clone();
        unknown_flags[10..12].copy_from_slice(&2_u16.to_le_bytes());
        assert!(matches!(
            VerifiedSimulationBundleV1::from_canonical_bytes(unknown_flags),
            Err(SimulationBundleErrorV1::InvalidFlags(2))
        ));
        let mut trailing = baseline;
        trailing.push(0);
        assert!(matches!(
            VerifiedSimulationBundleV1::from_canonical_bytes(trailing),
            Err(SimulationBundleErrorV1::TrailingOrMissingBytes)
        ));
    }
}
