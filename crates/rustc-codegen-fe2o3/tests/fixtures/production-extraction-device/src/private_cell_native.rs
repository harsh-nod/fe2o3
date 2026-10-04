use fe2o3_device::{DisjointSlice, kernel, thread};

fn reference(_point: usize, output: &mut u32, _input: u32) {
    *output = 7;
}

#[cfg(feature = "private-cell-native-unitlocal")]
#[inline(never)]
fn private_unit_helper() {
    let index = 0_usize;
    let mut values = [7_u32, 11_u32];
    values[index] = 13;
    let _observed = values[index];
}

#[kernel(
    typed,
    reference = reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn private_cell_native(mut output: DisjointSlice<u32>, _input: u32) {
    #[cfg(feature = "private-cell-native-unitlocal")]
    private_unit_helper();
    if let Some(element) = output.get_mut(thread::index_1d()) {
        #[cfg(not(feature = "private-cell-native-noop"))]
        {
            let mut local = _input;
            let reference = &mut local;
            *reference = _input;
            let first = *reference;
            *reference = first;
            let _second = *reference;
        }
        *element = 7;
    }
}
