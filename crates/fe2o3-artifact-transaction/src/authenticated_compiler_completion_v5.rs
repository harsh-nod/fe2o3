//! Signed, policy-bound completion authority for the V5 artifact transaction.

use std::{error::Error, fmt};

use ed25519_dalek::{Signature, VerifyingKey};
use fe2o3_compiler_ffi::{InertProductionCapabilityHandoffV5, InertSimulationBundleV8};
use fe2o3_compiler_lineage::{
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
    InertCompilerProofOwnerV5, InertMultiRootStaticCapabilityEvidenceAssociationV1,
    MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1, MAX_INERT_COMPILER_PROOF_OWNER_BYTES_V5,
    MAX_INERT_MULTI_ROOT_STATIC_CAPABILITY_EVIDENCE_BYTES_V1,
};
use sha2::{Digest, Sha256};

const MAGIC_V5: [u8; 8] = *b"F2CAVR05";
const VERSION_V5: u16 = 5;
const POLICY_VERSION_V5: u16 = 1;
const FIELD_COUNT_V5: u16 = 10;
const HEADER_BYTES_V5: usize = 24;
const FIELD_HEADER_BYTES_V5: usize = 8;
const DOMAIN_V5: &[u8] = b"FE2O3/AUTHENTICATED-COMPILER-CAPABILITY-VERIFIER-RESPONSE/V5\0";
const CLAIM_V5: &[u8] =
    b"sealed-213-proof-owner/checked-machine-refinement/exact-request-kernel-target-object";
const POLICY_IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/COMPILER-CAPABILITY-VERIFIER-POLICY-IDENTITY/V5\0";
const SIGNATURE_MESSAGE_DOMAIN_V5: &[u8] = b"FE2O3/COMPILER-CAPABILITY-VERIFIER-SIGNATURE/V5\0";
const RESPONSE_IDENTITY_DOMAIN_V5: &[u8] =
    b"FE2O3/COMPILER-CAPABILITY-VERIFIER-RESPONSE-IDENTITY/V5\0";
const EVIDENCE_IDENTITY_DOMAIN_V5: &[u8] =
    b"FE2O3/AUTHENTICATED-COMPILER-CAPABILITY-EVIDENCE-IDENTITY/V5\0";
const TRUSTED_KEY_HEX_V5: Option<&str> =
    option_env!("FE2O3_COMPILER_CAPABILITY_VERIFIER_V5_PUBLIC_KEY");

/// Maximum accepted canonical signed-response payload.
pub const MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5: usize = HEADER_BYTES_V5
    + FIELD_HEADER_BYTES_V5 * FIELD_COUNT_V5 as usize
    + DOMAIN_V5.len()
    + CLAIM_V5.len()
    + 32
    + 32
    + 40
    + 40
    + 40
    + MAX_INERT_COMPILER_PROOF_OWNER_BYTES_V5
    + MAX_INERT_MULTI_ROOT_STATIC_CAPABILITY_EVIDENCE_BYTES_V1
    + MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContentIdentityV5 {
    sha256: [u8; 32],
    byte_len: u64,
}

/// Exact identity of canonical checker evidence retained by the authenticated owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthenticatedCompilerCapabilityEvidenceIdentityV5 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl AuthenticatedCompilerCapabilityEvidenceIdentityV5 {
    /// Returns the domain-separated digest rederived from exact evidence bytes.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact checker-evidence byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Re-derives this typed identity from exact checker-evidence bytes.
    pub fn matches_bytes(self, bytes: &[u8]) -> bool {
        authenticated_compiler_capability_evidence_identity_v5(bytes)
            .is_ok_and(|identity| identity == self)
    }
}

/// Derives the typed V5 identity of exact canonical checker-evidence bytes.
pub fn authenticated_compiler_capability_evidence_identity_v5(
    evidence: &[u8],
) -> Result<
    AuthenticatedCompilerCapabilityEvidenceIdentityV5,
    AuthenticatedCompilerCapabilityCompletionErrorV5,
> {
    evidence_identity(evidence)
}

