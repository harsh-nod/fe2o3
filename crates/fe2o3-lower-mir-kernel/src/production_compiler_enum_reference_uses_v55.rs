#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceCompilerEnumReferenceUseV55 {
    block: BlockId,
    operation: usize,
    value: ValueId,
    custody: SourceCompilerEnumReferenceV55,
}

impl SourceAddressMemoryV29<'_> {
    fn compiler_reference_use_v55(
        &self,
        block: BlockId,
        position: usize,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceCompilerEnumReferenceUseV55>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.compiler_references.len(), budget)?;
        let Ok(index) = self
            .compiler_references
            .binary_search_by_key(&(block, position), |row| (row.block, row.operation))
        else {
            return Ok(None);
        };
        let row = self.compiler_references[index];
        budget.charge_work(4)?;
        if !matches!(operation.kind, OperationKind::Store { value, .. }
            | OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) if value == row.value)
            || self.exact(row.value, budget)? != Some(row.custody.slot)
            || row.custody.loan.carrier != ProductionSourceReferenceCarrierV38::MemoryPointer
        {
            return Err(source_enum_tag_error_v55());
        }
        Ok(Some(row))
    }
}
