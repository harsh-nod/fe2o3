impl SourceSession<'_> {
    // This is the already checked, original use. It creates no new issuer or
    // lifetime fact and does not mark a memory event consumed.
    pub(in crate::production_semantic_kir_v1) fn checked_bf16_read_lane(
        &mut self,
        body: &SemanticFunctionDeclV1,
        block: u32,
        call: &SemanticDirectCallV1,
    ) -> Result<ProductionScopedBf16LaneUseV1> {
        self.require_bf16_scope()?;
        self.graph.charge(1)?;
        check_call(self.owner, self.view, body, block, call)?;
        let row = self
            .bf16_rows
            .get(&block)
            .ok_or_else(|| reject("BF16 read source has no previously checked constructor lane"))?;
        if row.call != *call
            || row.result.contract != checked_load(self.owner, self.view, block, call)?
        {
            return Err(mismatch());
        }
        Ok(row.result)
    }
}
