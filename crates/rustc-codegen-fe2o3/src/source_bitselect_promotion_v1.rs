//! Experimental bounded Linux in-process source-candidate request/result API.
//! No serialized wire, compiler owner, resume token, or build/launch authority.

use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;
use fe2o3_source_isa_observation::source_edit_v1::validate_source_edit_path_v1;

pub(crate) const ARGUMENT_COUNT_CAP: usize = 4096;
pub(crate) const ARGUMENT_BYTES_CAP: usize = 1024 * 1024;
const ERROR_BYTES_CAP: usize = 4096;

/// An inert request, never a source/SSA binding, capability or resume token.
/// Revision is an exact file-byte conflict precondition, not source authority.
/// Selection is the existing sole eligible initializer in one sealed root.
pub struct BitselectPromotionRequestV1 {
    original: String,
    candidate: String,
    expected_original_sha256: [u8; 32],
    registers: Gfx942OrderedProgramRegistersV1,
}

impl BitselectPromotionRequestV1 {
    pub fn new(
        original: &str,
        candidate: &str,
        expected_original_sha256: [u8; 32],
        registers: Gfx942OrderedProgramRegistersV1,
    ) -> Result<Self, BitselectPromotionFailureV1> {
        for path in [original, candidate] {
            validate_source_edit_path_v1(path).map_err(|e| {
                BitselectPromotionFailureV1::before(FailurePhaseV1::Request, e.to_string())
            })?;
        }
        if original == candidate {
            return Err(BitselectPromotionFailureV1::before(
                FailurePhaseV1::Request,
                "source promotion requires a distinct candidate path".into(),
            ));
        }
        // Path validation bounds each allocation to the existing 1024-byte cap.
        Ok(Self {
            original: original.to_owned(),
            candidate: candidate.to_owned(),
            expected_original_sha256,
            registers,
        })
    }

    pub fn original_path(&self) -> &str {
        &self.original
    }
    pub fn candidate_path(&self) -> &str {
        &self.candidate
    }
    pub fn expected_original_sha256(&self) -> &[u8; 32] {
        &self.expected_original_sha256
    }
    pub fn registers(&self) -> Gfx942OrderedProgramRegistersV1 {
        self.registers
    }
}

/// Describes only this attempt's writes. It never asserts filesystem rollback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidatePublicationStateV1 {
    NotAttempted,
    /// Publication was called, or an entered callback did not finish and its
    /// publication state is unknown. The publisher's String error does not
    /// expose whether linkat succeeded; a post-publication recheck can also fail.
    MayHaveCreatedCandidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePhaseV1 {
    Request,
    Frontend,
    Eligibility,
    Publication,
    PostPublication,
}

#[derive(Debug)]
pub struct BitselectPromotionFailureV1 {
    phase: FailurePhaseV1,
    publication: CandidatePublicationStateV1,
    diagnostic: String,
    compiler_fatal: bool,
}

impl std::fmt::Display for BitselectPromotionFailureV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for BitselectPromotionFailureV1 {}

impl BitselectPromotionFailureV1 {
    pub fn phase(&self) -> FailurePhaseV1 {
        self.phase
    }
    pub fn publication(&self) -> CandidatePublicationStateV1 {
        self.publication
    }
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
    pub fn compiler_fatal(&self) -> bool {
        self.compiler_fatal
    }

    pub(crate) fn before(phase: FailurePhaseV1, diagnostic: String) -> Self {
        Self::new(phase, CandidatePublicationStateV1::NotAttempted, diagnostic)
    }

    pub(crate) fn after_attempt(phase: FailurePhaseV1, diagnostic: String) -> Self {
        Self::new(
            phase,
            CandidatePublicationStateV1::MayHaveCreatedCandidate,
            diagnostic,
        )
    }

    fn new(
        phase: FailurePhaseV1,
        publication: CandidatePublicationStateV1,
        mut diagnostic: String,
    ) -> Self {
        if diagnostic.len() > ERROR_BYTES_CAP {
            let mut end = ERROR_BYTES_CAP;
            while !diagnostic.is_char_boundary(end) {
                end -= 1;
            }
            diagnostic.truncate(end);
        }
        Self {
            phase,
            publication,
            diagnostic,
            compiler_fatal: false,
        }
    }
}

/// Historical byte/publication observations, not fresh-source admission.
/// Other writers can alter or replace the candidate after publication.
pub struct PublishedBitselectCandidateV1 {
    original_sha256: [u8; 32],
    candidate_sha256: [u8; 32],
    candidate_bytes: usize,
    registers: Gfx942OrderedProgramRegistersV1,
}

impl PublishedBitselectCandidateV1 {
    pub fn original_sha256(&self) -> &[u8; 32] {
        &self.original_sha256
    }
    pub fn candidate_sha256(&self) -> &[u8; 32] {
        &self.candidate_sha256
    }
    pub fn candidate_bytes(&self) -> usize {
        self.candidate_bytes
    }
    pub fn registers(&self) -> Gfx942OrderedProgramRegistersV1 {
        self.registers
    }

    pub(crate) fn observed(
        original_sha256: [u8; 32],
        candidate_sha256: [u8; 32],
        candidate_bytes: usize,
        registers: Gfx942OrderedProgramRegistersV1,
    ) -> Self {
        Self {
            original_sha256,
            candidate_sha256,
            candidate_bytes,
            registers,
        }
    }
}

