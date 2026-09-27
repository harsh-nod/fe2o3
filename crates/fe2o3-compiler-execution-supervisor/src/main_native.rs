use fe2o3_compiler_execution_supervisor::{
    MAX_PROTECTED_ISSUER_PROCESSES_V1 as CELLS, NATIVE_ISSUER_PROCESS_STORAGE_V2 as STORAGE,
    NATIVE_ISSUER_PROCESS_WORK_V2 as WORK, ProtectedIssuerCleanupServiceV2 as Cleanup,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::time::Duration;

fn serve() -> Option<()> {
    let wait = Wait::new(Wait::MAX_ATTEMPTS, Duration::from_secs(30)).ok()?;
    let exit = Wait::new(Wait::MAX_ATTEMPTS, Wait::MAX_TIMEOUT).ok()?;
    let session = Session::new(Duration::from_secs(30), wait, wait, wait, exit).ok()?;
    let dispatch = Dispatch::new(Dispatch::MAX_TURNS, wait, CELLS).ok()?;
    let account = Account::new(Work::new(1 << 30), Cleanup::STORAGE);
    let mut cleanup = Cleanup::admit(account).ok()?;
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(INPUT).ok()?;
    // SAFETY: sole entry in a dedicated binary; no Rust FD owners, threads,
    // reservations or pending children exist. Cleanup was freshly admitted.
    #[allow(unsafe_code)]
    let result = unsafe { run(session, dispatch, &mut cleanup, &mut b) };
    b.release_storage(INPUT).ok()?;
    // Every visit uses the same independently funded pool. A failed shutdown
    // keeps custody in that pool; bounded exhaustion is never called success.
    for _ in 0..4096 {
        if cleanup.shutdown().is_ok() {
            return result.ok().map(|_| ());
        }
        cleanup.pump(CELLS).ok()?;
    }
    None
}

fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    let _ = serve();
    // Startup refusal, dispatch refusal and exhaustion of the finite service
    // policy are all terminal stops, not a successful daemon or reaping receipt.
    std::process::exit(1);
}
