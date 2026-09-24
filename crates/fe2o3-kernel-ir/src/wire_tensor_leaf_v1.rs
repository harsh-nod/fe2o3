//! Allocation-free reuse of the frozen V8 tensor-layout wire grammar.

use super::{
    DecodeBudgetV12, KERNEL_IR_VERSION_V8, KernelIrDecodeError, KernelIrEncodeError, Reader,
    TensorLayoutContractV1, Writer, decode_tensor_layout_contract_v1,
    encode_tensor_layout_contract_v1,
};
use crate::{CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1};

/// Maximum tensor leaf size: profile(5) + width(2) + three fragments(36) + tail(2).
/// Each fragment has 7 fixed bytes, an affine map(23), and three two-byte choices.
pub const MAX_TENSOR_LAYOUT_LEAF_BYTES_V1: usize = 117;

/// Encodes the complete tensor-layout contract using the existing V8 leaf grammar.
///
/// No module header is emitted and no heap allocation or semantic verification
/// occurs. The enclosing format must identify this leaf version. The caller owns
/// and accounts the output buffer; this call leaves its storage ledger unchanged.
/// Successful work is the returned byte length plus two entry/completion tokens.
/// Each primitive is charged before writing. An error may leave a written prefix;
/// only a successful returned length designates usable bytes. The suffix is intact.
pub fn encode_tensor_layout_leaf_v1(
    value: TensorLayoutContractV1,
    out: &mut [u8; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1],
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<usize, KernelIrEncodeError> {
    let work = budget.work_budget_v1();
    work.charge_work(1)
        .map_err(KernelIrEncodeError::WorkLimit)?;
    let mut writer = BorrowedTensorWriterV1 {
        out,
        offset: 0,
        work,
    };
    encode_tensor_layout_contract_v1(&mut writer, value)?;
    writer
        .work
        .charge_work(1)
        .map_err(KernelIrEncodeError::WorkLimit)?;
    Ok(writer.offset)
}

/// Decodes exactly one complete V8 tensor-layout leaf without allocating.
///
/// This reconstructs syntax, not a verified layout or authenticated tensor fact.
/// Incompatible, opaque and unsupported values remain intact for the independent
/// verifier to reject. The enclosing format must identify this leaf version.
/// The input and returned fixed-size value belong to the caller's storage domain;
/// the inherited storage ledger is unchanged. Successful work is `bytes.len() + 2`.
pub fn decode_tensor_layout_leaf_v1(
    bytes: &[u8],
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<TensorLayoutContractV1, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, Some(DecodeBudgetV12::Resources(budget)));
    reader.charge_work(1)?;
    if bytes.len() > MAX_TENSOR_LAYOUT_LEAF_BYTES_V1 {
        return Err(KernelIrDecodeError::LimitExceeded {
            field: "tensor layout leaf bytes",
            actual: bytes.len(),
            max: MAX_TENSOR_LAYOUT_LEAF_BYTES_V1,
        });
    }
    reader.version = KERNEL_IR_VERSION_V8;
    let value = decode_tensor_layout_contract_v1(&mut reader)?;
    reader.charge_work(1)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(value)
}

// Only the shared tensor encoders need this fixed-width primitive subset.
pub(super) trait TensorLeafWriterV1 {
    fn version(&self) -> u16;
    fn u8(&mut self, value: u8) -> Result<(), KernelIrEncodeError>;
    fn u16(&mut self, value: u16) -> Result<(), KernelIrEncodeError>;
    fn u32(&mut self, value: u32) -> Result<(), KernelIrEncodeError>;
}

impl TensorLeafWriterV1 for Writer<'_> {
    fn version(&self) -> u16 {
        self.version
    }
    fn u8(&mut self, value: u8) -> Result<(), KernelIrEncodeError> {
        Writer::u8(self, value)
    }
    fn u16(&mut self, value: u16) -> Result<(), KernelIrEncodeError> {
        Writer::u16(self, value)
    }
    fn u32(&mut self, value: u32) -> Result<(), KernelIrEncodeError> {
        Writer::u32(self, value)
    }
}

struct BorrowedTensorWriterV1<'out, 'work> {
    out: &'out mut [u8; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1],
    offset: usize,
    work: &'work mut CanonicalKernelIrWorkBudgetV1,
}

impl BorrowedTensorWriterV1<'_, '_> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), KernelIrEncodeError> {
        let end = self
            .offset
            .checked_add(bytes.len())
            .ok_or(KernelIrEncodeError::Overflow {
                field: "tensor layout leaf bytes",
            })?;
        let destination =
            self.out
                .get_mut(self.offset..end)
                .ok_or(KernelIrEncodeError::LimitExceeded {
                    field: "tensor layout leaf bytes",
                    actual: end,
                    max: MAX_TENSOR_LAYOUT_LEAF_BYTES_V1,
                })?;
        self.work
            .charge_work(bytes.len())
            .map_err(KernelIrEncodeError::WorkLimit)?;
        destination.copy_from_slice(bytes);
        self.offset = end;
        Ok(())
    }
}

impl TensorLeafWriterV1 for BorrowedTensorWriterV1<'_, '_> {
    fn version(&self) -> u16 {
        KERNEL_IR_VERSION_V8
    }
    fn u8(&mut self, value: u8) -> Result<(), KernelIrEncodeError> {
        self.bytes(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), KernelIrEncodeError> {
        self.bytes(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<(), KernelIrEncodeError> {
        self.bytes(&value.to_le_bytes())
    }
}

#[cfg(test)]
#[path = "wire_tensor_leaf_v1_tests.rs"]
mod tests;
