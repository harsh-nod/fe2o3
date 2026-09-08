use fe2o3_device::{InitialEpoch, WorkgroupCapability};

fn reject<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
) {
    let mut storage = workgroup.allocate_lds::<u32, 64>().into_reusable();
    let mut phases = workgroup.into_reusable();

    phases.with_phase(|phase| {
        let first = phase.bind_reusable_lds(&mut storage);
        let second = phase.bind_reusable_lds(&mut storage);
        drop((first, second));
        let completion = phase.finish_reusable_phase();
        (completion, ())
    });
}

fn main() {}
