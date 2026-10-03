//! Canonical transport of a conditional aggregate, never unconditional coverage.
//!
//! The exact condition preimage must hash to the signed obligation. Embedded-key
//! import establishes internal consistency, not protected compiler origin or
//! actual packed-argument/launch discharge.

use std::{error::Error, fmt};

use fe2o3_compiler_lineage::MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4;
use fe2o3_functional_proof::{
    FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2, FunctionalRefinementBindingV2,
    FunctionalRefinementBoundaryV2, FunctionalRefinementImportErrorV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementSubjectsV2,
    ImportedFunctionalRefinementProofV2, SafeReferenceKindV2, VerusToolchainIdentityV2,
    inspect_untrusted_functional_refinement_receipt_v2,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use crate::{
    ProductionConditionalOutputVerusExecutionV1, conditional_output_verus_v1::OBLIGATION_DOMAIN,
};

const MAGIC: &[u8; 8] = b"F2CGVEV1";
const HEADER_BYTES: usize = 20;
const SUFFIX_BYTES: usize = 32 + FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 + 32;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-OUTPUT-EXECUTION-EVIDENCE/V1\0";

#[cfg(test)]
#[path = "../tests/support/conditional_output_evidence.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_functional_proof::FunctionalRefinementResultV2;
    use fixture::{digest, preimage, repair_identity, signed, signed_with};

    fn input() -> Vec<u8> {
        preimage([digest(1), digest(2), digest(3), digest(4)])
    }

    #[test]
    fn conditional_roundtrip_preserves_distinct_ordinals_without_authority() {
        let bytes = signed(&input());
        let decoded = CanonicalConditionalOutputEvidenceV1::decode(&bytes).unwrap();
        assert_eq!(decoded.canonical_bytes(), bytes);
        assert_eq!(decoded.obligation().encode(), input());
        assert_eq!(decoded.obligation().ranked_extent_argument(), 2);
        assert_eq!(decoded.obligation().reference_output_argument(), 0);
        assert_eq!(decoded.obligation().static_global_x_extent(), None);
        assert_eq!(decoded.obligation().workgroup_extents(), [64, 1, 1]);
        assert!(
            decoded
                .obligation()
                .requires_packed_extent_and_launch_discharge()
        );
        assert!(decoded.authenticates_signed_receipt_under_embedded_key());
        assert!(!decoded.authenticates_compiler_origin());
        assert!(!decoded.grants_runtime_authority());
        assert!(
            crate::CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(&bytes).is_err()
        );
    }

    #[test]
    fn every_valid_condition_field_is_bound_to_the_signed_obligation() {
        let original = signed(&input());
        let offsets = (0..4)
            .map(|index| fixture::DOMAIN.len() + 32 * index)
            .chain([
                fixture::LOCATIONS + 8,
                fixture::LOCATIONS + 40,
                fixture::FIELDS,
                fixture::FIELDS + 8,
                fixture::FIELDS + 16,
                fixture::FIELDS + 24,
                fixture::FIELDS + 40,
                fixture::FIELDS + 48,
                fixture::FIELDS + 56,
                fixture::FIELDS + 80,
                fixture::FIELDS + 96,
            ]);
        for offset in offsets {
            let mut bytes = original.clone();
            bytes[20 + offset] += 1;
            repair_identity(&mut bytes);
            assert!(
                matches!(
                    CanonicalConditionalOutputEvidenceV1::decode(&bytes),
                    Err(ConditionalOutputEvidenceErrorV1::SignedObligationMismatch)
                ),
                "offset {offset}"
            );
        }
    }

    #[test]
    fn malformed_profiles_fail_even_with_a_valid_signature() {
        for (field, value) in [
            (0, u64::MAX),
            (1, u64::MAX),
            (2, 0),
            (3, 0),
            (4, 4),
            (7, 0),
            (7, u64::MAX),
            (10, 0),
            (11, 2),
            (12, u64::MAX),
            (14, 0),
            (14, 2),
        ] {
            let mut input = input();
            input[fixture::FIELDS + field * 8..fixture::FIELDS + (field + 1) * 8]
                .copy_from_slice(&value.to_le_bytes());
            if field == 7 && value == u64::MAX {
                input[fixture::FIELDS + 8 * 8..fixture::FIELDS + 9 * 8]
                    .copy_from_slice(&2_u64.to_le_bytes());
            }
            assert!(
                CanonicalConditionalOutputEvidenceV1::decode(&signed(&input)).is_err(),
                "field {field}={value}"
            );
        }
        let mut trailing = input();
        trailing.push(0);
        assert!(CanonicalConditionalOutputEvidenceV1::decode(&signed(&trailing)).is_err());
        let mut utf8 = input();
        utf8[fixture::DOMAIN.len() + 128 + 8] = 0xff;
        assert!(CanonicalConditionalOutputEvidenceV1::decode(&signed(&utf8)).is_err());
    }

    #[test]
    fn full_workgroup_profile_enforces_all_dimensions() {
        let mut bytes = input();
        bytes[fixture::FIELDS + 11 * 8] = 1;
        bytes[fixture::FIELDS + 6 * 8] = 128;
        assert!(CanonicalConditionalOutputEvidenceV1::decode(&signed(&bytes)).is_ok());
        for offset in [6, 8, 9] {
            let mut bad = bytes.clone();
            bad[fixture::FIELDS + offset * 8] += 1;
            assert!(CanonicalConditionalOutputEvidenceV1::decode(&signed(&bad)).is_err());
        }
    }

    #[test]
    fn strict_import_rejects_wrong_boundaries_and_nonproof_results() {
        use FunctionalRefinementBoundaryV2 as B;
        use FunctionalRefinementResultV2 as R;
        for (boundary, result) in [
            (B::SafeReferenceMirToLivePliron, R::Proved),
            (B::SafeReferenceMirToKernelMir, R::Proved),
            (
                B::SafeReferenceMirToLivePlironConditionalCoverage,
                R::Failed,
            ),
            (
                B::SafeReferenceMirToLivePlironConditionalCoverage,
                R::Inconclusive,
            ),
        ] {
            assert!(matches!(
                CanonicalConditionalOutputEvidenceV1::decode(&signed_with(
                    &input(),
                    boundary,
                    result
                )),
                Err(ConditionalOutputEvidenceErrorV1::Receipt(_))
            ));
        }
    }

    #[test]
    fn untrusted_inspection_does_not_admit_a_bad_signature_or_key() {
        let original = signed(&input());
        let wire_start = 20 + input().len() + 32;
        for offset in [wire_start - 32, original.len() - 33] {
            let mut bytes = original.clone();
            bytes[offset] ^= 1;
            repair_identity(&mut bytes);
            assert!(
                inspect_untrusted_functional_refinement_receipt_v2(
                    &bytes[wire_start..bytes.len() - 32]
                )
                .is_ok()
            );
            assert!(matches!(
                CanonicalConditionalOutputEvidenceV1::decode(&bytes),
                Err(ConditionalOutputEvidenceErrorV1::Receipt(_))
            ));
        }
    }

    #[test]
    fn header_lengths_checksum_and_every_truncation_are_rejected() {
        let original = signed(&input());
        for length in 0..original.len() {
            assert!(CanonicalConditionalOutputEvidenceV1::decode(&original[..length]).is_err());
        }
        for offset in [0, 8, 10, 12, 16, original.len() - 1] {
            let mut bytes = original.clone();
            bytes[offset] ^= 1;
            assert!(CanonicalConditionalOutputEvidenceV1::decode(&bytes).is_err());
        }
        let mut bytes = original.clone();
        bytes.push(0);
        assert!(CanonicalConditionalOutputEvidenceV1::decode(&bytes).is_err());
        assert!(
            CanonicalConditionalOutputEvidenceV1::decode(&vec![
                0;
                MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4
                    + 1
            ])
            .is_err()
        );
    }
}

