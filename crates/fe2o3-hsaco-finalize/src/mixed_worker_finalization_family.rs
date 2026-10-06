//! Closed typed continuations of the same strict Worker transaction.

macro_rules! mixed_worker_finalization_family {
    ($artifact:ident, $artifact_error:ident, $finalize_artifact:ident,
     $inspect_artifact:ident, $owner:ident, $owner_docs:literal,
     $error:ident, $finalize_worker:ident, $identity:ident) => {
        use std::{error::Error, fmt};

        use crate::{
            ContentIdentityV1, $artifact, InertProtectedFirstBuildWorkerV3EvidenceV1,
            InspectedProtectedWorkerV3HsacoV1, $artifact_error, WorkerV3HsacoInspectionError,
            $finalize_artifact, $inspect_artifact,
            nominal_worker_common::{common_launch, export_manifest_matches},
            worker_v3_hsaco_admission::{
                inspect_protected_worker_with_launch, strict_kernel_launch_contract,
            },
        };

        #[doc = $owner_docs]
        #[derive(Debug)]
        pub struct $owner {
            raw: InspectedProtectedWorkerV3HsacoV1,
            finalized: $artifact,
            output: ContentIdentityV1,
            descriptor: ContentIdentityV1,
            identity: crate::FinalizedProtectedWorkerV3HsacoIdentityV1,
        }
        impl $owner {
            /// Exact protected build attempt retained by the first-build evidence.
            pub const fn attempt(&self) -> fe2o3_artifact_transaction::BuildAttempt {
                self.raw.attempt()
            }
            pub const fn identity(&self) -> crate::FinalizedProtectedWorkerV3HsacoIdentityV1 {
                self.identity
            }
            pub fn raw(&self) -> &InspectedProtectedWorkerV3HsacoV1 {
                &self.raw
            }
            pub fn finalized(&self) -> &$artifact {
                &self.finalized
            }
            pub fn output_identity(&self) -> ContentIdentityV1 {
                self.output
            }
            pub fn descriptor_identity(&self) -> ContentIdentityV1 {
                self.descriptor
            }
            pub const fn is_structural_only(&self) -> bool {
                true
            }
            pub const fn authenticates_compiler_origin(&self) -> bool {
                false
            }
            pub const fn grants_compiler_authority(&self) -> bool {
                false
            }
            pub const fn grants_proof_authority(&self) -> bool {
                false
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
            pub(crate) fn into_compact_replay_parts(
                self,
            ) -> crate::worker_v3_hsaco_finalization::OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1
            {
                crate::worker_v3_hsaco_finalization::OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1 {
                    identity: self.identity,
                    source: self.raw.into_source_evidence(),
                    finalized_bytes: self.finalized.into_bytes(),
                }
            }
        }

        #[derive(Debug)]
        pub enum $error<E> {
            Inspection(WorkerV3HsacoInspectionError),
            Finalization($artifact_error<E>),
            ExportManifestMismatch,
        }
        impl<E: fmt::Display> fmt::Display for $error<E> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Inspection(e) => write!(f, "conditional nominal worker inspection failed: {e}"),
                    Self::Finalization(e) => {
                        write!(f, "conditional nominal worker finalization failed: {e}")
                    }
                    Self::ExportManifestMismatch => {
                        f.write_str("nominal worker export manifest differs from retained receipt")
                    }
                }
            }
        }
        impl<E: Error + 'static> Error for $error<E> {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Inspection(e) => Some(e),
                    Self::Finalization(e) => Some(e),
                    _ => None,
                }
            }
        }
        impl<E> From<$artifact_error<E>> for $error<E> {
            fn from(e: $artifact_error<E>) -> Self {
                Self::Finalization(e)
            }
        }
        impl<E> From<WorkerV3HsacoInspectionError> for $error<E> {
            fn from(e: WorkerV3HsacoInspectionError) -> Self {
                Self::Inspection(e)
            }
        }

        /// Retain every mandatory contract and require the entire embedded zero-digest
        /// descriptor to equal the original strict ABI receipt. Shared strict lineage,
        /// symbol closure and export checks still run; there is no V1/V3 fallback.
        /// `prepaid_scratch` covers this family's descriptor scratch only. Retained
        /// evidence, ELF/AMDHSA allocations, strict inspection and identity serialization
        /// remain in their existing bounded domains outside the descriptor work callback.
        pub fn $finalize_worker<E>(
            source: InertProtectedFirstBuildWorkerV3EvidenceV1,
            prepaid_scratch: usize,
            charge: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> Result<$owner, $error<E>> {
            let launch = {
                let inspected =
                    $inspect_artifact(source.output_bytes(), prepaid_scratch, charge)?;
                let table = inspected.descriptor_table();
                common_launch(table.kernel_count(), |index| {
                    let kernel = table
                        .kernel(index, charge)
                        .map_err($artifact_error::Wire)?;
                    Ok::<_, $error<E>>(strict_kernel_launch_contract(
                        kernel.launch(),
                    )?)
                })?
            };
            let raw = inspect_protected_worker_with_launch(source, launch)?;
            let handoff = raw.outer_handoff();
            let receipts = handoff.capsule().receipts();
            if !export_manifest_matches(
                receipts.export_manifest().canonical_preimage(),
                handoff.module_handoff().symbol_manifest().canonical_bytes(),
                charge,
            )
            .map_err($artifact_error::Work)?
            {
                return Err($error::ExportManifestMismatch);
            }
            let finalized = $finalize_artifact(
                raw.source_evidence().output_bytes(),
                receipts.abi().canonical_preimage(),
                prepaid_scratch,
                charge,
            )?;
            charge(finalized.as_bytes().len() + finalized.descriptor_bytes().len())
                .map_err($artifact_error::Work)?;
            let output = ContentIdentityV1::calculate(finalized.as_bytes());
            let descriptor = ContentIdentityV1::calculate(finalized.descriptor_bytes());
            let identity =
                crate::worker_v3_hsaco_finalization::$identity(
                    &raw, &finalized, output, descriptor,
                );
            Ok($owner {
                raw,
                finalized,
                output,
                descriptor,
                identity,
            })
        }
    };
}

pub(crate) use mixed_worker_finalization_family;
