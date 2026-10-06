//! Copy inert completion inputs only from the original retired runtime owner.
use super::*;
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V3 as SUBJECT_SCRATCH,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V3 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV3 as Subject,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_COMPLETION_STORAGE_V1 as TERMINAL_SCRATCH,
    COMPILER_EXECUTION_ROOT_COMPLETION_WORK_V1 as TERMINAL_WORK,
    COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_NEW_WORK_V1 as COMPLETION_WORK,
    COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_STORAGE_V1 as COMPLETION_SCRATCH,
    CompilerExecutionRootCompletionRecordV1 as Terminal,
    CompilerExecutionRootIntakeRecordV4 as Intake,
    CompilerExecutionRootPublicationCompletionErrorV1 as CompletionError,
    CompilerExecutionRootPublicationCompletionV1 as Completion,
    CompilerExecutionRootTerminationV1 as Termination,
};

impl<T: Send + 'static> NativeAttempt<'_, T> {
    /// Inert transport record, not publication authority. The original account,
    /// locked backing, acknowledged root wait and complete trace retirement are
    /// checked here. No caller-supplied terminal status or publication ID enters
    /// this operation. Actual publication custody stays retained in this owner.
    /// Returns the FULL unreserved record charge, not a new execution account.
    pub(crate) fn publication_completion(
        &mut self,
        last: &Intake,
        b: &mut Budget<'_>,
    ) -> Result<(Completion, Storage)> {
        let floor = sum(&[self.retained, last.retained_storage()])?;
        b.with_prepaid_scope(floor, 0, 0, 0, |b| {
            self.account.with(self.retained, b, |b| {
                let publication = self
                    .publication
                    .as_ref()
                    .ok_or(Error::Invalid("completion lost original publication"))?;
                let issuer = self
                    .issuer
                    .as_ref()
                    .ok_or(Error::Invalid("completion lost original issuer"))?;
                self.trace.with_runtime_backing(b, |runtime, _, b| {
                    let wait = runtime
                        .root_completion()
                        .ok_or(Error::Invalid("completion lacks original root wait"))?;
                    require_successful_retirement(
                        runtime.is_trace_retired(),
                        wait.exit_code(),
                        wait.terminating_signal(),
                    )?;
                    issuer.child.with_resources(b, |payload, b| {
                        let (terminal, charge) = Terminal::new(last, Termination::Exited(0), b)
                            .map_err(completion_error)?;
                        b.reserve_storage(charge.additional_storage())?;
                        let (subject, charge) =
                            Subject::decode(publication.subject().canonical_bytes(), b)
                                .map_err(completion_error)?;
                        b.reserve_storage(charge.retained_storage())?;
                        let (manifest, charge) =
                            Manifest::decode(payload.manifest.manifest().canonical_bytes(), b)
                                .map_err(completion_error)?;
                        b.reserve_storage(charge.additional_storage())?;
                        let (ready, charge) = Ready::decode(issuer.ready.canonical_bytes(), b)
                            .map_err(completion_error)?;
                        b.reserve_storage(charge.additional_storage())?;
                        let (record, growth) =
                            Completion::new(terminal, subject, manifest, ready, b)
                                .map_err(completion_error)?;
                        b.reserve_storage(growth.additional_storage())?;
                        let charge = Storage(record.retained_storage());
                        Ok((record, charge))
                    })
                })
            })
        })
    }

    pub(crate) fn publication_completion_quota() -> Result<Quota> {
        let original = Self::runtime_backing_quota()?;
        Ok(Quota {
            work: sum(&[
                original.work(),
                Resources::<Payload<T>>::ACCESS_WORK,
                TERMINAL_WORK,
                SUBJECT_WORK,
                MANIFEST_WORK,
                READY_WORK,
                COMPLETION_WORK,
            ])?,
            scratch: sum(&[
                original.scratch(),
                Resources::<Payload<T>>::ACCESS_SCRATCH,
                TERMINAL_SCRATCH,
                SUBJECT_SCRATCH,
                MANIFEST_SCRATCH,
                READY_SCRATCH,
                // Includes all copied children while constructing the record.
                COMPLETION_SCRATCH,
            ])?,
        })
    }
}

fn require_successful_retirement(
    retired: bool,
    exit_code: Option<i32>,
    signal: Option<i32>,
) -> Result<()> {
    if !retired || exit_code != Some(0) || signal.is_some() {
        return Err(Error::Invalid(
            "completion requires original successful wait and whole trace retirement",
        ));
    }
    Ok(())
}

fn completion_error(error: impl Into<CompletionError>) -> Error {
    match error.into() {
        CompletionError::Resource(error) => error.into(),
        _ => Error::Invalid("original publication completion records differ"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_copy_requires_successful_wait_and_complete_tree_retirement() {
        assert!(require_successful_retirement(true, Some(0), None).is_ok());
        for retired in [false, true] {
            for pair in [
                (None, None),
                (Some(1), None),
                (Some(255), None),
                (None, Some(9)),
                (Some(0), Some(9)),
                (Some(-1), None),
            ] {
                assert!(require_successful_retirement(retired, pair.0, pair.1).is_err());
            }
        }
        assert!(require_successful_retirement(false, Some(0), None).is_err());
    }

    #[test]
    fn completion_quote_keeps_both_original_accesses_and_all_codec_work() {
        let access = NativeAttempt::<()>::runtime_backing_quota().unwrap();
        let completion = NativeAttempt::<()>::publication_completion_quota().unwrap();
        assert_eq!(
            completion.work(),
            access.work()
                + Resources::<Payload<()>>::ACCESS_WORK
                + TERMINAL_WORK
                + SUBJECT_WORK
                + MANIFEST_WORK
                + READY_WORK
                + COMPLETION_WORK
        );
        assert_eq!(
            completion.scratch(),
            access.scratch()
                + Resources::<Payload<()>>::ACCESS_SCRATCH
                + TERMINAL_SCRATCH
                + SUBJECT_SCRATCH
                + MANIFEST_SCRATCH
                + READY_SCRATCH
                + COMPLETION_SCRATCH
        );
    }
}
