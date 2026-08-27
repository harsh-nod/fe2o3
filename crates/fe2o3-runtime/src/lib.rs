#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use core::fmt;

use fe2o3_amdhsa_loader::{
    AdmittedProfile, KernelClosureError, KernelIdentityInputsV1, PlanError,
    SelectedKernelResourceBindingV1, validate,
};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_hsaco::{HiddenArgument, HiddenValueKind};
use fe2o3_kfd::{
    Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchPointerFixupV1, Gfx942KfdDispatchRequestErrorV1,
    Gfx942KfdDispatchRequestV1,
};

const COV6_IMPLICIT_KERNARG_BYTES_V1: usize = 256;
const DIRECT_KFD_KERNARG_ALIGNMENT_V1: u64 = 16;
const GFX942_WAVEFRONT_SIZE_V1: u32 = 64;
const GFX942_MAX_GROUP_SEGMENT_BYTES_V1: u64 = 64 * 1024;

/// Complete caller-owned data needed before loader and ABI admission.
#[must_use]
pub struct Gfx942RuntimeDispatchInputsV1 {
    explicit_kernarg: Vec<u8>,
    buffers: Vec<Gfx942KfdDispatchBufferV1>,
    pointer_fixups: Vec<Gfx942KfdDispatchPointerFixupV1>,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    timeout_milliseconds: u32,
}

impl Gfx942RuntimeDispatchInputsV1 {
    pub fn new(
        explicit_kernarg: Vec<u8>,
        buffers: Vec<Gfx942KfdDispatchBufferV1>,
        pointer_fixups: Vec<Gfx942KfdDispatchPointerFixupV1>,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> Self {
        Self {
            explicit_kernarg,
            buffers,
            pointer_fixups,
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
        }
    }
}

/// Loader-bound request plus exact immutable object and selected-kernel identities.
///
/// This value is not launch authority. Its only transition yields the unsafe
/// KFD mechanics request consumed later by the Worker V3 runtime gate.
#[must_use = "preparation does not execute or authorize the selected kernel"]
pub struct PreparedGfx942RuntimeDispatchV1 {
    request: Gfx942KfdDispatchRequestV1,
    identity: KernelIdentityInputsV1,
    kernel_name: String,
    descriptor_offset: u64,
    static_group_segment_bytes: u64,
    dynamic_group_segment_bytes: u32,
    packet_group_segment_bytes: u32,
}

impl fmt::Debug for PreparedGfx942RuntimeDispatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedGfx942RuntimeDispatchV1")
            .field("kernel_name", &self.kernel_name)
            .field("descriptor_offset", &self.descriptor_offset)
            .field(
                "static_group_segment_bytes",
                &self.static_group_segment_bytes,
            )
            .field(
                "dynamic_group_segment_bytes",
                &self.dynamic_group_segment_bytes,
            )
            .field(
                "packet_group_segment_bytes",
                &self.packet_group_segment_bytes,
            )
            .finish_non_exhaustive()
    }
}

impl PreparedGfx942RuntimeDispatchV1 {
    pub const fn identity(&self) -> KernelIdentityInputsV1 {
        self.identity
    }

    pub fn kernel_name(&self) -> &str {
        &self.kernel_name
    }

    pub const fn descriptor_offset(&self) -> u64 {
        self.descriptor_offset
    }

    pub const fn static_group_segment_bytes(&self) -> u64 {
        self.static_group_segment_bytes
    }

    pub const fn dynamic_group_segment_bytes(&self) -> u32 {
        self.dynamic_group_segment_bytes
    }

    pub const fn packet_group_segment_bytes(&self) -> u32 {
        self.packet_group_segment_bytes
    }

    /// Returns the mechanics-only request. Calling its KFD execution function
    /// still requires the complete unsafe Worker V3 contract.
    pub fn into_unchecked_kfd_request(self) -> Gfx942KfdDispatchRequestV1 {
        self.request
    }
}

