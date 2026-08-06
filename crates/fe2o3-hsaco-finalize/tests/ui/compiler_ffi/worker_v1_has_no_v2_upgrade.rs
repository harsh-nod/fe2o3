use fe2o3_hsaco_finalize::{WorkerRequestV1, WorkerRequestV2};

fn main() {
    fn upgrade(request: WorkerRequestV1) -> WorkerRequestV2 {
        request.into()
    }
}
