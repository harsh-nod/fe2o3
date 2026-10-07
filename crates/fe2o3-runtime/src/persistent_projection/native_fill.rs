//! Explicit native-family projection; the ordinary entry point stays closed.

use super::*;
use fe2o3_kfd::NativeConditionalFill64PremisesV1;

pub(super) fn binding(
    premises: Option<&NativeConditionalFill64PremisesV1>,
) -> Gfx942RuntimeInvocationBindingV1 {
    match premises {
        None => Gfx942RuntimeInvocationBindingV1::OrdinaryV1,
        Some(p) => Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 {
            contract_identity: *p.contract_identity(),
            premise_identity: *p.identity(),
        },
    }
}

impl PreparedGfx942RuntimeDispatchV1 {
    /// Consumes an unbranded inert recipe into the distinct native V5 family.
    ///
    /// This validates only transport, ABI, numeric and machine shape. The caller
    /// must separately retain genuine native source, proof, currentness and
    /// publication custody through the unsafe execution-authority contract.
    /// Neither old conditional family can enter here, and the resulting family
    /// cannot match an ordinary or nominal authority.
    pub fn into_native_conditional_fill64_projection_v1(
        self,
        hsaco: &[u8],
        premises: NativeConditionalFill64PremisesV1,
    ) -> Result<PreparedGfx942PersistentDispatchV1, Gfx942RuntimeProjectionErrorV1> {
        use Gfx942RuntimeProjectionErrorV1 as Error;
        if self.invocation_binding() != Gfx942RuntimeInvocationBindingV1::OrdinaryV1
            || self.request.conditional_premises_v1().is_some()
            || self.request.mixed_conditional_premises_v26().is_some()
        {
            return Err(Error::Mismatch(
                "native fill cannot rebrand an existing family",
            ));
        }
        let parts = self.request.inspection_v1();
        let [buffer] = parts.buffers else {
            return Err(Error::Mismatch("native fill requires one whole output"));
        };
        let [policy] = self.buffer_policies.as_slice() else {
            return Err(Error::Mismatch("native fill output policy"));
        };
        let [fixup] = parts.pointer_fixups else {
            return Err(Error::Mismatch("native fill requires one pointer fixup"));
        };
        if policy.access != Gfx942RuntimeBufferAccessV1::WriteOnly
            || policy.byte_length != buffer.bytes().len() as u64
            || policy.read_only_initial_bytes.is_some()
            || fixup.kernarg_offset() != 0
            || fixup.buffer_index() != 0
            || fixup.buffer_byte_offset() != 0
            || fixup.required_alignment() != 4
            || parts.kernarg_alignment != DIRECT_KFD_KERNARG_ALIGNMENT_V1
            || parts.kernarg_template.len() != 272
            || parts.kernarg_template[..8] != [0; 8]
            || u64::from_le_bytes(parts.kernarg_template[8..16].try_into().unwrap())
                != premises.output_elements()
            || self.description.static_group_segment_bytes != 0
            || self.description.dynamic_group_segment_bytes != 0
            || parts.group_segment_size != 0
            || parts.private_segment_size != 0
        {
            return Err(Error::Mismatch(
                "native fill exact ABI, storage or resource shape",
            ));
        }
        premises
            .validate_shape_v1(
                parts.geometry.grid(),
                parts.geometry.workgroup(),
                buffer.bytes().len() as u64,
            )
            .map_err(|_| Error::Mismatch("native fill numeric premises"))?;
        self.project_persistent_v1(hsaco, Some(premises))
    }
}