/// Inert coordinates in the retained ranked graph, not source or machine PCs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalOutputLocationV1 {
    block: u32,
    operation: u32,
}

impl ConditionalOutputLocationV1 {
    pub const fn block(self) -> u32 {
        self.block
    }
    pub const fn operation(self) -> u32 {
        self.operation
    }
}

/// Parsed claims of the closed singleton guarded-output profile. Decoding does
/// not recreate a live classifier owner or authenticate the claims' origin.
#[derive(Debug, Eq, PartialEq)]
pub struct InertConditionalOutputObligationV1 {
    identities: [DigestV1; 4],
    view_name: Box<str>,
    locations: [ConditionalOutputLocationV1; 4],
    fields: [u64; 15],
    component: [DigestV1; 14],
    subjects: FunctionalRefinementSubjectsV2,
}

impl InertConditionalOutputObligationV1 {
    fn decode(bytes: &[u8]) -> Result<Self, ConditionalOutputEvidenceErrorV1> {
        use ConditionalOutputEvidenceErrorV1 as E;
        if bytes.len()
            > MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4 - HEADER_BYTES - SUFFIX_BYTES
        {
            return Err(E::InvalidLength);
        }
        let mut cursor = Cursor(bytes);
        if cursor.take(OBLIGATION_DOMAIN.len())? != OBLIGATION_DOMAIN {
            return Err(E::InvalidObligation);
        }
        let mut identities = [DigestV1::ZERO; 4];
        for identity in &mut identities {
            *identity = cursor.digest()?;
        }
        if identities.iter().any(|identity| identity.is_zero()) {
            return Err(E::InvalidObligation);
        }
        let name_len = usize::try_from(cursor.u64()?).map_err(|_| E::InvalidLength)?;
        let view_name =
            std::str::from_utf8(cursor.take(name_len)?).map_err(|_| E::InvalidObligation)?;
        if view_name.is_empty() {
            return Err(E::InvalidObligation);
        }
        let mut locations = [ConditionalOutputLocationV1 {
            block: 0,
            operation: 0,
        }; 4];
        for location in &mut locations {
            location.block = u32::try_from(cursor.u64()?).map_err(|_| E::InvalidObligation)?;
            location.operation = u32::try_from(cursor.u64()?).map_err(|_| E::InvalidObligation)?;
        }
        let mut fields = [0_u64; 15];
        for field in &mut fields {
            *field = cursor.u64()?;
        }
        for index in [0, 1, 4, 12, 13] {
            u32::try_from(fields[index]).map_err(|_| E::InvalidObligation)?;
        }
        if fields[2] == 0 || fields[3] == 0 || fields[4] != 32 || fields[11] > 1 || fields[14] != 1
        {
            return Err(E::InvalidObligation);
        }
        let workgroup = &fields[7..10];
        if workgroup.contains(&0)
            || workgroup
                .iter()
                .copied()
                .try_fold(1_u64, u64::checked_mul)
                .is_none()
            || fields[10] == 0
            || (fields[11] == 1
                && (fields[8] != 1
                    || fields[9] != 1
                    || (fields[6] != 0 && !fields[6].is_multiple_of(fields[7]))))
        {
            return Err(E::InvalidObligation);
        }
        let mut component = [DigestV1::ZERO; 14];
        for identity in &mut component {
            *identity = cursor.digest()?;
        }
        if !cursor.0.is_empty()
            || component
                .iter()
                .enumerate()
                .any(|(index, identity)| index != 1 && identity.is_zero())
        {
            return Err(E::InvalidObligation);
        }
        let kind = if component[1].is_zero() {
            SafeReferenceKindV2::Mir
        } else {
            SafeReferenceKindV2::SourceAndMir
        };
        let subjects = FunctionalRefinementSubjectsV2::new(
            kind,
            component[0],
            component[1],
            component[2],
            component[3],
            component[4],
        )
        .map_err(E::Receipt)?;
        VerusToolchainIdentityV2::new(
            component[9],
            component[10],
            component[11],
            component[12],
            component[13],
        )
        .map_err(E::Receipt)?;
        let parsed = Self {
            identities,
            view_name: view_name.into(),
            locations,
            fields,
            component,
            subjects,
        };
        if parsed.encode() != bytes {
            return Err(E::InvalidObligation);
        }
        Ok(parsed)
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OBLIGATION_DOMAIN);
        for identity in self.identities {
            bytes.extend_from_slice(identity.as_bytes());
        }
        bytes.extend_from_slice(&(self.view_name.len() as u64).to_le_bytes());
        bytes.extend_from_slice(self.view_name.as_bytes());
        for location in self.locations {
            bytes.extend_from_slice(&u64::from(location.block).to_le_bytes());
            bytes.extend_from_slice(&u64::from(location.operation).to_le_bytes());
        }
        for value in self.fields {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for identity in self.component {
            bytes.extend_from_slice(identity.as_bytes());
        }
        bytes
    }

