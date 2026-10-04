use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(typed)]
pub fn shared_primitive_policy5(mut output: DisjointSlice<u32>, input: u32) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        let local = input;
        let reference = &local;
        #[cfg(feature = "shared-primitive-policy5-field1")]
        let value = {
            let holder = (23_u32, reference);
            *holder.1
        };
        #[cfg(not(feature = "shared-primitive-policy5-field1"))]
        let value = *reference;
        *element = value;
    }
}
