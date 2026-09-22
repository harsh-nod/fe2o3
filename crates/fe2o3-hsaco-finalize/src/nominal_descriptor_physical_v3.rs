//! Physical checks consume V3 cursors without erasing the stored nominal types.

use fe2o3_hsaco::{COV6_IMPLICIT_ARGUMENT_BYTES, InspectedKernelBindings};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DeviceDescriptorTableV3, RequiredWavefrontWidthV2,
};

use crate::{
    FinalizationError,
    nominal_descriptor_finalization_v3::{NominalFinalizationErrorV3, ResultV3},
    validate_kernel_tail_and_launch, validate_physical_argument,
};

pub(crate) fn cross_check<E>(
    bindings: &InspectedKernelBindings,
    table: &DeviceDescriptorTableV3<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<(), E> {
    // Fixed table facts, independent of the separately charged wire decoder.
    charge(64).map_err(NominalFinalizationErrorV3::Work)?;
    let hsaco = bindings.inspection();
    let matches = matches!(
        (table.code_object_version(), hsaco.code_object_version()),
        (CodeObjectVersion::V4, fe2o3_hsaco::CodeObjectVersion::V4)
            | (CodeObjectVersion::V5, fe2o3_hsaco::CodeObjectVersion::V5)
            | (CodeObjectVersion::V6, fe2o3_hsaco::CodeObjectVersion::V6)
    );
    if !matches {
        return Err(FinalizationError::CodeObjectVersionMismatch.into());
    }
    if table.device_target().as_amd_target_id() != hsaco.target() {
        return Err(FinalizationError::DeviceTargetMismatch.into());
    }
    // Both decoders enforce unique entry names. Equal counts plus the forward
    // membership checks below establish exact closure without another traversal.
    if table.kernel_count() != hsaco.kernels().len() {
        return Err(FinalizationError::KernelCountMismatch {
            descriptor: table.kernel_count(),
            metadata: hsaco.kernels().len(),
        }
        .into());
    }
    for index in 0..table.kernel_count() {
        let kernel = table.kernel(index, charge)?;
        // Symbol comparison plus fixed ABI, wave, hidden-tail and launch checks.
        charge(kernel.descriptor_symbol().len() + 128).map_err(NominalFinalizationErrorV3::Work)?;
        let entry = kernel.entry_name();
        charge(hsaco.kernels().len() * (entry.len() + 1))
            .map_err(NominalFinalizationErrorV3::Work)?;
        let metadata_index = hsaco
            .kernels()
            .iter()
            .position(|k| k.name() == entry)
            .ok_or_else(|| FinalizationError::DescriptorKernelMissingInMetadata {
                entry_name: entry.to_owned(),
            })?;
        let metadata = &hsaco.kernels()[metadata_index];
        if kernel.descriptor_symbol() != metadata.symbol() {
            return Err(FinalizationError::KernelDescriptorSymbolMismatch {
                entry_name: entry.to_owned(),
            }
            .into());
        }
        charge(bindings.bindings().len()).map_err(NominalFinalizationErrorV3::Work)?;
        let binding = bindings
            .bindings()
            .iter()
            .find(|b| b.kernel_index() == metadata_index)
            .ok_or_else(|| FinalizationError::KernelBindingClosureMismatch {
                entry_name: entry.to_owned(),
            })?;
        let layout = kernel.abi_layout();
        let width = match table.requirement(index, charge)?.wavefront_width() {
            RequiredWavefrontWidthV2::Wave32 => 32,
            RequiredWavefrontWidthV2::Wave64 => 64,
        };
        if metadata.wavefront_size() != width || binding.descriptor().wavefront_size() != width {
            return Err(FinalizationError::KernelWavefrontSizeMismatch {
                entry_name: entry.to_owned(),
                descriptor: width,
                metadata: metadata.wavefront_size(),
                hardware: binding.descriptor().wavefront_size(),
            }
            .into());
        }
        let total = u64::from(layout.kernarg_segment_size());
        let explicit = u64::from(layout.explicit_argument_size());
        let observed = metadata.kernarg_segment_size();
        // COV6 declares the complete implicit ABI even when LLVM omits unused
        // hidden arguments. Both physical forms must agree with the hardware .kd.
        let sizes_match = if table.code_object_version() == CodeObjectVersion::V6 {
            total == explicit + COV6_IMPLICIT_ARGUMENT_BYTES
                && if metadata.hidden_arguments().is_empty() {
                    observed == explicit
                        && metadata.implicit_argument_offset().is_none()
                        && metadata.implicit_argument_size() == 0
                } else {
                    observed == total
                        && metadata.implicit_argument_size() == COV6_IMPLICIT_ARGUMENT_BYTES
                }
        } else {
            total == observed
        };
        if !sizes_match || u64::from(binding.descriptor().kernarg_size()) != observed {
            return Err(FinalizationError::KernargSegmentSizeMismatch {
                entry_name: entry.to_owned(),
                descriptor: layout.kernarg_segment_size(),
                metadata: observed,
            }
            .into());
        }
        if u64::from(layout.kernarg_segment_alignment()) != metadata.kernarg_segment_alignment() {
            return Err(FinalizationError::KernargSegmentAlignmentMismatch {
                entry_name: entry.to_owned(),
                descriptor: layout.kernarg_segment_alignment(),
                metadata: metadata.kernarg_segment_alignment(),
            }
            .into());
        }
        if kernel.component_count() != metadata.explicit_arguments().len() {
            return Err(FinalizationError::ExplicitArgumentCountMismatch {
                entry_name: entry.to_owned(),
                descriptor: kernel.component_count(),
                metadata: metadata.explicit_arguments().len(),
            }
            .into());
        }
        let mut arguments = kernel.arguments();
        let mut physical_index = 0;
        while let Some(argument) = arguments.next(charge)? {
            let source = table.source_type(argument.source_type(), charge)?;
            for i in 0..argument.component_count() {
                let p = argument.component(i, charge)?;
                charge(32).map_err(NominalFinalizationErrorV3::Work)?;
                validate_physical_argument(
                    entry,
                    physical_index,
                    p.kind,
                    p.access,
                    p.alias,
                    p.offset,
                    p.size,
                    p.alignment,
                    source.descriptor().physical_scalar(),
                    &metadata.explicit_arguments()[physical_index],
                )?;
                physical_index += 1;
            }
        }
        validate_kernel_tail_and_launch(entry, layout, kernel.launch(), metadata, *binding)?;
    }
    Ok(())
}
