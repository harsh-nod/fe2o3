#[test]
fn retired_worker_v1_route_cannot_reenter_the_public_surface() {
    let public_root = include_str!("../src/lib.rs");
    let protocol = include_str!("../src/worker_protocol.rs");
    let construction = include_str!("../src/request_construction.rs");
    let executor = include_str!("../src/worker_executor.rs");

    for retired in [
        "WorkerRequestV1",
        "WorkerResponseV1",
        "WorkerOutputV1",
        "WorkerEvidenceClassV1",
        "InertWorkerExecutionV1",
        "WORKER_REQUEST_MAGIC_V1",
        "WORKER_RESPONSE_MAGIC_V1",
        "construct_worker_request_v1",
    ] {
        assert!(
            !public_root.contains(retired),
            "public root reintroduced retired Worker V1 item {retired}"
        );
    }

    assert!(!protocol.contains("pub struct WorkerRequestV1"));
    assert!(!protocol.contains("pub struct WorkerResponseV1"));
    assert!(!protocol.contains("pub struct WorkerOutputV1"));
    assert!(!protocol.contains("F3LREQ01"));
    assert!(!protocol.contains("F3LRSP01"));
    assert!(!construction.contains("construct_worker_request_v1"));
    assert!(!executor.contains("InertWorkerExecutionV1"));
    assert!(!executor.contains("WorkerRequestV1"));
}