/// Failure before native device mutation.
#[derive(Debug)]
#[non_exhaustive]
pub enum Gfx942RuntimePreparationErrorV1 {
    Envelope(PlanError),
    Kernel(KernelClosureError),
    ImageSize,
    DescriptorRange,
    UnsupportedResource(&'static str),
    WorkgroupMismatch,
    WorkgroupCountExceeded { axis: usize },
    KernargLayout,
    HiddenArgument { index: usize, detail: &'static str },
    KfdRequest(Gfx942KfdDispatchRequestErrorV1),
}

impl fmt::Display for Gfx942RuntimePreparationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942RuntimePreparationErrorV1 {}

impl From<PlanError> for Gfx942RuntimePreparationErrorV1 {
    fn from(value: PlanError) -> Self {
        Self::Envelope(value)
    }
}

impl From<KernelClosureError> for Gfx942RuntimePreparationErrorV1 {
    fn from(value: KernelClosureError) -> Self {
        Self::Kernel(value)
    }
}

impl From<Gfx942KfdDispatchRequestErrorV1> for Gfx942RuntimePreparationErrorV1 {
    fn from(value: Gfx942KfdDispatchRequestErrorV1) -> Self {
        Self::KfdRequest(value)
    }
}

/// Validates and materializes one exact COV6 kernel into an address-free KFD request.
///
/// The operation checks the complete object and selected descriptor, derives
/// resource fields from the closure, and initializes every declared hidden
/// argument. It performs no KFD operation and grants no execution authority.
pub fn prepare_gfx942_runtime_dispatch_v1(
    hsaco: &[u8],
    kernel_name: &str,
    inputs: Gfx942RuntimeDispatchInputsV1,
) -> Result<PreparedGfx942RuntimeDispatchV1, Gfx942RuntimePreparationErrorV1> {
    let closure = validate(hsaco, AdmittedProfile::Gfx942XnackOffCov6)?.bind_kernel(kernel_name)?;
    let resources = closure.resources();
    validate_resources(
        resources,
        inputs.geometry,
        inputs.dynamic_group_segment_bytes,
    )?;

    let kernel = closure.selected_kernel();
    let total_kernarg = usize::try_from(resources.kernarg_segment_size())
        .map_err(|_| Gfx942RuntimePreparationErrorV1::KernargLayout)?;
    let explicit_kernarg = inputs.explicit_kernarg.len();
    let implicit_offset = kernel
        .implicit_argument_offset()
        .map(usize::try_from)
        .transpose()
        .map_err(|_| Gfx942RuntimePreparationErrorV1::KernargLayout)?;
    let implicit_size = usize::try_from(kernel.implicit_argument_size())
        .map_err(|_| Gfx942RuntimePreparationErrorV1::KernargLayout)?;
    if implicit_offset != Some(explicit_kernarg)
        || implicit_size != COV6_IMPLICIT_KERNARG_BYTES_V1
        || explicit_kernarg
            .checked_add(implicit_size)
            .is_none_or(|bytes| bytes != total_kernarg)
    {
        return Err(Gfx942RuntimePreparationErrorV1::KernargLayout);
    }
    let mut kernarg = vec![0_u8; total_kernarg];
    kernarg[..explicit_kernarg].copy_from_slice(&inputs.explicit_kernarg);
    initialize_hidden_arguments(
        &mut kernarg,
        kernel.hidden_arguments(),
        inputs.geometry,
        inputs.dynamic_group_segment_bytes,
    )?;

    let plan = closure.envelope().plan();
    let image_bytes = usize::try_from(closure.envelope().materialization().image_len())
        .map_err(|_| Gfx942RuntimePreparationErrorV1::ImageSize)?;
    let mut image = vec![0_u8; image_bytes];
    closure
        .materialize_into(&mut image)
        .map_err(|_| Gfx942RuntimePreparationErrorV1::ImageSize)?;
    let binding = closure.selected_binding();
    let descriptor_offset = binding
        .descriptor_address()
        .checked_sub(plan.image_start())
        .ok_or(Gfx942RuntimePreparationErrorV1::DescriptorRange)?;
    if descriptor_offset
        .checked_add(64)
        .is_none_or(|end| end > plan.image_end() - plan.image_start())
    {
        return Err(Gfx942RuntimePreparationErrorV1::DescriptorRange);
    }

    let kernarg_alignment = resources
        .kernarg_segment_alignment()
        .max(DIRECT_KFD_KERNARG_ALIGNMENT_V1);
    let identity = closure.identity_inputs();
    let static_group_segment_bytes = resources.group_segment_fixed_size();
    // AQL carries the complete per-workgroup allocation, whereas the COV6
    // hidden field (when present) carries only the dynamic contribution.
    let packet_group_segment_bytes = static_group_segment_bytes
        .checked_add(u64::from(inputs.dynamic_group_segment_bytes))
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "total group segment representation",
        ))?;
    let request = Gfx942KfdDispatchRequestV1::new(
        image,
        descriptor_offset,
        kernarg,
        kernarg_alignment,
        inputs.buffers,
        inputs.pointer_fixups,
        inputs.geometry,
        0,
        packet_group_segment_bytes,
        inputs.timeout_milliseconds,
    )?;
    Ok(PreparedGfx942RuntimeDispatchV1 {
        request,
        identity,
        kernel_name: kernel_name.to_owned(),
        descriptor_offset,
        static_group_segment_bytes,
        dynamic_group_segment_bytes: inputs.dynamic_group_segment_bytes,
        packet_group_segment_bytes,
    })
}

