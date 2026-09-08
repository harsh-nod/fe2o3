//! Checked semantic-MIR-to-canonical-KIR refinement evidence.
//!
//! This envelope can only be issued from a live [`ProductionSemanticKirOwnerV1`]. The issuer
//! replays the complete lowering and correspondence relation before retaining its exact source,
//! KIR, and root-to-entry coordinates. The result is compiler-refinement evidence only; it grants
//! no artifact, publication, load, launch, runtime, or machine-refinement authority.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_mir_model::{
    SemanticU32InductionNoOverflowReportV1,
    semantic_mir_v1::{SemanticFunctionExportV1, SemanticFunctionIdV1},
};
use sha2::{Digest, Sha256};

use crate::{
    InertCanonicalMirToKirCorrespondenceEvidenceV5, MirToKirFunctionRoleEvidenceV5,
    ProductionCanonicalKernelIrVersionV1, ProductionCorrespondenceEvidenceErrorV5,
    ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1,
};

/// Canonical magic for a live-owner-issued source-refinement envelope.
pub const PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_MAGIC_V1: [u8; 8] = *b"F2SREFV1";
/// Exact wire version for checked source-refinement evidence.
pub const PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_VERSION_V1: u16 = 1;
/// Closed replay-and-complete-correspondence policy.
pub const PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_POLICY_V1: u16 = 1;
/// Hard bound for one source-refinement evidence envelope.
pub const MAX_PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_BYTES_V1: usize = 4 * 1024 * 1024;

const HEADER_BYTES_V1: usize = 136;
const ROOT_FIXED_BYTES_V1: usize = 76;
const TERMINAL_BYTES_V1: usize = 32;
const MAX_NAME_BYTES_V1: usize = 4096;
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/CHECKED-SOURCE-MIR-TO-KIR-REFINEMENT-EVIDENCE/V1\0";

/// One exact semantic root, binding, and final KIR kernel-entry mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSourceRefinementRootV1 {
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    kernel_entry_ordinal: u32,
    kernel_function_ordinal: u32,
    export_symbol: Box<str>,
    kernel_id: Box<str>,
    kernel_function: Box<str>,
}

impl ProductionSourceRefinementRootV1 {
    /// Returns the canonical semantic-function root index.
    pub const fn semantic_root(&self) -> u32 {
        self.semantic_root
    }

    /// Returns the exact semantic-function identity.
    pub const fn semantic_root_identity(&self) -> [u8; 32] {
        self.semantic_root_identity
    }

    /// Returns the exact compiler-issued kernel-binding identity.
    pub const fn kernel_binding(&self) -> [u8; 32] {
        self.kernel_binding
    }

    /// Returns the exact ordinal in the canonical KIR kernel roster.
    pub const fn kernel_entry_ordinal(&self) -> u32 {
        self.kernel_entry_ordinal
    }

    /// Returns the exact ordinal of the defined KIR entry function.
    pub const fn kernel_function_ordinal(&self) -> u32 {
        self.kernel_function_ordinal
    }

    /// Returns the authenticated semantic export symbol.
    pub fn export_symbol(&self) -> &str {
        &self.export_symbol
    }

    /// Returns the exact canonical KIR kernel identifier.
    pub fn kernel_id(&self) -> &str {
        &self.kernel_id
    }

    /// Returns the exact canonical KIR entry-function identifier.
    pub fn kernel_function(&self) -> &str {
        &self.kernel_function
    }
}

/// Move-only evidence issued after replaying one live semantic-MIR-to-KIR owner.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionSourceRefinementEvidenceV1 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    source_kir_sha256: [u8; 32],
    source_kir_bytes: u64,
    source_kir_version: ProductionCanonicalKernelIrVersionV1,
    correspondence_identity: [u8; 32],
    roots: Box<[ProductionSourceRefinementRootV1]>,
}

