use fe2o3_core::VmmMappedAllocation;

fn expose_address(owner: &VmmMappedAllocation) {
    let _pointer = unsafe { owner.raw_pointer() };
}

fn main() {}
