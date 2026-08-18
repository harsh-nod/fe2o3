#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = fe2o3_kfd::topology::discover_default_topology()?;
    let topology = snapshot.topology();
    let provenance = topology.provenance();
    println!(
        concat!(
            "root={} generation={} boot_id={} kernel_release={} ",
            "module_version={} module_srcversion={} platform={}:{}:{} nodes={} gpus={}"
        ),
        provenance.root().display(),
        provenance.generation(),
        snapshot.boot_id(),
        snapshot.kernel_release(),
        snapshot.amdgpu_module().version().unwrap_or("<absent>"),
        snapshot.amdgpu_module().srcversion().unwrap_or("<absent>"),
        provenance.platform().oem(),
        provenance.platform().id(),
        provenance.platform().revision(),
        topology.observed_node_count(),
        topology.gpu_nodes().len(),
    );
    for (gpu, render) in topology.gpu_nodes().iter().zip(snapshot.render_nodes()) {
        println!(
            concat!(
                "node={} gpu_id={} target={} render_minor={} unique_id={} hive_id={} ",
                "pci={} pci_revision={:#04x} partition={}/{} domain={} location_id={} ",
                "fw_version={} sdma_fw_version={} wavefront={} simds={} xccs={}"
            ),
            gpu.node_id(),
            gpu.gpu_id(),
            gpu.target().name(),
            gpu.drm_render_minor(),
            gpu.unique_id(),
            gpu.hive_id(),
            render.pci_address(),
            render.pci_revision(),
            render.partition().compute().name(),
            render.partition().memory().name(),
            gpu.domain(),
            gpu.location_id(),
            gpu.fw_version(),
            gpu.sdma_fw_version(),
            gpu.capacity().wavefront_size(),
            gpu.capacity().simd_count(),
            gpu.capacity().xcc_count(),
        );
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("KFD topology discovery is available only on Linux");
    std::process::exit(2);
}
