use fe2o3_external_anchor_service::{
    NATIVE_EXTERNAL_ANCHOR_PROCESS_STORAGE_LIMIT_V2, NATIVE_EXTERNAL_ANCHOR_PROCESS_WORK_LIMIT_V2,
    NativeExternalAnchorEntrypointErrorV2 as Error,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    let mut work = Work::new(NATIVE_EXTERNAL_ANCHOR_PROCESS_WORK_LIMIT_V2);
    let mut budget = Budget::new(&mut work, NATIVE_EXTERNAL_ANCHOR_PROCESS_STORAGE_LIMIT_V2);
    let result = (|| -> Result<(), Error> {
        budget.reserve_storage(INPUT)?;
        // SAFETY: this dedicated binary creates no descriptor owners or threads
        // before its sole inherited entry. It exits on failure and never retries.
        #[allow(unsafe_code)]
        let result = unsafe { run(&mut budget) };
        budget.release_storage(INPUT)?;
        let (report, charge) = result?;
        let bytes = charge.additional_storage();
        budget.reserve_storage(bytes)?;
        drop((report, charge));
        budget.release_storage(bytes)?;
        Ok(())
    })();
    if result.is_err() {
        std::process::exit(1);
    }
}
