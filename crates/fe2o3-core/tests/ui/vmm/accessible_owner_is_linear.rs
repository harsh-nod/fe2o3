use fe2o3_core::VmmAccessibleAllocation;

fn duplicate(owner: VmmAccessibleAllocation) {
    let _copy = owner.clone();
}

fn main() {}
