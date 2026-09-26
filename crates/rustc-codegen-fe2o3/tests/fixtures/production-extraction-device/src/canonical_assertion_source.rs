use fe2o3_device::kernel;

#[cfg(fe2o3_canonical_assertion_retained)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_retained(value: u64) {
    let first = value + 0_u64;
    let _last = first * 1_u64;
}

#[cfg(fe2o3_canonical_assertion_literal)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_literal(value: u64) {
    let _shifted = value << 3_i32;
}

#[cfg(fe2o3_canonical_assertion_masked)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_masked(value: u64, count: u32, tag: u32) {
    match tag {
        7 => {
            let _shifted = value << (count & 63_u32);
        }
        _ => {}
    }
}

#[cfg(fe2o3_canonical_assertion_shared)]
#[inline(never)]
fn shared_literal(value: u64) -> u64 {
    value << 3_i32
}

#[cfg(fe2o3_canonical_assertion_shared)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_shared_left(value: u64) {
    let _shifted = shared_literal(value);
}

#[cfg(fe2o3_canonical_assertion_shared)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_shared_right(value: u64) {
    let _shifted = shared_literal(value);
}

#[cfg(fe2o3_canonical_assertion_private)]
#[inline(never)]
fn private_unit_helper() {
    let index = 0_usize;
    let mut values = [7_u32, 11_u32];
    values[index] = 13;
    let _observed = values[index];
}

#[cfg(fe2o3_canonical_assertion_private)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_private_left(value: u64) {
    let _checked = value + 0_u64;
    private_unit_helper();
}

#[cfg(fe2o3_canonical_assertion_private)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_private_right(value: u64) {
    let _checked = value + 0_u64;
    private_unit_helper();
}

#[cfg(fe2o3_canonical_assertion_history_retained)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assertion_history_retained(value: u64) {
    let _next = (value & 255_u64) + 1_u64;
}
