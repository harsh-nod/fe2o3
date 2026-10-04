use fe2o3_core::VmmMappedAllocation;

fn duplicate(owner: VmmMappedAllocation) {
    let _copy = owner.clone();
}

fn main() {}