impl ProductionSourceRefinementEvidenceV1 {
    /// Replays the complete live lowering and emits exact, bijective root-to-entry evidence.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
        induction_report: &SemanticU32InductionNoOverflowReportV1,
    ) -> Result<Self, ProductionSourceRefinementEvidenceErrorV1> {
        owner
            .verify_equivalence()
            .map_err(ProductionSourceRefinementEvidenceErrorV1::LiveOwner)?;
        let correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV5::from_live_owner(
            owner,
            induction_report,
        )
        .map_err(ProductionSourceRefinementEvidenceErrorV1::Correspondence)?;
        correspondence
            .revalidate()
            .map_err(ProductionSourceRefinementEvidenceErrorV1::Correspondence)?;
        correspondence
            .validate_against_module(owner.module())
            .map_err(ProductionSourceRefinementEvidenceErrorV1::Correspondence)?;

        let source_kir = correspondence.nested_v4().canonical_kernel_ir_identity();
        let semantic = owner.semantic().semantic();
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(semantic.roots().len())
            .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::AllocationFailure)?;
        let mut kernel_ordinals = BTreeSet::new();
        let mut function_ordinals = BTreeSet::new();
        let mut previous_root = None;
        for root in semantic.roots() {
            let semantic_root = root.index();
            if previous_root.is_some_and(|previous| semantic_root <= previous) {
                return Err(ProductionSourceRefinementEvidenceErrorV1::NonCanonicalRootOrder);
            }
            previous_root = Some(semantic_root);
            let function =
                semantic
                    .functions()
                    .get(usize::try_from(semantic_root).map_err(|_| {
                        ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow
                    })?)
                    .ok_or(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster)?;
            let SemanticFunctionExportV1::Kernel(entry) = function
                .export()
                .ok_or(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster)?
            else {
                return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
            };
            let selection = semantic
                .select_kernel_body_for_root_v1(*root)
                .ok_or(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster)?;
            let entry_function = unique_entry_function(&correspondence, *root, selection.body())?;
            let mut matching_kernels = owner
                .module()
                .kernels
                .iter()
                .enumerate()
                .filter(|(_, kernel)| kernel.entry.as_str() == entry_function.kernel_ir_function());
            let Some((kernel_ordinal, kernel)) = matching_kernels.next() else {
                return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
            };
            if matching_kernels.next().is_some() {
                return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
            }
            let export_symbol = std::str::from_utf8(entry.export_symbol().as_bytes())
                .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster)?;
            if export_symbol != kernel.id.as_str()
                || export_symbol != entry_function.kernel_ir_function()
                || entry_function.kernel_ir_function_ordinal()
                    != u32::try_from(
                        owner
                            .module()
                            .functions
                            .iter()
                            .position(|candidate| candidate.id == kernel.entry)
                            .ok_or(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster)?,
                    )
                    .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?
            {
                return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
            }
            let kernel_ordinal = u32::try_from(kernel_ordinal)
                .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?;
            if kernel_ordinal
                != u32::try_from(roots.len())
                    .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?
                || !kernel_ordinals.insert(kernel_ordinal)
                || !function_ordinals.insert(entry_function.kernel_ir_function_ordinal())
            {
                return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
            }
            roots.push(ProductionSourceRefinementRootV1 {
                semantic_root,
                semantic_root_identity: *function.identity().as_bytes(),
                kernel_binding: *entry.kernel_binding_identity().as_bytes(),
                kernel_entry_ordinal: kernel_ordinal,
                kernel_function_ordinal: entry_function.kernel_ir_function_ordinal(),
                export_symbol: export_symbol.to_owned().into_boxed_str(),
                kernel_id: kernel.id.as_str().to_owned().into_boxed_str(),
                kernel_function: entry_function
                    .kernel_ir_function()
                    .to_owned()
                    .into_boxed_str(),
            });
        }
        if roots.is_empty()
            || roots.len() != owner.module().kernels.len()
            || kernel_ordinals.len() != owner.module().kernels.len()
        {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
        }
        let semantic_mir_sha256 = *semantic.semantic_sha256().as_bytes();
        let source_kir_sha256 = *source_kir.digest();
        let source_kir_bytes = source_kir.canonical_length();
        let correspondence_identity = *correspondence.identity();
        let canonical_bytes = encode(
            semantic_mir_sha256,
            source_kir_sha256,
            source_kir_bytes,
            source_kir.version(),
            correspondence_identity,
            &roots,
            correspondence.canonical_bytes(),
        )?;
        let decoded = Self::decode(&canonical_bytes)?;
        if decoded.roots.as_ref() != roots.as_slice() {
            return Err(ProductionSourceRefinementEvidenceErrorV1::NonCanonical);
        }
        Ok(decoded)
    }

    /// Strictly decodes and revalidates a canonical evidence envelope.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionSourceRefinementEvidenceErrorV1> {
        if bytes.len() > MAX_PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_BYTES_V1 {
            return Err(ProductionSourceRefinementEvidenceErrorV1::TooLarge);
        }
        let mut reader = ReaderV1::new(bytes);
        if reader.fixed::<8>()? != PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_MAGIC_V1
            || reader.u16()? != PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_VERSION_V1
            || reader.u16()? != PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_POLICY_V1
            || reader.u32()? != 0
        {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidHeader);
        }
        if reader.usize_u32()? != bytes.len() {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidLength);
        }
        let semantic_mir_sha256 = reader.fixed::<32>()?;
        let source_kir_version = decode_kir_version(reader.u16()?)?;
        if reader.u16()? != 0 {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidHeader);
        }
        let source_kir_bytes = reader.u64()?;
        let source_kir_sha256 = reader.fixed::<32>()?;
        let correspondence_identity = reader.fixed::<32>()?;
        if semantic_mir_sha256 == [0; 32]
            || source_kir_sha256 == [0; 32]
            || source_kir_bytes == 0
            || correspondence_identity == [0; 32]
        {
            return Err(ProductionSourceRefinementEvidenceErrorV1::ZeroIdentity);
        }
        let root_count = reader.usize_u32()?;
        let correspondence_len = reader.usize_u32()?;
        if root_count == 0 || root_count > fe2o3_kernel_ir::MAX_KERNELS_V1 {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
        }
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(root_count)
            .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::AllocationFailure)?;
        let mut kernel_ordinals = BTreeSet::new();
        let mut function_ordinals = BTreeSet::new();
        let mut root_identities = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        let mut previous_root = None;
        for _ in 0..root_count {
            let semantic_root = reader.u32()?;
            if previous_root.is_some_and(|previous| semantic_root <= previous) {
                return Err(ProductionSourceRefinementEvidenceErrorV1::NonCanonicalRootOrder);
            }
            previous_root = Some(semantic_root);
            let semantic_root_identity = reader.fixed::<32>()?;
            let kernel_binding = reader.fixed::<32>()?;
            let kernel_entry_ordinal = reader.u32()?;
            let kernel_function_ordinal = reader.u32()?;
            if semantic_root_identity == [0; 32]
                || kernel_binding == [0; 32]
                || !root_identities.insert(semantic_root_identity)
                || !bindings.insert(kernel_binding)
                || !kernel_ordinals.insert(kernel_entry_ordinal)
                || !function_ordinals.insert(kernel_function_ordinal)
            {
                return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
            }
            let export_symbol = reader.text()?;
            let kernel_id = reader.text()?;
            let kernel_function = reader.text()?;
            if export_symbol != kernel_id || kernel_id != kernel_function {
                return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
            }
            roots.push(ProductionSourceRefinementRootV1 {
                semantic_root,
                semantic_root_identity,
                kernel_binding,
                kernel_entry_ordinal,
                kernel_function_ordinal,
                export_symbol: export_symbol.into(),
                kernel_id: kernel_id.into(),
                kernel_function: kernel_function.into(),
            });
        }
        if kernel_ordinals
            != (0..u32::try_from(root_count)
                .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?)
                .collect()
        {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
        }
        if correspondence_len == 0 || correspondence_len > reader.remaining_without_terminal()? {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidLength);
        }
        let correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(
            reader.take(correspondence_len)?,
        )
        .map_err(ProductionSourceRefinementEvidenceErrorV1::Correspondence)?;
        let terminal = reader.fixed::<TERMINAL_BYTES_V1>()?;
        reader.finish()?;
        if correspondence.identity() != &correspondence_identity
            || correspondence.nested_v4().semantic_sha256() != &semantic_mir_sha256
            || correspondence
                .nested_v4()
                .canonical_kernel_ir_identity()
                .digest()
                != &source_kir_sha256
            || correspondence
                .nested_v4()
                .canonical_kernel_ir_identity()
                .canonical_length()
                != source_kir_bytes
            || correspondence
                .nested_v4()
                .canonical_kernel_ir_identity()
                .version()
                != source_kir_version
        {
            return Err(ProductionSourceRefinementEvidenceErrorV1::NestedIdentityMismatch);
        }
        validate_correspondence_roster(&roots, &correspondence)?;
        let identity = derive_identity(&bytes[..bytes.len() - TERMINAL_BYTES_V1]);
        if terminal == [0; 32] || terminal != identity {
            return Err(ProductionSourceRefinementEvidenceErrorV1::IdentityMismatch);
        }
        let reencoded = encode(
            semantic_mir_sha256,
            source_kir_sha256,
            source_kir_bytes,
            source_kir_version,
            correspondence_identity,
            &roots,
            correspondence.canonical_bytes(),
        )?;
        if reencoded != bytes {
            return Err(ProductionSourceRefinementEvidenceErrorV1::NonCanonical);
        }
        Ok(Self {
            canonical_bytes: reencoded.into_boxed_slice(),
            identity,
            semantic_mir_sha256,
            source_kir_sha256,
            source_kir_bytes,
            source_kir_version,
            correspondence_identity,
            roots: roots.into_boxed_slice(),
        })
    }

    /// Returns the complete canonical checked-relation bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated evidence identity.
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    /// Returns the exact semantic MIR digest replayed by the source stage.
    pub const fn semantic_mir_sha256(&self) -> [u8; 32] {
        self.semantic_mir_sha256
    }

    /// Returns the exact source KIR V13 digest produced by replayed lowering.
    pub const fn source_kir_sha256(&self) -> [u8; 32] {
        self.source_kir_sha256
    }

    /// Returns the exact source KIR V13 byte length.
    pub const fn source_kir_bytes(&self) -> u64 {
        self.source_kir_bytes
    }

    /// Returns the exact canonical source KIR wire version.
    pub const fn source_kir_version(&self) -> ProductionCanonicalKernelIrVersionV1 {
        self.source_kir_version
    }

    /// Returns the exact nested correspondence-evidence identity.
    pub const fn correspondence_identity(&self) -> [u8; 32] {
        self.correspondence_identity
    }

    /// Returns the complete canonical semantic-root-to-KIR-entry roster.
    pub fn roots(&self) -> &[ProductionSourceRefinementRootV1] {
        &self.roots
    }

    /// Checked source refinement grants no adjacent authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn unique_entry_function<'a>(
    correspondence: &'a InertCanonicalMirToKirCorrespondenceEvidenceV5,
    owner: SemanticFunctionIdV1,
    body: SemanticFunctionIdV1,
) -> Result<
    &'a crate::MirToKirFunctionCorrespondenceEvidenceV5,
    ProductionSourceRefinementEvidenceErrorV1,
