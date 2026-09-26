//! Shared physical checks borrow nominal rows; public V4 tables retain their contracts.

use fe2o3_hsaco::{COV6_IMPLICIT_ARGUMENT_BYTES, InspectedKernelBindings};
use fe2o3_kernel_descriptor::{
    ArgumentCursorV3, CodeObjectVersion, DescriptorWireErrorV3, DeviceDescriptorTableV3,
    DeviceDescriptorTableV4, DeviceDescriptorTableV5, DeviceTargetV1, KernelAbiLayoutV1,
    KernelTargetRequirementsV2, LaunchConstraintsV1, RequiredWavefrontWidthV2, RustTypeIdentity,
    SourceTypeRecordV3,
};

use crate::{
    FinalizationError, nominal_descriptor_common::Failure,
    nominal_descriptor_finalization_v3::NominalFinalizationErrorV3,
    nominal_descriptor_finalization_v4::NominalFinalizationErrorV4,
    nominal_descriptor_finalization_v5::NominalFinalizationErrorV5,
    validate_kernel_tail_and_launch, validate_physical_argument,
};

pub(crate) struct PhysicalKernel<'view, 'wire> {
    entry: &'wire str,
    symbol: &'wire str,
    layout: KernelAbiLayoutV1,
    launch: LaunchConstraintsV1,
    component_count: usize,
    arguments: ArgumentCursorV3<'view, 'wire>,
}

// The codec's QUERY storage covers its native kernel cursor; this additional
// projection can coexist with it, including the copied launch and ABI values.
pub(crate) const PHYSICAL_PROJECTION_STORAGE: usize = size_of::<PhysicalKernel<'static, 'static>>()
    + size_of::<LaunchConstraintsV1>()
    + size_of::<KernelAbiLayoutV1>();

pub(crate) trait PhysicalTable<'wire, E> {
    type Error: Failure<E> + From<DescriptorWireErrorV3<E>>;
    fn code_object_version(&self) -> CodeObjectVersion;
    fn device_target(&self) -> DeviceTargetV1;
    fn kernel_count(&self) -> usize;
    fn kernel(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<PhysicalKernel<'_, 'wire>, Self::Error>;
    fn requirement(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<KernelTargetRequirementsV2, Self::Error>;
    fn source_type(
        &self,
        identity: RustTypeIdentity,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<SourceTypeRecordV3, Self::Error>;
}

// Only the physical projection is shared. Each adapter queries its original
// validated table, and cannot expose a V3 table from a V4 descriptor.
macro_rules! physical_table {
    ($table:ident, $error:ident) => {
        impl<'wire, E> PhysicalTable<'wire, E> for $table<'wire> {
            type Error = $error<E>;
            fn code_object_version(&self) -> CodeObjectVersion {
                self.code_object_version()
            }
            fn device_target(&self) -> DeviceTargetV1 {
                self.device_target()
            }
            fn kernel_count(&self) -> usize {
                self.kernel_count()
            }
            fn kernel(
                &self,
                index: usize,
                charge: &mut impl FnMut(usize) -> Result<(), E>,
            ) -> Result<PhysicalKernel<'_, 'wire>, Self::Error> {
                let kernel = self.kernel(index, charge)?;
                Ok(PhysicalKernel {
                    entry: kernel.entry_name(),
                    symbol: kernel.descriptor_symbol(),
                    layout: kernel.abi_layout(),
                    launch: kernel.launch().clone(),
                    component_count: kernel.component_count(),
                    arguments: kernel.arguments(),
                })
            }
            fn requirement(
                &self,
                index: usize,
                charge: &mut impl FnMut(usize) -> Result<(), E>,
            ) -> Result<KernelTargetRequirementsV2, Self::Error> {
                Ok(self.requirement(index, charge)?)
            }
            fn source_type(
                &self,
                identity: RustTypeIdentity,
                charge: &mut impl FnMut(usize) -> Result<(), E>,
            ) -> Result<SourceTypeRecordV3, Self::Error> {
                Ok(self.source_type(identity, charge)?)
            }
        }
    };
}
physical_table!(DeviceDescriptorTableV3, NominalFinalizationErrorV3);
physical_table!(DeviceDescriptorTableV4, NominalFinalizationErrorV4);
physical_table!(DeviceDescriptorTableV5, NominalFinalizationErrorV5);

pub(crate) fn cross_check<'wire, E, T: PhysicalTable<'wire, E>>(
    bindings: &InspectedKernelBindings,
    table: &T,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), T::Error> {
    // Fixed table facts, independent of the separately charged wire decoder.
    charge(64).map_err(T::Error::work)?;
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
        charge(kernel.symbol.len() + 128).map_err(T::Error::work)?;
        let entry = kernel.entry;
        charge(hsaco.kernels().len() * (entry.len() + 1)).map_err(T::Error::work)?;
        let metadata_index = hsaco
            .kernels()
            .iter()
            .position(|k| k.name() == entry)
            .ok_or_else(|| FinalizationError::DescriptorKernelMissingInMetadata {
                entry_name: entry.to_owned(),
            })?;
        let metadata = &hsaco.kernels()[metadata_index];
        if kernel.symbol != metadata.symbol() {
            return Err(FinalizationError::KernelDescriptorSymbolMismatch {
                entry_name: entry.to_owned(),
            }
            .into());
        }
        charge(bindings.bindings().len()).map_err(T::Error::work)?;
        let binding = bindings
            .bindings()
            .iter()
            .find(|b| b.kernel_index() == metadata_index)
            .ok_or_else(|| FinalizationError::KernelBindingClosureMismatch {
                entry_name: entry.to_owned(),
            })?;
        let layout = kernel.layout;
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
        if kernel.component_count != metadata.explicit_arguments().len() {
            return Err(FinalizationError::ExplicitArgumentCountMismatch {
                entry_name: entry.to_owned(),
                descriptor: kernel.component_count,
                metadata: metadata.explicit_arguments().len(),
            }
            .into());
        }
        let mut arguments = kernel.arguments;
        let mut physical_index = 0;
        while let Some(argument) = arguments.next(charge)? {
            let source = table.source_type(argument.source_type(), charge)?;
            for i in 0..argument.component_count() {
                let p = argument.component(i, charge)?;
                charge(32).map_err(T::Error::work)?;
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
        validate_kernel_tail_and_launch(entry, layout, &kernel.launch, metadata, *binding)?;
    }
    Ok(())
}
