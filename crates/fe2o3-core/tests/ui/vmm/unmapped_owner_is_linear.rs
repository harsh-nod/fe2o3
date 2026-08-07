use fe2o3_core::VmmUnmappedAllocation;

fn duplicate(owner: VmmUnmappedAllocation) {
    let _copy = owner.clone();
}

fn main() {}
