//! Original-root completion custody on the unchanged parent resource account.
use super::*;
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V3 as SUBJECT_BYTES,
    InertCompilerExecutionSubjectStorageV3 as SubjectStorage,
    InertCompilerExecutionSubjectV3 as Subject,
};

impl<'b, 'work> RootCompleted<'b, 'work> {
    /// Consuming transfer into the existing native readiness owner. Neither the
    /// original account nor any borrowed input can be replaced by this operation.
    pub(crate) fn into_parts(self) -> (RootEvidence<'b>, &'b mut Budget<'work>) {
        (self.evidence, self.budget)
    }
}

impl<'b> RootEvidence<'b> {
    pub(crate) const REVALIDATION_WORK: usize = LOCAL_WORK
        + PARENT_MAX_STORAGE
        + 6 * 1024
        + Invocation::NATIVE_REVALIDATION_WORK
        + 8
        + 3 * 1024
        + Profile::IO_WORK
        + COMPLETION_MATCH_WORK;
    pub(crate) const REVALIDATION_SCRATCH: usize =
        FRAME + Invocation::NATIVE_OPERATION_SCRATCH + Profile::IO_STORAGE + COMPLETION_SCRATCH;
    pub(crate) const SUBJECT_MATCH_WORK: usize = 8 + 4 * SUBJECT_BYTES;

    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn profile(&self) -> &'b Profile {
        self.profile
    }
    pub(crate) const fn completion(&self) -> &Completion {
        &self.record
    }

    fn floor(&self) -> Result<usize> {
        self.retained
            .checked_add(self.parent.native_retained_storage()?)
            .and_then(|n| n.checked_add(self.profile.retained_storage()))
            .and_then(|n| n.checked_add(OUTPUT_OWNER_STORAGE))
            .and_then(|n| n.checked_add(size_of::<InvocationAuthority>()))
            .ok_or_else(|| Resource::Arithmetic.into())
    }

    pub(crate) fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(self.floor()?, 8, LOCAL_WORK, FRAME, |b| {
            require_account(&self.ledger, self.address, b)?;
            self.endpoint.revalidate()?;
            let peer = syscall(
                "recheck original root peer",
                net::sockopt::socket_peercred(&self.socket),
            )?;
            if root_sender(peer)? != self.sender {
                return Err(Error::Rejected("original root socket peer changed"));
            }
            self.profile.revalidate(b)?;
            self.parent.revalidate_native(b)?;
            validate_output(self.output)?;
            if !self.record.matches_intake(&self.last, b)? {
                return Err(Error::Rejected(
                    "original root completion transcript changed",
                ));
            }
            Ok(())
        })
    }

    /// The subject must come from the parent's actual retained V5 lease/token.
    /// Equality with this authenticated original-root observation is additional
    /// to the existing signed-carriage, invocation, and currentness checks.
    pub(crate) fn require_subject(&self, subject: &Subject, b: &mut Budget<'_>) -> Result<()> {
        let floor = self
            .floor()?
            .checked_add(size_of::<(Subject, SubjectStorage)>())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, Self::SUBJECT_MATCH_WORK, 0, |b| {
            require_account(&self.ledger, self.address, b)?;
            require_same_subject(self.record.subject(), subject)
        })
    }

    pub(crate) fn require_parent(&self, parent: &Parent, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(self.floor()?, 8, 8, 0, |b| {
            require_account(&self.ledger, self.address, b)?;
            if !std::ptr::eq(self.parent, parent) {
                return Err(Error::Rejected("completion lost original parent/account"));
            }
            Ok(())
        })
    }

    pub(crate) fn require_output_path(
        &self,
        path: &std::path::Path,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        b.with_prepaid_scope(self.floor()?, 8, LOCAL_WORK, FRAME, |b| {
            require_account(&self.ledger, self.address, b)?;
            validate_output(self.output)?;
            // This is the path derived from the still-owned descriptor, not an
            // accepted caller pathname or a reopened display-path identity.
            if path != self.output.child_path() {
                return Err(Error::Rejected(
                    "publication does not use original output descriptor",
                ));
            }
            Ok(())
        })
    }
}

fn require_account(ledger: &Ledger, address: usize, b: &Budget<'_>) -> Result<()> {
    if ledger != &b.work_ledger_identity_v1() || address != b as *const Budget<'_> as usize {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

fn require_same_subject(observed: &Subject, current: &Subject) -> Result<()> {
    if observed.canonical_bytes() != current.canonical_bytes() {
        return Err(Error::Rejected(
            "locked publication differs from original root subject",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "protected_compiler_root_completion_tests.rs"]
mod tests;
