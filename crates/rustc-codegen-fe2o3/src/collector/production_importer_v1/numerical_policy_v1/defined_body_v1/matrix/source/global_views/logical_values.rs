//! Inert coordinate and bit-placement contract, not a memory/issuer proof.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Fp4E2M1,
    Fp8E4M3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    A,
    B,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalValue {
    pub row_delta: u32,
    pub column_delta: u32,
    pub dword: u8,
    pub shift: u8,
    pub mask: u8,
}

pub const WAVE_WIDTH: u32 = 64;
pub const VALUES_PER_LANE: u32 = 32;
pub const DWORDS: usize = 8;

pub fn logical_value(format: Format, role: Role, lane: u32, item: u32) -> Option<LogicalValue> {
    if lane >= WAVE_WIDTH || item >= VALUES_PER_LANE {
        return None;
    }
    let group = lane / 16;
    let (depth, dword, shift, mask) = match format {
        Format::Fp4E2M1 => (group * 32 + item, item / 8, (item % 8) * 4, 15),
        Format::Fp8E4M3 => (
            (item / 16) * 64 + group * 16 + item % 16,
            item / 4,
            (item % 4) * 8,
            255,
        ),
    };
    let (row_delta, column_delta) = match role {
        Role::A => (lane % 16, depth),
        Role::B => (depth, lane % 16),
    };
    Some(LogicalValue {
        row_delta,
        column_delta,
        dword: dword as u8,
        shift: shift as u8,
        mask,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalView {
    pub offset: u64,
    pub rows: u64,
    pub columns: u64,
    pub stride: u64,
    pub allocation_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtentError {
    InvalidStride,
    ExtentOverflow,
    OutOfBounds { required: u64, actual: u64 },
}

impl LogicalView {
    /// Mirrors the original checked helper's error ordering. No proof is issued.
    pub fn check_extent(self) -> Result<(), ExtentError> {
        if self.rows != 0 && self.columns != 0 && self.stride < self.columns {
            return Err(ExtentError::InvalidStride);
        }
        let required = if self.rows == 0 || self.columns == 0 {
            self.offset
        } else {
            let span = (self.rows - 1)
                .checked_mul(self.stride)
                .and_then(|span| span.checked_add(self.columns))
                .ok_or(ExtentError::ExtentOverflow)?;
            self.offset
                .checked_add(span)
                .ok_or(ExtentError::ExtentOverflow)?
        };
        if required > self.allocation_len {
            return Err(ExtentError::OutOfBounds {
                required,
                actual: self.allocation_len,
            });
        }
        Ok(())
    }

    /// None means zero-fill without a memory event, not an address of zero.
    pub fn source_byte(self, value: LogicalValue, row_base: u64, column_base: u64) -> Option<u64> {
        let row = row_base.checked_add(u64::from(value.row_delta))?;
        let column = column_base.checked_add(u64::from(value.column_delta))?;
        if row >= self.rows || column >= self.columns {
            return None;
        }
        let index = row
            .checked_mul(self.stride)?
            .checked_add(self.offset)?
            .checked_add(column)?;
        (index < self.allocation_len).then_some(index)
    }
}

/// Packs already obtained logical source bytes. It performs no memory reads.
pub fn pack_logical_values(
    format: Format,
    values: &[u8; VALUES_PER_LANE as usize],
) -> [u32; DWORDS] {
    let mut registers = [0; DWORDS];
    for (item, byte) in values.iter().enumerate() {
        let value = logical_value(format, Role::A, 0, item as u32).unwrap();
        registers[usize::from(value.dword)] |= u32::from(byte & value.mask) << value.shift;
    }
    registers
}

#[cfg(test)]
#[path = "logical_values_tests.rs"]
mod tests;
