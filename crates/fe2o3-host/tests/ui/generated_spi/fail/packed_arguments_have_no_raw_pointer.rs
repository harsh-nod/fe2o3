use fe2o3_host::GeneratedArgumentPackingPlanV1;

fn extract_pointer(plan: &GeneratedArgumentPackingPlanV1) {
    let input = plan.scalar_u32(0, 7).unwrap();
    let packed = plan.pack([input]).unwrap();
    let _ = packed.as_ptr();
}

fn main() {}
