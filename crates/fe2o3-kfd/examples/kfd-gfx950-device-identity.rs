#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use fe2o3_kfd::topology::{GfxTarget, discover_default_topology_for_target};
    use fe2o3_kfd::{DeviceSelector, OpenedKfd, gfx950_device_observation_profile_sha256_v1};

    let mut arguments = std::env::args().skip(1);
    let selector = arguments.next();
    if arguments.next().is_some() {
        return Err("usage: kfd-gfx950-device-identity [--all|unique-id]".into());
    }
    let selected = match selector.as_deref() {
        Some("--all") => discover_default_topology_for_target(GfxTarget::Gfx950)?
            .topology()
            .gpu_nodes()
            .iter()
            .map(|gpu| gpu.unique_id())
            .collect(),
        Some(value) => vec![match value.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16)?,
            None => value.parse()?,
        }],
        None => vec![
            discover_default_topology_for_target(GfxTarget::Gfx950)?
                .topology()
                .gpu_nodes()
                .first()
                .ok_or("no GPU")?
                .unique_id(),
        ],
    };
    let profile: String = gfx950_device_observation_profile_sha256_v1()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let descriptor_count =
        || -> Result<usize, std::io::Error> { Ok(std::fs::read_dir("/proc/self/fd")?.count()) };
    let before = descriptor_count()?;
    for unique_id in selected {
        let mut device = OpenedKfd::open_default()?
            .admit_uapi()?
            .bind_gfx950_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
        device.check_observable_currentness()?;
        device.check_observable_currentness()?;
        let observation = device.observation();
        println!(
            "target=gfx950:xnack- profile_sha256={} gpu_id={} unique_id={} render_minor={} drm_device={:?} descriptors={} apertures={} currentness=contracted-clear authority=checked-observation-only explicit_vm_acquisition=false vm_authority=false memory=false queue=false dispatch=false xnack_set=false",
            profile,
            observation.kfd_gpu_id(),
            observation.unique_id(),
            observation.render_minor(),
            observation.drm().device(),
            device.descriptor_count(),
            device.process_apertures().len()
        );
        drop(device);
        let after = descriptor_count()?;
        if after != before {
            return Err(format!("descriptor count changed from {before} to {after}").into());
        }
    }
    println!(
        "descriptor_cleanup=passed before={before} after={}",
        descriptor_count()?
    );
    Ok(())
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    eprintln!("MI350 device observations require Linux x86_64");
    std::process::exit(2);
}
