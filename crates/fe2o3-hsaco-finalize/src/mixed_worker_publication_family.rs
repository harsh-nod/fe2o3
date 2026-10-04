//! Shared strict publication and recovery for distinct mixed descriptor owners.

macro_rules! mixed_worker_publication_family {
    ($owner:ident, $prepared:ident, $recovered:ident, $published:ident, $compact:ident, $prepare:ident, $persist:ident, $recover:ident, $publish:ident, $into_owner:ident, $variant:ident, $recovery_docs:literal) => {
        use super::*;
        use crate::$owner;
        type E = WorkerV3HsacoPublicationErrorV1;

        /// Move-only nominal intent; cannot be passed to the descriptor-V1 persistence API.
        #[derive(Debug)]
        pub struct $prepared {
            prepared: PreparedProtectedWorkerV3HsacoPublicationV1,
        }
        impl $prepared {
            pub fn publication_intent(&self) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
                self.prepared.intent
            }
            pub fn exact_finalized_hsaco(&self) -> &[u8] {
                self.prepared.exact_finalized_hsaco()
            }
            pub const fn grants_publication_authority(&self) -> bool {
                false
            }
            pub const fn grants_load_authority(&self) -> bool {
                false
            }
            pub const fn grants_launch_authority(&self) -> bool {
                false
            }
        }

        /// Independently reconstructed nominal lineage. Recovery is not compiler/proof authority.
        #[doc = $recovery_docs]
        #[derive(Debug)]
        pub struct $recovered {
            outcome: WorkerV3PublicationIntentOutcomeV1,
            pub(super) record: WorkerV3PublicationIntentRecordV1,
            pub(super) finalized: $owner,
            pub(super) intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
        }
        impl $recovered {
            pub const fn outcome(&self) -> WorkerV3PublicationIntentOutcomeV1 {
                self.outcome
            }
            pub const fn storage_record(&self) -> WorkerV3PublicationIntentRecordV1 {
                self.record
            }
            pub const fn publication_intent(
                &self,
            ) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
                self.intent
            }
            pub fn exact_finalized_hsaco(&self) -> &[u8] {
                self.finalized.finalized().as_bytes()
            }
            pub fn finalized_evidence(&self) -> &$owner {
                &self.finalized
            }
            pub fn compiler_execution_subject_v1(
                &self,
            ) -> Result<InertCompilerExecutionSubjectV1, CompilerExecutionSubjectErrorV1> {
                let raw = self.finalized.raw();
                InertCompilerExecutionSubjectV1::from_replay_evidence(
                    raw.attempt(),
                    raw.handoff_slot(),
                    raw.transaction_identity(),
                    raw.outer_handoff(),
                )
            }
            pub fn publication_binding(
                &self,
                closure: CompilerClosureV2,
            ) -> Result<WorkerV3PublicationBindingV1, E> {
                RecoveredPublicationRef::$variant(self).publication_binding(closure)
            }
            pub const fn grants_publication_authority(&self) -> bool {
                false
            }
            pub const fn grants_load_authority(&self) -> bool {
                false
            }
            pub const fn grants_launch_authority(&self) -> bool {
                false
            }
        }

        /// Exact published nominal occurrence with its non-clone current-publication lease.
        /// Publication does not authenticate compiler execution or grant GPU launch authority.
        #[derive(Debug)]
        pub struct $published {
            recovered: $recovered,
            publication: AttemptScopedHsacoPublicationResultV3,
        }
        impl $published {
            pub const fn recovered_evidence(&self) -> &$recovered {
                &self.recovered
            }
            pub const fn publication_result(&self) -> &AttemptScopedHsacoPublicationResultV3 {
                &self.publication
            }
            pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
                self.publication.published_claim()
            }
            pub fn compiler_execution_subject_v1(
                &self,
            ) -> Result<InertCompilerExecutionSubjectV1, CompilerExecutionSubjectErrorV1> {
                self.recovered.compiler_execution_subject_v1()
            }
            pub const fn grants_compiler_authority(&self) -> bool {
                false
            }
            pub const fn grants_proof_authority(&self) -> bool {
                false
            }
            pub const fn grants_load_authority(&self) -> bool {
                false
            }
            pub const fn grants_launch_authority(&self) -> bool {
                false
            }
            pub fn into_load_envelope_parts_v1(
                self,
            ) -> Result<PublishedProtectedWorkerV3LoadEnvelopePartsV1, E> {
                let storage_record = self.recovered.record;
                let claim = self.publication.published_claim().clone();
                let current_lease = self.publication.into_current_lease();
                let replay = crate::$compact(self.recovered.finalized)?.into_parts();
                Ok(PublishedProtectedWorkerV3LoadEnvelopePartsV1 {
                    replay,
                    storage_record,
                    claim,
                    current_lease,
                })
            }
        }

        pub fn $prepare(producer: &ProducerIdentity, finalized: $owner) -> Result<$prepared, E> {
            Ok($prepared {
                prepared: prepare_versioned_publication(
                    producer,
                    FinalizedOwner::$variant(finalized),
                )?,
            })
        }
        pub fn $persist(
            output_dir: &Path,
            producer: &ProducerIdentity,
            prepared: $prepared,
        ) -> Result<$recovered, E> {
            validate_nominal_recovery(
                producer,
                persist_versioned_publication(output_dir, producer, prepared.prepared)?,
            )
        }
        pub fn $recover(
            output_dir: &Path,
            producer: &ProducerIdentity,
            attempt: BuildAttempt,
        ) -> Result<$recovered, E> {
            validate_nominal_recovery(
                producer,
                recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)?,
            )
        }
        pub fn $publish(
            output_dir: &Path,
            producer: &ProducerIdentity,
            compiler_closure: CompilerClosureV2,
            recovered: $recovered,
        ) -> Result<$published, E> {
            let publication = publish_recovered_versioned(
                output_dir,
                producer,
                compiler_closure,
                RecoveredPublicationRef::$variant(&recovered),
            )?;
            Ok($published {
                recovered,
                publication,
            })
        }

        fn validate_nominal_recovery(
            producer: &ProducerIdentity,
            recovered: RecoveredWorkerV3PublicationIntentV1,
        ) -> Result<$recovered, E> {
            let ValidatedRecoveredPublication {
                outcome,
                record,
                finalized,
                intent,
            } = validate_recovered_versioned(producer, recovered)?;
            Ok($recovered {
                outcome,
                record,
                finalized: finalized.$into_owner()?,
                intent,
            })
        }
    };
}

pub(crate) use mixed_worker_publication_family;
