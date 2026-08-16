use fe2o3_host::MoeCompletedRoutingExpertBridgeV2;

fn escape(value: MoeCompletedRoutingExpertBridgeV2<'_, '_, '_, '_>) {
    let _ = value.compile();
    let _ = value.finalize();
    let _ = value.load();
    let _ = value.dispatch();
}

fn main() {}
