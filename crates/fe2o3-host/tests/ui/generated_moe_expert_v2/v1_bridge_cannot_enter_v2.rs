use fe2o3_core::DeviceBufferViewMut;
use fe2o3_host::{
    GeneratedMoeExpertV2HostAdapterV2, MoeExpertWeightArtifactBindingV2,
    MoeHostObservedRoutingExpertBridgeV1, ObservedContext,
};

fn reject<'a, 'b, 'c, 'd, 'e, 'f>(
    observed: &ObservedContext,
    routing: MoeHostObservedRoutingExpertBridgeV1<'a, 'b>,
    weights: MoeExpertWeightArtifactBindingV2<'c>,
    expert_output: DeviceBufferViewMut<'d, f32>,
    compact_output: DeviceBufferViewMut<'e, f32>,
    combined_output: DeviceBufferViewMut<'f, f32>,
) {
    let _ = GeneratedMoeExpertV2HostAdapterV2::prepare(
        observed,
        routing,
        weights,
        expert_output,
        compact_output,
        combined_output,
    );
}

fn main() {}