/// Canonical but inert response assembled by the protected #213/#214 service adapter.
///
/// This value is not authority. Public construction and decoding are safe because only a valid
/// signature under the compile-time V5 trust anchor can create
/// [`AuthenticatedCompilerCapabilityCompletionV5`].
#[derive(Debug)]
pub struct InertCompilerCapabilityVerifierResponseV5 {
    canonical_bytes: Box<[u8]>,
    policy_identity: [u8; 32],
    transaction_identity: [u8; 32],
    handoff_identity: ContentIdentityV5,
    simulation_bundle_identity: ContentIdentityV5,
    object_identity: ContentIdentityV5,
    proof_owner: InertCompilerProofOwnerV5,
    capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
    checker_evidence: Box<[u8]>,
    checker_evidence_identity: AuthenticatedCompilerCapabilityEvidenceIdentityV5,
}

impl InertCompilerCapabilityVerifierResponseV5 {
    /// Builds the exact response to sign under the compiled production verifier policy.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        transaction_identity: [u8; 32],
        handoff: &InertProductionCapabilityHandoffV5,
        simulation_bundle: &InertSimulationBundleV8,
        object_bytes: &[u8],
        proof_owner: InertCompilerProofOwnerV5,
        capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
        checker_evidence: impl Into<Vec<u8>>,
    ) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        let key = trusted_verifying_key_v5()?;
        Self::new_with_verifying_key(
            transaction_identity,
            handoff,
            simulation_bundle,
            object_bytes,
            proof_owner,
            capability_associations,
            checker_evidence.into(),
            &key,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_verifying_key(
        transaction_identity: [u8; 32],
        handoff: &InertProductionCapabilityHandoffV5,
        simulation_bundle: &InertSimulationBundleV8,
        object_bytes: &[u8],
        proof_owner: InertCompilerProofOwnerV5,
        capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
        checker_evidence: Vec<u8>,
        verifying_key: &VerifyingKey,
    ) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        if transaction_identity == [0; 32] || object_bytes.is_empty() {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidIdentity);
        }
        let machine_refinement = machine_refinement(&checker_evidence)?;
        validate_nested_owners(&proof_owner, &capability_associations, &machine_refinement)?;
        let checker_evidence_identity =
            authenticated_compiler_capability_evidence_identity_v5(&checker_evidence)?;
        let handoff_identity = handoff.identity();
        let simulation_bundle_identity = simulation_bundle.identity();
        let response = Self {
            canonical_bytes: Box::new([]),
            policy_identity: policy_identity_v5(verifying_key),
            transaction_identity,
            handoff_identity: ContentIdentityV5 {
                sha256: handoff_identity.sha256(),
                byte_len: handoff_identity.byte_len(),
            },
            simulation_bundle_identity: ContentIdentityV5 {
                sha256: simulation_bundle_identity.sha256(),
                byte_len: simulation_bundle_identity.byte_len(),
            },
            object_identity: content_identity(object_bytes)?,
            proof_owner,
            capability_associations,
            checker_evidence: checker_evidence.into_boxed_slice(),
            checker_evidence_identity,
        };
        response.with_encoded_bytes()
    }

    /// Strictly decodes one complete response without granting authority.
    pub fn decode(bytes: &[u8]) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        if bytes.len() > MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5 {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::TooLarge);
        }
        if bytes.len() < HEADER_BYTES_V5 || bytes[..8] != MAGIC_V5 {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse);
        }
        if read_u16(bytes, 8)? != VERSION_V5
            || read_u16(bytes, 10)? != POLICY_VERSION_V5
            || read_u16(bytes, 12)? != FIELD_COUNT_V5
            || read_u16(bytes, 14)? != 0
            || read_u64(bytes, 16)? != bytes.len() as u64
        {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse);
        }
        let fields = decode_fields(bytes)?;
        if fields[0] != DOMAIN_V5 || fields[1] != CLAIM_V5 {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::WrongPolicy);
        }
        let policy_identity = array::<32>(fields[2])?;
        let transaction_identity = array::<32>(fields[3])?;
        let handoff_identity = decode_identity(fields[4])?;
        let simulation_bundle_identity = decode_identity(fields[5])?;
        let object_identity = decode_identity(fields[6])?;
        if transaction_identity == [0; 32]
            || handoff_identity.sha256 == [0; 32]
            || handoff_identity.byte_len == 0
            || simulation_bundle_identity.sha256 == [0; 32]
            || simulation_bundle_identity.byte_len == 0
            || object_identity.sha256 == [0; 32]
            || object_identity.byte_len == 0
        {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidIdentity);
        }
        let proof_owner = InertCompilerProofOwnerV5::decode(fields[7])
            .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidProofOwner)?;
        let capability_associations = InertMultiRootStaticCapabilityEvidenceAssociationV1::decode(
            fields[8],
        )
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidAssociations)?;
        let checker_evidence = fields[9].to_vec();
        let machine_refinement = machine_refinement(&checker_evidence)?;
        validate_nested_owners(&proof_owner, &capability_associations, &machine_refinement)?;
        let checker_evidence_identity =
            authenticated_compiler_capability_evidence_identity_v5(&checker_evidence)?;
        let response = Self {
            canonical_bytes: Box::new([]),
            policy_identity,
            transaction_identity,
            handoff_identity,
            simulation_bundle_identity,
            object_identity,
            proof_owner,
            capability_associations,
            checker_evidence: checker_evidence.into_boxed_slice(),
            checker_evidence_identity,
        }
        .with_encoded_bytes()?;
        if response.canonical_bytes.as_ref() != bytes {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::NonCanonical);
        }
        Ok(response)
    }

    /// Returns the exact bytes the protected verifier must sign.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the identity derived from the exact checker bytes carried by this response.
    pub const fn checker_evidence_identity(
        &self,
    ) -> AuthenticatedCompilerCapabilityEvidenceIdentityV5 {
        self.checker_evidence_identity
    }

    /// Returns the domain-separated message the protected verifier must sign with Ed25519.
    pub fn signing_message(
        &self,
    ) -> Result<Vec<u8>, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        signature_message(&self.canonical_bytes)
    }

    /// Reports that this unsigned record grants no completion authority.
    pub const fn grants_completion_authority(&self) -> bool {
        false
    }

    fn with_encoded_bytes(
        mut self,
    ) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        let handoff = encode_identity(self.handoff_identity);
        let bundle = encode_identity(self.simulation_bundle_identity);
        let object = encode_identity(self.object_identity);
        let fields: [&[u8]; FIELD_COUNT_V5 as usize] = [
            DOMAIN_V5,
            CLAIM_V5,
            &self.policy_identity,
            &self.transaction_identity,
            &handoff,
            &bundle,
            &object,
            self.proof_owner.canonical_bytes(),
            self.capability_associations.canonical_bytes(),
            &self.checker_evidence,
        ];
        self.canonical_bytes = encode_fields(&fields)?.into_boxed_slice();
        Ok(self)
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fixture(
        transaction_identity: [u8; 32],
        handoff: &InertProductionCapabilityHandoffV5,
        simulation_bundle: &InertSimulationBundleV8,
        object_bytes: &[u8],
        proof_owner: InertCompilerProofOwnerV5,
        capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
        checker_evidence: Vec<u8>,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        Self::new_with_verifying_key(
            transaction_identity,
            handoff,
            simulation_bundle,
            object_bytes,
            proof_owner,
            capability_associations,
            checker_evidence,
            &signing_key.verifying_key(),
        )
    }
}

