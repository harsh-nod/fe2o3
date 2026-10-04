use fe2o3_device::{DisjointSlice, kernel, thread};

#[inline(never)]
fn private_unit_helper() {
    let index = 0_usize;
    let mut values = [7_u32, 11_u32];
    values[index] = 13;
    let _observed = values[index];
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn dominance_and(mut output: DisjointSlice<u32>, lhs: u32, rhs: u32, choose: u32) {
    private_unit_helper();
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = lhs & rhs;
        if choose != 0 {
            let quotient = choose / choose;
            if quotient != 0 {
                *element = rhs & lhs;
            }
        }
    }
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn dominance_or(mut output: DisjointSlice<u32>, lhs: u32, rhs: u32, choose: u32) {
    private_unit_helper();
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = lhs | rhs;
        if choose != 0 {
            let quotient = choose / choose;
            if quotient != 0 {
                *element = rhs | lhs;
            }
        }
    }
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn dominance_xor(mut output: DisjointSlice<u32>, lhs: u32, rhs: u32, choose: u32) {
    private_unit_helper();
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = lhs ^ rhs;
        if choose != 0 {
            let quotient = choose / choose;
            if quotient != 0 {
                *element = rhs ^ lhs;
            }
        }
    }
}
