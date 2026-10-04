use fe2o3_runtime::RuntimeContextV1;
use fe2o3_sim_runtime::SimRuntimeBackendV1;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = SimRuntimeBackendV1::gfx942([0x53; 32])?;
    let evidence = backend.evidence();
    let context = RuntimeContextV1::open(backend)?;
    let device = &context.devices()[0];
    println!(
        "{{\"mode\":\"{}\",\"simulated\":{},\"hardware\":{},\"performance_prediction\":{},\"target\":\"{}\"}}",
        evidence.mode,
        evidence.simulated,
        evidence.hardware,
        evidence.performance_prediction,
        device.target()
    );
    context
        .shutdown()
        .map_err(|_| std::io::Error::other("simulator shutdown failed"))?;
    Ok(())
}