/// Move-only completion authority issued only by the fixed-policy signature verifier.
///
/// There is no unchecked constructor, arbitrary-key constructor, decoder, or conversion from
/// inert proof bytes. The only production constructor verifies Ed25519 under the public key baked
/// into this crate by `FE2O3_COMPILER_CAPABILITY_VERIFIER_V5_PUBLIC_KEY` at compile time.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::AuthenticatedCompilerCapabilityCompletionV5;
/// fn duplicate(owner: AuthenticatedCompilerCapabilityCompletionV5) {
///     let _copy = owner.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::AuthenticatedCompilerCapabilityCompletionV5;
/// fn forge(bytes: Box<[u8]>) {
///     let _ = unsafe {
///         AuthenticatedCompilerCapabilityCompletionV5::from_independently_authenticated_checker_unchecked(
///             bytes, [7; 32], 1, todo!(), todo!(),
///         )
///     };
/// }
/// ```
#[must_use = "dropping authenticated completion custody abandons checked evidence"]
#[derive(Debug)]
pub struct AuthenticatedCompilerCapabilityCompletionV5 {
    transaction_identity: [u8; 32],
    handoff_identity: ContentIdentityV5,
    simulation_bundle_identity: ContentIdentityV5,
    object_identity: ContentIdentityV5,
    machine_refinement: InertCapabilityRefinementReceiptV1,
    capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
    proof_owner: InertCompilerProofOwnerV5,
    checker_evidence: Box<[u8]>,
    checker_evidence_identity: AuthenticatedCompilerCapabilityEvidenceIdentityV5,
    signed_response_identity: [u8; 32],
}

