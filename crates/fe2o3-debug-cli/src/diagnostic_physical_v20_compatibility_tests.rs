//! Byte-compatibility controls for the unchanged default V20 route.
use super::*;
#[test]
fn v20_configuration_and_page_keep_the_exact_prior_domain_and_order() {
    let files = Files::new(false, 0, false);
    let (input, mut b) = backend(&files);
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-debug-physical-entry-v20-cpu-config-v1\0");
    hash.update(input.canonical().identity().digest());
    hash.update(
        input
            .canonical()
            .identity()
            .canonical_length()
            .to_le_bytes(),
    );
    hash.update(input.request_digest());
    hash.update((input.request_bytes() as u64).to_le_bytes());
    for n in [
        8192usize,
        65536,
        64,
        4096,
        1 << 29,
        512 * 1024 * 1024,
        8192,
        8192 * 128 + 65536 * 4,
        1024,
        8192 * 1024 * 4 + 65536 * 4 + 65536,
        1,
        768,
        8,
        16384,
    ] {
        hash.update((n as u64).to_le_bytes());
    }
    let l = input.limits();
    for n in [
        l.max_canonical_bytes,
        l.max_reachable_functions,
        l.max_reachable_operations,
        l.max_call_depth,
        l.max_ssa_values,
        l.max_allocations,
        l.max_allocation_bytes,
        l.max_total_bytes,
        l.max_resident_bytes,
        l.max_memory_access_records,
    ] {
        hash.update((n as u64).to_le_bytes());
    }
    for n in [
        l.max_invocations,
        l.max_workgroups,
        l.max_scheduled_slots,
        l.max_steps,
        l.max_events,
    ] {
        hash.update(n.to_le_bytes());
    }
    let identity = OpaqueIdentityV1::new(hash.finalize().into()).unwrap();
    assert_eq!(b.configuration, identity);
    assert_ne!(
        configuration_for(
            Profile::GlobalCopyV21,
            input.canonical().identity().digest(),
            input.canonical().identity().canonical_length(),
            input.request_digest(),
            input.request_bytes(),
            input.limits()
        )
        .unwrap(),
        identity
    );
    let i = (0..b.session.records_len())
        .find(|&i| b.session.record(i).unwrap().binding_count(0).unwrap_or(0) > 1)
        .unwrap();
    seek(&mut b, i as u64 + 1);
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: None,
            limit: 1,
        },
    );
    let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
        panic!("values")
    };
    let DebugResultV1::Values {
        next_cursor: Some(page),
        ..
    } = *result
    else {
        panic!("page")
    };
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-debug-physical-v20-ssa-page-v1\0");
    hash.update(identity.as_bytes());
    hash.update(b.sequence().to_le_bytes());
    hash.update(b.revision.to_le_bytes());
    assert_eq!(
        page.query_identity,
        OpaqueIdentityV1::new(hash.finalize().into()).unwrap()
    );
    assert_eq!(page.position, 1);
}
#[test]
fn v20_created_and_stale_error_jsonl_bytes_are_unchanged() {
    let files = Files::new(false, 0, false);
    let (_input, mut b) = backend(&files);
    let identity = serde_json::to_string(&b.configuration).unwrap();
    let state = r#"{"status":"ok","schema":"fe2o3-debug-response-v1","request_id":1,"operation":"get_state","session":{"backend":"cpu_kir_simulator","execution_kind":"cpu_kir_simulation","state":"created","revision":0,"configuration_identity":"IDENTITY","cursor":{"configuration_identity":"IDENTITY","event_sequence":0,"state_revision":0},"simulated":true,"hardware_observed":false,"performance_prediction":false},"result":{"result":"state","snapshot":{"status":"unavailable","reason":"not_captured"}}}"#;
    let error = r#"{"status":"error","schema":"fe2o3-debug-response-v1","request_id":1,"operation":"get_state","session":{"backend":"cpu_kir_simulator","execution_kind":"cpu_kir_simulation","state":"created","revision":0,"configuration_identity":"IDENTITY","cursor":{"configuration_identity":"IDENTITY","event_sequence":0,"state_revision":0},"simulated":true,"hardware_observed":false,"performance_prediction":false},"error":{"stage":"session","code":"stale_revision","message":"kir_v20_debug_stale_revision","state_changed":false}}"#;
    for (revision, expected) in [(0, state), (1, error)] {
        let request = DebugRequestV1::GetState {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: revision,
        };
        let mut actual = Vec::new();
        protocol::respond(&mut b, request, &mut actual, protocol_limits()).unwrap();
        let expected = expected.replace("\"IDENTITY\"", &identity) + "\n";
        assert_eq!(actual, expected.as_bytes());
        assert_eq!(b.revision, 0);
        assert_eq!(b.session.cursor(), None);
    }
}
