//! One fresh source callback; no file output, owner export or process spawning.
use super::{
    Callbacks, Compilation, Compiler, TyCtxt, require_canonical_overflow_checks_v1,
    transaction_in_active_session_v1,
};
use crate::collector::source_census_v1::bitselect_feasibility::retained::RetainedInput;
use crate::source_local_order_recipe_api_v1::{
    SourceLocalOrderRecipeAttemptV1 as Attempt, SourceLocalOrderRecipeFailurePhaseV1 as Phase,
    SourceLocalOrderRecipeFailureV1 as Failure, SourceLocalOrderRecipeOutputV1 as Output,
    SourceLocalOrderRecipeRequestV1 as Request, finish_callback, validate_arguments,
};

use std::time::{Duration, Instant};

struct RecipeCallbacks {
    request: Request,
    input: Option<RetainedInput>,
    calls: usize,
    compiler_entries: usize,
    measure_callback_stage: bool,
    callback_stage_elapsed: Option<Duration>,
    #[cfg(test)]
    injecting_reentry: bool,
    result: Option<Result<Output, Failure>>,
    #[cfg(test)]
    observer: Option<crate::source_local_order_recipe_api_v1::test_support::Observer>,
    #[cfg(test)]
    repeat_after_first: bool,
    #[cfg(test)]
    fatal_after_first: bool,
}
impl RecipeCallbacks {
    fn new(request: Request) -> Self {
        Self {
            request,
            input: None,
            calls: 0,
            compiler_entries: 0,
            measure_callback_stage: false,
            callback_stage_elapsed: None,
            result: None,
            #[cfg(test)]
            injecting_reentry: false,
            #[cfg(test)]
            observer: None,
            #[cfg(test)]
            repeat_after_first: false,
            #[cfg(test)]
            fatal_after_first: false,
        }
    }
}
impl Callbacks for RecipeCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        #[cfg(test)]
        if !self.injecting_reentry {
            self.compiler_entries = self.compiler_entries.saturating_add(1);
        }
        #[cfg(not(test))]
        {
            self.compiler_entries = self.compiler_entries.saturating_add(1);
        }
        if self.calls != 0 {
            self.calls = 2;
            if !matches!(self.result.as_ref(), Some(Err(_))) {
                self.result = Some(Err(Failure::new(
                    Phase::Frontend,
                    "local-order recipe received a repeated live callback".into(),
                )));
            }
            return Compilation::Stop;
        }
        self.calls = 1;
        let Some(input) = self.input.take() else {
            self.result = Some(Err(Failure::new(
                Phase::Frontend,
                "local-order recipe retained source unavailable".into(),
            )));
            return Compilation::Stop;
        };
        let measure_callback_stage = self.measure_callback_stage;
        let mut callback_stage_elapsed = None;
        let operation = || {
            let started = measure_callback_stage.then(Instant::now);
            let result = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|message| Failure::new(Phase::Frontend, message))
            .and_then(|transaction| {
                transaction.compile_source_local_order_recipe_v1(input, &self.request)
            });
            callback_stage_elapsed = started.map(|started| started.elapsed());
            result
        };
        #[cfg(test)]
        let result = crate::source_local_order_recipe_api_v1::test_support::with_observer(
            self.observer.take(),
            operation,
        );
        #[cfg(not(test))]
        let result = operation();
        self.callback_stage_elapsed = callback_stage_elapsed;
        self.result = Some(result);
        #[cfg(test)]
        {
            if self.fatal_after_first {
                tcx.dcx()
                    .fatal("controlled rustc fatal after actual local-order recipe callback");
            }
            if self.repeat_after_first {
                self.injecting_reentry = true;
                let result = self.after_analysis(_compiler, tcx);
                self.injecting_reentry = false;
                return result;
            }
        }
        Compilation::Stop
    }
}

/// Caller owns process isolation, cwd, environment, deadline and complete
/// targeted rustc arguments. This function does not itself publish recipe/LLVM
/// files, spawn processes, change environment, compile a second variant or grant
/// protected/native authority. Rustc arguments retain ordinary compiler effects.
/// The immutable recipe-byte snapshot has no recipe-file-currentness guarantee;
/// the caller owns retaining/rechecking any file from which those bytes came.
pub fn run_source_local_order_recipe_driver_v1(args: &[String], request: Request) -> Attempt {
    run(args, RecipeCallbacks::new(request))
}

/// Opt-in timing of the original callback transaction creation through recipe
/// return. This still uses one fresh frontend and consumes the genuine current
/// transaction/source owner once. It is NOT a warmed-session replay benchmark.
/// Frontend analysis, retained-source opening before rustc, output publication,
/// storage accounting and caller work are excluded. API-internal checks remain
/// included. Refused stages may carry elapsed time; absence is never zero.
/// Ordinary driver calls above do not read the clock. No owners are cloned,
/// reconstructed or exported and no external callback/recipe schema is added.
pub fn run_source_local_order_recipe_driver_measured_v1(
    args: &[String],
    request: Request,
) -> Attempt {
    let mut callbacks = RecipeCallbacks::new(request);
    callbacks.measure_callback_stage = true;
    run(args, callbacks)
}

fn run(args: &[String], mut callbacks: RecipeCallbacks) -> Attempt {
    if let Err(error) = validate_arguments(args) {
        return Attempt::new(callbacks.request, Err(error), 0, 0);
    }
    if let Err(message) = require_canonical_overflow_checks_v1(args) {
        return Attempt::new(
            callbacks.request,
            Err(Failure::new(Phase::Request, message)),
            0,
            0,
        );
    }
    let input = match RetainedInput::open(callbacks.request.source_path(), false) {
        Ok(input) => input,
        Err(message) => {
            return Attempt::new(
                callbacks.request,
                Err(Failure::new(Phase::Request, message)),
                0,
                0,
            );
        }
    };
    callbacks.input = Some(input);
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks))
            .is_err();
    let result = finish_callback(callbacks.result, callbacks.calls, fatal);
    Attempt::new(
        callbacks.request,
        result,
        callbacks.calls,
        callbacks.compiler_entries,
    )
    .with_callback_stage_elapsed_v1(callbacks.callback_stage_elapsed, fatal)
}

/// Compiler-test-only probe. The normal public driver has no callback parameter.
#[cfg(test)]
pub(crate) fn run_source_local_order_recipe_probe_v1(
    args: &[String],
    request: Request,
    observer: Option<crate::source_local_order_recipe_api_v1::test_support::Observer>,
    repeat_after_first: bool,
    fatal_after_first: bool,
) -> Attempt {
    if repeat_after_first && fatal_after_first {
        return Attempt::new(
            request,
            Err(Failure::new(
                Phase::Request,
                "recipe reentry and fatal probes must be separate".into(),
            )),
            0,
            0,
        );
    }
    let mut callbacks = RecipeCallbacks::new(request);
    callbacks.observer = observer;
    callbacks.repeat_after_first = repeat_after_first;
    callbacks.fatal_after_first = fatal_after_first;
    run(args, callbacks)
}

#[cfg(test)]
#[path = "source_local_order_recipe_warm_series_v1_tests.rs"]
mod warm_series;
