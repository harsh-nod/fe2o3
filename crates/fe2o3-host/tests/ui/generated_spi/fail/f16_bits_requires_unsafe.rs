use fe2o3_host::GeneratedArgumentPackingPlanV1;

fn rejected(plan: &GeneratedArgumentPackingPlanV1) {
    let _ = plan.scalar_f16_bits(0, 0x3e00);
}

fn main() {}
