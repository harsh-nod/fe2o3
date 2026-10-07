//! Shared genuine native startup. No fixture bytes, alternate profiles or legacy fallback.
use fe2o3_artifact_transaction::ProducerIdentity;
use fe2o3_compiler_closure_capability::{
    ProductionCompilerExecutionDeploymentV3 as Compiler,
    ProductionNativeApplicationProofProfileV1 as Profile,
};
use fe2o3_compiler_execution_client::CompilerExecutionClientV3;
use fe2o3_host::{
    PreparedNativeConditionalFillApplicationV1 as Prepared,
    ProvedNativeConditionalFillApplicationV1 as Proved,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    fmt::Debug,
    path::Path,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, String>;
pub struct Summary {
    pub schema: &'static str,
    /// Complete JSON member list from a private, validated fixture oracle.
    pub fields: String,
}
pub fn checked<T>(result: std::result::Result<T, impl Debug>) -> Result<T> {
    result.map_err(|error| format!("{error:?}"))
}

pub fn run(
    producer_source: &str,
    execute: impl for<'work> FnOnce(&mut Proved<'work>, &mut Budget<'work>, Instant) -> Result<Summary>,
) -> Result<()> {
    run_with_report(producer_source, execute, Report::Runtime)
}

/// Runs the same protected intake under one consuming original account, then
/// lends its actual proof owner to the caller's executor without nested joins.
/// Startup and final currentness checks remain synchronous bounded control I/O.
pub async fn run_async(
    producer_source: &str,
    execute: impl for<'work> AsyncFnOnce(
        &mut Proved<'work>,
        &mut Budget<'work>,
        Instant,
    ) -> Result<Summary>,
) -> Result<()> {
    let account = OwnedBudget::new(Work::new(1_000_000_000_000), 2 * 1024 * 1024 * 1024);
    let (_account, result) = account
        .into_budget_scope_async_v1(async |budget| {
            let deadline = Instant::now() + Duration::from_secs(300);
            let mut proved = admit(producer_source, deadline, budget)?;
            let summary = execute(&mut proved, budget, deadline).await?;
            finish(proved, budget, deadline, summary, Report::Runtime)
        })
        .await;
    result
}

/// Genuine proof/currentness qualification only. This entry performs no device
/// discovery, KFD open, runtime Context creation, allocation, launch or copy.
pub fn run_proof_only(
    producer_source: &str,
    schema: &'static str,
    fields: &'static str,
) -> Result<()> {
    run_with_report(
        producer_source,
        |_, _, _| {
            Ok(Summary {
                schema,
                fields: fields.into(),
            })
        },
        Report::ProofOnly,
    )
}

enum Report {
    Runtime,
    ProofOnly,
}

fn run_with_report(
    producer_source: &str,
    execute: impl for<'work> FnOnce(&mut Proved<'work>, &mut Budget<'work>, Instant) -> Result<Summary>,
    report: Report,
) -> Result<()> {
    // One process-lifetime canonical account. Device memory, result credits and
    // admitted analyzer subprocesses retain their separate resource domains.
    let mut account = OwnedBudget::new(Work::new(1_000_000_000_000), 2 * 1024 * 1024 * 1024);
    account.with_budget(|budget| {
        let deadline = Instant::now() + Duration::from_secs(300);
        let mut proved = admit(producer_source, deadline, budget)?;
        let summary = execute(&mut proved, budget, deadline)?;
        finish(proved, budget, deadline, summary, report)
    })
}

