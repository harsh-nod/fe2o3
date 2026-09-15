use super::*;

impl GuardedBf16SourceEventsV1<'_, '_> {
    /// Lends actual read producer/result identities after replaying the exact
    /// source key on the existing owner/loan/work session. Root materialization
    /// must independently join allocation, physical/scalar inputs and CFG.
    pub(super) fn with_live_final_read_ports<T>(
        &mut self,
        index: usize,
        inspect: impl FnOnce(
            ProductionGlobalBf16SourceRowV1<'_, '_>,
            root_physical::RootPhysicalV1<'_>,
            ProductionScopedBf16LaneUseV1,
            live::read_observations::LiveReadSequence<'_>,
            formula::Charge<'_>,
        ) -> formula::Result<T>,
    ) -> Result<T> {
        let event = self.rows.get(index).ok_or(E::CorrespondenceMismatch)?;
        let row = self
            .inputs
            .batch
            .row(index)
            .ok_or(E::CorrespondenceMismatch)?;
        self.inputs
            .session
            .with_checked_global_bf16_read_source(row, |row, lane, charge| {
                if source_key(row, lane, charge)? != event.key {
                    return Err(E::CorrespondenceMismatch);
                }
                let physical = root_physical::project(row, charge)?;
                if !physical.belongs_to(row) {
                    return Err(E::CorrespondenceMismatch);
                }
                with_formula_budget(charge, |f| {
                    let expected = formula::reference(
                        lane.contract().operand().role == SemanticMfmaOperandRoleV1::B,
                        f,
                    )?;
                    event
                        .graph
                        .with_live_read_sequence(&expected, f, |sequence, f| {
                            inspect(row, physical, lane, sequence, f)
                        })
                })
            })
    }

    #[cfg(test)]
    pub(crate) fn test_live_final_read_ports(&mut self) {
        let mut roots = std::collections::BTreeMap::new();
        let mut execution = None;
        // Actual source-to-live-IR handoff test only. No root memory admission,
        // final arithmetic proof or source Session completion is performed.
        for index in 0..self.inputs.len() {
            let source = self.rows[index].key.clone();
            self.with_live_final_read_ports(index, |row, physical, lane, sequence, charge| {
                assert!(physical.belongs_to(row));
                let role = usize::from(row.contract().operand().role == SemanticMfmaOperandRoleV1::B);
                let origin = (physical.root_argument(), physical.root_local(), physical.root_value());
                if let Some(previous) = roots.insert(role, origin) {
                    assert_eq!(previous, origin, "repeated source role retains its original ABI slice");
                }
                let issuers = [lane.context_value(), lane.workgroup_value(), lane.subgroup_value()];
                if let Some(previous) = execution.replace(issuers) {
                    assert_eq!(previous, issuers, "four actual loads share the original execution issuers");
                }
                assert_eq!(source.lane, lane);
                assert_eq!(source.normal_edge, row.call().destination().unwrap().edge());
                assert_eq!(source.normal_edge.role(), SemanticEdgeRoleV1::CallReturn);
                assert_eq!(source.unwind, row.call().unwind());
                let original = &row.view().body().blocks()[row.load_block() as usize];
                assert_eq!(source.preceding_statements, original.statements().len());
                assert!(matches!(original.terminator().kind(), SemanticTerminatorKindV1::Call(actual)
                    if std::ptr::eq(actual, row.call())));
                assert_eq!(row.contract().operand().wave_width, 64);
                let results = sequence.result_ports(charge)?;
                sequence.check_result_ports(&results, charge)?;
                for ordinal in 0..4 {
                    let event = sequence.observation(ordinal).unwrap();
                    assert_eq!(event.result(), results[ordinal].value());
                    assert_eq!(
                        event.preceding(),
                        ordinal
                            .checked_sub(1)
                            .map(|i| sequence.observation(i).unwrap().producer())
                    );
                }
                assert!(sequence.observation(4).is_none());
                let mut changed = results;
                changed.swap(0, 1);
                assert_eq!(
                    sequence.check_result_ports(&changed, charge),
                    Err(formula::Error::Changed)
                );
                changed[0] = changed[1];
                assert_eq!(
                    sequence.check_result_ports(&changed, charge),
                    Err(formula::Error::Changed)
                );
                assert_eq!(
                    sequence.check_result_ports(&results[..3], charge),
                    Err(formula::Error::Roster)
                );
                Ok(())
            })
            .expect("original physical ABI slice, execution issuers, CFG and volatile read/result ports");
        }
        assert_eq!(roots.len(), 2);
        assert_ne!(roots[&0].0, roots[&1].0, "A/B cannot substitute their physical ABI inputs");
        assert_ne!(roots[&0].2, roots[&1].2, "A/B retain different original entry SSA values");
    }
}
