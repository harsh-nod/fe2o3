use fe2o3_device::WriteOnlyDisjointSlice;

fn require_copy<T: Copy>() {}

fn main() {
    require_copy::<WriteOnlyDisjointSlice<u32>>();
}
