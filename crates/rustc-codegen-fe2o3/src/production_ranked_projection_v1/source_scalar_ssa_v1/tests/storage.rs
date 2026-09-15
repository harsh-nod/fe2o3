use super::*;

#[test]
fn scalar_source_storage_precharges_retained_and_replacement_capacity() {
    let mut values = Vec::<(u32, u32)>::new();
    let mut work = 0;
    let mut expected = 0;
    for count in 1..=8 {
        expected += (2 * count * std::mem::size_of::<(u32, u32)>()
            + 2 * std::mem::size_of::<Vec<(u32, u32)>>())
        .div_ceil(std::mem::size_of::<usize>());
        reserve_slot(&mut values, &mut work).unwrap();
        assert_eq!(values.capacity(), count);
        assert_eq!(work, expected);
        values.push((count as u32, 0));
    }
    values.clear();
    reserve_slot(&mut values, &mut work).unwrap();
    assert_eq!(work, expected);
    assert_eq!(values.capacity(), 8);
}

#[test]
fn scalar_source_storage_exhaustion_does_not_allocate_or_refund_work() {
    let limit = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
    let mut values = Vec::<(u32, u32)>::new();
    let mut work = limit - 1;
    assert_eq!(
        reserve_slot(&mut values, &mut work),
        Err("GPU semantic expression exceeds its bounded node budget")
    );
    assert_eq!(values.capacity(), 0);
    assert_eq!(work, limit + 1);
}

#[test]
fn scalar_source_storage_lookup_charges_each_examined_value() {
    let values = [(1, 2), (3, 4), (5, 6)];
    let mut work = 0;
    assert!(contains(&values, &(3, 4), &mut work).unwrap());
    assert_eq!(work, 2);
    assert!(!contains(&values, &(9, 10), &mut work).unwrap());
    assert_eq!(work, 5);
    let limit = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
    let mut work = limit - 1;
    assert_eq!(
        contains(&values, &(3, 4), &mut work),
        Err("GPU semantic expression exceeds its bounded node budget")
    );
    assert_eq!(work, limit + 1);
}
