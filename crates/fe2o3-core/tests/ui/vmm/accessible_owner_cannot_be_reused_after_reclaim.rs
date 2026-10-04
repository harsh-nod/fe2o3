use fe2o3_core::VmmAccessibleAllocation;

fn reclaim_twice(owner: VmmAccessibleAllocation) {
    let _first = owner.reclaim();
    let _second = owner.reclaim();
}

fn main() {}
