use fe2o3_host::AuthenticatedServiceQueueSessionV1;

fn escape(queue: AuthenticatedServiceQueueSessionV1<1>) {
    let _raw = queue.into_raw();
}

fn main() {}
