use fe2o3_host::CheckedMoeCompletedRoutingReadbackV2;

fn value<T>() -> T {
    panic!()
}

fn forge() -> CheckedMoeCompletedRoutingReadbackV2 {
    CheckedMoeCompletedRoutingReadbackV2 {
        checked: value(),
        routing: value(),
        batch: value(),
        dispatch_context: value(),
        dispatch_stream: value(),
        lifecycle_transcript_sha256: [0; 32],
    }
}

fn main() {}
