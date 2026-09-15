#![no_std]

use fe2o3_device::{KernelContext, kernel};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn three_phases(mut context: KernelContext<'_>, value: u32) {
    context.with_workgroup(|workgroup| {
        let mut storage = workgroup.allocate_lds::<f32, 64>().into_reusable();
        let mut phases = workgroup.into_reusable();
        let first = phases.with_phase(|phase| {
            let _lease = phase.bind_reusable_lds(&mut storage);
            (phase.finish_reusable_phase(), value)
        });
        let second = phases.with_phase(|phase| {
            let _lease = phase.bind_reusable_lds(&mut storage);
            (phase.finish_reusable_phase(), first)
        });
        let third = phases.with_phase(|phase| {
            let _lease = phase.bind_reusable_lds(&mut storage);
            (phase.finish_reusable_phase(), second)
        });
        if third == 0 {
            fe2o3_device::trap();
        }
    });
}
