use fe2o3_kernel_ir::{DebugSourceMapErrorV1, DebugSourceMapFileV1};

#[test]
fn public_visitor_observes_original_path_capacity_without_changing_owner() {
    let mut path = String::with_capacity(137);
    path.push_str("kernel.rs");
    let capacity = path.capacity();
    let file = DebugSourceMapFileV1::new([3; 32], 19, path).unwrap();
    let original = file.clone();
    let mut calls = Vec::new();
    file.visit_retained_heap_storage_v1(|count, width| {
        calls.push((count, width));
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(calls, vec![(capacity, size_of::<u8>())]);
    assert!(capacity > file.display_path().len());
    assert_eq!(file, original);
}

#[test]
fn public_visitor_propagates_first_error_without_retry_or_mutation() {
    let file = DebugSourceMapFileV1::new([4; 32], 7, "a.rs".into()).unwrap();
    let original = file.clone();
    let mut calls = 0;
    let error = file.visit_retained_heap_storage_v1(|_, _| {
        calls += 1;
        Err::<(), _>("caller refused")
    });
    assert_eq!(error, Err("caller refused"));
    assert_eq!(calls, 1);
    assert_eq!(file, original);
}

#[test]
fn observation_api_does_not_relax_original_file_admission() {
    for (identity, bytes, path) in [
        ([0; 32], 1, "a.rs".to_owned()),
        ([1; 32], 0, "a.rs".to_owned()),
        ([1; 32], 1, String::new()),
        ([1; 32], 1, "bad\0path".to_owned()),
        ([1; 32], 1, "x".repeat(4097)),
    ] {
        assert_eq!(
            DebugSourceMapFileV1::new(identity, bytes, path),
            Err(DebugSourceMapErrorV1::InvalidFile),
        );
    }
}
