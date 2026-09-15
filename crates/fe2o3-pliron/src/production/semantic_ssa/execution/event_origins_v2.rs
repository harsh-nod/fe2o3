//! Fixed storage for the adapter's actual source-site event spans.
use super::*;

/// Each block owns one endpoint per statement followed by its terminator.
/// Empty spans are retained too. No statement index shares a sentinel with None.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in super::super) struct ExecutionEventOriginsV1 {
    offsets: Vec<u32>,
    ends: Vec<u32>,
    next: usize,
    block: usize,
    invalid: bool,
}

impl ExecutionEventOriginsV1 {
    pub(in super::super) fn for_function(
        id: SemanticFunctionIdV1,
        function: &SemanticFunctionDeclV1,
        limits: ProductionSemanticSsaLimitsV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let blocks = function.blocks().len();
        u32::try_from(blocks).map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        // Bound the shape scan before visiting blocks; then reserve the entire
        // roster before allocating either array or running the adapter.
        enforce_function_resource_limit_v1(id, Self::reservation(blocks, blocks)?, limits)?;
        let mut sites = 0_usize;
        for block in function.blocks() {
            u32::try_from(block.statements().len())
                .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            sites = sites
                .checked_add(block.statements().len())
                .and_then(|count| count.checked_add(1))
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            u32::try_from(sites).map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            enforce_function_resource_limit_v1(id, Self::reservation(blocks, sites)?, limits)?;
        }
        let mut result = Self {
            offsets: Self::zeroed(
                blocks
                    .checked_add(1)
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            )?,
            ends: Self::zeroed(sites)?,
            ..Self::default()
        };
        let mut offset = 0_usize;
        for (index, block) in function.blocks().iter().enumerate() {
            offset = offset
                .checked_add(block.statements().len())
                .and_then(|count| count.checked_add(1))
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            result.offsets[index + 1] = u32::try_from(offset)
                .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        }
        Ok(result)
    }

    fn zeroed(length: usize) -> Result<Vec<u32>, ProductionSemanticSsaErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        if values.capacity() != length {
            return Err(ProductionSemanticSsaErrorV1::ResourceOverflow);
        }
        values.resize(length, 0);
        Ok(values)
    }

    fn reservation(
        blocks: usize,
        sites: usize,
    ) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
        let word = std::mem::size_of::<usize>();
        let array_words = |count: usize| {
            count
                .checked_mul(std::mem::size_of::<u32>())
                .map(|bytes| bytes.div_ceil(word))
        };
        let storage_words = blocks
            .checked_add(1)
            .and_then(array_words)
            .and_then(|offsets| array_words(sites)?.checked_add(offsets))
            .and_then(|one| one.checked_mul(2))
            .and_then(|both| both.checked_add(2 * std::mem::size_of::<Self>().div_ceil(word)))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        // Two shape passes, initialization, recording, hashing and replay Eq.
        // Queries still charge their existing caller and create no new budget.
        let work_units = blocks
            .checked_add(sites)
            .and_then(|count| count.checked_add(1))
            .and_then(|count| count.checked_mul(12))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        Ok(SemanticSsaAuxiliaryResourcesV1 {
            storage_words,
            work_units,
        })
    }

    pub(in super::super) fn record(
        &mut self,
        block: u32,
        statement: Option<u32>,
        events: std::ops::Range<usize>,
    ) {
        if self.invalid {
            return;
        }
        let valid = (|| {
            if self.block != block as usize {
                return None;
            }
            let start = *self.offsets.get(self.block)? as usize;
            let end = *self.offsets.get(self.block + 1)? as usize;
            if self.next < start || self.next >= end {
                return None;
            }
            let expected = if self.next + 1 == end {
                None
            } else {
                Some(u32::try_from(self.next - start).ok()?)
            };
            if expected != statement {
                return None;
            }
            let previous = if self.next == start {
                0
            } else {
                *self.ends.get(self.next - 1)? as usize
            };
            if events.start != previous || events.end < events.start {
                return None;
            }
            let value = u32::try_from(events.end).ok()?;
            *self.ends.get_mut(self.next)? = value;
            self.next += 1;
            if self.next == end {
                self.block += 1;
            }
            Some(())
        })();
        if valid.is_none() {
            self.invalid = true;
        }
    }

    pub(in super::super) fn block_ends(&self, block: u32) -> Option<&[u32]> {
        if self.invalid {
            return None;
        }
        let start = *self.offsets.get(block as usize)? as usize;
        let end = *self.offsets.get(block as usize + 1)? as usize;
        // An error can request diagnostics during recording. Never return
        // unrecorded zero-filled slots as source correspondence.
        if end > self.next {
            return None;
        }
        self.ends.get(start..end)
    }

    pub(in super::super) fn statement(&self, block: u32, event: u32) -> Option<Option<u32>> {
        let ends = self.block_ends(block)?;
        let slot = ends.partition_point(|end| *end <= event);
        ends.get(slot)?;
        Some(if slot + 1 == ends.len() {
            None
        } else {
            Some(u32::try_from(slot).ok()?)
        })
    }

    pub(in super::super) fn resources(
        &self,
    ) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
        if self.invalid
            || self.next != self.ends.len()
            || self.block != self.offsets.len().saturating_sub(1)
            || self.offsets.capacity() != self.offsets.len()
            || self.ends.capacity() != self.ends.len()
        {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        if self.offsets.is_empty() {
            return Ok(SemanticSsaAuxiliaryResourcesV1 {
                storage_words: 2 * std::mem::size_of::<Self>()
                    .div_ceil(std::mem::size_of::<usize>()),
                work_units: 12,
            });
        }
        Self::reservation(self.block, self.ends.len())
    }

    pub(in super::super) fn hash_into(&self, digest: &mut Sha256) {
        digest.update(b"fe2o3.semantic-ssa.adapter-event-spans.v2\0");
        digest.update((self.next as u64).to_le_bytes());
        digest.update((self.block as u64).to_le_bytes());
        digest.update([u8::from(self.invalid)]);
        for values in [&self.offsets, &self.ends] {
            digest.update((values.len() as u64).to_le_bytes());
            for value in values {
                digest.update(value.to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests;
