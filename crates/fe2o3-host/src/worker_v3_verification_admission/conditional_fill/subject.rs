use super::super::{CompilerGeneratedKernelExpectationV1, WorkerV3VerificationRequestV1};
use super::{ConditionalFillArtifactView, WorkerV3ConditionalFillPendingErrorV1};
use crate::{
    CheckedWorkerV3CompilerClosureV1, WorkerV3HostLineageIdentityV1,
    WorkerV3VerificationChallengeIdentityV1,
};
use fe2o3_hsaco_finalize::ContentIdentityV1;
use fe2o3_kernel_analysis::check_gfx942_fill_analysis_v1;
use fe2o3_verifier::{
    OwnedConditionalFillRefinementExecutionV1, check_conditional_fill_program_v1,
};
use std::fmt;

const DOMAIN: &[u8] = b"FE2O3/WORKER-V3-CONDITIONAL-FILL-SUBJECT/V1\0";
const CANONICAL_BYTES: usize = DOMAIN.len() + 32 + 32 + 1 + 40 + 40 + 32;

/// Identifies an exact checked compiler/proof association and its executed refinement.
///
/// This is inert matching data, not retained proof custody, a fresh nonce,
/// publication currentness, protected compiler-origin evidence or launch authority.
/// Copying these bytes cannot transfer any of those properties. The pending
/// artifact or proof custodian must retain all original owners independently of this value.
/// Equal canonical evidence may have the same subject; original object and process
/// occurrences must be authenticated separately.
///
/// Encoding is the versioned domain, host lineage (32 bytes), deterministic host
/// request challenge (32), refinement boundary (u8), obligation identity and signed
/// receipt identity (each SHA-256 then little-endian u64 length), and receipt key (32).
/// The lineage/challenge commit the compiler, publication, finalizer and generated
/// host contract. The original obligation commits the compiler/analyzer inputs,
/// generated proof source and recipes; no projected receipt schema is duplicated.
///
/// ```
/// use fe2o3_host::InertWorkerV3ConditionalFillSubjectV1;
/// fn inert_traits<T: Clone + Send + Sync + 'static>() {}
/// inert_traits::<InertWorkerV3ConditionalFillSubjectV1>();
/// ```
///
/// ```compile_fail,E0451
/// use fe2o3_host::InertWorkerV3ConditionalFillSubjectV1;
/// use fe2o3_hsaco_finalize::ContentIdentityV1;
/// let _ = InertWorkerV3ConditionalFillSubjectV1 {
///     canonical: std::array::from_fn(|_| 0),
///     identity: ContentIdentityV1::calculate(&[]),
/// };
/// ```
///
/// ```compile_fail,E0308
/// use fe2o3_host::{AuthenticatedWorkerV3ExecutableV1,
///     CompilerGeneratedKernelExpectationV1, InertWorkerV3ConditionalFillSubjectV1};
/// fn cannot_promote<K: CompilerGeneratedKernelExpectationV1>(
///     subject: InertWorkerV3ConditionalFillSubjectV1,
/// ) -> AuthenticatedWorkerV3ExecutableV1<K> { subject }
/// ```
///
/// ```compile_fail,E0308
/// use fe2o3_host::InertWorkerV3ConditionalFillSubjectV1;
/// use fe2o3_verifier::OwnedConditionalFillRefinementExecutionV1;
/// fn cannot_rebuild(subject: InertWorkerV3ConditionalFillSubjectV1)
///     -> OwnedConditionalFillRefinementExecutionV1 { subject }
/// ```
///
/// ```compile_fail,E0594
/// use fe2o3_host::InertWorkerV3ConditionalFillSubjectV1;
/// fn cannot_mutate(subject: &mut InertWorkerV3ConditionalFillSubjectV1) {
///     subject.canonical_bytes()[0] ^= 1;
/// }
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct InertWorkerV3ConditionalFillSubjectV1 {
    canonical: [u8; CANONICAL_BYTES],
    identity: ContentIdentityV1,
}

impl fmt::Debug for InertWorkerV3ConditionalFillSubjectV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InertWorkerV3ConditionalFillSubjectV1")
            .field("identity", &self.identity)
            .field("launch_authority", &false)
            .finish_non_exhaustive()
    }
}

impl InertWorkerV3ConditionalFillSubjectV1 {
    pub(super) fn check<K: CompilerGeneratedKernelExpectationV1>(
        request: &WorkerV3VerificationRequestV1<'_, K>,
        refinement: &OwnedConditionalFillRefinementExecutionV1,
    ) -> Result<Self, WorkerV3ConditionalFillPendingErrorV1> {
        use WorkerV3ConditionalFillPendingErrorV1 as E;
        let program = check_conditional_fill_program_v1(refinement.inputs(), refinement.lineage())
            .map_err(E::Program)?;
        let machine = check_gfx942_fill_analysis_v1(
            refinement.analysis_execution(),
            program.function_symbol(),
        )
        .map_err(E::Machine)?;
        let association = request
            .check_conditional_fill_analysis_v1(&program, &machine)
            .map_err(E::Association)?;
        if association.generated_host_contract_identity()
            != request.generated_host_contract_identity()
        {
            return Err(E::Marker("generated host contract"));
        }
        Ok(Self::from_checked_roots(
            request.lineage_identity(),
            request.challenge_identity(),
            refinement,
        ))
    }

