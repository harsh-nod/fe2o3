//! Explicit pre-ranked source diagnostic export. No worker/artifact continuation.
use super::{
    Callbacks, Compilation, Compiler, Path, TyCtxt, publish_new_inert_output,
    require_canonical_overflow_checks_v1, transaction_in_active_session_v1,
};

struct PhysicalCallbacks<'a> {
    output: &'a Path,
    calls: usize,
    result: Option<Result<(), String>>,
}
impl Callbacks for PhysicalCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(extract(tcx, self.output));
        Compilation::Stop
    }
}
/// Produces diagnostic KIR22, unchanged canonical LLVM and a bounded native
/// observation sidecar from one live authenticated MIR39 source preparation.
///
/// This is explicitly PRE-RANKED: the mandatory ranked/formal checks and normal
/// descriptor/worker handoff are not implemented by this diagnostic endpoint.
/// Files do not retain source/compiler custody or authorize artifacts/launch.
/// No native compiler, simulator, GPU or hardware debugger is invoked. Ordinary
/// production routes continue to refuse this profile. The output directory must
/// not already exist and is created only after source/profile checks succeed.
pub fn run_diagnostic_physical_lds_exchange_extraction_driver_v22(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    let mut callbacks = PhysicalCallbacks {
        output,
        calls: 0,
        result: None,
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    if fatal.is_some() {
        return Err("diagnostic physical-lds-exchange V22 rustc analysis failed".into());
    }
    if callbacks.calls != 1 {
        return Err(
            "diagnostic physical-lds-exchange V22 requires exactly one actual rustc callback"
                .into(),
        );
    }
    callbacks.result.unwrap_or_else(|| {
        Err("diagnostic physical-lds-exchange V22 callback produced no result".into())
    })
}
fn extract(tcx: TyCtxt<'_>, output: &Path) -> Result<(), String> {
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    if !transaction.has_authenticated_physical_lds_exchange_v22() {
        return Err(
            "diagnostic V22 requires actual authenticated physical-lds-exchange markers".into(),
        );
    }
    let owner = transaction
        .lower_physical_lds_exchange_diagnostic_v22()
        .map_err(|e| e.to_string())?;
    publish(&owner, output)?;
    eprintln!(
        "fe2o3 diagnostic physical-lds-exchange V22: actual Rust -> MIR39 -> pre-ranked exact KIR22 -> canonical LLVM; ranked/formal/descriptor continuation unavailable; serialized outputs are inert diagnostic observations, not source/compiler custody; native execution, hardware observation and artifact/launch authority false"
    );
    Ok(())
}
pub(super) fn publish(
    owner:&crate::production_pipeline::physical_lds_exchange_diagnostic_v22::AuthenticatedPhysicalLdsExchangeDiagnosticV22,
    output: &Path,
) -> Result<(), String> {
    // Do not merge, overwrite or recycle an existing directory. Partial output
    // after an I/O error is an incomplete inert diagnostic export, never a commit.
    std::fs::create_dir(output)
        .map_err(|e| format!("fresh physical-lds-exchange diagnostic directory: {e}"))?;
    for (name, bytes, limit) in [
        (
            "canonical-v22.bin",
            owner.materialized().executable().canonical_bytes(),
            fe2o3_kernel_ir::MAX_MODULE_BYTES_V1,
        ),
        ("canonical.ll", owner.llvm_ir().as_bytes(), 64 * 1024),
        (
            "native-observation-input-v22.txt",
            owner.native_observation().as_bytes(),
            32 * 1024,
        ),
    ] {
        publish_new_inert_output(&output.join(name), bytes, limit, name)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_lds_exchange_diagnostic_requires_explicit_overflow_before_rustc() {
        let error = run_diagnostic_physical_lds_exchange_extraction_driver_v22(
            &["rustc".into()],
            Path::new("/unopened-physical-v22"),
        )
        .unwrap_err();
        assert!(error.contains("exactly one canonical"));
    }
}
