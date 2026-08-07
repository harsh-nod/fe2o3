use fe2o3_core::VmmAccessibleAllocation;

fn expose_address(owner: &VmmAccessibleAllocation) {
    let _pointer = owner.raw_pointer();
}

fn main() {}