/// Returns the original inert request on every normal Result path. No live
/// compiler session, retained source descriptor or old compilation is returned.
pub struct BitselectPromotionAttemptV1 {
    request: BitselectPromotionRequestV1,
    result: Result<PublishedBitselectCandidateV1, BitselectPromotionFailureV1>,
}

impl BitselectPromotionAttemptV1 {
    pub fn request(&self) -> &BitselectPromotionRequestV1 {
        &self.request
    }
    pub fn result(&self) -> Result<&PublishedBitselectCandidateV1, &BitselectPromotionFailureV1> {
        self.result.as_ref()
    }
    pub fn into_parts(
        self,
    ) -> (
        BitselectPromotionRequestV1,
        Result<PublishedBitselectCandidateV1, BitselectPromotionFailureV1>,
    ) {
        (self.request, self.result)
    }
    pub(crate) fn new(
        request: BitselectPromotionRequestV1,
        result: Result<PublishedBitselectCandidateV1, BitselectPromotionFailureV1>,
    ) -> Self {
        Self { request, result }
    }
}

pub(crate) fn validate_arguments(args: &[String]) -> Result<(), BitselectPromotionFailureV1> {
    let fail = || {
        BitselectPromotionFailureV1::before(
            FailurePhaseV1::Request,
            "source promotion requires bounded complete rustc arguments".into(),
        )
    };
    if args.is_empty() || args.len() > ARGUMENT_COUNT_CAP {
        return Err(fail());
    }
    let mut bytes = 0usize;
    for arg in args {
        bytes = bytes.checked_add(arg.len()).ok_or_else(fail)?;
        if bytes > ARGUMENT_BYTES_CAP {
            return Err(fail());
        }
    }
    Ok(())
}

pub(crate) fn finish_callback(
    result: Option<Result<PublishedBitselectCandidateV1, BitselectPromotionFailureV1>>,
    calls: usize,
    fatal: bool,
) -> Result<PublishedBitselectCandidateV1, BitselectPromotionFailureV1> {
    // Preserve an existing exact refusal, including its write classification.
    let result = match result {
        Some(Err(mut error)) => {
            error.compiler_fatal |= fatal;
            return Err(error);
        }
        other => other,
    };
    let mut failure = if calls != 1 || result.is_none() {
        let message = "source promotion did not finish exactly one live callback".into();
        if calls == 0 {
            BitselectPromotionFailureV1::before(FailurePhaseV1::Frontend, message)
        } else {
            BitselectPromotionFailureV1::after_attempt(FailurePhaseV1::Frontend, message)
        }
    } else if fatal {
        BitselectPromotionFailureV1::after_attempt(
            FailurePhaseV1::Frontend,
            "rustc reported a fatal error after the promotion callback".into(),
        )
    } else {
        return result.expect("checked one callback result");
    };
    failure.compiler_fatal = fatal;
    Err(failure)
}

#[cfg(test)]
#[path = "source_bitselect_promotion_v1_tests.rs"]
mod tests;

// Compiler-test-only fault seam. This entire module is absent in a normal build.
#[cfg(test)]
pub(crate) mod live_test_support {
    use super::BitselectPromotionRequestV1 as Request;
    use std::cell::RefCell;

    pub(crate) type AfterPublication = Box<dyn FnOnce(&Request) + Send>;

    thread_local! {
        static AFTER_PUBLICATION: RefCell<Option<AfterPublication>> = const { RefCell::new(None) };
    }

    struct ClearHook;
    impl Drop for ClearHook {
        fn drop(&mut self) {
            AFTER_PUBLICATION.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }

    // Install on the live compiler callback thread, not the harness thread.
    // A nested installation refuses; unwinding always removes the pending hook.
    pub(crate) fn with_after_publication<T>(
        hook: AfterPublication,
        action: impl FnOnce() -> T,
    ) -> T {
        AFTER_PUBLICATION.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(slot.is_none(), "one scoped post-publication test hook");
            *slot = Some(hook);
        });
        let _clear = ClearHook;
        let result = action();
        assert!(
            AFTER_PUBLICATION.with(|slot| slot.borrow().is_none()),
            "the selected actual publication must consume its test hook"
        );
        result
    }

    pub(crate) fn after_publication(request: &Request) {
        let hook = AFTER_PUBLICATION.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook(request);
        }
    }

    fn request() -> Request {
        Request::new(
            "original.rs",
            "candidate.rs",
            [0; 32],
            fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn source_candidate_postpublication_hook_clears_on_panic() {
        for invoke_hook in [false, true] {
            let failed = std::panic::catch_unwind(|| {
                with_after_publication(Box::new(|_| panic!("controlled hook panic")), || {
                    if invoke_hook {
                        after_publication(&request());
                    }
                    panic!("controlled action panic");
                });
            });
            assert!(failed.is_err());
            assert!(AFTER_PUBLICATION.with(|slot| slot.borrow().is_none()));
            with_after_publication(Box::new(|_| {}), || after_publication(&request()));
        }
    }

    #[test]
    fn source_candidate_postpublication_hook_is_thread_local_and_one_shot() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        with_after_publication(
            Box::new(move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
            }),
            || {
                std::thread::spawn(|| after_publication(&request()))
                    .join()
                    .unwrap();
                assert_eq!(calls.load(Ordering::SeqCst), 0);
                after_publication(&request());
                after_publication(&request());
            },
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(AFTER_PUBLICATION.with(|slot| slot.borrow().is_none()));
    }
}