    fn from_checked_roots(
        lineage: WorkerV3HostLineageIdentityV1,
        challenge: WorkerV3VerificationChallengeIdentityV1,
        refinement: &OwnedConditionalFillRefinementExecutionV1,
    ) -> Self {
        let canonical = SubjectRoots {
            lineage: *lineage.as_bytes(),
            challenge: *challenge.as_bytes(),
            boundary: refinement.boundary() as u8,
            obligation: ContentIdentityV1::calculate(refinement.obligation_preimage()),
            receipt: ContentIdentityV1::calculate(refinement.signed_receipt_wire()),
            key: *refinement.receipt_verifying_key(),
        }
        .encode();
        Self {
            identity: ContentIdentityV1::calculate(&canonical),
            canonical,
        }
    }

    pub const fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub const fn identity(&self) -> ContentIdentityV1 {
        self.identity
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl CheckedWorkerV3CompilerClosureV1<'_> {
    /// Independently joins a retained executed proof to the complete checked closure.
    ///
    /// Derives the closed fill's contract and deterministic matching challenge without a
    /// generated marker or application-supplied contract digest. This neither acquires a
    /// publication token nor consumes FD195. The result is inert: the caller must retain
    /// the original proof and separately authenticate application occurrence, session,
    /// compiler deployment and publication currentness before any invocation admission.
    pub fn check_conditional_fill_refinement_v1(
        &self,
        refinement: &OwnedConditionalFillRefinementExecutionV1,
    ) -> Result<InertWorkerV3ConditionalFillSubjectV1, WorkerV3ConditionalFillPendingErrorV1> {
        use WorkerV3ConditionalFillPendingErrorV1 as E;
        let program = check_conditional_fill_program_v1(refinement.inputs(), refinement.lineage())
            .map_err(E::Program)?;
        let machine = check_gfx942_fill_analysis_v1(
            refinement.analysis_execution(),
            program.function_symbol(),
        )
        .map_err(E::Machine)?;
        let contract = ConditionalFillArtifactView::from_closure(self)
            .check(&program, &machine)
            .map_err(E::Association)?;
        let challenge =
            super::super::derive_challenge(self.lineage_identity(), self.descriptor(), contract);
        Ok(InertWorkerV3ConditionalFillSubjectV1::from_checked_roots(
            self.lineage_identity(),
            challenge,
            refinement,
        ))
    }
}

#[derive(Clone, Copy)]
struct SubjectRoots {
    lineage: [u8; 32],
    challenge: [u8; 32],
    boundary: u8,
    obligation: ContentIdentityV1,
    receipt: ContentIdentityV1,
    key: [u8; 32],
}

impl SubjectRoots {
    fn encode(&self) -> [u8; CANONICAL_BYTES] {
        let mut bytes = [0; CANONICAL_BYTES];
        let mut tail = bytes.as_mut_slice();
        for field in [
            DOMAIN,
            &self.lineage,
            &self.challenge,
            &[self.boundary],
            self.obligation.sha256(),
            &self.obligation.byte_len().to_le_bytes(),
            self.receipt.sha256(),
            &self.receipt.byte_len().to_le_bytes(),
            &self.key,
        ] {
            let (destination, rest) = tail.split_at_mut(field.len());
            destination.copy_from_slice(field);
            tail = rest;
        }
        debug_assert!(tail.is_empty());
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> SubjectRoots {
        SubjectRoots {
            lineage: [1; 32],
            challenge: [2; 32],
            boundary: 5,
            obligation: ContentIdentityV1::from_parts([3; 32], 0x0807060504030201),
            receipt: ContentIdentityV1::from_parts([4; 32], 0x1817161514131211),
            key: [5; 32],
        }
    }

    #[test]
    fn conditional_subject_encoding_has_closed_stable_layout() {
        let bytes = roots().encode();
        assert_eq!(bytes, roots().encode());
        assert_eq!(&bytes[..DOMAIN.len()], DOMAIN);
        let fields = &bytes[DOMAIN.len()..];
        assert_eq!(fields.len(), 177);
        assert_eq!(&fields[..32], &[1; 32]);
        assert_eq!(&fields[32..64], &[2; 32]);
        assert_eq!(fields[64], 5);
        assert_eq!(&fields[65..97], &[3; 32]);
        assert_eq!(&fields[97..105], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&fields[105..137], &[4; 32]);
        assert_eq!(&fields[137..145], &[17, 18, 19, 20, 21, 22, 23, 24]);
        assert_eq!(&fields[145..], &[5; 32]);
    }

    #[test]
    fn conditional_subject_encoding_commits_each_original_root() {
        let original = roots();
        let bytes = original.encode();
        let identity = ContentIdentityV1::calculate(&bytes);
        for field in 0..8 {
            let mut changed = original;
            match field {
                0 => changed.lineage[0] ^= 1,
                1 => changed.challenge[0] ^= 1,
                2 => changed.boundary ^= 1,
                3 => {
                    let mut hash = *original.obligation.sha256();
                    hash[0] ^= 1;
                    changed.obligation =
                        ContentIdentityV1::from_parts(hash, original.obligation.byte_len());
                }
                4 => {
                    changed.obligation = ContentIdentityV1::from_parts(
                        *original.obligation.sha256(),
                        original.obligation.byte_len() + 1,
                    );
                }
                5 => {
                    let mut hash = *original.receipt.sha256();
                    hash[0] ^= 1;
                    changed.receipt =
                        ContentIdentityV1::from_parts(hash, original.receipt.byte_len());
                }
                6 => {
                    changed.receipt = ContentIdentityV1::from_parts(
                        *original.receipt.sha256(),
                        original.receipt.byte_len() + 1,
                    );
                }
                7 => changed.key[0] ^= 1,
                _ => unreachable!(),
            }
            assert_ne!(changed.encode(), bytes, "field {field}");
            assert_ne!(ContentIdentityV1::calculate(&changed.encode()), identity);
        }
    }
}
