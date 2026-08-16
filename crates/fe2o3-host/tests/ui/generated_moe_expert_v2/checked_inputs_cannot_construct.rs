use fe2o3_host::CheckedMoeCompletedRoutingExpertInputsV2;

fn value<T>() -> T {
    panic!()
}

fn forge() -> CheckedMoeCompletedRoutingExpertInputsV2 {
    CheckedMoeCompletedRoutingExpertInputsV2 {
        readback: value(),
        route_weights: [0.5; 16],
        packed_activation_tiles: [0; 1024],
        input_transcript_sha256: [0; 32],
    }
}

fn main() {}
