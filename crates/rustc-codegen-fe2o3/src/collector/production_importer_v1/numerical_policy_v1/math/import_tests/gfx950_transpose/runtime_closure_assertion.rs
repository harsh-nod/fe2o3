pub(super) fn assert_runtime_read_closure(
    closure: &crate::collector::AuthenticatedCollectedKernelClosureV1<'_>,
) {
    use crate::closure_profile_v1::{
        ClosureCaptureModeV1, ClosureCustodyV1, HigherOrderCapabilityTerminalV1,
    };
    let mut matrix_calls = 0;
    let mut runtime_read_environments = 0;
    for plan in closure
        .collection
        .functions
        .iter()
        .filter_map(|function| function.closure_plan.as_ref())
    {
        for call in plan
            .higher_order_calls()
            .iter()
            .filter(|call| call.terminal == HigherOrderCapabilityTerminalV1::WithMatrix)
        {
            matrix_calls += 1;
            let ClosureCustodyV1::EnvironmentLocal(local) = call.closure_custody else {
                panic!("both transpose closures require original captured environment custody");
            };
            let environment = plan
                .environments()
                .iter()
                .find(|environment| environment.local == local)
                .expect("the higher-order call must name its retained environment");
            assert!(environment.size_bytes > 0);
            if environment.size_bytes == 4
                && environment.captures.len() == 2
                && environment
                    .captures
                    .iter()
                    .all(|capture| capture.mode == ClosureCaptureModeV1::ByValue)
            {
                let mut sizes = environment
                    .captures
                    .iter()
                    .map(|capture| capture.layout.size_bytes)
                    .collect::<Vec<_>>();
                sizes.sort_unstable();
                assert_eq!(sizes, [0, 4]);
                runtime_read_environments += 1;
            }
        }
    }
    assert_eq!(
        matrix_calls, 2,
        "stage and published-read closure calls stay retained"
    );
    assert_eq!(
        runtime_read_environments, 1,
        "published ZST plus the actual runtime read selector"
    );
}
