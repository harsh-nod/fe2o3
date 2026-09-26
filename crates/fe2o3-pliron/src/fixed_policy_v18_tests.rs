use super::*;
use crate::neutral_optimization_v1::storage_v18::tests::{SPACE, WORK, fixture, input, observe};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn actual_v18_execution_frame_binds_table_endpoints_fixed_passes_and_observed_epochs() {
    let input = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(19 + input.storage).unwrap();
    let output = observe(&input, &mut budget);
    let bytes = output.execution().canonical_bytes();
    assert_eq!(bytes.len(), 816);
    assert_eq!(&bytes[..8], &[3, 0, 1, 0, 8, 0, 18, 0]);
    assert_eq!(&bytes[8..40], input.owner.identity().digest());
    assert_eq!(&bytes[48..80], output.owner().identity().digest());
    let table = input
        .owner
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    assert_eq!(&bytes[88..120], table.digest());
    assert_eq!(
        u64::from_le_bytes(bytes[120..128].try_into().unwrap()),
        table.encoded_length()
    );
    assert_eq!(&bytes[304..336], output.map().digest());
    for ((row, tag), report) in bytes[336..]
        .chunks_exact(60)
        .zip([2, 5, 3, 1, 4, 6, 1, 5])
        .zip(output.report().passes())
    {
        assert_eq!(row[0], tag);
        assert_eq!(row[1], u8::from(report.changed()));
        assert_eq!(&row[2..4], &[0, 0]);
        assert_eq!(
            u64::from_le_bytes(row[28..36].try_into().unwrap()),
            report.input_epoch().sequence()
        );
        assert_eq!(
            u64::from_le_bytes(row[36..44].try_into().unwrap()),
            report.output_epoch().sequence()
        );
    }
    assert!(!output.execution().grants_authority());
}
