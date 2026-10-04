//! Real observer guards through the production record/commit and guard-finishing functions.
//! The caller uses test keys and a same-UID helper, not production issuer admission.
use super::*;
use fe2o3_artifact_transaction::{RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1};
use std::fs::File;
use std::os::unix::fs::PermissionsExt;

pub(crate) fn exercise(
    observer: &crate::ProtectedCompilerExecutionObserverV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    key: &SigningKey,
    fail_commit: bool,
) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let root = || File::open(directory.path()).unwrap().into();
    let mut ledger = IssuerLedgerV2::recover(root(), policy, key).unwrap();
    let guard =
        ProtectedCompilerExecutionOccurrenceGuardV1::Remote(Box::new(observer.begin().unwrap()));
    let result = with_occurrence_guard(guard, |guard| {
        guard.revalidate_immediately_before_signing()?;
        let next = ledger.record.prepare(guard, [7; 32], policy, key)?;
        observer
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::from)?;
        if fail_commit {
            let mut failure = FailCommit;
            ledger.commit_with_hooks(next, &mut failure)?;
            panic!("commit hook did not fail");
        }
        ledger.commit(next)?;
        assert_eq!(ledger.record.stage, IssuerStageV2::Prepared);
        observer
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::from)?;
        Ok(())
    });
    if fail_commit {
        assert!(result.is_err());
        assert!(ledger.poisoned);
        assert!(matches!(
            observer.validate_continuity(),
            Err(crate::CompilerExecutionObserverErrorV1::Poisoned)
        ));
        assert!(matches!(
            observer.begin(),
            Err(crate::CompilerExecutionObserverErrorV1::Poisoned)
        ));
        return;
    }
    result.unwrap();
    let request = CompilerExecutionAttestationRequestV1::new(
        ledger.record.challenge.clone().unwrap(),
        ledger.record.subject.clone().unwrap(),
    )
    .unwrap();
    let guard =
        ProtectedCompilerExecutionOccurrenceGuardV1::Remote(Box::new(observer.begin().unwrap()));
    let publication = with_occurrence_guard(guard, |guard| {
        let next = ledger
            .record
            .issue(guard, request.canonical_bytes(), policy, key)?;
        let publication = next.receipt_publication()?;
        observer
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::from)?;
        ledger.commit(next)?;
        observer
            .validate_continuity()
            .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::from)?;
        Ok(publication)
    })
    .unwrap();
    assert_eq!(ledger.record.stage, IssuerStageV2::Issued);
    let committed = ledger.record.canonical;
    drop(ledger);
    let recovered = IssuerLedgerV2::recover(root(), policy, key).unwrap();
    assert_eq!(recovered.record.canonical, committed);
    assert_eq!(recovered.record.receipt_publication().unwrap(), publication);
    observer.validate_continuity().unwrap();
}

struct FailCommit;
impl RetainedDurableDirectoryHooksV1 for FailCommit {
    fn record(
        &mut self,
        _: RetainedDurableRecordBoundaryV1,
        _: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        Err(io::Error::other(
            "injected observer-guarded journal commit failure",
        ))
    }
}
