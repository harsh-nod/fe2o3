use fe2o3_core::{KernelParams, Stream};
use fe2o3_host::CooperativeLaunchAdmission;

unsafe fn cross<K>(
    admission: CooperativeLaunchAdmission<'_, '_, K>,
    other_stream: &Stream,
    params: &mut KernelParams,
) {
    let _ = unsafe { admission.launch_raw(other_stream, params) };
}

fn main() {}
