//! Consuming, non-authorizing translation into the existing fixed queue format.

use fe2o3_amdhsa_loader::{KernelDispatchAbiErrorV1, KernelGlobalBufferAbiV1, SegmentOrdinal};
use fe2o3_hsaco::{ArgumentAccess, ExplicitValueKind};
use fe2o3_kfd::{
    Gfx942DispatchBindingErrorV1, Gfx942DispatchBufferBindingV1, Gfx942FixedDispatchPacketV1,
    Gfx942KfdDispatchRequestPartsV1, project_gfx942_fixed_host_packet_v1,
};

use super::*;

/// A complete inert host recipe checked against the existing persistent queue.
///
/// The original contract digest remains authoritative only when independently
/// admitted by Worker V3. The packet's normalized hidden suffix is not a new
/// authority identity. This type has no execution, completion or Clone operation.
/// Its timeout is retained data, not an implemented persistent timeout policy.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::PreparedGfx942PersistentDispatchV1;
/// fn duplicate(value: PreparedGfx942PersistentDispatchV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_runtime::PreparedGfx942PersistentDispatchV1;
/// fn extract(value: PreparedGfx942PersistentDispatchV1) { let _ = value.data; }
/// ```
#[must_use]
pub struct PreparedGfx942PersistentDispatchV1 {
    packet: Gfx942FixedDispatchPacketV1,
    data: PersistentDispatchDataV1,
}

pub(crate) struct PersistentDispatchDataV1 {
    source_identity: std::sync::Arc<()>,
    description: RuntimeDispatchDescriptionV1,
    executable_image: Vec<u8>,
    kernarg_alignment: u64,
    buffers: Vec<Gfx942KfdDispatchBufferV1>,
    pointer_fixups: Vec<Gfx942KfdDispatchPointerFixupV1>,
    buffer_policies: Vec<Gfx942RuntimePreparedBufferPolicyV1>,
    timeout_milliseconds: u32,
}

impl PersistentDispatchDataV1 {
    pub(crate) fn source_identity(&self) -> &std::sync::Arc<()> {
        &self.source_identity
    }
    pub fn kernel_name(&self) -> &str {
        &self.description.kernel_name
    }

    pub const fn identity(&self) -> KernelIdentityInputsV1 {
        self.description.identity
    }

    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.description.finalized_hsaco_length
    }

    pub(crate) fn complete_buffer_policies_match(&self) -> bool {
        complete_buffer_policies_match(&self.buffer_policies, &self.buffers)
    }

    pub const fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.description.dispatch_contract_sha256
    }

    pub const fn descriptor_offset(&self) -> u64 {
        self.description.descriptor_offset
    }

    pub const fn kernarg_alignment(&self) -> u64 {
        self.kernarg_alignment
    }

    pub const fn timeout_milliseconds(&self) -> u32 {
        self.timeout_milliseconds
    }

    pub fn executable_image(&self) -> &[u8] {
        &self.executable_image
    }

    pub fn buffers(&self) -> &[Gfx942KfdDispatchBufferV1] {
        &self.buffers
    }

    pub fn pointer_fixups(&self) -> &[Gfx942KfdDispatchPointerFixupV1] {
        &self.pointer_fixups
    }

    pub fn buffer_access(&self, index: usize) -> Option<Gfx942RuntimeBufferAccessV1> {
        self.buffer_policies.get(index).map(|policy| policy.access)
    }
}

// Both inert representations expose the same immutable description, never control.
macro_rules! persistent_data_accessors {
    ($name:ident) => {
        impl $name {
            pub fn kernel_name(&self) -> &str {
                self.data.kernel_name()
            }
            pub const fn identity(&self) -> KernelIdentityInputsV1 {
                self.data.identity()
            }
            pub const fn finalized_hsaco_length(&self) -> u64 {
                self.data.finalized_hsaco_length()
            }
            pub const fn dispatch_contract_sha256(&self) -> [u8; 32] {
                self.data.dispatch_contract_sha256()
            }
            pub const fn descriptor_offset(&self) -> u64 {
                self.data.descriptor_offset()
            }
            pub const fn kernarg_alignment(&self) -> u64 {
                self.data.kernarg_alignment()
            }
            pub const fn timeout_milliseconds(&self) -> u32 {
                self.data.timeout_milliseconds()
            }
            pub fn executable_image(&self) -> &[u8] {
                self.data.executable_image()
            }
            pub fn buffers(&self) -> &[Gfx942KfdDispatchBufferV1] {
                self.data.buffers()
            }
            pub fn pointer_fixups(&self) -> &[Gfx942KfdDispatchPointerFixupV1] {
                self.data.pointer_fixups()
            }
            pub fn buffer_access(&self, index: usize) -> Option<Gfx942RuntimeBufferAccessV1> {
                self.data.buffer_access(index)
            }
            pub(crate) fn data(&self) -> &PersistentDispatchDataV1 {
                &self.data
            }
        }
    };
}

