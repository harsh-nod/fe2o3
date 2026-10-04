use fe2o3_host::AuthenticatedServiceQueueSessionV1;

fn destroy_twice(queue: AuthenticatedServiceQueueSessionV1<1>) {
    let _first = queue.destroy_and_release();
    let _second = queue.destroy_and_release();
}

fn main() {}
