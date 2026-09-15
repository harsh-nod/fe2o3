//! Observe an identical production scan before rollback; no alternate admission.
use super::*;

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(in super::super::super) fn optional_uncommitted_scan_for_test_v1(
        &mut self,
        uses: &ProjectedGlobalSemanticUsesV1,
        allocations: &[Option<AllocationContractV1>],
        extents: &mut [Option<u32>],
    ) -> Result<Vec<Option<ProjectedDeterministicSwitchV1>>, ProductionRankedProjectionErrorV1>
    {
        assert!(self.optional_source.is_none());
        assert!(matches!(
            self.roots,
            TotalUnsignedIndexRootsV1::Invocation { .. }
        ));
        assert!(!uses.source_unsigned_comparisons.is_empty());
        // Match only the wrapper's checkpoint setup. The scanner, emissions,
        // journal, and every work charge are the unmodified production methods.
        self.assertion_proofs.charge(
            std::mem::size_of::<OptionalSourceComparisonV1>()
                .div_ceil(std::mem::size_of::<usize>())
                + self.states.capacity()
                + 3,
        )?;
        self.optional_source = Some(OptionalSourceComparisonV1 {
            exhausted: false,
            extent_writes: Vec::new(),
            site: None,
        });
        let result = self.source_unsigned_switches_v1(uses, allocations, extents);
        let state = self.optional_source.take().unwrap();
        assert!(
            !state.exhausted,
            "this observation tests shared work, not local precision"
        );
        assert_eq!(state.extent_writes, [0]);
        assert_eq!(
            extents[0],
            Some(9),
            "a fresh extent was published before the failure"
        );
        result
    }
}
