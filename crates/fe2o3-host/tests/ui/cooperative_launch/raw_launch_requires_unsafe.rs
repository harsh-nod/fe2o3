use fe2o3_core::KernelParams;
use fe2o3_host::{CooperativeLaunchAdmission, CooperativeLaunchError};

fn bypass<K>(
    admission: CooperativeLaunchAdmission<'_, '_, K>,
    params: &mut KernelParams,
) -> Result<(), CooperativeLaunchError> {
    admission.launch_raw(params)
}

fn main() {}
