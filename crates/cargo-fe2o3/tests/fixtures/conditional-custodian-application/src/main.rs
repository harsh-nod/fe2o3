fn main() -> Result<(), Box<dyn std::error::Error>> {
    use fe2o3_conditional_custodian_application::fill_write_only_gpu;
    use fe2o3_host::{
        CompilerGeneratedKernelExpectationV1, KernelId,
        consume_inherited_worker_v3_application_custodian_handoff_v1,
    };
    use std::time::{Duration, Instant};

    if std::env::args_os().count() != 1 {
        return Err("this admission application accepts no arguments".into());
    }
    let deadline = Instant::now() + Duration::from_secs(180);
    let kernel = KernelId::from_bytes(fill_write_only_gpu::Marker::KERNEL_BINDING_ID_V1);
    // SAFETY: single-threaded startup exclusively consumes Cargo's inherited handoff once.
    let application =
        unsafe { consume_inherited_worker_v3_application_custodian_handoff_v1(kernel)? };
    let artifact =
        application.into_remote_conditional_fill::<fill_write_only_gpu::Marker>(deadline)?;
    artifact.revalidate(deadline)?;
    assert_eq!(artifact.descriptor().kernel_id(), kernel);
    assert!(!artifact.grants_launch_authority());
    println!("genuine compiler audit and retained conditional proof admitted");
    Ok(())
}
