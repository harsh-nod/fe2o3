// Complete source inputs from the full Session. No constructor, lane, bounds,
// observed-read, or numerical proof is manufactured by this capture step.
pub(super) struct GlobalBf16SourceBatchV1<'a> {
    rows: Vec<GlobalBf16SourceBatchRowV1<'a>>,
}

pub(super) struct GlobalBf16SourceBatchRowV1<'a> {
    inputs: GlobalBf16SourceUseV1<'a>,
    frame: GlobalBf16ConstructorFrameV1<'a>,
}

impl<'a> GlobalBf16SourceBatchRowV1<'a> {
    pub(super) fn inputs(&self) -> &GlobalBf16SourceUseV1<'a> {
        &self.inputs
    }
    pub(super) fn frame(&self) -> &GlobalBf16ConstructorFrameV1<'a> {
        &self.frame
    }
}

impl<'a> GlobalBf16SourceBatchV1<'a> {
    pub(super) fn rows(&self) -> &[GlobalBf16SourceBatchRowV1<'a>] {
        &self.rows
    }
}

fn source_batch_storage_words(count: usize) -> Option<usize> {
    count
        .checked_mul(std::mem::size_of::<GlobalBf16SourceBatchRowV1<'_>>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<Vec<GlobalBf16SourceBatchRowV1<'_>>>())
        })
        .map(|bytes| bytes.div_ceil(std::mem::size_of::<usize>()))
}

impl<'a> ProductionScopedMatrixSourceSessionV1<'a> {
    pub(super) fn capture_global_bf16_inputs(
        &mut self,
    ) -> Result<GlobalBf16SourceBatchV1<'a>, ProductionSemanticKirErrorV1> {
        self.with_bf16_source_graph(|owner, view, context, graph, expected| {
            // The actual reachable source census is owned by Session. There is
            // no caller-supplied list whose omissions could hide a read family.
            let words = source_batch_storage_words(expected.len())
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            graph.charge(words)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(expected.len())
                .map_err(|_| reject("global BF16 source batch reservation failed"))?;
            if rows.capacity() != expected.len() {
                return Err(reject("global BF16 source batch capacity changed"));
            }
            // Reuse the same graph/cache and compare the retained SSA plan
            // once, before this batch, rather than once per terminal.
            let mut query = GlobalBf16SourceQueryV1::new(owner, view.root(), context, graph)?;
            for &block in expected {
                query.graph.charge(1)?;
                let inputs = query.capture(block)?;
                let frame = inputs.constructor_frame(&mut query)?;
                rows.push(GlobalBf16SourceBatchRowV1 { inputs, frame });
            }
            Ok(GlobalBf16SourceBatchV1 { rows })
        })
    }
}

#[cfg(test)]
mod source_session_storage_tests {
    use super::*;

    #[test]
    fn global_bf16_batch_charges_row_capacity_and_header_without_overflow() {
        let word = std::mem::size_of::<usize>();
        let row = std::mem::size_of::<GlobalBf16SourceBatchRowV1<'_>>();
        let header = std::mem::size_of::<Vec<GlobalBf16SourceBatchRowV1<'_>>>();
        for count in [0, 1, 4, 1024] {
            assert_eq!(
                source_batch_storage_words(count),
                Some((header + count * row).div_ceil(word))
            );
        }
        assert_eq!(source_batch_storage_words(usize::MAX / row + 1), None);
        assert_eq!(source_batch_storage_words(usize::MAX), None);
    }
}
