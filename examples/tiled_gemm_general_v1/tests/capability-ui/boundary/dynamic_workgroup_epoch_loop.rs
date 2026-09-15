// expected-rejection: FE2O3-CAP-GEMM legacy-epoch-reassignment
#![no_std]

#[cfg(not(target_arch = "amdgpu"))]
compile_error!("GEMM device UI must compile the actual AMD target");

use fe2o3_device::{InitialEpoch, WorkgroupCapability};

fn dynamic_epochs<'workgroup, Brand>(
    mut workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    phases: usize,
) {
    let mut phase = 0;
    while phase < phases {
        let initialized = workgroup
            .allocate_lds::<u16, 64>()
            .initialize_by_invocation(&workgroup, 0);
        let (next, _published) = workgroup.publish_lds(initialized);
        workgroup = next;
        phase += 1;
    }
}
