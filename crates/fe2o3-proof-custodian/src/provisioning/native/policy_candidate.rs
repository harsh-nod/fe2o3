//! Offline inert export, not policy approval, signature verification or recovery.
use super::*;
use fe2o3_verifier::{
    encode_native_conditional_root_policy_file_v1 as encode,
    reconstruct_inert_native_conditional_policy_roster_in_original_account_v1 as reconstruct,
    with_decoded_native_conditional_source_packet_v2 as with_source,
};
use std::fmt::Write as _;

const SOURCE_MAX: usize = 4 * 1024 * 1024;
const STORAGE: usize = 256 * 1024 * 1024;
const FRAME: usize = 8192;
// 4096 signers, ten toolchain identities and fixed metadata fit without growth.
const REPORT_MAX: usize = 512 * 1024;

struct Export {
    policy: Vec<u8>,
    report: String,
}

fn prepare(source: &[u8], roster: &[u8], budget: &mut Budget<'_>) -> io::Result<Export> {
    budget.charge_work(1024).map_err(other)?;
    require(
        source.len() <= SOURCE_MAX && roster.len() <= POLICY_MAX,
        "native policy candidate input bounds",
    )?;
    require(
        budget.storage()
            >= source.len().checked_add(roster.len()).ok_or_else(|| {
                io::Error::other("native policy candidate input accounting overflow")
            })?,
        "native policy candidate inputs are not prepaid",
    )?;
    budget.reserve_storage(FRAME).map_err(other)?;
    let (semantic_root, kernel_binding, max_grid_x) = with_source(source, budget, |view, b| {
        b.charge_work(64).map_err(other)?;
        require(
            view.roots.len() == 1 && view.canonical_kernel_order == [0],
            "native policy candidate requires one source-ordered root",
        )?;
        let root = &view.roots[0];
        let launch = root.launch.launch();
        let grid = launch.max_grid();
        require(
            root.launch_rank == 1
                && launch.rank() == 1
                && launch.exact_workgroup() == Some([64, 1, 1])
                && grid[0] != 0
                && grid[1..] == [1, 1],
            "native policy candidate requires the closed fill64 launch shape",
        )?;
        Ok::<_, io::Error>((root.semantic_root, root.launch.kernel_binding(), grid[0]))
    })
    .map_err(other)??;
    let (values, charge) = reconstruct(roster, source, budget).map_err(other)?;
    budget
        .reserve_storage(charge.retained_storage())
        .map_err(other)?;
    let (policy, charge) = encode(roster, budget)?;
    budget.reserve_storage(charge).map_err(other)?;
    budget
        .reserve_storage(REPORT_MAX + std::mem::size_of::<String>())
        .map_err(other)?;
    budget.charge_work(REPORT_MAX).map_err(other)?;
    let mut report = String::new();
    report.try_reserve_exact(REPORT_MAX).map_err(other)?;
    require(
        report.capacity() == REPORT_MAX,
        "native candidate report allocation exceeds prepayment",
    )?;
    writeln!(report, "native_policy_candidate_format=1").map_err(other)?;
    for (name, bytes) in [
        ("source", source),
        ("roster", roster),
        ("candidate", &policy),
    ] {
        writeln!(
            report,
            "{name}_sha256={}",
            hex(Sha256::digest(bytes).into())
        )
        .map_err(other)?;
        writeln!(report, "{name}_bytes={}", bytes.len()).map_err(other)?;
    }
    writeln!(
        report,
        "profile=gfx942:xnack-/wave64/production-history-v1/boundary6"
    )
    .map_err(other)?;
    writeln!(report, "semantic_root={semantic_root}").map_err(other)?;
    writeln!(report, "kernel_binding={}", hex(kernel_binding)).map_err(other)?;
    writeln!(report, "max_grid_x={max_grid_x}").map_err(other)?;
    values
        .with_root_policies(budget, |roots, b| {
            require(roots.len() == 1, "native candidate policy root count")?;
            let root = &roots[0];
            let effects = root.effects.signer_identities();
            b.charge_work(
                effects
                    .len()
                    .checked_mul(128)
                    .ok_or_else(|| io::Error::other("native candidate report work overflow"))?,
            )
            .map_err(other)?;
            writeln!(report, "effect_signer_count={}", effects.len()).map_err(other)?;
            for (index, signer) in effects.iter().enumerate() {
                writeln!(report, "effect_signer_{index}={}", hex(*signer.as_bytes()))
                    .map_err(other)?;
            }
            for (prefix, toolchain) in [
                ("effect", root.effects.toolchain()),
                ("formula", root.formula.toolchain()),
            ] {
                for (name, digest) in [
                    ("verus_executable", toolchain.verus_executable()),
                    ("verus_configuration", toolchain.verus_configuration()),
                    ("solver_executable", toolchain.solver_executable()),
                    ("solver_configuration", toolchain.solver_configuration()),
                    ("runtime_closure", toolchain.runtime_closure()),
                ] {
                    writeln!(report, "{prefix}_{name}={}", hex(*digest.as_bytes()))
                        .map_err(other)?;
                }
            }
            writeln!(
                report,
                "formula_verifying_key={}",
                hex(*root.formula.verifying_key())
            )
            .map_err(other)?;
            writeln!(report, "formula_boundary={}", root.formula.boundary() as u8)
                .map_err(other)?;
            Ok::<_, io::Error>(())
        })
        .map_err(other)??;
    writeln!(report, "grants_authority=false\nsource_target_verified=false\nfill_semantics_verified=false\npolicy_independently_approved=false")
        .map_err(other)?;
    require(
        report.len() <= REPORT_MAX && report.capacity() == REPORT_MAX,
        "native candidate report exceeded its fixed storage",
    )?;
    Ok(Export { policy, report })
}

fn export_using(
    source: &Path,
    source_pin: [u8; 32],
    roster: &Path,
    roster_pin: [u8; 32],
    output: &Path,
    budget: &mut Budget<'_>,
) -> io::Result<String> {
    budget.reserve_storage(FRAME).map_err(other)?;
    let source = read_pinned(source, source_pin, SOURCE_MAX, budget)?;
    let roster = read_pinned(roster, roster_pin, POLICY_MAX, budget)?;
    let value = prepare(&source, &roster, budget)?;
    files::write_candidate_atomic(output, &value.policy, budget)?;
    Ok(value.report)
}

pub(in super::super) fn export(
    source: &Path,
    source_pin: [u8; 32],
    roster: &Path,
    roster_pin: [u8; 32],
    output: &Path,
) -> io::Result<()> {
    // This role check does not admit a proof profile. Export needs no installed
    // files, currentness endpoint, signing key, analyzer or compiler execution.
    current_inspection_credentials()?;
    let mut account = Owned::new(Work::new(ACCOUNT_WORK), STORAGE);
    account.with_budget(|budget| {
        let report = export_using(source, source_pin, roster, roster_pin, output, budget)?;
        budget.charge_work(REPORT_MAX).map_err(other)?;
        require(
            rustix::io::write(std::io::stdout(), report.as_bytes())? == report.len(),
            "native policy candidate published but report write was short",
        )
    })
}

#[cfg(test)]
mod tests;
