use fe2o3_device::kernel;

#[cfg(any(
    all(fe2o3_private_call_shared, fe2o3_private_call_cross_block),
    all(fe2o3_private_call_shared, fe2o3_private_call_normal),
    all(fe2o3_private_call_cross_block, fe2o3_private_call_normal)
))]
compile_error!("private/call source cases must be mutually exclusive");

#[inline(never)]
fn typed_pair(value: u64, mask: u64) -> u64 {
    value ^ mask
}

#[cfg(fe2o3_private_call_shared)]
#[inline(never)]
fn private_cell() {
    let mut values = [7_u64, 11_u64];
    let index = 0_usize;
    values[index] = 13;
    let _seen = values[index];
}

#[cfg(fe2o3_private_call_shared)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_call_first(value: u64) {
    let value = value ^ 0;
    let _answer = typed_pair(value, value);
    private_cell();
    private_cell();
}

#[cfg(fe2o3_private_call_shared)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_call_second(value: u64) {
    let value = value ^ 0;
    private_cell();
    let _answer = typed_pair(value, value);
}

#[cfg(fe2o3_private_call_cross_block)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_call_cross_block(value: u64, choose: u32) {
    let mut values = [7_u64, 11_u64];
    let index = 0_usize;
    values[index] = 13;
    if choose == 0 {
        let _left = typed_pair(value, value);
    } else {
        let _right = typed_pair(value, value);
    }
    let loaded = values[index];
    let _answer = typed_pair(loaded, value ^ 0);
}

#[cfg(fe2o3_private_call_normal)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_call_normal(value: u64) {
    let _answer = typed_pair(value, value);
}
