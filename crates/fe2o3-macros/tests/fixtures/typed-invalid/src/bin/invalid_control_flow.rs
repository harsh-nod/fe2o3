use gpu_device::kernel;

#[kernel]
fn undeclared_loop() {
    loop {
        break;
    }
}

#[kernel(control_flow(loop_bounds(0)))]
fn zero_bound() {}

#[kernel(control_flow(loop_bounds(4, 8)))]
fn mismatched_bounds() {
    while false {}
}

#[kernel(control_flow(integer_switches(usize)))]
fn target_dependent_switch(value: usize) {
    match value {
        _ => {}
    }
}

#[kernel(control_flow(integer_switches(u32)))]
fn non_integer_switch(value: Option<u32>) {
    match value {
        Some(_) => {}
        None => {}
    }
}

#[kernel(control_flow(integer_switches(u32)))]
fn range_switch(value: u32) {
    match value {
        0..=3 => {}
        _ => {}
    }
}

#[kernel(control_flow(integer_switches(u32)))]
fn guarded_switch(value: u32) {
    match value {
        candidate if candidate == 0 => {}
        _ => {}
    }
}

#[kernel(control_flow(loop_bounds(1)))]
fn valued_break() {
    let _ = loop {
        break 1_u32;
    };
}

#[kernel(
    control_flow(loop_bounds(1)),
    unsafe_asm(
        target = "gfx942",
        operands(sgpr),
        options(nomem),
        effects(control_flow)
    )
)]
unsafe fn opaque_control_flow_assembly() {
    loop {
        break;
    }
}

fn main() {}