> {
    let mut matches = correspondence.functions().iter().filter(|record| {
        record.correspondence_owner() == owner.index()
            && record.semantic_function() == body.index()
            && record.role() == MirToKirFunctionRoleEvidenceV5::KernelEntry
    });
    let result = matches
        .next()
        .ok_or(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster)?;
    if matches.next().is_some() {
        return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
    }
    Ok(result)
}

fn validate_correspondence_roster(
    roots: &[ProductionSourceRefinementRootV1],
    correspondence: &InertCanonicalMirToKirCorrespondenceEvidenceV5,
) -> Result<(), ProductionSourceRefinementEvidenceErrorV1> {
    let entries = correspondence
        .functions()
        .iter()
        .filter(|record| record.role() == MirToKirFunctionRoleEvidenceV5::KernelEntry)
        .collect::<Vec<_>>();
    if entries.len() != roots.len() {
        return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
    }
    for root in roots {
        let matches = entries
            .iter()
            .filter(|record| {
                record.correspondence_owner() == root.semantic_root
                    && record.kernel_ir_function_ordinal() == root.kernel_function_ordinal
                    && record.kernel_ir_function() == root.kernel_function.as_ref()
            })
            .count();
        if matches != 1 {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
        }
    }
    Ok(())
}

fn encode(
    semantic_mir_sha256: [u8; 32],
    source_kir_sha256: [u8; 32],
    source_kir_bytes: u64,
    source_kir_version: ProductionCanonicalKernelIrVersionV1,
    correspondence_identity: [u8; 32],
    roots: &[ProductionSourceRefinementRootV1],
    correspondence: &[u8],
) -> Result<Vec<u8>, ProductionSourceRefinementEvidenceErrorV1> {
    let roots_bytes = roots.iter().try_fold(0_usize, |total, root| {
        [
            root.export_symbol.len(),
            root.kernel_id.len(),
            root.kernel_function.len(),
        ]
        .into_iter()
        .try_fold(
            total
                .checked_add(ROOT_FIXED_BYTES_V1)
                .ok_or(ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?,
            |value, length| {
                value
                    .checked_add(4)
                    .and_then(|value| value.checked_add(length))
                    .ok_or(ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)
            },
        )
    })?;
    let total = HEADER_BYTES_V1
        .checked_add(roots_bytes)
        .and_then(|value| value.checked_add(correspondence.len()))
        .and_then(|value| value.checked_add(TERMINAL_BYTES_V1))
        .ok_or(ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?;
    if total > MAX_PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_BYTES_V1 {
        return Err(ProductionSourceRefinementEvidenceErrorV1::TooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::AllocationFailure)?;
    bytes.extend_from_slice(&PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_MAGIC_V1);
    bytes.extend_from_slice(&PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&PRODUCTION_SOURCE_REFINEMENT_EVIDENCE_POLICY_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(total)
            .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&semantic_mir_sha256);
    bytes.extend_from_slice(&encode_kir_version(source_kir_version).to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&source_kir_bytes.to_le_bytes());
    bytes.extend_from_slice(&source_kir_sha256);
    bytes.extend_from_slice(&correspondence_identity);
    push_u32(&mut bytes, roots.len())?;
    push_u32(&mut bytes, correspondence.len())?;
    for root in roots {
        bytes.extend_from_slice(&root.semantic_root.to_le_bytes());
        bytes.extend_from_slice(&root.semantic_root_identity);
        bytes.extend_from_slice(&root.kernel_binding);
        bytes.extend_from_slice(&root.kernel_entry_ordinal.to_le_bytes());
        bytes.extend_from_slice(&root.kernel_function_ordinal.to_le_bytes());
        push_text(&mut bytes, &root.export_symbol)?;
        push_text(&mut bytes, &root.kernel_id)?;
        push_text(&mut bytes, &root.kernel_function)?;
    }
    bytes.extend_from_slice(correspondence);
    let identity = derive_identity(&bytes);
    bytes.extend_from_slice(&identity);
    debug_assert_eq!(bytes.len(), total);
    Ok(bytes)
}

fn push_u32(
    bytes: &mut Vec<u8>,
    value: usize,
) -> Result<(), ProductionSourceRefinementEvidenceErrorV1> {
    bytes.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}

fn push_text(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), ProductionSourceRefinementEvidenceErrorV1> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES_V1 {
        return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
    }
    push_u32(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn derive_identity(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

const fn encode_kir_version(version: ProductionCanonicalKernelIrVersionV1) -> u16 {
    match version {
        ProductionCanonicalKernelIrVersionV1::V8 => 8,
        ProductionCanonicalKernelIrVersionV1::V9 => 9,
        ProductionCanonicalKernelIrVersionV1::V11 => 11,
        ProductionCanonicalKernelIrVersionV1::V12 => 12,
        ProductionCanonicalKernelIrVersionV1::V13 => 13,
    }
}

fn decode_kir_version(
    version: u16,
) -> Result<ProductionCanonicalKernelIrVersionV1, ProductionSourceRefinementEvidenceErrorV1> {
    match version {
        8 => Ok(ProductionCanonicalKernelIrVersionV1::V8),
        9 => Ok(ProductionCanonicalKernelIrVersionV1::V9),
        11 => Ok(ProductionCanonicalKernelIrVersionV1::V11),
        12 => Ok(ProductionCanonicalKernelIrVersionV1::V12),
        13 => Ok(ProductionCanonicalKernelIrVersionV1::V13),
        _ => Err(ProductionSourceRefinementEvidenceErrorV1::UnsupportedCanonicalKirVersion),
    }
}

struct ReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], ProductionSourceRefinementEvidenceErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionSourceRefinementEvidenceErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionSourceRefinementEvidenceErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, ProductionSourceRefinementEvidenceErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionSourceRefinementEvidenceErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionSourceRefinementEvidenceErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn usize_u32(&mut self) -> Result<usize, ProductionSourceRefinementEvidenceErrorV1> {
        usize::try_from(self.u32()?)
            .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::ArithmeticOverflow)
    }

    fn text(&mut self) -> Result<Box<str>, ProductionSourceRefinementEvidenceErrorV1> {
        let length = self.usize_u32()?;
        if length == 0 || length > MAX_NAME_BYTES_V1 {
            return Err(ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster);
        }
        std::str::from_utf8(self.take(length)?)
            .map(str::to_owned)
            .map(String::into_boxed_str)
            .map_err(|_| ProductionSourceRefinementEvidenceErrorV1::InvalidRootRoster)
    }

    fn remaining_without_terminal(
        &self,
    ) -> Result<usize, ProductionSourceRefinementEvidenceErrorV1> {
        self.bytes
            .len()
            .checked_sub(self.offset)
            .and_then(|value| value.checked_sub(TERMINAL_BYTES_V1))
            .ok_or(ProductionSourceRefinementEvidenceErrorV1::InvalidLength)
    }

    fn finish(self) -> Result<(), ProductionSourceRefinementEvidenceErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProductionSourceRefinementEvidenceErrorV1::TrailingBytes)
        }
    }
}

