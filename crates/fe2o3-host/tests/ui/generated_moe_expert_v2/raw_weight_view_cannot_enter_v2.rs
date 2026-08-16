use fe2o3_core::{DeviceBufferView, DeviceBufferViewMut};
use fe2o3_host::{
    GeneratedMoeExpertV2HostAdapterV2, MoeCompletedRoutingExpertBridgeV2, ObservedContext,
};

fn reject<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h>(
    observed: &ObservedContext,
    routing: MoeCompletedRoutingExpertBridgeV2<'a, 'b, 'c, 'd>,
    raw_weights: DeviceBufferView<'e, u16>,
    expert_output: DeviceBufferViewMut<'f, f32>,
    compact_output: DeviceBufferViewMut<'g, f32>,
    combined_output: DeviceBufferViewMut<'h, f32>,
) {
    let _ = GeneratedMoeExpertV2HostAdapterV2::prepare(
        observed,
        routing,
        raw_weights,
        expert_output,
        compact_output,
        combined_output,
    );
}

fn main() {}
