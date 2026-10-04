use fe2o3_kfd::OpenedKfd;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kfd = OpenedKfd::open_default()?;
    let observed = kfd.observe_uapi()?;
    println!(
        "observed_kfd_uapi={}.{}",
        observed.reported_version().major,
        observed.reported_version().minor
    );

    let admitted = kfd.admit_uapi()?;
    println!("admitted_schema={}", admitted.uapi_identity().schema_id());
    Ok(())
}