/// Fail-closed checked source-refinement evidence error.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionSourceRefinementEvidenceErrorV1 {
    /// The live semantic/KIR owner failed exact replay.
    LiveOwner(ProductionSemanticKirErrorV1),
    /// Exact V5 correspondence evidence was invalid or incomplete.
    Correspondence(ProductionCorrespondenceEvidenceErrorV5),
    /// Evidence exceeds its fixed byte limit.
    TooLarge,
    /// A bounded allocation failed.
    AllocationFailure,
    /// Length/count arithmetic overflowed.
    ArithmeticOverflow,
    /// The envelope ended before a complete field.
    Truncated,
    /// Header magic, version, policy, flags, or reserved bytes differ.
    InvalidHeader,
    /// Declared and actual lengths differ.
    InvalidLength,
    /// The source endpoint uses no admitted canonical KIR wire version.
    UnsupportedCanonicalKirVersion,
    /// A required identity is the reserved zero value.
    ZeroIdentity,
    /// Roots are omitted, duplicated, reordered, or not bijective with KIR entries.
    InvalidRootRoster,
    /// Semantic roots are not in their unique increasing order.
    NonCanonicalRootOrder,
    /// Nested correspondence, source MIR, or KIR coordinates differ.
    NestedIdentityMismatch,
    /// The terminal content identity differs.
    IdentityMismatch,
    /// Decoding and re-encoding did not reproduce the exact bytes.
    NonCanonical,
    /// Bytes remain after the terminal identity.
    TrailingBytes,
}

impl fmt::Display for ProductionSourceRefinementEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid checked source-refinement evidence: {self:?}"
        )
    }
}

impl Error for ProductionSourceRefinementEvidenceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LiveOwner(error) => Some(error),
            Self::Correspondence(error) => Some(error),
            _ => None,
        }
    }
}
