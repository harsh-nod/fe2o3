use fe2o3_external_anchor_provisioner::{
    NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2,
    NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_WORK_V2,
    NativeExternalAnchorProvisioningHelperErrorV2 as Error,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::convert::Infallible;

fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    let mut work = Work::new(NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_WORK_V2);
    let mut b = Budget::new(&mut work, NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2);
    let result = (|| -> Result<Infallible, Error> {
        b.reserve_storage(INPUT)?;
        // SAFETY: sole entry of a dedicated binary with no Rust descriptor owners or threads.
        // Successful exec replaces this process; every return is terminal failure.
        #[allow(unsafe_code)]
        let result = unsafe { run(&mut b) };
        b.release_storage(INPUT)?;
        result
    })();
    match result {
        Ok(never) => match never {},
        Err(_) => std::process::exit(1),
    }
}