impl AuthenticatedCompilerCapabilityCompletionV5 {
    /// Verifies and consumes one exact response under the compile-time production trust anchor.
    pub fn from_signed_verifier_response(
        response: InertCompilerCapabilityVerifierResponseV5,
        signature: [u8; 64],
    ) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        let key = trusted_verifying_key_v5()?;
        Self::from_signed_response_with_key(response, signature, &key)
    }

    fn from_signed_response_with_key(
        response: InertCompilerCapabilityVerifierResponseV5,
        signature: [u8; 64],
        key: &VerifyingKey,
    ) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        let response =
            InertCompilerCapabilityVerifierResponseV5::decode(response.canonical_bytes())?;
        if response.policy_identity != policy_identity_v5(key) {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::WrongPolicy);
        }
        key.verify_strict(
            &signature_message(response.canonical_bytes())?,
            &Signature::from_bytes(&signature),
        )
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidSignature)?;
        let checker_evidence_identity =
            authenticated_compiler_capability_evidence_identity_v5(&response.checker_evidence)?;
        if checker_evidence_identity != response.checker_evidence_identity {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidIdentity);
        }
        let signed_response_identity = response_identity(response.canonical_bytes())?;
        let machine_refinement = machine_refinement(&response.checker_evidence)?;
        Ok(Self {
            transaction_identity: response.transaction_identity,
            handoff_identity: response.handoff_identity,
            simulation_bundle_identity: response.simulation_bundle_identity,
            object_identity: response.object_identity,
            machine_refinement,
            capability_associations: response.capability_associations,
            proof_owner: response.proof_owner,
            checker_evidence: response.checker_evidence,
            checker_evidence_identity,
            signed_response_identity,
        })
    }

    /// Returns the exact checker evidence retained by the verified response.
    pub fn checker_evidence_bytes(&self) -> &[u8] {
        &self.checker_evidence
    }

    /// Returns the evidence identity rederived after signature verification.
    pub const fn checker_evidence_identity(
        &self,
    ) -> AuthenticatedCompilerCapabilityEvidenceIdentityV5 {
        self.checker_evidence_identity
    }

    /// Returns the identity of the exact signed canonical response.
    pub const fn signed_response_identity(&self) -> [u8; 32] {
        self.signed_response_identity
    }

    /// Reports whether the signed response binds these exact finalized object bytes.
    ///
    /// This exposes only an equality predicate over the already authenticated object coordinate;
    /// it grants no completion, publication, load, or launch authority.
    pub fn authenticates_exact_object_bytes(&self, object_bytes: &[u8]) -> bool {
        content_identity(object_bytes).ok() == Some(self.object_identity)
    }

    pub(crate) fn matches_transaction(
        &self,
        transaction_identity: &[u8; 32],
        handoff: &InertProductionCapabilityHandoffV5,
        simulation_bundle: &InertSimulationBundleV8,
        object_bytes: &[u8],
    ) -> bool {
        let handoff_identity = handoff.identity();
        let bundle_identity = simulation_bundle.identity();
        self.transaction_identity == *transaction_identity
            && self.handoff_identity.sha256 == handoff_identity.sha256()
            && self.handoff_identity.byte_len == handoff_identity.byte_len()
            && self.simulation_bundle_identity.sha256 == bundle_identity.sha256()
            && self.simulation_bundle_identity.byte_len == bundle_identity.byte_len()
            && content_identity(object_bytes).ok() == Some(self.object_identity)
    }

    pub(crate) fn machine_refinement(&self) -> &InertCapabilityRefinementReceiptV1 {
        &self.machine_refinement
    }

    pub(crate) fn capability_associations(
        &self,
    ) -> &InertMultiRootStaticCapabilityEvidenceAssociationV1 {
        &self.capability_associations
    }

    pub(crate) fn proof_owner(&self) -> &InertCompilerProofOwnerV5 {
        &self.proof_owner
    }

    pub(crate) fn into_evidence_parts(
        self,
    ) -> (Box<[u8]>, AuthenticatedCompilerCapabilityEvidenceIdentityV5) {
        (self.checker_evidence, self.checker_evidence_identity)
    }

    #[cfg(test)]
    pub(crate) fn from_fixture_response(
        response: InertCompilerCapabilityVerifierResponseV5,
        signature: [u8; 64],
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<Self, AuthenticatedCompilerCapabilityCompletionErrorV5> {
        Self::from_signed_response_with_key(response, signature, &signing_key.verifying_key())
    }
}

