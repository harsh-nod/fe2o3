//! Shared numeric packing queries, never conversion between proof wire families.
use super::*;
use fe2o3_kernel_descriptor::mixed_conditional_v26::{
    MIXED_CONTRACT_CODEC_STORAGE_V26, MixedArgumentV26, MixedContractSubjectsV26,
    MixedOccurrenceV26,
};
use fe2o3_kernel_descriptor::mixed_conditional_v86::{
    MIXED_CONTRACT_CODEC_STORAGE_V86, MixedAccessGuardV86, MixedOccurrenceV86,
};

pub(super) struct PackingOccurrenceV88 {
    pub argument: u16,
    pub writing: bool,
    pub element_bytes: u64,
    pub alignment: u32,
    pub access_envelope: MixedIndexEnvelopeV26,
    pub formation_envelope: MixedIndexEnvelopeV26,
    pub invocation_axis: u8,
}

pub(super) const PACKING_CONTRACT_STORAGE_V88: usize = size_of::<PackingOccurrenceV88>()
    + 2 * size_of::<Result<PackingOccurrenceV88>>()
    + size_of::<[usize; 4]>();

// This private adapter has only the two concrete decoder-backed implementations.
// Original bytes and complete versioned row identities never pass through a V26
// re-encoding; only the common numerical obligations share the packing algorithm.
pub(super) trait MixedPackingContractV88 {
    const CODEC_STORAGE: usize;
    fn identity(&self) -> &[u8; 32];
    fn subjects(&self) -> &MixedContractSubjectsV26;
    fn argument_count(&self) -> usize;
    fn occurrence_count(&self) -> usize;
    fn argument(&self, index: usize, budget: &mut Budget<'_>) -> Result<MixedArgumentV26>;
    fn argument_identity(&self, index: usize, budget: &mut Budget<'_>) -> Result<[u8; 32]>;
    fn occurrence(&self, index: usize, budget: &mut Budget<'_>) -> Result<PackingOccurrenceV88>;
    fn occurrence_identity(&self, index: usize, budget: &mut Budget<'_>) -> Result<[u8; 32]>;
}

macro_rules! contract {
    ($contract:ident, $storage:ident, $project:ident) => {
        impl MixedPackingContractV88 for $contract<'_> {
            const CODEC_STORAGE: usize = $storage;
            fn identity(&self) -> &[u8; 32] {
                $contract::identity(self)
            }
            fn subjects(&self) -> &MixedContractSubjectsV26 {
                $contract::subjects(self)
            }
            fn argument_count(&self) -> usize {
                $contract::argument_count(self)
            }
            fn occurrence_count(&self) -> usize {
                $contract::occurrence_count(self)
            }
            fn argument(&self, index: usize, budget: &mut Budget<'_>) -> Result<MixedArgumentV26> {
                Ok($contract::argument(self, index, &mut |n| {
                    budget.charge_work(n)
                })?)
            }
            fn argument_identity(&self, index: usize, budget: &mut Budget<'_>) -> Result<[u8; 32]> {
                Ok($contract::argument_identity(self, index, &mut |n| {
                    budget.charge_work(n)
                })?)
            }
            fn occurrence(
                &self,
                index: usize,
                budget: &mut Budget<'_>,
            ) -> Result<PackingOccurrenceV88> {
                let row = $contract::occurrence(self, index, &mut |n| budget.charge_work(n))?;
                $project(row, budget)
            }
            fn occurrence_identity(
                &self,
                index: usize,
                budget: &mut Budget<'_>,
            ) -> Result<[u8; 32]> {
                Ok($contract::occurrence_identity(self, index, &mut |n| {
                    budget.charge_work(n)
                })?)
            }
        }
    };
}

macro_rules! numerical_row {
    ($row:ident) => {
        PackingOccurrenceV88 {
            argument: $row.argument,
            writing: $row.writing,
            element_bytes: $row.element_bytes,
            alignment: $row.alignment,
            access_envelope: $row.access_envelope,
            formation_envelope: $row.formation_envelope,
            invocation_axis: $row.invocation_axis,
        }
    };
}

fn occurrence_v26(
    row: MixedOccurrenceV26,
    budget: &mut Budget<'_>,
) -> Result<PackingOccurrenceV88> {
    budget.charge_work(8)?;
    Ok(numerical_row!(row))
}

fn occurrence_v86(
    row: MixedOccurrenceV86,
    budget: &mut Budget<'_>,
) -> Result<PackingOccurrenceV88> {
    budget.charge_work(16)?;
    if matches!(
        row.output_guard,
        MixedAccessGuardV86::ExplicitPredicate { .. }
    ) {
        // Select(predicate, invocation_index, 0) forms an address even for an
        // empty slice. LogicalExtent alone omits that zero-offset formation.
        // Retain the conservative whole-invocation bound until the wire format
        // carries a separately checked selected-zero formation envelope.
        if !matches!(row.formation_envelope,
            MixedIndexEnvelopeV26::InvocationAxis { axis } if axis == row.invocation_axis)
        {
            return Err(binding(
                "predicated mixed formation needs its invocation envelope",
            ));
        }
    }
    Ok(numerical_row!(row))
}

contract!(
    MixedContractV26,
    MIXED_CONTRACT_CODEC_STORAGE_V26,
    occurrence_v26
);
contract!(
    MixedContractV86,
    MIXED_CONTRACT_CODEC_STORAGE_V86,
    occurrence_v86
);
