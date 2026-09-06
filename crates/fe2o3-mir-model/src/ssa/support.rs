//! Resource accounting and compact bitset support for the SSA planner.

use std::mem::size_of;

use super::*;

pub(super) fn validate_variable(
    variable: SsaVariableIdV1,
    variable_count: usize,
    site: SsaInputSiteV1,
) -> Result<(), SsaPlannerErrorV1> {
    if variable.get() as usize >= variable_count {
        Err(SsaPlannerErrorV1::UnknownVariable {
            site,
            variable,
            variable_count,
        })
    } else {
        Ok(())
    }
}

pub(super) fn validate_definition_order(
    definitions: &[SsaVariableIdV1],
    edge: Option<SsaEdgeIdV1>,
) -> Result<(), SsaPlannerErrorV1> {
    if definitions.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(SsaPlannerErrorV1::NonCanonicalDefinitions { edge })
    } else {
        Ok(())
    }
}

pub(super) fn require_resource(
    resource: SsaPlannerResourceV1,
    required: usize,
    limit: usize,
) -> Result<(), SsaPlannerErrorV1> {
    if required > limit {
        Err(SsaPlannerErrorV1::ResourceLimitExceeded {
            resource,
            required,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add_resource(
    resource: SsaPlannerResourceV1,
    current: usize,
    added: usize,
    limit: usize,
) -> Result<usize, SsaPlannerErrorV1> {
    let required = current.saturating_add(added);
    require_resource(resource, required, limit)?;
    Ok(required)
}

pub(super) fn checked_scale(value: usize, factor: usize) -> Result<usize, SsaPlannerErrorV1> {
    value
        .checked_mul(factor)
        .ok_or(SsaPlannerErrorV1::IdentityOverflow)
}

pub(super) fn charge_storage_items<T>(
    current: &mut usize,
    count: usize,
    limit: usize,
) -> Result<(), SsaPlannerErrorV1> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
    let words = bytes.div_ceil(size_of::<u64>());
    *current = checked_add_resource(SsaPlannerResourceV1::StorageWords, *current, words, limit)?;
    Ok(())
}

pub(super) fn take_definition_value(next: &mut u32) -> Result<SsaValueV1, SsaPlannerErrorV1> {
    let value = SsaValueV1::Definition(SsaDefinitionIdV1::new(*next));
    *next = next
        .checked_add(1)
        .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
    Ok(value)
}

pub(super) fn charge_sort_work(
    work: &mut WorkBudget,
    items: usize,
) -> Result<(), SsaPlannerErrorV1> {
    if items < 2 {
        return Ok(());
    }
    let passes = usize::BITS as usize - (items - 1).leading_zeros() as usize;
    work.charge(checked_scale(items, passes)?)
}

pub(super) struct WorkBudget {
    pub(super) consumed: usize,
    limit: usize,
}

impl WorkBudget {
    pub(super) const fn new(limit: usize) -> Self {
        Self { consumed: 0, limit }
    }

    pub(super) fn charge(&mut self, units: usize) -> Result<(), SsaPlannerErrorV1> {
        self.consumed = checked_add_resource(
            SsaPlannerResourceV1::WorkUnits,
            self.consumed,
            units,
            self.limit,
        )?;
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct BitMatrix {
    words_per_row: usize,
    words: Vec<u64>,
}

impl BitMatrix {
    pub(super) fn try_new(rows: usize, words_per_row: usize) -> Result<Self, SsaPlannerErrorV1> {
        let word_count = rows
            .checked_mul(words_per_row)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        Ok(Self {
            words_per_row,
            words: vec![0; word_count],
        })
    }

    pub(super) fn word(&self, row: usize, word: usize) -> u64 {
        self.words[row * self.words_per_row + word]
    }

    pub(super) fn set_word(&mut self, row: usize, word: usize, value: u64) -> bool {
        let slot = &mut self.words[row * self.words_per_row + word];
        let changed = *slot != value;
        *slot = value;
        changed
    }

    pub(super) fn insert(&mut self, row: usize, bit: usize) {
        let word = row * self.words_per_row + bit / u64::BITS as usize;
        self.words[word] |= 1_u64 << (bit % u64::BITS as usize);
    }

    pub(super) fn contains(&self, row: usize, bit: usize) -> bool {
        let word = row * self.words_per_row + bit / u64::BITS as usize;
        self.words
            .get(word)
            .is_some_and(|word| word & (1_u64 << (bit % u64::BITS as usize)) != 0)
    }

    pub(super) fn ones(&self, row: usize, bit_count: usize) -> BitOnes<'_> {
        let start = row * self.words_per_row;
        BitOnes {
            words: &self.words[start..start + self.words_per_row],
            next_word: 0,
            current: 0,
            base: 0,
            bit_count,
        }
    }
}

pub(super) struct BitOnes<'a> {
    words: &'a [u64],
    next_word: usize,
    current: u64,
    base: usize,
    bit_count: usize,
}

impl Iterator for BitOnes<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current != 0 {
                let bit = self.current.trailing_zeros() as usize;
                self.current &= self.current - 1;
                let identity = self.base + bit;
                if identity < self.bit_count {
                    return Some(identity);
                }
            } else {
                self.current = *self.words.get(self.next_word)?;
                self.base = self.next_word * u64::BITS as usize;
                self.next_word += 1;
            }
        }
    }
}

pub(super) fn bit_contains(words: &[u64], bit: usize) -> bool {
    words
        .get(bit / u64::BITS as usize)
        .is_some_and(|word| word & (1_u64 << (bit % u64::BITS as usize)) != 0)
}

pub(super) fn bit_insert(words: &mut [u64], bit: usize) {
    words[bit / u64::BITS as usize] |= 1_u64 << (bit % u64::BITS as usize);
}

pub(super) fn bit_remove(words: &mut [u64], bit: usize) {
    words[bit / u64::BITS as usize] &= !(1_u64 << (bit % u64::BITS as usize));
}

pub(super) fn intersect_dominator_paths(
    mut left: usize,
    mut right: usize,
    immediate: &[Option<usize>],
    rpo_index: &[usize],
    work: &mut WorkBudget,
) -> Result<usize, SsaPlannerErrorV1> {
    while left != right {
        work.charge(1)?;
        while rpo_index[left] > rpo_index[right] {
            work.charge(1)?;
            left = immediate[left].expect("processed predecessor has an immediate dominator");
        }
        while rpo_index[right] > rpo_index[left] {
            work.charge(1)?;
            right = immediate[right].expect("processed predecessor has an immediate dominator");
        }
    }
    Ok(left)
}