fn validate_resources(
    resources: SelectedKernelResourceBindingV1,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
) -> Result<(), Gfx942RuntimePreparationErrorV1> {
    if resources.wavefront_size() != GFX942_WAVEFRONT_SIZE_V1 {
        return Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "non-wave64 kernel",
        ));
    }
    if resources.private_segment_fixed_size() != 0
        || resources.sgpr_spill_count().unwrap_or(0) != 0
        || resources.vgpr_spill_count().unwrap_or(0) != 0
    {
        return Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "private segment or spill scratch",
        ));
    }
    if resources.cluster_dims().is_some() {
        return Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "cluster launch",
        ));
    }
    let workgroup = geometry.workgroup().map(u32::from);
    if resources
        .required_workgroup_size()
        .is_some_and(|required| required != workgroup)
        || workgroup.into_iter().product::<u32>() > resources.max_flat_workgroup_size()
    {
        return Err(Gfx942RuntimePreparationErrorV1::WorkgroupMismatch);
    }
    let grid = geometry.grid();
    for (axis, maximum) in resources.max_workgroups().into_iter().enumerate() {
        let count = ceil_div_u32(grid[axis], workgroup[axis]);
        if maximum.is_some_and(|maximum| count > maximum) {
            return Err(Gfx942RuntimePreparationErrorV1::WorkgroupCountExceeded { axis });
        }
    }
    if resources
        .group_segment_fixed_size()
        .checked_add(u64::from(dynamic_group_segment_bytes))
        .is_none_or(|total| total > GFX942_MAX_GROUP_SEGMENT_BYTES_V1)
    {
        return Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "group segment capacity",
        ));
    }
    let alignment = resources.kernarg_segment_alignment();
    if alignment == 0 || alignment > 4096 || !alignment.is_power_of_two() {
        return Err(Gfx942RuntimePreparationErrorV1::KernargLayout);
    }
    Ok(())
}

fn initialize_hidden_arguments(
    kernarg: &mut [u8],
    hidden: &[HiddenArgument],
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
) -> Result<(), Gfx942RuntimePreparationErrorV1> {
    let mut observed_dynamic_lds = false;
    for (index, argument) in hidden.iter().copied().enumerate() {
        let value = hidden_value(argument.value_kind(), geometry, dynamic_group_segment_bytes)
            .map_err(|detail| Gfx942RuntimePreparationErrorV1::HiddenArgument { index, detail })?;
        let offset = usize::try_from(argument.offset()).map_err(|_| {
            Gfx942RuntimePreparationErrorV1::HiddenArgument {
                index,
                detail: "offset conversion",
            }
        })?;
        let size = usize::try_from(argument.size()).map_err(|_| {
            Gfx942RuntimePreparationErrorV1::HiddenArgument {
                index,
                detail: "size conversion",
            }
        })?;
        let end =
            offset
                .checked_add(size)
                .ok_or(Gfx942RuntimePreparationErrorV1::HiddenArgument {
                    index,
                    detail: "range overflow",
                })?;
        let destination = kernarg.get_mut(offset..end).ok_or(
            Gfx942RuntimePreparationErrorV1::HiddenArgument {
                index,
                detail: "range outside kernarg",
            },
        )?;
        if destination.len() != value.len() {
            return Err(Gfx942RuntimePreparationErrorV1::HiddenArgument {
                index,
                detail: "kind width mismatch",
            });
        }
        destination.copy_from_slice(&value);
        observed_dynamic_lds |= argument.value_kind() == HiddenValueKind::DynamicLdsSize;
    }
    if dynamic_group_segment_bytes != 0 && !observed_dynamic_lds {
        return Err(Gfx942RuntimePreparationErrorV1::HiddenArgument {
            index: hidden.len(),
            detail: "dynamic LDS requested without ABI field",
        });
    }
    Ok(())
}

