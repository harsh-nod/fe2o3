use fe2o3_host::GeneratedArgumentPackingPlanV1;

fn relabel(plan: &GeneratedArgumentPackingPlanV1) {
    let input = plan.scalar_u32(0, 7).unwrap();
    let _ = input.argument_index;
}

fn main() {}
