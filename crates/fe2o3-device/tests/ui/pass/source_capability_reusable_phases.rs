use fe2o3_device::{InitialEpoch, WorkgroupCapability};

fn dynamic_k_phases<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    phase_count: usize,
) {
    let mut a_lds = workgroup.allocate_lds::<u32, 64>().into_reusable();
    let mut b_lds = workgroup.allocate_lds::<u32, 64>().into_reusable();
    let mut phases = workgroup.into_reusable();

    let mut phase_index = 0;
    while phase_index < phase_count {
        phases.with_phase(|phase| {
            let a = phase.bind_reusable_lds(&mut a_lds);
            let b = phase.bind_reusable_lds(&mut b_lds);
            let a = a.initialize_by_invocation(&phase, phase_index as u32);
            let b = b.initialize_by_invocation(&phase, phase_index as u32);
            let (phase, a, b) = phase.publish_lds_pair(a, b);
            let _ = a.read(&phase, 0);
            let _ = b.read(&phase, 0);
            let completion = phase.finish_reusable_phase();
            (completion, ())
        });
        phase_index += 1;
    }
}

fn main() {
    let _ = dynamic_k_phases::<()>;
}
