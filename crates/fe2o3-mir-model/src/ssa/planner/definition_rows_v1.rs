//! Exact immutable kill rows with a conservative dense fallback.

use super::{BitMatrix, SsaPlannerErrorV1, charge_storage_items};
use std::ops::Range;

#[derive(Clone, Default)]
pub(super) struct WordWindow {
    first: usize,
    words: Box<[u64]>,
}

pub(super) enum DefinitionRows {
    Dense(BitMatrix),
    Windowed(Vec<WordWindow>),
}

/// Promotable rank is monotone and its distance cannot exceed local-ID distance.
/// The extra word accounts for an unknown alignment of the first dense rank.
pub(super) fn word_span_upper(first: usize, last: usize, width: usize) -> usize {
    ((last - first) / u64::BITS as usize + 2).min(width)
}

impl DefinitionRows {
    /// Select windows only if their complete upper bound is smaller than dense.
    /// Return just the fixed allocation; payload is charged before each copy.
    pub(super) fn layout(
        rows: usize,
        width: usize,
        window_upper: usize,
    ) -> Result<(bool, usize), SsaPlannerErrorV1> {
        let dense = rows
            .checked_mul(width)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        let headers = rows
            .checked_mul(size_of::<WordWindow>())
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?
            .div_ceil(size_of::<u64>());
        let windowed = headers
            .checked_add(window_upper)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        Ok(if windowed < dense {
            (true, headers)
        } else {
            (false, dense)
        })
    }

    pub(super) fn try_new(
        rows: usize,
        width: usize,
        windowed: bool,
    ) -> Result<Self, SsaPlannerErrorV1> {
        Ok(if windowed {
            Self::Windowed(vec![WordWindow::default(); rows])
        } else {
            Self::Dense(BitMatrix::try_new(rows, width)?)
        })
    }

    pub(super) fn insert_dense(&mut self, row: usize, bit: usize) {
        if let Self::Dense(matrix) = self {
            matrix.insert(row, bit);
        }
    }

    /// Reuse the existing upward-use scratch, which already contains every kill.
    /// Interior zero words remain present; only known zero prefix/suffix is absent.
    pub(super) fn finish_window(
        &mut self,
        row: usize,
        defined: &[u64],
        range: Option<Range<usize>>,
        storage: &mut usize,
        limit: usize,
    ) -> Result<(), SsaPlannerErrorV1> {
        let Self::Windowed(rows) = self else {
            return Ok(());
        };
        let Some(range) = range else {
            rows[row] = WordWindow::default();
            return Ok(());
        };
        let words = defined
            .get(range.clone())
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        charge_storage_items::<u64>(storage, words.len(), limit)?;
        rows[row] = WordWindow {
            first: range.start,
            words: words.into(),
        };
        Ok(())
    }

    pub(super) fn word(&self, row: usize, word: usize) -> u64 {
        match self {
            Self::Dense(matrix) => matrix.word(row, word),
            Self::Windowed(rows) => {
                let row = &rows[row];
                word.checked_sub(row.first)
                    .and_then(|index| row.words.get(index))
                    .copied()
                    .unwrap_or(0)
            }
        }
    }
}

#[cfg(test)]
#[path = "definition_rows_v1/tests.rs"]
mod tests;
