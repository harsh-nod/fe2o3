//! Shared genuine loader/mapping mechanism; callers supply their own phase fences.
use super::{
    Context, Kernel, MAX_KERNELS, Result, add_counter, describe_kernel, explain, record_elapsed,
};
use crate::engineering_gfx950_profile::PAGE_BYTES;
use crate::engineering_wire::MAX_KERNARG_BYTES_V1;
use fe2o3_amdhsa_loader::AdmittedProfile;
use fe2o3_kfd_uapi::KfdAllocMemoryFlags;
use sha2::{Digest, Sha256};

impl Context {
    pub(super) fn materialize_admitted_kernel(
        &mut self,
        object: Vec<u8>,
        expected_hash: [u8; 32],
        symbol: String,
    ) -> Result<Kernel> {
        if self.kernels.len() >= MAX_KERNELS
            || <[u8; 32]>::from(Sha256::digest(&object)) != expected_hash
        {
            return Err("kernel count or object hash".into());
        }
        let admission_started = self.profile_started();
        let closure = fe2o3_amdhsa_loader::validate(&object, AdmittedProfile::Gfx950XnackOffCov6)
            .map_err(explain)?
            .bind_kernel(&symbol)
            .map_err(explain)?;
        let resources = closure.resources();
        let inspected = closure.selected_kernel().clone();
        if admission_started.is_some() {
            add_counter(&mut self.counters.kernel_admissions, 1)?;
            record_elapsed(&mut self.counters.kernel_admission_ns, admission_started)?;
        }
        if resources.wavefront_size() != 64
            || resources.private_segment_fixed_size() != 0
            || resources.group_segment_fixed_size() > 160 * 1024
            || resources.kernarg_segment_size() > u64::from(MAX_KERNARG_BYTES_V1)
            || resources.kernarg_segment_alignment() > PAGE_BYTES as u64
            || resources.cluster_dims().is_some()
        {
            return Err("unsupported gfx950 engineering kernel resources".into());
        }
        let metadata = describe_kernel(closure.selected_kernel(), expected_hash)?;
        let descriptor_offset = closure
            .selected_binding()
            .descriptor_address()
            .checked_sub(closure.envelope().plan().image_start())
            .ok_or("descriptor image offset")?;
        let image_len =
            usize::try_from(closure.envelope().materialization().image_len()).map_err(explain)?;
        if descriptor_offset
            .checked_add(64)
            .is_none_or(|end| end > image_len as u64)
            || !descriptor_offset.is_multiple_of(64)
        {
            return Err("descriptor image range/alignment".into());
        }
        let code = self.allocate_resource(image_len, KfdAllocMemoryFlags::EXECUTABLE, |bytes| {
            closure.materialize_into(bytes).map_err(explain)
        })?;
        drop(closure);
        Ok(Kernel {
            object,
            inspected,
            metadata,
            resources,
            code,
            descriptor_offset,
        })
    }
}
