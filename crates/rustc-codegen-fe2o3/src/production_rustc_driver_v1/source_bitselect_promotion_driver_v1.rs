//! Experimental bounded headless in-process entry, not a new command or wire protocol.
use super::{
    Callbacks, Compilation, Compiler, TyCtxt, require_canonical_overflow_checks_v1,
    transaction_in_active_session_v1,
};
use crate::collector::source_census_v1::bitselect_feasibility::retained::RetainedInput;
use crate::source_bitselect_promotion_v1::{
    BitselectPromotionAttemptV1 as Attempt, BitselectPromotionFailureV1 as Failure,
    BitselectPromotionRequestV1 as Request, FailurePhaseV1 as Phase,
    PublishedBitselectCandidateV1 as Published, finish_callback, validate_arguments,
};

struct PromotionCallbacks {
    request: Request,
    input: Option<RetainedInput>,
    calls: usize,
    result: Option<Result<Published, Failure>>,
}

impl Callbacks for PromotionCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.calls != 0 {
            self.calls = 2;
            // Preserve an earlier exact refusal and never attempt another write.
            if !matches!(self.result.as_ref(), Some(Err(_))) {
                self.result = Some(Err(Failure::after_attempt(
                    Phase::Frontend,
                    "source promotion received a repeated live callback".into(),
                )));
            }
            return Compilation::Stop;
        }
        self.calls = 1;
        let Some(input) = self.input.take() else {
            self.result = Some(Err(Failure::before(
                Phase::Frontend,
                "source promotion retained input unavailable".into(),
            )));
            return Compilation::Stop;
        };
        self.result = Some(
            transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| Failure::before(Phase::Frontend, e))
            .and_then(|transaction| {
                transaction.materialize_bitselect_source_v1(input, &self.request)
            }),
        );
        Compilation::Stop
    }
}

/// Run one already-targeted rustc invocation in an otherwise stable process.
/// Caller owns process isolation, fixed working directory/environment, arguments
/// and deadline; this function neither changes cwd/environment nor spawns work.
/// It returns only an inert candidate observation. Explicit fresh compilation
/// is a separate ordinary action; this never auto-builds, launches or resumes.
pub fn run_bitselect_source_promotion_driver_v1(args: &[String], request: Request) -> Attempt {
    if let Err(error) = validate_arguments(args) {
        return Attempt::new(request, Err(error));
    }
    if let Err(error) = require_canonical_overflow_checks_v1(args) {
        return Attempt::new(request, Err(Failure::before(Phase::Request, error)));
    }
    let input = match RetainedInput::open(request.original_path(), false) {
        Ok(input) => input,
        Err(error) => return Attempt::new(request, Err(Failure::before(Phase::Request, error))),
    };
    let mut callbacks = PromotionCallbacks {
        request,
        input: Some(input),
        calls: 0,
        result: None,
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks))
            .is_err();
    let result = finish_callback(callbacks.result, callbacks.calls, fatal);
    Attempt::new(callbacks.request, result)
}

// Deliberate callback-method reentry/fatal injection only in the compiler's test build.
// This does not change the normal public entry or ask rustc to issue a repeat.
#[cfg(test)]
pub(super) const LIVE_FATAL_DIAGNOSTIC: &str =
    "controlled rustc fatal after actual source promotion callback";
#[cfg(test)]
pub(super) fn run_live_callback_probe_v1(
    args: &[String],
    request: Request,
    repeat: bool,
    fatal_after_first: bool,
    hook: Option<crate::source_bitselect_promotion_v1::live_test_support::AfterPublication>,
    observe_first: impl FnMut(&Request, &Result<Published, Failure>) + Send,
) -> (Attempt, usize, usize) {
    use crate::source_bitselect_promotion_v1::live_test_support;

    struct Probe<F> {
        inner: PromotionCallbacks,
        repeat: bool,
        fatal_after_first: bool,
        hook: Option<live_test_support::AfterPublication>,
        observe_first: F,
        compiler_entries: usize,
    }

    impl<F: FnMut(&Request, &Result<Published, Failure>) + Send> Callbacks for Probe<F> {
        fn after_analysis<'tcx>(&mut self, compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            self.compiler_entries += 1;
            assert_eq!(self.compiler_entries, 1, "one genuine rustc callback entry");
            let first = if let Some(hook) = self.hook.take() {
                live_test_support::with_after_publication(hook, || {
                    self.inner.after_analysis(compiler, tcx)
                })
            } else {
                self.inner.after_analysis(compiler, tcx)
            };
            assert!(matches!(first, Compilation::Stop));
            assert_eq!(self.inner.calls, 1);
            assert!(
                self.inner.input.is_none(),
                "retained input was consumed once"
            );
            (self.observe_first)(
                &self.inner.request,
                self.inner
                    .result
                    .as_ref()
                    .expect("actual first callback result"),
            );
            if self.fatal_after_first {
                // The inner production callback already returned and its actual
                // result/file observations were retained. Emit a genuine rustc
                // fatal diagnostic: do not synthesize the finish_callback flag
                // or use an ordinary panic as a substitute for FatalErrorMarker.
                tcx.dcx().fatal(LIVE_FATAL_DIAGNOSTIC);
            }
            if self.repeat {
                let second = self.inner.after_analysis(compiler, tcx);
                assert!(matches!(second, Compilation::Stop));
                assert_eq!(self.inner.calls, 2);
                assert!(self.inner.input.is_none());
            }
            Compilation::Stop
        }
    }

    assert!(
        !(repeat && fatal_after_first),
        "independently qualified repeat and post-callback fatal modes"
    );
    // The same actual argument and retained-input preconditions as the normal
    // entry are required. A setup error is a test failure, not a designated refusal.
    validate_arguments(args).expect("bounded complete actual invocation");
    require_canonical_overflow_checks_v1(args).expect("canonical overflow option");
    let input = RetainedInput::open(request.original_path(), false)
        .expect("actual retained source before the compiler session");
    let mut callbacks = Probe {
        inner: PromotionCallbacks {
            request,
            input: Some(input),
            calls: 0,
            result: None,
        },
        repeat,
        fatal_after_first,
        hook,
        observe_first,
        compiler_entries: 0,
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks))
            .is_err();
    assert_eq!(
        fatal, fatal_after_first,
        "fatal status comes from the actual rustc catch boundary"
    );
    assert_eq!(callbacks.compiler_entries, 1);
    let entries = callbacks.compiler_entries;
    let method_calls = callbacks.inner.calls;
    let result = finish_callback(callbacks.inner.result, method_calls, fatal);
    (
        Attempt::new(callbacks.inner.request, result),
        entries,
        method_calls,
    )
}
