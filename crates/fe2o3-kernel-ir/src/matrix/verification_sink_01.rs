use super::*;
use sha2::{Digest as _, Sha256};

pub(super) const MATRIX_FIXED_MESSAGE_WORK_UPPER_V1: usize = 512;
pub(super) const MATRIX_TYPE_NODE_MESSAGE_WORK_UPPER_V1: usize = 128;

pub(super) fn matrix_type_message_work_upper_v1<S: MatrixOperationIssueSinkV1>(
    mut ty: &Type,
    sink: &mut S,
) -> Result<usize, S::Error> {
    let mut nodes = 0_usize;
    loop {
        sink.charge_work(1)?;
        nodes = nodes.saturating_add(1);
        ty = match ty {
            Type::Pointer(pointer) => pointer.pointee.as_ref(),
            Type::Slice(slice) => slice.element.as_ref(),
            Type::Unit | Type::Scalar(_) | Type::Vector(_) => break,
        };
    }
    Ok(MATRIX_FIXED_MESSAGE_WORK_UPPER_V1
        .saturating_add(nodes.saturating_mul(MATRIX_TYPE_NODE_MESSAGE_WORK_UPPER_V1)))
}

pub(super) fn matrix_types_equal_v1<S: MatrixOperationIssueSinkV1>(
    mut actual: &Type,
    mut expected: &Type,
    sink: &mut S,
) -> Result<bool, S::Error> {
    loop {
        sink.charge_work(1)?;
        match (actual, expected) {
            (Type::Unit, Type::Unit) => return Ok(true),
            (Type::Scalar(actual), Type::Scalar(expected)) => return Ok(actual == expected),
            (Type::Vector(actual), Type::Vector(expected)) => return Ok(actual == expected),
            (Type::Pointer(actual_pointer), Type::Pointer(expected_pointer)) => {
                if actual_pointer.address_space != expected_pointer.address_space
                    || actual_pointer.access != expected_pointer.access
                {
                    return Ok(false);
                }
                actual = actual_pointer.pointee.as_ref();
                expected = expected_pointer.pointee.as_ref();
            }
            (Type::Slice(actual_slice), Type::Slice(expected_slice)) => {
                if actual_slice.address_space != expected_slice.address_space
                    || actual_slice.access != expected_slice.access
                {
                    return Ok(false);
                }
                actual = actual_slice.element.as_ref();
                expected = expected_slice.element.as_ref();
            }
            _ => return Ok(false),
        }
    }
}

pub(super) fn emit_matrix_fixed_v1<S: MatrixOperationIssueSinkV1>(
    sink: &mut S,
    kind: MatrixVerificationIssueKind,
    message: &'static str,
) -> Result<(), S::Error> {
    sink.emit(kind, message.len(), format_args!("{message}"))
}

pub(crate) trait MatrixOperationIssueSinkV1 {
    type Error;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error>;

    /// Owns two logical scalar cells per coordinate until release. Metered
    /// implementations admit allocation and cleanup before allocating; legacy
    /// public adapters retain their existing infallible allocation behavior.
    fn allocate_coordinates(&mut self, count: usize) -> Result<Vec<[u64; 2]>, Self::Error>;
    fn release_coordinates(
        &mut self,
        coordinates: Vec<[u64; 2]>,
        count: usize,
    ) -> Result<(), Self::Error>;

    fn emit(
        &mut self,
        kind: MatrixVerificationIssueKind,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error>;
}

pub(super) trait MatrixOperandTypeViewV1 {
    fn len(&self) -> usize;
    fn get_type(&self, index: usize) -> Option<Option<&Type>>;
}

impl MatrixOperandTypeViewV1 for [Option<Type>] {
    fn len(&self) -> usize {
        <[Option<Type>]>::len(self)
    }

    fn get_type(&self, index: usize) -> Option<Option<&Type>> {
        self.get(index).map(Option::as_ref)
    }
}

impl MatrixOperandTypeViewV1 for [Option<&Type>] {
    fn len(&self) -> usize {
        <[Option<&Type>]>::len(self)
    }

