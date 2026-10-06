//! Checked callback-work bounds for the existing nominal/V5 borrowed decoder.
//! These count its logical debits, not CPU instructions, allocations or proof.
use crate::conditional_invocation_rows_v1 as row;
use crate::model::MAX_CAPABILITIES;
use crate::*;

/// Componentwise bounds at the existing wire/count limits. No decoder is run,
/// no budget is created, and no input or execution is admitted by this record.
/// Queries refer to one call; callers fund repeated calls separately. The
/// complete decoder bound includes mandatory contracts and their nominal joins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorWorkBoundsV5 {
    decode: usize,
    kernel: usize,
    argument_next: usize,
    component: usize,
    source_type: usize,
    device_layout: usize,
    requirement: usize,
    conditional_contract: usize,
}

impl DescriptorWorkBoundsV5 {
    /// Refuses arithmetic overflow rather than saturating on smaller targets.
    pub fn admitted_limits() -> Option<Self> {
        let k = MAX_KERNELS;
        let a = MAX_ARGUMENTS_PER_KERNEL;
        let n = MAX_NAME_BYTES;
        let caps = MAX_CAPABILITIES;
        // Reader::take pays 3*n+1: at most four per nonempty byte.
        // Text additionally pays n+1. Explicit +1 covers a zero-length
        // capability/component skip. Fixed row extents come from wire_v3.
        let kernel = sum(&[
            mul(4, sum(&[232, mul(3, n)?, mul(2, caps)?])?)?,
            mul(3, n.checked_add(1)?)?,
            1,
            32,
            1,
        ])?;
        let argument_next = sum(&[
            mul(
                4,
                sum(&[78, n, mul(16, MAX_PHYSICAL_COMPONENTS_PER_KERNEL)?])?,
            )?,
            n,
            1,
            1,
            1,
        ])?;
        let component = 1 + 4 * 16;
        let source_row = sum(&[
            4 * 36,
            RUST_TYPE_DOMAIN_V1.len().max(RUST_TYPE_DOMAIN_V3.len()),
            8,
            4,
        ])?;
        let layout_row = sum(&[4 * 44, DEVICE_LAYOUT_DOMAIN_V1.len(), 8, 12])?;
        // Binary search has no more iterations than rows. Each probe reads
        // exactly 32 bytes, then the selected row is revalidated once.
        let source_type = sum(&[mul(MAX_TYPE_RECORDS, 3 * 32 + 1)?, source_row])?;
        let device_layout = sum(&[mul(MAX_LAYOUT_RECORDS, 3 * 32 + 1)?, layout_row])?;
        let requirement = 1 + 3 * 48 + 1;
        let argument_walk = sum(&[mul(a, argument_next)?, 1])?;
        let prefix_header = sum(&[
            1,
            mul(4, sum(&[90, mul(5, MAX_TEXT_BYTES)?])?)?,
            mul(5, MAX_TEXT_BYTES + 1)?,
            mul(3, MAX_TEXT_BYTES)?,
            1,
        ])?;
        let prefix = sum(&[
            prefix_header,
            mul(MAX_TYPE_RECORDS, sum(&[source_row, 32, 3 * 36 + 1])?)?,
            mul(MAX_LAYOUT_RECORDS, sum(&[layout_row, 32, 3 * 44 + 1])?)?,
            k,
            mul(k, sum(&[kernel, argument_walk])?)?,
            mul(3 * 48, k)?,
            1,
        ])?;

        // validate_view: repeated name comparisons, argument/type/layout
        // queries and at most two canonical components per argument. Invalid
        // larger component counts refuse before the component loop.
        let argument_validation = sum(&[
            argument_next,
            1,
            mul(a, n + 1)?,
            source_type,
            device_layout,
            mul(2, component + 16)?,
            32 + 2 * 16,
        ])?;
        let kernel_validation = sum(&[
            kernel,
            32,
            mul(3 * k, n + 1)?,
            mul(caps, 1 + (1 + 3 * 2 + 1))?,
            1,
            mul(a, argument_validation)?,
            1,
            8,
            requirement,
            32 + 32,
            mul(4, caps)?,
        ])?;
        let nominal = sum(&[
            prefix,
            MAX_TYPE_RECORDS,
            MAX_LAYOUT_RECORDS,
            mul(3, k)?,
            a,
            caps,
            2,
            32,
            mul(k, kernel_validation)?,
            MAX_TYPE_RECORDS,
            MAX_LAYOUT_RECORDS,
        ])?;

        // Conditional codec prepays 256*wire+4096+1. The join may rescan
        // all nominal arguments for each binding/output/read occurrence.
        let conditional_contract = sum(&[mul(256, MAX_CONDITIONAL_INVOCATION_BYTES_V2)?, 4096, 1])?;
        let projection = sum(&[mul(a, argument_next)?, source_type])?;
        let argument_query = 4 * row::ARGUMENT + 1;
        let read_next = 4 * row::READ + 1;
        let join = sum(&[
            33,
            mul(
                MAX_CONDITIONAL_ARGUMENTS_V2,
                sum(&[argument_query, projection, 80])?,
            )?,
            1,
            argument_query,
            projection,
            mul(
                MAX_CONDITIONAL_READS_V2,
                sum(&[read_next, argument_query, projection, 4])?,
            )?,
            1,
        ])?;
        let contract_row = sum(&[
            3 * 4 + 1,
            3 * 32 + 1,
            mul(3, MAX_CONDITIONAL_INVOCATION_BYTES_V2)?,
            1,
            conditional_contract,
            32,
            kernel,
            join,
        ])?;
        let decode = sum(&[nominal, 2 * (3 * 2 + 1), mul(2, k)?, mul(k, contract_row)?])?;
        Some(Self {
            decode,
            kernel,
            argument_next,
            component,
            source_type,
            device_layout,
            requirement,
            conditional_contract,
        })
    }
    /// Complete V5 decode, including nominal validation and contract joins.
    pub const fn decode(self) -> usize {
        self.decode
    }
    /// One indexed kernel query, excluding its separate conditional query.
    pub const fn kernel(self) -> usize {
        self.kernel
    }
    /// One nominal argument-cursor step, including its component-byte skip.
    pub const fn argument_next(self) -> usize {
        self.argument_next
    }
    /// One indexed physical component query.
    pub const fn component(self) -> usize {
        self.component
    }
    /// One source-type lookup and selected-row identity validation.
    pub const fn source_type(self) -> usize {
        self.source_type
    }
    /// One device-layout lookup and selected-row identity validation.
    pub const fn device_layout(self) -> usize {
        self.device_layout
    }
    /// One indexed target-requirement query.
    pub const fn requirement(self) -> usize {
        self.requirement
    }
    /// One complete V2 contract query on an already decoded V5 kernel.
    pub const fn conditional_contract(self) -> usize {
        self.conditional_contract
    }
}

fn sum(values: &[usize]) -> Option<usize> {
    values.iter().try_fold(0usize, |n, v| n.checked_add(*v))
}
fn mul(a: usize, b: usize) -> Option<usize> {
    a.checked_mul(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_schedule_is_finite_and_arithmetic_refuses_overflow() {
        let bounds = DescriptorWorkBoundsV5::admitted_limits().unwrap();
        assert!(bounds.decode() > bounds.conditional_contract());
        assert!(bounds.decode() > bounds.kernel() * MAX_KERNELS);
        assert_eq!(sum(&[usize::MAX, 1]), None);
        assert_eq!(mul(usize::MAX, 2), None);
    }
}