impl PersistentDispatchDataV1 {
    pub(crate) fn validate_native_fill_v1(&self) -> Result<(), Gfx942RuntimeProjectionErrorV1> {
        let Some(premises) = self.native_fill.as_ref() else {
            return Ok(());
        };
        let [buffer] = self.buffers.as_slice() else {
            return Err(Gfx942RuntimeProjectionErrorV1::Mismatch(
                "native fill retained output",
            ));
        };
        if self.buffer_access(0) != Some(Gfx942RuntimeBufferAccessV1::WriteOnly)
            || !self.complete_buffer_policies_match()
            || self.description.static_group_segment_bytes != 0
            || self.description.dynamic_group_segment_bytes != 0
            || self.kernarg_alignment != DIRECT_KFD_KERNARG_ALIGNMENT_V1
        {
            return Err(Gfx942RuntimeProjectionErrorV1::Mismatch(
                "native fill retained shape",
            ));
        }
        premises
            .validate_shape_v1(
                self.description.geometry.grid(),
                self.description.geometry.workgroup(),
                buffer.bytes().len() as u64,
            )
            .map_err(|_| Gfx942RuntimeProjectionErrorV1::Mismatch("native fill retained premises"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RuntimeGfx942GeneratedReservationErrorV1, RuntimeGfx942GeneratedSourceV1};

    fn hsaco() -> &'static [u8] {
        include_bytes!("../../tests/fixtures/native-fill-worker/kernel.hsaco")
    }

    fn prepare(
        count: u64,
        bytes: usize,
        grid: u32,
        access: Gfx942RuntimeBufferAccessV1,
    ) -> PreparedGfx942RuntimeDispatchV1 {
        prepare_from(hsaco(), count, bytes, grid, access)
    }

    fn prepare_from(
        artifact: &[u8],
        count: u64,
        bytes: usize,
        grid: u32,
        access: Gfx942RuntimeBufferAccessV1,
    ) -> PreparedGfx942RuntimeDispatchV1 {
        let mut explicit = vec![0; 16];
        explicit[8..].copy_from_slice(&count.to_le_bytes());
        prepare_gfx942_runtime_dispatch_v1(
            artifact,
            "fill_write_only",
            Gfx942RuntimeDispatchInputsV1::new(
                explicit,
                vec![Gfx942RuntimeDispatchBufferV1::new(vec![0; bytes], access).unwrap()],
                vec![Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
                AqlDispatchGeometryV1::new([grid, 1, 1], [64, 1, 1]).unwrap(),
                0,
                100,
            ),
        )
        .unwrap()
    }

    struct Authority<'a> {
        projection: &'a PreparedGfx942PersistentDispatchV1,
        binding: Gfx942RuntimeInvocationBindingV1,
    }
    // SAFETY: CPU-only identity tests never open a device or invoke a native hook.
    #[allow(unsafe_code)]
    unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for Authority<'_> {
        type CurrentnessError = ();
        fn finalized_hsaco_sha256(&self) -> [u8; 32] {
            self.projection.identity().object_sha256()
        }
        fn finalized_hsaco_length(&self) -> u64 {
            self.projection.finalized_hsaco_length()
        }
        fn kernel_name(&self) -> &str {
            self.projection.kernel_name()
        }
        fn dispatch_contract_sha256(&self) -> [u8; 32] {
            self.projection.dispatch_contract_sha256()
        }
        fn invocation_binding(&self) -> Gfx942RuntimeInvocationBindingV1 {
            self.binding
        }
        fn device_unique_id(&self) -> u64 {
            42
        }
        fn revalidate_currentness(&self) -> Result<(), ()> {
            Ok(())
        }
    }

