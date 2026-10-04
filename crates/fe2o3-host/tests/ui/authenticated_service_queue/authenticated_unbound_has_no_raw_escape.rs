use fe2o3_host::AuthenticatedServiceQueueUnboundSessionV1;

fn escape(queue: AuthenticatedServiceQueueUnboundSessionV1) {
    let _raw = queue.into_raw();
}

fn main() {}
