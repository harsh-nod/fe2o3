use fe2o3_host::{
    GeneratedArgumentPackingPlanV1, GeneratedReadDeviceSlice,
    __generated::GeneratedAlphaZetaCov6ArgumentBindingV1,
};

fn escape<'allocation>(
    plan: &GeneratedArgumentPackingPlanV1,
    slice: &GeneratedReadDeviceSlice<'allocation, u32>,
) -> GeneratedAlphaZetaCov6ArgumentBindingV1<'static> {
    let pair = slice.bind_argument_pair(plan, 0).unwrap();
    unsafe {
        GeneratedAlphaZetaCov6ArgumentBindingV1::from_compiler_generated_parts_v1(
            Vec::new(),
            vec![pair],
        )
    }
}

fn main() {}
