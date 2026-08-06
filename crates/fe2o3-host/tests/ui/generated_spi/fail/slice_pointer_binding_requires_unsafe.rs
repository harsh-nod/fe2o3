use fe2o3_artifacts::{Access, AddressSpace, PointerWidth};
use fe2o3_host::GeneratedArgumentPackingPlanV1;

fn bind_pointer(plan: &GeneratedArgumentPackingPlanV1) {
    let _ = plan.slice(
        0,
        core::ptr::null(),
        0,
        PointerWidth::Bits64,
        AddressSpace::Global,
        Access::ReadOnly,
    );
}

fn main() {}