    pub const fn middle_end_identity(&self) -> DigestV1 {
        self.identities[0]
    }
    pub const fn source_semantic_identity(&self) -> DigestV1 {
        self.identities[1]
    }
    pub const fn ranked_kernel_identity(&self) -> DigestV1 {
        self.identities[2]
    }
    pub const fn generated_source_identity(&self) -> DigestV1 {
        self.identities[3]
    }
    pub fn view_name(&self) -> &str {
        &self.view_name
    }
    pub const fn view_definition(&self) -> ConditionalOutputLocationV1 {
        self.locations[0]
    }
    pub const fn contract_location(&self) -> ConditionalOutputLocationV1 {
        self.locations[1]
    }
    pub const fn guard_location(&self) -> ConditionalOutputLocationV1 {
        self.locations[2]
    }
    pub const fn write_location(&self) -> ConditionalOutputLocationV1 {
        self.locations[3]
    }
    /// Ranked entry argument ordinal, never implicitly a source ABI ordinal.
    pub const fn ranked_extent_argument(&self) -> u32 {
        self.fields[0] as u32
    }
    /// Logical reference-output ordinal; physical ABI interpretation is separate.
    pub const fn reference_output_argument(&self) -> u32 {
        self.fields[1] as u32
    }
    pub const fn allocation_origin(&self) -> u64 {
        self.fields[2]
    }
    pub const fn noalias_class(&self) -> u64 {
        self.fields[3]
    }
    pub const fn element_width_bits(&self) -> u32 {
        self.fields[4] as u32
    }
    pub const fn grid_identity(&self) -> u64 {
        self.fields[5]
    }
    pub const fn static_global_x_extent(&self) -> Option<u64> {
        if self.fields[6] == 0 {
            None
        } else {
            Some(self.fields[6])
        }
    }
    pub const fn workgroup_extents(&self) -> [u64; 3] {
        [self.fields[7], self.fields[8], self.fields[9]]
    }
    pub const fn subgroup_size(&self) -> u64 {
        self.fields[10]
    }
    pub const fn requires_full_physical_workgroups(&self) -> bool {
        self.fields[11] == 1
    }
    pub const fn effect_location(&self) -> ConditionalOutputLocationV1 {
        ConditionalOutputLocationV1 {
            block: self.fields[12] as u32,
            operation: self.fields[13] as u32,
        }
    }
    pub const fn subjects(&self) -> FunctionalRefinementSubjectsV2 {
        self.subjects
    }
    pub const fn requires_packed_extent_and_launch_discharge(&self) -> bool {
        true
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Move-only signed conditional evidence. The embedded key authenticates the
/// packet internally; protected compiler origin and launch admission are separate.
///
/// ```compile_fail
/// use fe2o3_verifier::CanonicalConditionalOutputEvidenceV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<CanonicalConditionalOutputEvidenceV1>();
/// ```
#[derive(Debug)]
#[must_use = "retain the pending launch condition with its signed receipt"]
pub struct CanonicalConditionalOutputEvidenceV1 {
    obligation: InertConditionalOutputObligationV1,
    imported: ImportedFunctionalRefinementProofV2,
    canonical_bytes: Box<[u8]>,
}

impl CanonicalConditionalOutputEvidenceV1 {
    pub fn from_execution(
        execution: &ProductionConditionalOutputVerusExecutionV1,
    ) -> Result<Self, ConditionalOutputEvidenceErrorV1> {
        let bytes = encode(
            execution.obligation_preimage(),
            execution.receipt_verifying_key(),
            execution.signed_receipt_wire(),
        )?;
        let decoded = Self::decode(&bytes)?;
        let claims = decoded.obligation();
        let coverage = execution.staging().coverage();
        let locations_match = [
            (claims.view_definition(), coverage.view_definition()),
            (claims.contract_location(), coverage.contract_location()),
            (claims.guard_location(), coverage.guard_location()),
            (claims.write_location(), coverage.write_location()),
        ]
        .into_iter()
        .all(|(decoded, live)| {
            usize::try_from(decoded.block()).ok() == Some(live.block())
                && usize::try_from(decoded.operation()).ok() == Some(live.operation())
        });
        if decoded.imported.binding() != execution.binding()
            || claims.middle_end_identity().as_bytes() != execution.staging().evidence_identity()
            || claims.source_semantic_identity().as_bytes() != execution.source_semantic_identity()
            || claims.ranked_kernel_identity().as_bytes() != execution.ranked_kernel_identity()
            || claims.generated_source_identity() != execution.generated_source_identity()
            || !locations_match
            || claims.view_name() != coverage.view_name()
            || usize::try_from(claims.ranked_extent_argument()).ok()
                != Some(coverage.ranked_extent_argument())
            || claims.reference_output_argument() != execution.staging().reference_output_argument()
            || claims.allocation_origin() != coverage.allocation_origin()
            || claims.noalias_class() != coverage.noalias_class()
            || claims.element_width_bits() != coverage.element_width()
            || claims.grid_identity() != coverage.grid_identity()
            || claims.static_global_x_extent() != coverage.static_global_x_extent()
            || claims.workgroup_extents() != coverage.workgroup_extents()
            || claims.subgroup_size() != coverage.subgroup_size()
            || claims.requires_full_physical_workgroups()
                != coverage.requires_full_physical_workgroups()
        {
            return Err(ConditionalOutputEvidenceErrorV1::ExecutionMismatch);
        }
        Ok(decoded)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ConditionalOutputEvidenceErrorV1> {
        use ConditionalOutputEvidenceErrorV1 as E;
        if bytes.len() < HEADER_BYTES + SUFFIX_BYTES
            || bytes.len() > MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4
        {
            return Err(E::InvalidLength);
        }
        let mut cursor = Cursor(bytes);
        if cursor.take(8)? != MAGIC || cursor.u16()? != 1 || cursor.u16()? != 0 {
            return Err(E::InvalidHeader);
        }
        if usize::try_from(cursor.u32()?).ok() != Some(bytes.len()) {
            return Err(E::InvalidLength);
        }
        let preimage_len = usize::try_from(cursor.u32()?).map_err(|_| E::InvalidLength)?;
        if preimage_len.checked_add(HEADER_BYTES + SUFFIX_BYTES) != Some(bytes.len()) {
            return Err(E::InvalidLength);
        }
        let preimage = cursor.take(preimage_len)?;
        let obligation = InertConditionalOutputObligationV1::decode(preimage)?;
        let verifying_key = cursor.fixed::<32>()?;
        let wire = cursor.take(FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2)?;
        if cursor.fixed::<32>()? != identity(&bytes[..bytes.len() - 32]) || !cursor.0.is_empty() {
            return Err(E::IdentityMismatch);
        }
        let expected = FunctionalRefinementBindingV2::from_subjects(
            obligation.subjects(),
            DigestV1::from_untrusted_bytes(Sha256::digest(preimage).into()),
        )
        .map_err(E::Receipt)?;
        let (untrusted_binding, untrusted_toolchain) =
            inspect_untrusted_functional_refinement_receipt_v2(wire).map_err(E::Receipt)?;
        if untrusted_binding != expected {
            return Err(E::SignedObligationMismatch);
        }
        let policy = FunctionalRefinementImportPolicyV2::new(
            verifying_key,
            untrusted_toolchain,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePlironConditionalCoverage,
        )
        .map_err(E::Receipt)?;
        let mut importer =
            FunctionalRefinementReceiptImporterV2::new(policy, 1).map_err(E::Receipt)?;
        let imported = importer
            .import(FunctionalRefinementImportExpectationV2::new(expected), wire)
            .map_err(E::Receipt)?;
        Ok(Self {
            obligation,
            imported,
            canonical_bytes: bytes.into(),
        })
    }

    pub const fn obligation(&self) -> &InertConditionalOutputObligationV1 {
        &self.obligation
    }
    pub const fn imported_proof(&self) -> &ImportedFunctionalRefinementProofV2 {
        &self.imported
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub const fn authenticates_signed_receipt_under_embedded_key(&self) -> bool {
        self.imported.signature_and_policy_verified()
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConditionalOutputEvidenceErrorV1 {
    InvalidHeader,
    InvalidLength,
    InvalidObligation,
    IdentityMismatch,
    SignedObligationMismatch,
    ExecutionMismatch,
    Receipt(FunctionalRefinementImportErrorV2),
}

impl fmt::Display for ConditionalOutputEvidenceErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Receipt(error) => write!(f, "conditional receipt import: {error}"),
            other => write!(f, "conditional output evidence rejected: {other:?}"),
        }
    }
}
impl Error for ConditionalOutputEvidenceErrorV1 {}

fn encode(
    preimage: &[u8],
    key: &[u8; 32],
    wire: &[u8],
) -> Result<Vec<u8>, ConditionalOutputEvidenceErrorV1> {
    let length = preimage
        .len()
        .checked_add(HEADER_BYTES + SUFFIX_BYTES)
        .ok_or(ConditionalOutputEvidenceErrorV1::InvalidLength)?;
    if length > MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4
        || wire.len() != FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2
    {
        return Err(ConditionalOutputEvidenceErrorV1::InvalidLength);
    }
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(&(preimage.len() as u32).to_le_bytes());
    bytes.extend_from_slice(preimage);
    bytes.extend_from_slice(key);
    bytes.extend_from_slice(wire);
    bytes.extend_from_slice(&identity(&bytes));
    Ok(bytes)
}

fn identity(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(IDENTITY_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ConditionalOutputEvidenceErrorV1> {
        let (value, rest) = self
            .0
            .split_at_checked(count)
            .ok_or(ConditionalOutputEvidenceErrorV1::InvalidLength)?;
        self.0 = rest;
        Ok(value)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ConditionalOutputEvidenceErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ConditionalOutputEvidenceErrorV1::InvalidLength)
    }
    fn u16(&mut self) -> Result<u16, ConditionalOutputEvidenceErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }
    fn u32(&mut self) -> Result<u32, ConditionalOutputEvidenceErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn u64(&mut self) -> Result<u64, ConditionalOutputEvidenceErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
    fn digest(&mut self) -> Result<DigestV1, ConditionalOutputEvidenceErrorV1> {
        Ok(DigestV1::from_untrusted_bytes(self.fixed()?))
    }
}
