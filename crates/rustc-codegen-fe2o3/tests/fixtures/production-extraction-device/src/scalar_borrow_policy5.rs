use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(typed)]
pub fn scalar_borrow_policy5(mut output: DisjointSlice<u32>, input: u32) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        let mut local = input;
        let reference = &mut local;
        // This independently observable global store separates initialization
        // from both reads. It is not an alias of the private scalar allocation.
        *element = input;
        let first = *reference;
        #[cfg(feature = "scalar-borrow-policy5-barrier")]
        {
            *element = input;
        }
        let second = *reference;
        *element = if first == second { 7 } else { 11 };
    }
}