    #[test]
    fn native_fill_projection_retains_full_premise_and_rejects_other_authority_families() {
        let prepared = prepare(65, 260, 128, Gfx942RuntimeBufferAccessV1::WriteOnly);
        let original_digest = prepared.description.dispatch_contract_sha256;
        let image = prepared.request.inspection_v1().executable_image.as_ptr();
        let output = prepared.request.inspection_v1().buffers[0].bytes().as_ptr();
        let p = NativeConditionalFill64PremisesV1::new([1; 32], 65, 128).unwrap();
        let binding = binding(Some(&p));
        let projection = prepared
            .into_native_conditional_fill64_projection_v1(hsaco(), p)
            .unwrap();
        assert_eq!(projection.invocation_binding(), binding);
        assert_ne!(projection.dispatch_contract_sha256(), original_digest);
        assert_eq!(projection.executable_image().as_ptr(), image);
        assert_eq!(projection.buffers()[0].bytes().as_ptr(), output);
        assert_eq!(
            projection
                .native_conditional_fill64_premises_v1()
                .unwrap()
                .output_elements(),
            65
        );
        let authority = Authority {
            projection: &projection,
            binding,
        };
        assert!(
            RuntimeGfx942GeneratedSourceV1::new(&projection, hsaco(), &authority)
                .validate(42)
                .is_ok()
        );
        for wrong in [
            Gfx942RuntimeInvocationBindingV1::OrdinaryV1,
            Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 {
                contract_identity: [1; 32],
                premise_identity: [2; 32],
            },
            Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 {
                contract_identity: [1; 32],
                premise_identity: [2; 32],
            },
            Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 {
                contract_identity: [1; 32],
                premise_identity: [2; 32],
            },
        ] {
            let authority = Authority {
                projection: &projection,
                binding: wrong,
            };
            assert!(matches!(
                RuntimeGfx942GeneratedSourceV1::new(&projection, hsaco(), &authority).validate(42),
                Err(RuntimeGfx942GeneratedReservationErrorV1::AuthorityMismatch)
            ));
        }
        // An ordinary/direct request cannot match the genuine native family,
        // even if every other supplied identity is copied from the projection.
        assert!(
            crate::authorized_execution::validate_authority_identity_bindings_v1(
                &authority,
                projection.identity().object_sha256(),
                projection.finalized_hsaco_length(),
                projection.kernel_name(),
                projection.dispatch_contract_sha256(),
                Gfx942RuntimeInvocationBindingV1::OrdinaryV1,
                42,
            )
            .is_err()
        );
        let storage = projection.into_generated_storage_v1();
        assert_eq!(storage.invocation_binding(), binding);
        assert!(storage.native_conditional_fill64_premises_v1().is_some());
    }

    #[test]
    fn native_fill_projection_rejects_count_extent_grid_access_and_old_premises() {
        let short = include_bytes!(
            "../../../fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco"
        );
        let prepared = prepare_from(short, 64, 256, 64, Gfx942RuntimeBufferAccessV1::WriteOnly);
        assert!(
            prepared
                .into_native_conditional_fill64_projection_v1(
                    short,
                    NativeConditionalFill64PremisesV1::new([1; 32], 64, 64).unwrap()
                )
                .is_err()
        );
        for (count, bytes, grid, access, expected_count, max) in [
            (
                64,
                260,
                128,
                Gfx942RuntimeBufferAccessV1::WriteOnly,
                65,
                128,
            ),
            (
                65,
                264,
                128,
                Gfx942RuntimeBufferAccessV1::WriteOnly,
                65,
                128,
            ),
            (65, 260, 64, Gfx942RuntimeBufferAccessV1::WriteOnly, 65, 128),
            (
                65,
                260,
                192,
                Gfx942RuntimeBufferAccessV1::WriteOnly,
                65,
                128,
            ),
            (
                65,
                260,
                128,
                Gfx942RuntimeBufferAccessV1::ReadWrite,
                65,
                128,
            ),
        ] {
            let prepared = prepare(count, bytes, grid, access);
            let premises =
                NativeConditionalFill64PremisesV1::new([1; 32], expected_count, max).unwrap();
            assert!(
                prepared
                    .into_native_conditional_fill64_projection_v1(hsaco(), premises)
                    .is_err()
            );
        }
        let mut prepared = prepare(64, 256, 64, Gfx942RuntimeBufferAccessV1::WriteOnly);
        let inspection = prepared.request.inspection_v1();
        let old = fe2o3_kfd::ConditionalDispatchPremisesV1::new(
            [1; 32],
            [2; 32],
            [3; 32],
            &inspection.kernarg_template[..16],
            inspection.geometry,
            &[fe2o3_kfd::ConditionalDispatchSliceV1 {
                generated_field: 0,
                pointer_offset: 0,
                length_offset: 8,
                buffer_index: Some(0),
                buffer_byte_offset: 0,
                length: 64,
                element_bytes: 4,
                alignment: 4,
            }],
            0,
            fe2o3_kfd::ConditionalDispatchDomainV1::GuardedOutput,
            &[],
        )
        .unwrap();
        prepared.request = prepared.request.with_conditional_premises_v1(old).unwrap();
        assert!(matches!(
            prepared.into_native_conditional_fill64_projection_v1(
                &[],
                NativeConditionalFill64PremisesV1::new([1; 32], 64, 64).unwrap()
            ),
            Err(Gfx942RuntimeProjectionErrorV1::Mismatch(
                "native fill cannot rebrand an existing family"
            ))
        ));
    }
}
