use fe2o3_host::{AllocationProvenance, ObservedContext};

fn declare<'a>(context: &ObservedContext, owner: &'a ()) -> AllocationProvenance<'a> {
    AllocationProvenance::from_raw_parts(context, owner, 0x1000 as *mut u8, 64).unwrap()
}

fn main() {}
