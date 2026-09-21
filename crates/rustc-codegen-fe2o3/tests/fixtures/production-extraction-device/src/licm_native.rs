use fe2o3_device::{DisjointSlice, kernel, thread};

fn reference(_point: usize, output: &mut u32, _control: u32, _bound: u64) {
    *output = 7;
}

#[cfg(feature = "licm-native-unitlocal")]
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
pub fn licm_native(mut output: DisjointSlice<u32>, _control: u32, _bound: u64) {
    #[cfg(feature = "licm-native-unitlocal")]
    private_unit_helper();
    if let Some(element) = output.get_mut(thread::index_1d()) {
        #[cfg(not(feature = "licm-native-noop"))]
        {
            let mut local = 0_u32;
            let cell = &mut local;
            let mut counter = 0_u64;
            while counter < _bound {
                let masked = _control & 1;
                *cell = 99;
                if masked == 0 {
                    let observed = *cell;
                    *cell = observed;
                }
                counter += 1;
            }
        }
        *element = 7;
    }
}