fn hidden_value(
    kind: HiddenValueKind,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
) -> Result<Vec<u8>, &'static str> {
    let grid = geometry.grid();
    let workgroup = geometry.workgroup().map(u32::from);
    let u32_value = |value: u32| value.to_le_bytes().to_vec();
    let u16_value = |value: u16| value.to_le_bytes().to_vec();
    let u64_value = |value: u64| value.to_le_bytes().to_vec();
    match kind {
        HiddenValueKind::BlockCountX => Ok(u32_value(ceil_div_u32(grid[0], workgroup[0]))),
        HiddenValueKind::BlockCountY => Ok(u32_value(ceil_div_u32(grid[1], workgroup[1]))),
        HiddenValueKind::BlockCountZ => Ok(u32_value(ceil_div_u32(grid[2], workgroup[2]))),
        HiddenValueKind::GroupSizeX => Ok(u16_value(workgroup[0] as u16)),
        HiddenValueKind::GroupSizeY => Ok(u16_value(workgroup[1] as u16)),
        HiddenValueKind::GroupSizeZ => Ok(u16_value(workgroup[2] as u16)),
        HiddenValueKind::RemainderX => Ok(u16_value((grid[0] % workgroup[0]) as u16)),
        HiddenValueKind::RemainderY => Ok(u16_value((grid[1] % workgroup[1]) as u16)),
        HiddenValueKind::RemainderZ => Ok(u16_value((grid[2] % workgroup[2]) as u16)),
        HiddenValueKind::GlobalOffsetX
        | HiddenValueKind::GlobalOffsetY
        | HiddenValueKind::GlobalOffsetZ
        | HiddenValueKind::None
        | HiddenValueKind::PrintfBuffer
        | HiddenValueKind::HostcallBuffer
        | HiddenValueKind::HeapV1
        | HiddenValueKind::DefaultQueue
        | HiddenValueKind::CompletionAction
        | HiddenValueKind::MultigridSyncArgument
        | HiddenValueKind::QueuePointer => Ok(u64_value(0)),
        HiddenValueKind::GridDimensions => Ok(u16_value(geometry.dimensions())),
        HiddenValueKind::DynamicLdsSize => Ok(u32_value(dynamic_group_segment_bytes)),
        HiddenValueKind::PrivateBase | HiddenValueKind::SharedBase => {
            Err("gfx942 aperture ABI field is unsupported")
        }
    }
}

const fn ceil_div_u32(value: u32, divisor: u32) -> u32 {
    value / divisor + if value % divisor == 0 { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> AqlDispatchGeometryV1 {
        AqlDispatchGeometryV1::new([130, 4, 1], [64, 2, 1]).unwrap()
    }

    #[test]
    fn geometry_hidden_values_are_derived_without_native_addresses() {
        assert_eq!(
            hidden_value(HiddenValueKind::BlockCountX, geometry(), 256).unwrap(),
            3_u32.to_le_bytes()
        );
        assert_eq!(
            hidden_value(HiddenValueKind::BlockCountY, geometry(), 256).unwrap(),
            2_u32.to_le_bytes()
        );
        assert_eq!(
            hidden_value(HiddenValueKind::RemainderX, geometry(), 256).unwrap(),
            2_u16.to_le_bytes()
        );
        assert_eq!(
            hidden_value(HiddenValueKind::GridDimensions, geometry(), 256).unwrap(),
            2_u16.to_le_bytes()
        );
        assert_eq!(
            hidden_value(HiddenValueKind::DynamicLdsSize, geometry(), 256).unwrap(),
            256_u32.to_le_bytes()
        );
    }

    #[test]
    fn optional_runtime_pointers_are_zero_and_gfx8_apertures_reject() {
        for kind in [
            HiddenValueKind::HostcallBuffer,
            HiddenValueKind::MultigridSyncArgument,
            HiddenValueKind::HeapV1,
            HiddenValueKind::DefaultQueue,
            HiddenValueKind::CompletionAction,
            HiddenValueKind::QueuePointer,
        ] {
            assert_eq!(hidden_value(kind, geometry(), 0).unwrap(), [0; 8]);
        }
        assert!(hidden_value(HiddenValueKind::PrivateBase, geometry(), 0).is_err());
        assert!(hidden_value(HiddenValueKind::SharedBase, geometry(), 0).is_err());
    }

    #[test]
    fn ceil_division_is_exact_at_and_after_boundaries() {
        assert_eq!(ceil_div_u32(64, 64), 1);
        assert_eq!(ceil_div_u32(65, 64), 2);
        assert_eq!(ceil_div_u32(u32::MAX, u16::MAX.into()), 65_537);
    }
}
