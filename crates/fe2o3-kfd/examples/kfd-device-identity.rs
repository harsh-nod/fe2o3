use fe2o3_kfd::topology::discover_default_topology;
use fe2o3_kfd::{DEVICE_ADMISSION_PROFILE_SHA256_V1, DeviceSelector, OpenedKfd};

fn parse_unique_id(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if let Some(hex) = value.strip_prefix("0x") {
        Ok(u64::from_str_radix(hex, 16)?)
    } else {
        Ok(value.parse()?)
    }
}

fn admit_and_print(selected: u64) -> Result<(), Box<dyn std::error::Error>> {
    let mut device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(selected))?;
    let currentness = device.check_observable_currentness()?;
    let observation = device.observation();
    let aperture = observation.aperture();
    let snapshot = device.topology_snapshot();
    let gpu = snapshot
        .topology()
        .gpu_nodes()
        .iter()
        .find(|gpu| gpu.unique_id() == observation.unique_id())
        .ok_or("selected GPU disappeared from retained topology")?;
    let module_version = snapshot
        .amdgpu_module()
        .version()
        .ok_or("admitted amdgpu module version disappeared")?;
    let module_srcversion = snapshot
        .amdgpu_module()
        .srcversion()
        .ok_or("admitted amdgpu module srcversion disappeared")?;
    println!(
        "profile=gfx942:xnack- target=gfx942 wavefront={} profile_sha256={} boot_id={} kernel={} amdgpu={} amdgpu_srcversion={} partition=SPX/NPS1 node={} gpu_id={} unique_id={:016x} pci={} renderD{} drm={}.{}.{} firmware={}/{} descriptors={} vram_lost_counter={} currentness=contracted-clear aperture_lds={:#x}..={:#x} aperture_scratch={:#x}..={:#x} aperture_gpuvm={:#x}..={:#x}",
        gpu.capacity().wavefront_size(),
        DEVICE_ADMISSION_PROFILE_SHA256_V1,
        snapshot.boot_id(),
        snapshot.kernel_release().as_str(),
        module_version,
        module_srcversion,
        observation.topology_node_id(),
        observation.kfd_gpu_id(),
        observation.unique_id(),
        observation.pci(),
        observation.render_minor(),
        observation.drm().driver_version().major,
        observation.drm().driver_version().minor,
        observation.drm().driver_version().patch,
        gpu.fw_version(),
        gpu.sdma_fw_version(),
        device.descriptor_count(),
        currentness.vram_lost_counter(),
        aperture.lds().base(),
        aperture.lds().limit(),
        aperture.scratch().base(),
        aperture.scratch().limit(),
        aperture.gpuvm().base(),
        aperture.gpuvm().limit(),
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1) {
        Some(value) if value == "--all" => {
            let unique_ids = discover_default_topology()?
                .topology()
                .gpu_nodes()
                .iter()
                .map(|gpu| gpu.unique_id())
                .collect::<Vec<_>>();
            for unique_id in unique_ids {
                admit_and_print(unique_id)?;
            }
        }
        Some(value) => admit_and_print(parse_unique_id(&value)?)?,
        None => {
            let unique_id = discover_default_topology()?
                .topology()
                .gpu_nodes()
                .first()
                .ok_or("no topology GPU")?
                .unique_id();
            admit_and_print(unique_id)?;
        }
    }
    Ok(())
}
