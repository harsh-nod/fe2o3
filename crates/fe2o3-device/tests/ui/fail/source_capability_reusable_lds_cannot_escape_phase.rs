use fe2o3_device::{InitialEpoch, WorkgroupCapability};

fn reject<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
) {
    let mut storage = workgroup.allocate_lds::<u32, 64>().into_reusable();
    let mut phases = workgroup.into_reusable();
    let escaped = phases.with_phase(|phase| {
        let lds = phase.bind_reusable_lds(&mut storage);
        let completion = phase.finish_reusable_phase();
        (completion, lds)
    });
    phases.with_phase(|phase| {
        let completion = phase.finish_reusable_phase();
        (completion, ())
    });
    drop(escaped);
}

fn main() {}
