//! The existing tensor grammar as an inert inner payload, never a Module.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, mem::size_of, panic::AssertUnwindSafe};

/// Maximum of the existing closed V12 tensor field alternatives.
pub const MAX_TENSOR_LAYOUT_PAYLOAD_BYTES_V12: usize =
    5 + 2 + 3 * (1 + 4 + 1 + 1 + 23 + 2 + 2 + 2) + 2;

#[derive(Debug)]
pub enum TensorLayoutPayloadErrorV12 {
    Resource(Resource),
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Length,
    NonCanonical,
    Panicked,
}
impl fmt::Display for TensorLayoutPayloadErrorV12 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert tensor layout payload: {self:?}")
    }
}
impl Error for TensorLayoutPayloadErrorV12 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Length | Self::NonCanonical | Self::Panicked => None,
        }
    }
}
impl From<Resource> for TensorLayoutPayloadErrorV12 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type E = TensorLayoutPayloadErrorV12;

/// Complete returned payload/header extent, transferred unreserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TensorLayoutPayloadStorageV12(usize);
impl TensorLayoutPayloadStorageV12 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only raw payload bytes. No tensor legality, Module or source authority.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::InertTensorLayoutPayloadV12;
/// fn duplicate(value: InertTensorLayoutPayloadV12) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{InertTensorLayoutPayloadV12, Module};
/// fn promote(value: InertTensorLayoutPayloadV12) -> Module { value.into() }
/// ```
pub struct InertTensorLayoutPayloadV12 {
    bytes: Vec<u8>,
}
impl InertTensorLayoutPayloadV12 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn scope<T>(
    budget: &mut Budget<'_>,
    body: impl FnOnce(&mut Budget<'_>) -> Result<(T, usize), E>,
) -> Result<(T, TensorLayoutPayloadStorageV12), E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const _;
    let result =
        std::panic::catch_unwind(AssertUnwindSafe(|| body(budget))).unwrap_or(Err(E::Panicked));
    if budget.work_ledger_identity_v1() != ledger
        || !std::ptr::eq(slot, budget)
        || budget.storage() < floor
    {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    match result {
        Ok((value, retained)) => {
            if budget.storage() - floor < retained {
                drop(value);
                budget.rollback_storage(floor)?;
                return Err(Resource::Accounting.into());
            }
            budget.rollback_storage(floor)?;
            Ok((value, TensorLayoutPayloadStorageV12(retained)))
        }
        Err(error) => {
            budget.rollback_storage(floor)?;
            Err(error)
        }
    }
}

/// Encodes the exact existing V12 tensor payload, without a Module header.
/// Input/siblings are prepaid. Reserve the entire returned receipt before any
/// further controlled work. Internal writer scratch is released on return.
pub fn encode_tensor_layout_payload_v12(
    contract: TensorLayoutContractV1,
    budget: &mut Budget<'_>,
) -> Result<(InertTensorLayoutPayloadV12, TensorLayoutPayloadStorageV12), E> {
    scope(budget, |budget| {
        budget.charge_work(2)?;
        let header =
            size_of::<InertTensorLayoutPayloadV12>() + size_of::<TensorLayoutPayloadStorageV12>();
        budget.reserve_storage(header + size_of::<Writer<'_>>())?;
        let length = {
            let mut writer = Writer::counter(KERNEL_IR_VERSION_V12, budget.work_budget_v1());
            encode_tensor_layout_contract_v1(&mut writer, contract).map_err(E::Encode)?;
            writer.length()
        };
        if length > MAX_TENSOR_LAYOUT_PAYLOAD_BYTES_V12 {
            return Err(E::Length);
        }
        budget.reserve_storage(length)?;
        let writer =
            Writer::with_exact_capacity(KERNEL_IR_VERSION_V12, length, budget.work_budget_v1())
                .map_err(E::Encode)?;
        let Writer { bytes, .. } = writer;
        let capacity = bytes.capacity();
        budget.reserve_storage(capacity.checked_sub(length).ok_or(Resource::Accounting)?)?;
        let mut writer = Writer {
            bytes,
            length: 0,
            peak_auxiliary_bytes: 0,
            mode: WriterModeV1::Materialize,
            version: KERNEL_IR_VERSION_V12,
            budget: Some(budget.work_budget_v1()),
        };
        encode_tensor_layout_contract_v1(&mut writer, contract).map_err(E::Encode)?;
        if writer.length != length || writer.bytes.len() != length {
            return Err(E::NonCanonical);
        }
        // Module finishers patch bytes 12..16; raw tensor bytes have no such field.
        let Writer { bytes, .. } = writer;
        Ok((
            InertTensorLayoutPayloadV12 { bytes },
            header.checked_add(capacity).ok_or(Resource::Arithmetic)?,
        ))
    })
}

/// Reads and exactly re-encodes a complete V12 tensor inner payload.
/// This checks canonical syntax only, not tensor or ranked-kernel admission.
/// Borrowed backing/siblings stay prepaid; the returned Copy model remains inert.
pub fn decode_tensor_layout_payload_v12(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<(TensorLayoutContractV1, TensorLayoutPayloadStorageV12), E> {
    scope(budget, |budget| {
        budget.charge_work(2)?;
        if bytes.len() > MAX_TENSOR_LAYOUT_PAYLOAD_BYTES_V12 {
            return Err(E::Length);
        }
        let retained =
            size_of::<TensorLayoutContractV1>() + size_of::<TensorLayoutPayloadStorageV12>();
        let scratch = size_of::<Reader<'_, '_>>().max(size_of::<Writer<'_>>());
        budget.reserve_storage(retained + scratch)?;
        let mut reader = Reader::new(bytes, Some(DecodeBudgetV12::Resources(budget)));
        reader.version = KERNEL_IR_VERSION_V12;
        let contract = decode_tensor_layout_contract_v1(&mut reader).map_err(E::Decode)?;
        if !reader.is_finished() {
            return Err(E::Decode(KernelIrDecodeError::TrailingBytes));
        }
        drop(reader);
        let mut writer =
            Writer::comparing(KERNEL_IR_VERSION_V12, bytes, Some(budget.work_budget_v1()));
        encode_tensor_layout_contract_v1(&mut writer, contract).map_err(E::Encode)?;
        let canonical = writer.length == bytes.len()
            && matches!(&writer.mode, WriterModeV1::Compare { matches: true, .. });
        drop(writer);
        budget.charge_work(1)?;
        if !canonical {
            return Err(E::NonCanonical);
        }
        Ok((contract, retained))
    })
}

#[cfg(test)]
#[path = "tensor_layout_payload_v12_tests.rs"]
mod tests;
