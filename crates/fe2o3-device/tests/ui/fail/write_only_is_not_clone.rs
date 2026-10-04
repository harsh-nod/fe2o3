use fe2o3_device::WriteOnlyDisjointSlice;

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<WriteOnlyDisjointSlice<u32>>();
}