fn admit<'work>(
    producer_source: &str,
    deadline: Instant,
    budget: &mut Budget<'work>,
) -> Result<Proved<'work>> {
    let producer = checked(ProducerIdentity::from_codegen(
        "fe2o3_conditional_custodian_application",
        Some(Path::new(producer_source)),
    ))?;
    let mut producer_storage = size_of::<ProducerIdentity>();
    producer.visit_retained_heap_storage_v1(|count, width| -> Result<()> {
        producer_storage = producer_storage
            .checked_add(
                count
                    .checked_mul(width)
                    .ok_or("producer storage overflow")?,
            )
            .ok_or("producer storage overflow")?;
        Ok(())
    })?;
    checked(budget.reserve_storage(producer_storage))?;
    let (compiler, charge) = checked(Compiler::open(budget))?;
    checked(budget.reserve_storage(charge.additional_storage()))?;
    let (profile, charge) = checked(Profile::open(&compiler, budget))?;
    checked(budget.reserve_storage(charge.additional_storage()))?;
    // SAFETY: these fixture entrypoints remain single-threaded, create no
    // descendants or signal handlers, and exclusively consume the four slots.
    let (prepared, charge) =
        checked(unsafe { Prepared::admit_inherited(&producer, &compiler, &profile, budget) })?;
    checked(budget.reserve_storage(charge.additional_storage()))?;
    let (registered, charge) =
        checked(prepared.register_custodian(compiler, profile, deadline, budget))?;
    checked(budget.reserve_storage(charge.additional_storage()))?;
    let (ready, charge) = checked(registered.await_currentness_ready(deadline, budget))?;
    checked(budget.reserve_storage(charge.additional_storage()))?;
    checked(budget.reserve_storage(CompilerExecutionClientV3::PEER_STORAGE))?;
    // SAFETY: the four-slot claim left the sole inherited service FD195
    // untouched. Transfer it exactly once, including every failure exit.
    let (mut proved, charge) =
        checked(unsafe { ready.verify_acknowledge_and_prove(deadline, budget) })?;
    checked(budget.reserve_storage(charge.additional_storage()))?;
    checked(proved.revalidate(deadline, budget))?;
    Ok(proved)
}

fn finish<'work>(
    mut proved: Proved<'work>,
    budget: &mut Budget<'work>,
    deadline: Instant,
    summary: Summary,
    report: Report,
) -> Result<()> {
    checked(proved.revalidate(deadline, budget))?;
    let proof = proved.proof();
    let parts = proof.evidence().parts();
    let completion = match report {
        Report::Runtime => "\"shutdown\":\"released\",\"result_credits\":\"refunded\",",
        // The application observes live proof custody here; only the root
        // manager and external supervisor can attest later CPU cleanup.
        Report::ProofOnly => "\"application_proof_observed\":true,\"currentness_verified\":true,",
    };
    println!(
        concat!(
            "{{\"schema\":\"{}\",{},",
            "\"source\":{},\"final_kernel_ir\":{},\"hsaco\":{},\"readiness\":{},",
            "\"analysis_request\":{},\"analysis_bundle\":{},\"analysis_receipt\":{},",
            "\"generated_source\":{},\"obligation\":{},\"signed_receipt\":{},",
            "\"policy_identity\":\"{}\",\"carriage_identity\":\"{}\",",
            "\"subject_identity\":\"{}\",\"proof_identity\":\"{}\",",
            "\"proof_session\":\"{}\",\"registration_identity\":\"{}\",",
            "{}",
            "\"physical_overlap_measured\":false,\"all_host_devices_qualified\":false}}"
        ),
        summary.schema,
        summary.fields,
        blob(parts.native_handoff),
        blob(parts.final_kernel_ir),
        blob(proof.inputs().payload()),
        blob(proof.inputs().readiness()),
        blob(parts.analysis_request),
        blob(parts.analysis_bundle),
        blob(parts.analysis_receipt),
        blob(parts.generated_source),
        blob(parts.obligation),
        blob(parts.signed_receipt),
        hex(parts.policy_identity),
        hex(parts.carriage_identity),
        hex(parts.subject_identity),
        hex(proof.evidence().identity()),
        hex(proof.inputs().session_identity()),
        hex(proof.inputs().registration_identity()),
        completion,
    );
    // The original publication/currentness/proof outlive all native cleanup.
    drop(proved);
    Ok(())
}

fn blob((sha256, bytes): ([u8; 32], u64)) -> String {
    format!("{{\"sha256\":\"{}\",\"bytes\":{bytes}}}", hex(sha256))
}
fn hex(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .into_iter()
        .flat_map(|b| {
            [
                char::from(DIGITS[usize::from(b >> 4)]),
                char::from(DIGITS[usize::from(b & 15)]),
            ]
        })
        .collect()
}
