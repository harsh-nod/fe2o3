//! Prepared V5 graph/custody -> V3 issuer subject -> exact receipt transport.
//! Called under the native client's uninterrupted original-account borrow.
use super::{Budget, Error, Prepared, ProductionCompilerCustody, R, Resource};
use fe2o3_artifact_transaction::{
    CompilerExecutionReceiptTransportReceiptV3 as TransportReceipt,
    CompilerModuleHandoffReceiptV5 as Receipt, InertCompilerExecutionSubjectV3 as Subject,
    publish_compiler_execution_receipt_transport_v3 as publish_transport,
    publish_compiler_module_handoff_v5 as publish_handoff,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV3 as Carriage;
use std::mem::size_of;

pub(in crate::production_pipeline) struct Published {
    prepared: Prepared,
    receipt: Receipt,
}

impl Prepared {
    fn require_account(&self, b: &Budget<'_>) -> R<()> {
        if b.storage() < self.retained_floor
            || b.storage_limit() > fe2o3_compiler_lineage::MAX_NATIVE_CONDITIONAL_STORAGE_V1
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    fn publication_custody(
        &self,
        b: &mut Budget<'_>,
    ) -> R<fe2o3_artifact_transaction::BuildAttempt> {
        self.require_account(b)?;
        match &self
            .prefix
            .preparation
            .bindings
            .transaction
            .compiler_custody
        {
            ProductionCompilerCustody::ProtectedV3 {
                invocation,
                attempt,
            } => {
                // Image streams are metered; complete live argv/environment and
                // capability capture still require the native migration boundary.
                invocation
                    .revalidate_for_publication_with_image_budget(b)
                    .map_err(Error::LiveInvocation)?;
                // Both immutable descriptors passed the canonical size bound;
                // this pays byte comparisons and their bounded field headers.
                b.charge_work(4 * fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3)?;
                if self.handoff.capsule().invocation() != invocation.descriptor() {
                    return Err(Error::Mismatch("native publication invocation changed"));
                }
                Ok(*attempt)
            }
            ProductionCompilerCustody::ExtractionOnly => Err(Error::Mismatch(
                "native publication requires original protected compiler custody",
            )),
        }
    }

    /// Retains the whole preparation, including both original proof accounts and
    /// source/target owners. The publication receipt grants no execution authority.
    pub(in crate::production_pipeline) fn publish(
        self,
        b: &mut Budget<'_>,
    ) -> R<(Subject, Published)> {
        let attempt = self.publication_custody(b)?;
        b.reserve_storage(size_of::<Published>() - size_of::<Prepared>())?;
        let transaction = &self.prefix.preparation.bindings.transaction;
        let receipt = publish_handoff(
            &transaction.output_dir,
            &transaction.producer,
            attempt,
            &self.handoff,
            b,
        )
        .map_err(Error::Publication)?;
        let (subject, charge) = Subject::from_publication(receipt, &self.handoff, b)
            .map_err(Error::SubjectPublication)?;
        b.reserve_storage(charge.retained_storage())?;
        Ok((
            subject,
            Published {
                prepared: self,
                receipt,
            },
        ))
    }
}

impl Published {
    /// Runs only after native receipt verification and the client's peer closure.
    /// Reconstructs the exact published subject; never trusts a caller-supplied
    /// subject identity or substitutes a different prepared graph.
    pub(in crate::production_pipeline) fn finish(
        self,
        carriage: Carriage,
        b: &mut Budget<'_>,
    ) -> R<Subject> {
        self.prepared.require_account(b)?;
        let (subject, charge) = Subject::from_publication(self.receipt, &self.prepared.handoff, b)
            .map_err(Error::SubjectPublication)?;
        b.reserve_storage(charge.retained_storage())?;
        require_exact_subject(carriage.request().subject(), &subject, b)?;
        self.prepared.publication_custody(b)?;
        let transaction = &self.prepared.prefix.preparation.bindings.transaction;
        // The returned receipt is fixed metadata, not a retained wire owner.
        // Prepay it and its comparison before the external publication effect.
        b.reserve_storage(size_of::<TransportReceipt>())?;
        b.charge_work(32 + size_of::<usize>())?;
        let receipt = publish_transport(
            &transaction.output_dir,
            &transaction.producer,
            &subject,
            carriage.canonical_bytes(),
            b,
        )
        .map_err(Error::ReceiptTransport)?;
        if receipt.subject() != subject.identity()
            || receipt.length() != carriage.canonical_bytes().len()
        {
            return Err(Error::Mismatch(
                "native receipt transport changed subject or length",
            ));
        }
        b.release_storage(size_of::<TransportReceipt>())?;
        Ok(subject)
    }
}

fn require_exact_subject(actual: &Subject, expected: &Subject, b: &mut Budget<'_>) -> R<()> {
    b.charge_work(fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V3)?;
    if actual.canonical_bytes() != expected.canonical_bytes() {
        return Err(Error::Mismatch(
            "native receipt differs from published V5 subject",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_pipeline_conditional_native_publication_v3_tests.rs"]
mod tests;