persistent_data_accessors!(PreparedGfx942PersistentDispatchV1);
persistent_data_accessors!(GeneratedGfx942PersistentStorageV1);

impl PreparedGfx942PersistentDispatchV1 {
    pub const fn packet(&self) -> &Gfx942FixedDispatchPacketV1 {
        &self.packet
    }

    /// Moves the complete inert recipe into closed generated custody without copying.
    pub fn into_generated_storage_v1(self) -> GeneratedGfx942PersistentStorageV1 {
        GeneratedGfx942PersistentStorageV1 {
            data: self.data,
            control: Some(self.packet),
        }
    }
}

/// Complete generated source storage with a private one-shot control transfer.
/// Descriptive access grants neither native ownership nor publication authority.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::GeneratedGfx942PersistentStorageV1;
/// fn duplicate(value: GeneratedGfx942PersistentStorageV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_runtime::GeneratedGfx942PersistentStorageV1;
/// fn extract(value: GeneratedGfx942PersistentStorageV1) { let _ = value.control; }
/// ```
/// ```compile_fail,E0599
/// use fe2o3_runtime::GeneratedGfx942PersistentStorageV1;
/// fn packet(value: &GeneratedGfx942PersistentStorageV1) { let _ = value.packet(); }
/// ```
/// ```compile_fail,E0624
/// use fe2o3_runtime::GeneratedGfx942PersistentStorageV1;
/// fn take(value: &mut GeneratedGfx942PersistentStorageV1) {
///     value.transfer_control_into(&mut None);
/// }
/// ```
#[doc(hidden)]
#[must_use]
pub struct GeneratedGfx942PersistentStorageV1 {
    data: PersistentDispatchDataV1,
    // Only the runtime's exact rooted adoption destination may take this packet.
    control: Option<Gfx942FixedDispatchPacketV1>,
}

impl GeneratedGfx942PersistentStorageV1 {
    pub(crate) fn control_available(&self) -> bool {
        self.control.is_some()
    }

    pub(crate) fn transfer_control_into(
        &mut self,
        destination: &mut Option<Gfx942FixedDispatchPacketV1>,
    ) -> bool {
        if destination.is_some() || self.control.is_none() {
            return false;
        }
        // No callback, allocation or fallible native work separates these moves.
        *destination = self.control.take();
        true
    }
}

