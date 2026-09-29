use super::*;

#[path = "production_private_array_cfg_initialization_v2.rs"]
mod cfg_v2;
pub(super) use cfg_v2::derive;

#[derive(Clone, Copy)]
pub(super) struct ReadInitializationV2 {
    offset: u64,
    initialized: bool,
}

impl PrivateArrayFinalRelationV1<'_> {
    pub(super) fn check_read_initialization(
        &self,
        read: &PrivateArrayEffectV1,
        slot: &PrivateArraySlotV1,
        offset: u64,
        work: &mut PrivateArrayCorrelationWorkV1<'_>,
    ) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        let mismatch = || ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch {
            location: FunctionOperationLocation::new(
                read.memory_location.block,
                read.memory_location.operation,
            ),
        };
        work.charge_private_array_work(2)?;
        let role = private_array_role_key_v1(read.role).ok_or_else(mismatch)?;
        let index = private_array_binary_search_v1(
            self.effects,
            |row| {
                let role = private_array_role_key_v1(row.role).unwrap_or((u8::MAX, u32::MAX));
                [
                    row.semantic_block as usize,
                    row.semantic_statement as usize,
                    role.0 as usize,
                    role.1 as usize,
                    row.original_index.component() as usize,
                ]
            },
            [
                read.semantic_block as usize,
                read.semantic_statement as usize,
                role.0 as usize,
                role.1 as usize,
                read.original_index.component() as usize,
            ],
            work,
        )?
        .map_err(|_| mismatch())?;
        work.charge_private_array_work(3)?;
        if slot.local != read.local
            || !std::ptr::eq(&self.effects[index], read)
            || !self
                .initialized_reads
                .as_ref()
                .and_then(|rows| rows.get(index))
                .is_some_and(|fact| fact.initialized && fact.offset == offset)
        {
            return Err(mismatch());
        }
        Ok(())
    }
}
