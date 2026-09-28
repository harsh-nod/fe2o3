//! V18 entry and explicit roles; all payload walkers remain shared.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};

pub(super) fn require(
    writer: &Writer<'_>,
    feature: &'static str,
) -> Result<(), KernelIrEncodeError> {
    if writer.version == KERNEL_IR_VERSION_V18 {
        Ok(())
    } else {
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: writer.version,
            feature,
        })
    }
}
pub(super) fn encode_role(
    writer: &mut Writer<'_>,
    role: FunctionRole,
) -> Result<(), KernelIrEncodeError> {
    writer.u8(match role {
        FunctionRole::KernelEntry => 1,
        FunctionRole::InternalHelper => 2,
        FunctionRole::DeviceFfiExport => 3,
        FunctionRole::ExternalImport => 4,
    })
}
pub(super) fn decode_role(
    reader: &mut Reader<'_, '_>,
) -> Result<FunctionRole, KernelIrDecodeError> {
    Ok(match reader.u8()? {
        1 => FunctionRole::KernelEntry,
        2 => FunctionRole::InternalHelper,
        3 => FunctionRole::DeviceFfiExport,
        4 => FunctionRole::ExternalImport,
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "V18 function role",
                tag,
            });
        }
    })
}
pub(crate) fn decode_module_v18_with_allocation_budget_v1(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<Module, KernelIrDecodeError> {
    decode_module_impl_v1(
        bytes,
        KERNEL_IR_VERSION_V18,
        false,
        Some(DecodeBudgetV12::Resources(budget)),
    )
}
/// One prechecked schema extent is followed by the same materializing walk.
pub(crate) fn encode_counted_module_v18(
    module: &Module,
    length: usize,
    budget: &mut CanonicalKernelIrWorkBudgetV1,
) -> Result<Vec<u8>, KernelIrEncodeError> {
    let mut writer = Writer::with_exact_capacity(KERNEL_IR_VERSION_V18, length, budget)?;
    if writer.bytes.capacity() != length {
        return Err(KernelIrEncodeError::Allocation);
    }
    write_module_v1(module, &mut writer, false)?;
    let bytes = writer.finish_module()?;
    if bytes.len() != length {
        return Err(KernelIrEncodeError::NonCanonical {
            field: "V18 counted length",
        });
    }
    Ok(bytes)
}
fn outcome_headers<T>() -> Result<usize, ResourceError> {
    // Two simultaneous value/move slots and two full caller/callee results.
    std::mem::size_of::<T>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<Result<T, KernelIrDecodeError>>()))
        .ok_or(ResourceError::Arithmetic)
}
/// Added outer codec header envelope. Shared heap/producer scratch keeps its
/// existing explicit schedule; this is not a machine stack/RSS claim.
pub(crate) fn storage_codec_headers_v18() -> Result<usize, ResourceError> {
    use crate::{StorageFieldV1, StorageLayoutV1, StorageVariantV1};
    let recursive_types = outcome_headers::<Type>()?
        // Accepted depths 0..=MAX plus the first rejecting callee frame.
        .checked_mul(MAX_TYPE_DEPTH_V1 + 2)
        .ok_or(ResourceError::Arithmetic)?;
    let fixed = [
        std::mem::size_of::<Writer<'_>>(),
        std::mem::size_of::<Reader<'_, '_>>(),
        2 * std::mem::size_of::<Result<Writer<'_>, KernelIrEncodeError>>(),
        2 * std::mem::size_of::<Result<KernelIrV12WireExtentV1, KernelIrEncodeError>>(),
        2 * std::mem::size_of::<Result<bool, KernelIrEncodeError>>(),
        2 * std::mem::size_of::<Result<(), KernelIrEncodeError>>(),
        outcome_headers::<Module>()?,
        outcome_headers::<Function>()?,
        outcome_headers::<Signature>()?,
        outcome_headers::<FunctionBody>()?,
        outcome_headers::<Kernel>()?,
        outcome_headers::<BasicBlock>()?,
        outcome_headers::<Operation>()?,
        outcome_headers::<OperationKind>()?,
        outcome_headers::<ValueDef>()?,
        outcome_headers::<Terminator>()?,
        outcome_headers::<String>()?,
        outcome_headers::<BTreeSet<TargetCapability>>()?,
        outcome_headers::<BTreeSet<AddressSpace>>()?,
        outcome_headers::<TargetCapability>()?,
        outcome_headers::<InlineAssembly>()?,
        outcome_headers::<MatrixOperation>()?,
        outcome_headers::<TensorLayoutContractV1>()?,
        outcome_headers::<TensorFragmentLayoutV1>()?,
        outcome_headers::<StorageLayoutV1>()?,
        outcome_headers::<StorageFieldV1>()?,
        outcome_headers::<StorageVariantV1>()?,
        outcome_headers::<Vec<StorageLayoutV1>>()?,
        outcome_headers::<Vec<StorageFieldV1>>()?,
        outcome_headers::<Box<[StorageFieldV1]>>()?,
        outcome_headers::<Vec<StorageVariantV1>>()?,
        outcome_headers::<Box<[StorageVariantV1]>>()?,
        outcome_headers::<Vec<Function>>()?,
        outcome_headers::<Vec<Kernel>>()?,
        outcome_headers::<Vec<BasicBlock>>()?,
        outcome_headers::<Vec<Operation>>()?,
        outcome_headers::<Vec<ValueDef>>()?,
        outcome_headers::<Vec<ValueId>>()?,
        outcome_headers::<Vec<Type>>()?,
        recursive_types,
    ];
    fixed.into_iter().try_fold(0_usize, |sum, item| {
        sum.checked_add(item).ok_or(ResourceError::Arithmetic)
    })
}