    fn get_type(&self, index: usize) -> Option<Option<&Type>> {
        self.get(index).copied()
    }
}

pub(super) fn try_visit_matrix_required_capabilities_v1<E>(
    operation: &MatrixOperation,
    mut visitor: impl FnMut(TargetCapabilityRefV1<'_>) -> Result<(), E>,
) -> Result<(), E> {
    let (wave, extension, bfloat16, workgroup_memory) = match operation.kind {
        MatrixOperationKind::MultiplyAccumulate { profile, .. } => (
            profile.wave_width,
            BF16_F32_M16N16K16_CAPABILITY,
            true,
            false,
        ),
        MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } => {
            let extension = if operation.tensor_layout.as_ref().is_some_and(|contract| {
                contract.profile
                    == TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64
            }) {
                SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY
            } else if profile == MatrixMultiplyProfile::fp4_e2m1_f32_m16n16k128_wave64() {
                SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY
            } else {
                SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY
            };
            (profile.wave_width, extension, false, false)
        }
        MatrixOperationKind::LdsLoad { profile, .. }
        | MatrixOperationKind::LdsStore { profile, .. } => (
            profile.wave_width,
            LDS_TILE_16X16_XOR4_CAPABILITY,
            profile.element == MatrixElement::Bf16,
            true,
        ),
    };
    if bfloat16 {
        visitor(TargetCapabilityRefV1::BFloat16)?;
    }
    visitor(TargetCapabilityRefV1::Subgroups)?;
    visitor(TargetCapabilityRefV1::SubgroupSize(wave.lanes()))?;
    if workgroup_memory {
        visitor(TargetCapabilityRefV1::WorkgroupMemory)?;
    }
    if let Some(binding) = &operation.frontend_binding {
        visitor(TargetCapabilityRefV1::extension_lower_hex(
            MATRIX_PROJECTED_KERNARG_POLICY_NAMESPACE_V1,
            &binding.projected_kernarg.digest,
        ))?;
    }
    visitor(TargetCapabilityRefV1::extension(
        MATRIX_CAPABILITY_NAMESPACE,
        extension,
    ))?;
    if let Some(binding) = &operation.frontend_binding {
        visitor(TargetCapabilityRefV1::extension_lower_hex(
            MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2,
            &binding.observed_source.digest,
        ))?;
    }
    visitor(TargetCapabilityRefV1::WaveWidth(wave))
}

pub(super) struct LegacyMatrixOperationIssueSinkV1<'a> {
    pub(super) issues: &'a mut Vec<MatrixVerificationIssue>,
}

impl MatrixOperationIssueSinkV1 for LegacyMatrixOperationIssueSinkV1<'_> {
    type Error = std::convert::Infallible;

    fn charge_work(&mut self, _amount: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn allocate_coordinates(&mut self, count: usize) -> Result<Vec<[u64; 2]>, Self::Error> {
        Ok(Vec::with_capacity(count))
    }

    fn release_coordinates(
        &mut self,
        coordinates: Vec<[u64; 2]>,
        _count: usize,
    ) -> Result<(), Self::Error> {
        drop(coordinates);
        Ok(())
    }

    fn emit(
        &mut self,
        kind: MatrixVerificationIssueKind,
        _message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        self.issues.push(MatrixVerificationIssue {
            kind,
            message: arguments.to_string(),
        });
        Ok(())
    }
}

impl MatrixProjectedKernargPolicyV1 {
    pub(super) fn matches_canonical_without_allocation_v1(&self) -> bool {
        let parameters = [
            projected(0, 0, MatrixElement::Bf16, 0, 2, 2),
            projected(0, 1, MatrixElement::Bf16, 2, 2, 2),
            projected(0, 2, MatrixElement::Bf16, 4, 2, 2),
            projected(0, 3, MatrixElement::Bf16, 6, 2, 2),
            projected(1, 0, MatrixElement::Bf16, 8, 2, 2),
            projected(1, 1, MatrixElement::Bf16, 10, 2, 2),
            projected(1, 2, MatrixElement::Bf16, 12, 2, 2),
            projected(1, 3, MatrixElement::Bf16, 14, 2, 2),
            projected(2, 0, MatrixElement::F32, 16, 4, 4),
            projected(2, 1, MatrixElement::F32, 20, 4, 4),
            projected(2, 2, MatrixElement::F32, 24, 4, 4),
            projected(2, 3, MatrixElement::F32, 28, 4, 4),
        ];
        if self.parameters != parameters
            || self.explicit_argument_size != 32
            || self.implicit_argument_bytes != 256
            || self.kernarg_segment_size != 288
            || self.kernarg_segment_alignment != 8
        {
            return false;
        }
        let mut digest = Sha256::new();
        digest.update(b"FE2O3/MATRIX-PROJECTED-KERNARG-POLICY/V1\0");
        for parameter in parameters {
            digest.update([
                parameter.source,
                parameter.lane,
                match parameter.element {
                    MatrixElement::Bf16 => 1,
                    MatrixElement::F32 => 2,
                    MatrixElement::Fp8E4M3 => 3,
                    MatrixElement::Fp4E2M1 => 4,
                },
            ]);
            digest.update(parameter.offset.to_le_bytes());
            digest.update([parameter.size, parameter.alignment]);
        }
        digest.update(self.explicit_argument_size.to_le_bytes());
        digest.update(self.implicit_argument_bytes.to_le_bytes());
        digest.update(self.kernarg_segment_size.to_le_bytes());
        digest.update([self.kernarg_segment_alignment]);
        self.digest.as_slice() == digest.finalize().as_slice()
    }
}