impl fmt::Debug for PreparedGfx942PersistentDispatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedGfx942PersistentDispatchV1")
            .field("description", &self.data.description)
            .field("buffers", &self.data.buffers.len())
            .field("timeout_milliseconds", &self.data.timeout_milliseconds)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Gfx942RuntimeProjectionErrorV1 {
    Preparation(Gfx942RuntimePreparationErrorV1),
    Mismatch(&'static str),
    Abi(KernelDispatchAbiErrorV1),
    FixedDispatch(Gfx942DispatchBindingErrorV1),
}

impl fmt::Display for Gfx942RuntimeProjectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942RuntimeProjectionErrorV1 {}

impl PreparedGfx942RuntimeDispatchV1 {
    /// Moves complete storage into a checked fixed-host recipe without native work.
    ///
    /// The original HSACO must match the selected immutable closure. All buffers,
    /// unused storage, fixups, policies and timeout are retained; no bound-window
    /// snapshot, allocation duplication or address substitution occurs. Profiles
    /// unsupported by the existing fixed-dispatch planner reject before effects.
    /// This does not bind runtime allocation generations or authorize publication.
    pub fn into_persistent_projection_v1(
        self,
        hsaco: &[u8],
    ) -> Result<PreparedGfx942PersistentDispatchV1, Gfx942RuntimeProjectionErrorV1> {
        use Gfx942RuntimeProjectionErrorV1 as Error;
        let Self {
            request,
            buffer_policies,
            description,
        } = self;
        let request = request.into_parts_v1();
        let kernel = validate(hsaco, AdmittedProfile::Gfx942XnackOffCov6)
            .map_err(|error| Error::Preparation(error.into()))?
            .bind_kernel(&description.kernel_name)
            .map_err(|error| Error::Preparation(error.into()))?;
        validate_projection_source(
            &description,
            &request,
            &buffer_policies,
            &kernel,
            hsaco.len(),
        )?;

        let arguments = kernel.selected_kernel().explicit_arguments();
        if arguments
            .iter()
            .filter(|arg| arg.value_kind() == ExplicitValueKind::GlobalBuffer)
            .count()
            != request.pointer_fixups.len()
        {
            return Err(Error::Mismatch("complete global-buffer fixup roster"));
        }
        let mut rows = Vec::with_capacity(request.pointer_fixups.len());
        let mut bindings = Vec::with_capacity(request.pointer_fixups.len());
        for fixup in &request.pointer_fixups {
            let (ordinal, argument) = arguments
                .iter()
                .enumerate()
                .find(|(_, argument)| argument.offset() == fixup.kernarg_offset() as u64)
                .filter(|(_, argument)| argument.value_kind() == ExplicitValueKind::GlobalBuffer)
                .ok_or(Error::Mismatch("fixup does not name a global argument"))?;
            let name = argument
                .name()
                .ok_or(Error::Mismatch("unnamed global argument"))?;
            let buffer = request
                .buffers
                .get(fixup.buffer_index())
                .ok_or(Error::Mismatch("fixup buffer index"))?;
            let byte_len = buffer
                .bytes()
                .len()
                .checked_sub(fixup.buffer_byte_offset())
                .filter(|&length| length != 0)
                .ok_or(Error::Mismatch("fixup buffer extent"))?;
            let access = match buffer_policies[fixup.buffer_index()].access {
                Gfx942RuntimeBufferAccessV1::ReadOnly => ArgumentAccess::ReadOnly,
                Gfx942RuntimeBufferAccessV1::WriteOnly => ArgumentAccess::WriteOnly,
                Gfx942RuntimeBufferAccessV1::ReadWrite => ArgumentAccess::ReadWrite,
            };
            rows.push((
                ordinal,
                name.to_owned(),
                argument.offset(),
                fixup.required_alignment(),
                access,
            ));
            bindings.push(Gfx942DispatchBufferBindingV1::new(
                ordinal,
                fixup.buffer_index(),
                fixup.buffer_byte_offset() as u64,
                byte_len as u64,
            ));
        }
        let abi = rows
            .iter()
            .map(|(ordinal, name, offset, alignment, access)| {
                KernelGlobalBufferAbiV1::new(*ordinal, name, *offset, *alignment, *access)
            })
            .collect::<Vec<_>>();
        let kernel = kernel
            .reconcile_dispatch_abi(description.dispatch_contract_sha256, &abi)
            .map_err(Error::Abi)?;
        let lengths = request
            .buffers
            .iter()
            .map(|buffer| buffer.bytes().len())
            .collect::<Vec<_>>();
        let packet = Gfx942FixedDispatchPacketV1::new(
            0,
            request.geometry,
            description.dynamic_group_segment_bytes,
            request.kernarg_template.into_boxed_slice(),
            bindings.into_boxed_slice(),
        );
        let packet = project_gfx942_fixed_host_packet_v1(&kernel, packet, &lengths)
            .map_err(Error::FixedDispatch)?;
        Ok(PreparedGfx942PersistentDispatchV1 {
            packet,
            data: PersistentDispatchDataV1 {
                source_identity: std::sync::Arc::new(()),
                description,
                executable_image: request.executable_image,
                kernarg_alignment: request.kernarg_alignment,
                buffers: request.buffers,
                pointer_fixups: request.pointer_fixups,
                buffer_policies,
                timeout_milliseconds: request.timeout_milliseconds,
            },
        })
    }
}

fn validate_projection_source(
    description: &RuntimeDispatchDescriptionV1,
    request: &Gfx942KfdDispatchRequestPartsV1,
    policies: &[Gfx942RuntimePreparedBufferPolicyV1],
    kernel: &fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>,
    hsaco_length: usize,
) -> Result<(), Gfx942RuntimeProjectionErrorV1> {
    use Gfx942RuntimeProjectionErrorV1 as Error;
    let resources = kernel.resources();
    if description.finalized_hsaco_length != hsaco_length as u64
        || description.identity != kernel.identity_inputs()
        || description.descriptor_offset != request.descriptor_offset
        || Some(request.descriptor_offset)
            != kernel
                .selected_binding()
                .descriptor_address()
                .checked_sub(kernel.envelope().plan().image_start())
        || !image_matches_materialization(
            &request.executable_image,
            kernel.envelope().materialization(),
        )
    {
        return Err(Error::Mismatch(
            "exact object, selected closure or materialized image",
        ));
    }
    validate_resources(
        resources,
        request.geometry,
        description.dynamic_group_segment_bytes,
    )
    .map_err(Error::Preparation)?;
    if description.geometry != request.geometry
        || description.static_group_segment_bytes != resources.group_segment_fixed_size()
        || description.packet_group_segment_bytes != request.group_segment_size
        || Some(u64::from(request.group_segment_size))
            != resources
                .group_segment_fixed_size()
                .checked_add(u64::from(description.dynamic_group_segment_bytes))
        || request.private_segment_size != 0
        || request.kernarg_alignment
            != resources
                .kernarg_segment_alignment()
                .max(DIRECT_KFD_KERNARG_ALIGNMENT_V1)
    {
        return Err(Error::Mismatch(
            "geometry, descriptor resources or kernarg alignment",
        ));
    }
    if !complete_buffer_policies_match(policies, &request.buffers) {
        return Err(Error::Mismatch("complete buffer-policy roster"));
    }
    let digest = derive_dispatch_contract_sha256_v1(
        description.finalized_hsaco_length,
        description.identity.into(),
        &description.kernel_name,
        &request.executable_image,
        request.descriptor_offset,
        &request.kernarg_template,
        request.kernarg_alignment,
        policies
            .iter()
            .zip(&request.buffers)
            .map(|(policy, buffer)| (policy.access, buffer.bytes())),
        &request.pointer_fixups,
        request.geometry,
        request.group_segment_size,
        request.timeout_milliseconds,
    );
    if digest != description.dispatch_contract_sha256 {
        return Err(Error::Mismatch("complete original dispatch contract"));
    }
    Ok(())
}

fn complete_buffer_policies_match(
    policies: &[Gfx942RuntimePreparedBufferPolicyV1],
    buffers: &[Gfx942KfdDispatchBufferV1],
) -> bool {
    policies.len() == buffers.len()
        && policies.iter().zip(buffers).all(|(policy, buffer)| {
            policy.byte_length == buffer.bytes().len() as u64
                && match policy.access {
                    Gfx942RuntimeBufferAccessV1::ReadOnly => {
                        policy.read_only_initial_bytes.as_deref() == Some(buffer.bytes())
                    }
                    _ => policy.read_only_initial_bytes.is_none(),
                }
        })
}

// Compare the canonical zero-then-copy image without allocating another image.
fn image_matches_materialization(
    image: &[u8],
    plan: &fe2o3_amdhsa_loader::MaterializationPlan<'_>,
) -> bool {
    if image.len() as u64 != plan.image_len() {
        return false;
    }
    let mut cursor = 0;
    for ordinal in [
        SegmentOrdinal::First,
        SegmentOrdinal::Second,
        SegmentOrdinal::Third,
    ] {
        let copy = plan.copy_phase().segment(ordinal);
        let Ok(start) = usize::try_from(copy.destination().offset_from_image_start()) else {
            return false;
        };
        let Some(end) = start.checked_add(copy.source().bytes().len()) else {
            return false;
        };
        if start < cursor
            || end > image.len()
            || image[cursor..start].iter().any(|&byte| byte != 0)
            || image[start..end] != *copy.source().bytes()
        {
            return false;
        }
        cursor = end;
    }
    image[cursor..].iter().all(|&byte| byte == 0)
}

#[cfg(test)]
mod tests;
