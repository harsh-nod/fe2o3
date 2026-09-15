#![no_std]

use fe2o3_device::{KernelContext, kernel};

#[allow(dead_code)]
struct ReusableWorkgroupBrand<T>(core::marker::PhantomData<T>);

#[kernel(typed, launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [1, 1, 1]))]
pub fn reusable_phase_import(mut context: KernelContext<'_>, value: f32) {
    let _value = context.with_workgroup(|workgroup| {
        let mut storage = workgroup.allocate_lds::<[f32; 2], 256>().into_reusable();
        let mut phases = workgroup.into_reusable();
        let first = phases.with_phase(|phase| {
            let values = phase.bind_reusable_lds(&mut storage);
            let values = values.initialize_by_invocation(&phase, [value, 0.0]);
            let (phase, values) = phase.publish_lds(values);
            let result = values.read(&phase, 0).unwrap_or([0.0; 2]);
            (phase.finish_reusable_phase(), result[0])
        });
        phases.with_phase(|phase| {
            let values = phase.bind_reusable_lds(&mut storage);
            let values = values.initialize_by_invocation(&phase, [first, value]);
            let (phase, values) = phase.publish_lds(values);
            let result = values.read(&phase, 1).unwrap_or([0.0; 2]);
            (phase.finish_reusable_phase(), result[1])
        })
    });
}