/// Rejection from strict response decoding, trust-policy validation, or signature verification.
#[derive(Debug)]
#[non_exhaustive]
pub enum AuthenticatedCompilerCapabilityCompletionErrorV5 {
    /// No compile-time production verifier key was provisioned.
    TrustedVerifierKeyUnavailable,
    /// The compile-time production verifier key is not exactly 32-byte hexadecimal Ed25519 data.
    InvalidTrustedVerifierKey,
    /// The response exceeds its fixed resource bound.
    TooLarge,
    /// A length cannot be represented safely.
    LengthOverflow,
    /// The canonical response framing is malformed or truncated.
    MalformedResponse,
    /// The response does not bind the fixed V5 policy and trust key.
    WrongPolicy,
    /// A required identity is zero or has an invalid length.
    InvalidIdentity,
    /// Nested proof-owner bytes are not strict canonical V5 bytes.
    InvalidProofOwner,
    /// Nested capability-association bytes are not strict canonical bytes.
    InvalidAssociations,
    /// The owner, association roster, selected subject, or machine receipt do not agree.
    NestedOwnerMismatch,
    /// The checker evidence is empty, oversized, or otherwise invalid.
    InvalidCheckerEvidence,
    /// The response is a noncanonical alternate encoding.
    NonCanonical,
    /// The Ed25519 signature is invalid for the exact response and fixed policy.
    InvalidSignature,
}

impl fmt::Display for AuthenticatedCompilerCapabilityCompletionErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid authenticated compiler completion V5: {self:?}"
        )
    }
}

impl Error for AuthenticatedCompilerCapabilityCompletionErrorV5 {}

fn validate_nested_owners(
    owner: &InertCompilerProofOwnerV5,
    associations: &InertMultiRootStaticCapabilityEvidenceAssociationV1,
    machine: &InertCapabilityRefinementReceiptV1,
) -> Result<(), AuthenticatedCompilerCapabilityCompletionErrorV5> {
    let ordinal = usize::try_from(owner.selected_subject_ordinal())
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::NestedOwnerMismatch)?;
    let selected = associations
        .entries()
        .get(ordinal)
        .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::NestedOwnerMismatch)?;
    if owner.proof_lineage().is_none()
        || owner.inputs().capability_association() != associations.identity()
        || owner.inputs().machine_refinement() != machine.identity()
        || owner.subjects().len() != associations.entries().len()
        || owner.subjects().iter().copied().ne(associations.subjects())
        || owner.selected_subject() != selected.subject()
        || owner.selected_capability_association() != selected.identity()
    {
        return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::NestedOwnerMismatch);
    }
    Ok(())
}

