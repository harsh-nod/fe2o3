//! Explicit raw diagnostic export from the existing live scoped tile observer.
use super::{
    Callbacks, Compilation, Compiler, Path, TyCtxt, lower_hex_v1,
    require_canonical_overflow_checks_v1, transaction_in_active_session_v1,
};
use crate::production_pipeline::ProductionPipelineError as PipelineError;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;
use sha2::{Digest, Sha256};

#[path = "scoped_tile_diagnostic_publication_v18.rs"]
mod publication;
use publication::StagedOutput;

#[derive(Clone, Copy, Debug)]
pub(super) struct ExportObservation {
    pub(super) source: [u8; 32],
    pub(super) pending: [u8; 32],
    pub(super) scalar: [u8; 32],
    pub(super) schedule: [u8; 32],
    pub(super) raw_sha256: [u8; 32],
    pub(super) bytes: u64,
}

struct TileCallbacks<'a> {
    staged: &'a mut StagedOutput,
    order: Order,
    calls: usize,
    result: Option<Result<ExportObservation, String>>,
}
impl Callbacks for TileCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(if self.calls == 1 {
            extract(tcx, self.order, self.staged)
        } else {
            Err("diagnostic V18 requires exactly one rustc analysis callback".into())
        });
        Compilation::Stop
    }
}

/// Exports the exact scalarized canonical V18 bytes observed from actual Rust.
///
/// The order is an explicit diagnostic distribution, not an equivalence proof.
/// The existing source/candidate owners and original ledger remain live during
/// staging. Only a single completed observation and its expected diagnostic
/// terminal refusal permit fresh no-clobber publication. No canonical owner or
/// source custody escapes, and normal compilation never resumes. The output is
/// raw CPU/debug input, not a simulation bundle, source map, native artifact,
/// formal proof, compiler authentication, or load/launch authority.
pub fn run_diagnostic_scoped_tile_kir_extraction_driver_v18(
    args: &[String],
    output: &Path,
    order: Order,
) -> Result<(), String> {
    let report = run_with_observation(args, output, order)?;
    eprintln!(
        "fe2o3 diagnostic scoped tile V18: actual Rust -> scoped semantic MIR V29 -> exact scalar candidate KIR V18; order={order:?}, canonical_identity {}, {} byte(s), source_semantic {}, pending_identity {}, schedule_identity {}; observation_only=true, source_authentication_exported=false, ranked/formal/protected/artifact/load/launch/hardware_authority=false; different orders need not implement equivalent whole-kernel output",
        lower_hex_v1(&report.scalar),
        report.bytes,
        lower_hex_v1(&report.source),
        lower_hex_v1(&report.pending),
        lower_hex_v1(&report.schedule),
    );
    Ok(())
}

pub(super) fn run_with_observation(
    args: &[String],
    output: &Path,
    order: Order,
) -> Result<ExportObservation, String> {
    require_canonical_overflow_checks_v1(args)?;
    let mut staged = StagedOutput::new(output)?;
    let mut callbacks = TileCallbacks {
        staged: &mut staged,
        order,
        calls: 0,
        result: None,
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    if fatal.is_some() {
        return Err("diagnostic V18 rustc analysis failed; no output was promoted".into());
    }
    if callbacks.calls != 1 {
        return Err("diagnostic V18 requires exactly one actual rustc callback".into());
    }
    let report = callbacks
        .result
        .take()
        .ok_or_else(|| "diagnostic V18 callback produced no result".to_owned())??;
    drop(callbacks);
    staged.promote(report.bytes, &report.raw_sha256)?;
    Ok(report)
}

fn extract(
    tcx: TyCtxt<'_>,
    order: Order,
    staged: &mut StagedOutput,
) -> Result<ExportObservation, String> {
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    let mut observation = None;
    let mut publication_error = None;
    let mut calls = 0;
    let result = transaction.observe_scoped_tile_candidate_in_order_v29(order, |view, budget| {
        calls += 1;
        if calls != 1 {
            return Err(PipelineError::ExtractionCannotPublish);
        }
        let owner = view.canonical();
        let bytes = owner.canonical_bytes();
        let observed = stage_bytes(bytes, budget, staged)
            .map_err(resource)?
            .map(|raw_sha256| ExportObservation {
                source: *view.source_semantic_sha256(),
                pending: *view.pending_identity().digest(),
                scalar: *owner.identity().digest(),
                schedule: *view.schedule_identity(),
                raw_sha256,
                bytes: owner.identity().canonical_length(),
            });
        match observed {
            Ok(report) => {
                observation = Some(report);
                Ok(())
            }
            Err(error) => {
                publication_error = Some(error);
                Err(PipelineError::ExtractionCannotPublish)
            }
        }
    });
    require_completed_observation(calls, observation, publication_error, result)
}

fn stage_bytes(
    bytes: &[u8],
    budget: &mut Budget<'_>,
    staged: &mut StagedOutput,
) -> Result<Result<[u8; 32], String>, Resource> {
    let work = bytes.len().checked_mul(2).ok_or(Resource::Arithmetic)?;
    // This prepays the original-ledger byte scan and fixed inert report. File
    // publication retains its separate bounded IO contract; no canonical copy
    // or CPU view escapes. Entry work is part of, not additional to, total work.
    budget.with_prepaid_scope(
        budget.storage(),
        work,
        work,
        std::mem::size_of::<ExportObservation>() + std::mem::size_of::<Sha256>(),
        |_| Ok(staged.write(bytes).map(|()| Sha256::digest(bytes).into())),
    )
}

fn resource(error: Resource) -> PipelineError {
    PipelineError::ContextHandoff(error.into())
}

fn require_completed_observation(
    calls: usize,
    observation: Option<ExportObservation>,
    publication_error: Option<String>,
    result: Result<(), Box<PipelineError>>,
) -> Result<ExportObservation, String> {
    if let Some(error) = publication_error {
        return Err(error);
    }
    match result {
        Err(error) if matches!(*error, PipelineError::ScopedTileObservationIncomplete) => {
            if calls != 1 {
                return Err(
                    "diagnostic V18 requires exactly one completed candidate callback".into(),
                );
            }
            observation.ok_or_else(|| "diagnostic V18 terminal refusal without staged bytes".into())
        }
        Err(error) => Err(format!(
            "diagnostic V18 source observation failed ({calls} candidate callback(s)): {error}"
        )),
        Ok(()) => Err("diagnostic V18 unexpectedly continued compilation".into()),
    }
}

#[cfg(test)]
#[path = "scoped_tile_diagnostic_export_v18_tests.rs"]
mod tests;
