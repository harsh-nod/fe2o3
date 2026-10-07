use super::super::{CompletionCustodySnapshotV1, CompletionOwnerPhaseV1};
use super::*;

#[derive(Debug, Eq, PartialEq)]
pub(in crate::queue) struct SourceRollbackSnapshot {
    custody: CompletionCustodySnapshotV1,
    phase: CompletionOwnerPhaseV1,
    next_event_id: u64,
    next_reader_lease_id: u64,
    events: HashMap<u64, ExactCompletionOccurrenceV1>,
    readers: HashMap<DependencyReaderUseKeyV1, ActiveDependencyReaderV1>,
}

impl SourceRollbackSnapshot {
    // Independent expected-state oracle: never calls a production cleanup helper.
    pub(in crate::queue) fn expect_terminal_prefix(
        &mut self,
        batch: u64,
        release: bool,
        cancel: bool,
        event_pins: u32,
    ) {
        let target: Vec<_> = self
            .events
            .iter()
            .filter(|(_, row)| row.batch_id == batch)
            .map(|(id, row)| (*id, *row))
            .collect();
        assert_eq!(target.len(), 3);
        assert_eq!(
            target
                .iter()
                .map(|(_, row)| row.slot.index)
                .collect::<HashSet<_>>()
                .len(),
            3
        );
        assert!(!cancel || release);
        for (id, row) in target {
            assert!(row.packet_id.is_none());
            let slot = &mut self.custody.slots[row.slot.index as usize];
            assert_eq!(slot.generation, row.slot.generation);
            assert_eq!(slot.event_pins, event_pins);
            assert_eq!(slot.native_reader_pins, 0);
            if release {
                assert_eq!(event_pins, 1);
                self.events.remove(&id).unwrap();
                slot.event_pins = 0;
            }
            if cancel {
                assert_eq!(slot.phase, CompletionSlotPhaseV1::Bound { batch_id: batch });
                slot.phase = CompletionSlotPhaseV1::Available;
            }
        }
        self.phase = CompletionOwnerPhaseV1::Poisoned;
    }
}

impl CompletionSignalArenaOwnerV1 {
    pub(in crate::queue) fn clear_bound_event_pins_for_test(&mut self, batch: u64) {
        let mut count = 0;
        for slot in self.slots.iter_mut() {
            if slot.phase == (CompletionSlotPhaseV1::Bound { batch_id: batch }) {
                assert_eq!(slot.event_pins, 1);
                assert_eq!(slot.native_reader_pins, 0);
                slot.event_pins = 0;
                count += 1;
            }
        }
        assert_eq!(count, 3);
    }

    pub(in crate::queue) fn source_rollback_snapshot_for_test(&self) -> SourceRollbackSnapshot {
        SourceRollbackSnapshot {
            custody: self.custody_snapshot_for_test(),
            phase: self.phase,
            next_event_id: self.dependency_ledger.next_event_id,
            next_reader_lease_id: self.dependency_ledger.next_reader_lease_id,
            events: self.dependency_ledger.events.clone(),
            readers: self.dependency_ledger.readers.clone(),
        }
    }

    pub(in crate::queue) fn invalidate_last_bound_phase_for_test(&mut self, batch: u64) {
        let targets: Vec<_> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.phase == CompletionSlotPhaseV1::Bound { batch_id: batch })
            .map(|(index, _)| index)
            .collect();
        assert_eq!(targets.len(), 3);
        self.slots[*targets.last().unwrap()].phase =
            CompletionSlotPhaseV1::Published { batch_id: batch };
    }
}