fn machine_refinement(
    evidence: &[u8],
) -> Result<InertCapabilityRefinementReceiptV1, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        InertCapabilityRefinementReceiptKindV1::Machine,
        evidence.to_vec(),
    )
    .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidCheckerEvidence)
}

fn trusted_verifying_key_v5()
-> Result<VerifyingKey, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    let encoded = TRUSTED_KEY_HEX_V5
        .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::TrustedVerifierKeyUnavailable)?;
    let bytes = decode_hex_key(encoded)?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidTrustedVerifierKey)
}

fn decode_hex_key(
    encoded: &str,
) -> Result<[u8; 32], AuthenticatedCompilerCapabilityCompletionErrorV5> {
    if encoded.len() != 64 {
        return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidTrustedVerifierKey);
    }
    let mut bytes = [0; 32];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = hex_nibble(pair[0])?
            .checked_mul(16)
            .and_then(|high| high.checked_add(hex_nibble(pair[1]).ok()?))
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidTrustedVerifierKey)?;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidTrustedVerifierKey),
    }
}

fn policy_identity_v5(key: &VerifyingKey) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(POLICY_IDENTITY_DOMAIN_V5);
    digest.update(VERSION_V5.to_le_bytes());
    digest.update(POLICY_VERSION_V5.to_le_bytes());
    digest.update(key.as_bytes());
    digest.finalize().into()
}

fn signature_message(
    response: &[u8],
) -> Result<Vec<u8>, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    let length = u64::try_from(response.len())
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
    let mut message = Vec::with_capacity(SIGNATURE_MESSAGE_DOMAIN_V5.len() + 8 + response.len());
    message.extend_from_slice(SIGNATURE_MESSAGE_DOMAIN_V5);
    message.extend_from_slice(&length.to_le_bytes());
    message.extend_from_slice(response);
    Ok(message)
}

#[cfg(test)]
pub(crate) fn sign_fixture_response_v5(
    response: &InertCompilerCapabilityVerifierResponseV5,
    signing_key: &ed25519_dalek::SigningKey,
) -> [u8; 64] {
    use ed25519_dalek::Signer as _;
    signing_key
        .sign(
            &response
                .signing_message()
                .expect("bounded fixture response"),
        )
        .to_bytes()
}

fn evidence_identity(
    evidence: &[u8],
) -> Result<
    AuthenticatedCompilerCapabilityEvidenceIdentityV5,
    AuthenticatedCompilerCapabilityCompletionErrorV5,
> {
    let byte_len = u64::try_from(evidence.len())
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
    let mut digest = Sha256::new();
    digest.update(EVIDENCE_IDENTITY_DOMAIN_V5);
    digest.update(byte_len.to_le_bytes());
    digest.update(evidence);
    Ok(AuthenticatedCompilerCapabilityEvidenceIdentityV5 {
        sha256: digest.finalize().into(),
        byte_len,
    })
}

fn response_identity(
    response: &[u8],
) -> Result<[u8; 32], AuthenticatedCompilerCapabilityCompletionErrorV5> {
    let length = u64::try_from(response.len())
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
    let mut digest = Sha256::new();
    digest.update(RESPONSE_IDENTITY_DOMAIN_V5);
    digest.update(length.to_le_bytes());
    digest.update(response);
    Ok(digest.finalize().into())
}

fn content_identity(
    bytes: &[u8],
) -> Result<ContentIdentityV5, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    if bytes.is_empty() {
        return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::InvalidIdentity);
    }
    let byte_len = u64::try_from(bytes.len())
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
    Ok(ContentIdentityV5 {
        sha256: Sha256::digest(bytes).into(),
        byte_len,
    })
}

fn encode_identity(identity: ContentIdentityV5) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..32].copy_from_slice(&identity.sha256);
    bytes[32..].copy_from_slice(&identity.byte_len.to_le_bytes());
    bytes
}

