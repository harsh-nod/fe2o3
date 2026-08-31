use fe2o3_device::WriteOnlyDisjointSlice;

fn escape(output: &WriteOnlyDisjointSlice<u32>) -> *mut u32 {
    output.ptr
}

fn main() {}
