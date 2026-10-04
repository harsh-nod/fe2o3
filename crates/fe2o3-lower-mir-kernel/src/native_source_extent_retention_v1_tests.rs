use super::*;
use crate::{ProductionRankedAccessSourceV1, ProductionRankedOutputExtentSourceV1};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_pliron::{ProductionRankedValueIdV1, ProductionRankedValueV1};

#[test]
fn inline_extent_copy_uses_actual_row_size_and_restores_caller_floor() {
    let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
    let proposal = ProductionRankedOutputExtentSourceV1::new(
        7,
        local(4),
        ProductionRankedValueV1::Argument(0),
        local(3),
    );
    let row = ProductionRankedAccessSourceV1::new(9, Some(2), 0, 3, 1).with_output_extent(proposal);
    let bytes = std::mem::size_of::<ProductionRankedAccessSourceV1>();
    for (capacity, succeeds) in [(bytes - 1, false), (bytes, true)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, 37 + capacity);
        budget.reserve_storage(37).unwrap();
        budget.charge_work(17).unwrap();
        let result = native_source_transfer_v1(&mut budget, |budget| {
            let mut rows = native_source_vector_v1(1, budget)?;
            budget.charge_work(1)?;
            rows.extend_from_slice(&[row]);
            assert_eq!(rows[0].output_extent(), Some(proposal));
            assert_eq!(native_source_vector_capacity_v1(&rows)?, bytes);
            Ok(rows)
        });
        if succeeds {
            let rows = result.unwrap();
            assert_eq!(rows[0], row);
            assert_eq!(budget.work(), 18);
            assert_eq!(budget.peak_storage(), 37 + bytes);
        } else {
            assert!(matches!(
                result,
                Err(NativeSourceReplayErrorV1::Resource(_))
            ));
            assert_eq!(budget.work(), 17);
        }
        assert_eq!(budget.storage(), 37);
    }
}
