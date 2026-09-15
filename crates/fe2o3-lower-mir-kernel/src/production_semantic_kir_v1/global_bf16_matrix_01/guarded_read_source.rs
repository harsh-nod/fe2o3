impl<'a> ProductionScopedMatrixSourceSessionV1<'a> {
    /// Lends one previously checked BF16 source/lane pair and the same work
    /// owner to its explicit read-event producer. This does not close the read
    /// roster, grant allocation authority or fabricate a numeric SSA lane.
    pub fn with_checked_global_bf16_read_source<T>(
        &mut self,
        row: ProductionGlobalBf16SourceRowV1<'_, 'a>,
        consume: impl FnOnce(
            ProductionGlobalBf16SourceRowV1<'_, 'a>,
            ProductionScopedBf16LaneUseV1,
            &mut dyn FnMut(usize) -> Result<(), ProductionSemanticKirErrorV1>,
        ) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let lane = self.checked_bf16_read_lane(row.view().body(), row.load_block(), row.call())?;
        self.with_bf16_source_graph(|owner, view, _, graph, expected| {
            graph.charge(3)?;
            if !std::ptr::eq(owner, row.owner())
                || !std::ptr::eq(view, row.view())
                || !expected.contains(&row.load_block())
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            consume(row, lane, &mut |words| graph.charge(words))
        })
    }
}
