// Frozen from 75dc08e4b2092e632951989c55451c06703b1df4 for differential tests.
impl ContextQueuedWriterJournalV1 {
    pub(super) fn resolve_reads_baseline(&mut self, root: Root, status: ContextProducerReadStatusV1) {
        let mut next = root.read_head;
        for _ in 0..root.read_count {
            let entry = self.queued_reads[next.expect("validated read list")]
                .as_mut()
                .expect("validated read");
            next = entry.next;
            entry.status = status;
            if status == ContextProducerReadStatusV1::Success {
                let state = self
                    .inner
                    .lookup_allocation(entry.request.allocation.allocation)
                    .expect("settled allocation retained by read");
                entry.version = Some((state.attempt_epoch, state.content_lineage));
            }
            entry.previous = None;
            entry.next = None;
        }
        let root = self.roots[root.writer.slot]
            .as_mut()
            .expect("retained outer root");
        root.read_head = None;
        root.read_count = 0;
    }
}
