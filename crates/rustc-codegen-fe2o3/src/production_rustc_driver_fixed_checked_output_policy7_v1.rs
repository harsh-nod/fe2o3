//! Literal fixed Policy7 extraction of actual J, without protected authority.
use super::*;
use crate::collector::source_census_v1::ExtractionMode as CensusMode;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const DIAGNOSTIC_POLICY: u16 = 7;

struct FixedCheckedOutputCallbacksPolicy7V1<'a> {
    output: &'a Path,
    result: Option<Result<(), String>>,
    census: Option<SourceCensusRecorder>,
    #[cfg(test)]
    observer: Option<fixed_census_invocation_observer_v1_tests::Observer>,
}

impl Callbacks for FixedCheckedOutputCallbacksPolicy7V1<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        #[cfg(test)]
        let result =
            fixed_census_invocation_observer_v1_tests::in_callback(self.observer.take(), || {
                extract_in_active_session(tcx, self.output, self.census.as_ref())
            });
        #[cfg(not(test))]
        let result = extract_in_active_session(tcx, self.output, self.census.as_ref());
        self.result = Some(result);
        Compilation::Stop
    }
}

fn extract_in_active_session(
    tcx: TyCtxt<'_>,
    output: &Path,
    census: Option<&SourceCensusRecorder>,
) -> Result<(), String> {
    let ranked = transaction_with_census_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        census,
    )?
    .verify_general_kernel_checks()
    .map_err(|error| error.to_string())?;
    let mut work = Work::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
            .map_err(|_| "fixed checked-output work limit is not representable".to_owned())?,
    );
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    ranked
        .with_fixed_checked_output_policy7_extraction_v1(
            &mut budget,
            |observed, handoff, descriptor| {
                let bytes = handoff.module_bytes();
                publish_new_extraction_bytes_v1(
                    output,
                    bytes,
                    dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES,
                    "fixed checked-output LLVM extraction",
                )?;
                eprintln!(
                    "fe2o3 fixed checked-output extraction: original N {}, distinct E {:?}, historical I {}, actual J {}, fixed policy {}, target {:?}, {} ordered kernel(s), {} LLVM byte(s), retained logical floor {}; extraction-only observation, proof/compiler/artifact/load/launch authority false",
                    lower_hex_v1(&observed.original),
                    observed.erased.as_ref().map(|digest| lower_hex_v1(digest)),
                    lower_hex_v1(&observed.input),
                    lower_hex_v1(&observed.output),
                    observed.policy,
                    handoff.target(),
                    descriptor.table().kernels().len(),
                    bytes.len(),
                    observed.retained_floor,
                );
                Ok(())
            },
        )
        .map_err(|error| error.to_string())?
}

/// Extracts actual final-J LLVM through the compiled Policy7 schedule.
/// Authenticated source custody selects Direct or UnitLocal without a selector.
/// This is neither default compilation nor protected compiler publication.
pub fn run_production_fixed_checked_output_policy7_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    let census = match SourceCensusRecorder::from_environment(
        args,
        &[output],
        CensusMode::FixedCheckedOutput {
            policy: DIAGNOSTIC_POLICY,
        },
    ) {
        Ok(recorder) => recorder,
        Err(error) => {
            eprintln!("fe2o3 diagnostic source census unavailable: {error}");
            None
        }
    };
    let mut callbacks = FixedCheckedOutputCallbacksPolicy7V1 {
        output,
        result: None,
        census,
        #[cfg(test)]
        observer: fixed_census_invocation_observer_v1_tests::take_for_invocation(),
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    let result = callbacks.result.unwrap_or_else(|| {
        Err("fixed checked-output extraction callback did not reach rustc analysis".to_owned())
    });
    if let Some(recorder) = callbacks.census
        && let Err(error) = recorder.finish(fatal.is_none() && result.is_ok())
    {
        eprintln!("fe2o3 diagnostic source census unavailable: {error}");
    }
    if let Some(fatal) = fatal {
        fatal.raise();
    }
    result
}

#[cfg(test)]
#[path = "production_rustc_driver_fixed_checked_output_policy7_v1_tests.rs"]
mod tests;