fn decode_identity(
    bytes: &[u8],
) -> Result<ContentIdentityV5, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    if bytes.len() != 40 {
        return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse);
    }
    Ok(ContentIdentityV5 {
        sha256: array(&bytes[..32])?,
        byte_len: u64::from_le_bytes(array(&bytes[32..])?),
    })
}

fn encode_fields(
    fields: &[&[u8]; FIELD_COUNT_V5 as usize],
) -> Result<Vec<u8>, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    let payload_bytes = fields.iter().try_fold(0usize, |total, field| {
        total
            .checked_add(field.len())
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)
    })?;
    let total = HEADER_BYTES_V5
        .checked_add(FIELD_HEADER_BYTES_V5 * fields.len())
        .and_then(|value| value.checked_add(payload_bytes))
        .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
    if total > MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5 {
        return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::TooLarge);
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&MAGIC_V5);
    bytes.extend_from_slice(&VERSION_V5.to_le_bytes());
    bytes.extend_from_slice(&POLICY_VERSION_V5.to_le_bytes());
    bytes.extend_from_slice(&FIELD_COUNT_V5.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    for (index, field) in fields.iter().enumerate() {
        bytes.extend_from_slice(
            &u16::try_from(index + 1)
                .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(field.len())
                .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(field);
    }
    Ok(bytes)
}

fn decode_fields(
    bytes: &[u8],
) -> Result<[&[u8]; FIELD_COUNT_V5 as usize], AuthenticatedCompilerCapabilityCompletionErrorV5> {
    let mut fields = [&[][..]; FIELD_COUNT_V5 as usize];
    let mut offset = HEADER_BYTES_V5;
    for (index, field) in fields.iter_mut().enumerate() {
        if read_u16(bytes, offset)? != (index + 1) as u16 || read_u16(bytes, offset + 2)? != 0 {
            return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse);
        }
        let length = usize::try_from(read_u32(bytes, offset + 4)?)
            .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
        let start = offset
            .checked_add(FIELD_HEADER_BYTES_V5)
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
        let end = start
            .checked_add(length)
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::LengthOverflow)?;
        *field = bytes
            .get(start..end)
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse)?;
        offset = end;
    }
    if offset != bytes.len() {
        return Err(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse);
    }
    Ok(fields)
}

fn read_u16(
    bytes: &[u8],
    offset: usize,
) -> Result<u16, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    Ok(u16::from_le_bytes(array(
        bytes
            .get(offset..offset + 2)
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse)?,
    )?))
}

fn read_u32(
    bytes: &[u8],
    offset: usize,
) -> Result<u32, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    Ok(u32::from_le_bytes(array(
        bytes
            .get(offset..offset + 4)
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse)?,
    )?))
}

fn read_u64(
    bytes: &[u8],
    offset: usize,
) -> Result<u64, AuthenticatedCompilerCapabilityCompletionErrorV5> {
    Ok(u64::from_le_bytes(array(
        bytes
            .get(offset..offset + 8)
            .ok_or(AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse)?,
    )?))
}

fn array<const N: usize>(
    bytes: &[u8],
) -> Result<[u8; N], AuthenticatedCompilerCapabilityCompletionErrorV5> {
    bytes
        .try_into()
        .map_err(|_| AuthenticatedCompilerCapabilityCompletionErrorV5::MalformedResponse)
}

#[cfg(test)]
mod evidence_identity_tests {
    use super::*;

    #[test]
    fn checker_identity_is_exact_domain_separated_content_identity() {
        let evidence = b"canonical-machine-refinement-receipt";
        let identity = authenticated_compiler_capability_evidence_identity_v5(evidence).unwrap();

        assert!(identity.matches_bytes(evidence));
        assert!(!identity.matches_bytes(b"canonical-machine-refinement-receipt!"));
        assert_ne!(
            identity.sha256(),
            <[u8; 32]>::from(Sha256::digest(evidence))
        );
        assert_eq!(identity.byte_len(), evidence.len() as u64);
    }
}
