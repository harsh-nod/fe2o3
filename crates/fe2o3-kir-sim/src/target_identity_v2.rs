/// Closed target identity shared by the schedule and reduction wire formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TargetIdentityWireV2 {
    #[serde(rename = "little_endian_index32_v1")]
    LittleEndianIndex32V1,
    #[serde(rename = "amdgpu_64_little_endian_v1")]
    Amdgpu64LittleEndianV1,
    #[serde(rename = "amdgpu_gfx942_little_endian_v2")]
    AmdgpuGfx942LittleEndianV2,
    #[serde(rename = "amdgpu_gfx950_little_endian_v2")]
    AmdgpuGfx950LittleEndianV2,
}

#[cfg(test)]
mod target_identity_tests {
    use super::*;

    #[test]
    fn closed_target_identity_preserves_legacy_absence_and_checked_widths() {
        assert_eq!(
            SimulationTargetV1::amdgpu_64(),
            SimulationTargetV1::little_endian(IndexWidthV1::Bits64)
        );
        assert_eq!(SimulationTargetV1::amdgpu_64().amd_profile(), None);
        for (tag, bits, profile) in [
            ("little_endian_index32_v1", 32, None),
            ("amdgpu_64_little_endian_v1", 64, None),
            (
                "amdgpu_gfx942_little_endian_v2",
                64,
                Some(fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942),
            ),
            (
                "amdgpu_gfx950_little_endian_v2",
                64,
                Some(fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950),
            ),
        ] {
            let identity: TargetIdentityWireV2 =
                serde_json::from_str(&format!("\"{tag}\"")).unwrap();
            let target = identity.target(bits).unwrap();
            assert_eq!(target.identity_tag(), tag);
            assert_eq!(target.amd_profile(), profile);
            assert_eq!(TargetIdentityWireV2::from_target(target), identity);
            assert_eq!(identity.target(if bits == 64 { 32 } else { 64 }), None);
            assert_eq!(identity.target(16), None);
        }
        assert!(
            serde_json::from_str::<TargetIdentityWireV2>("\"amdgpu_gfx999_little_endian_v2\"")
                .is_err()
        );
    }
}

impl TargetIdentityWireV2 {
    pub(crate) const fn from_target(target: SimulationTargetV1) -> Self {
        use fe2o3_amd_target::ProductionAmdTargetProfileV1;
        match target.amd_profile {
            Some(ProductionAmdTargetProfileV1::Gfx942) => Self::AmdgpuGfx942LittleEndianV2,
            Some(ProductionAmdTargetProfileV1::Gfx950) => Self::AmdgpuGfx950LittleEndianV2,
            None => match target.index_width {
                IndexWidthV1::Bits32 => Self::LittleEndianIndex32V1,
                IndexWidthV1::Bits64 => Self::Amdgpu64LittleEndianV1,
            },
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::LittleEndianIndex32V1 => "little_endian_index32_v1",
            Self::Amdgpu64LittleEndianV1 => "amdgpu_64_little_endian_v1",
            Self::AmdgpuGfx942LittleEndianV2 => "amdgpu_gfx942_little_endian_v2",
            Self::AmdgpuGfx950LittleEndianV2 => "amdgpu_gfx950_little_endian_v2",
        }
    }

    pub(crate) const fn target(self, index_bits: u16) -> Option<SimulationTargetV1> {
        use fe2o3_amd_target::ProductionAmdTargetProfileV1;
        let target = match self {
            Self::LittleEndianIndex32V1 => SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
            // This historical name denotes width only, never an AMD pointer profile.
            Self::Amdgpu64LittleEndianV1 => SimulationTargetV1::little_endian(IndexWidthV1::Bits64),
            Self::AmdgpuGfx942LittleEndianV2 => {
                SimulationTargetV1::amdgpu_profile(ProductionAmdTargetProfileV1::Gfx942)
            }
            Self::AmdgpuGfx950LittleEndianV2 => {
                SimulationTargetV1::amdgpu_profile(ProductionAmdTargetProfileV1::Gfx950)
            }
        };
        if target.index_width.bits() == index_bits {
            Some(target)
        } else {
            None
        }
    }
}

impl SimulationTargetV1 {
    /// Returns the explicit AMD profile, if one was selected; width alone grants none.
    pub const fn amd_profile(self) -> Option<fe2o3_amd_target::ProductionAmdTargetProfileV1> {
        self.amd_profile
    }

    /// Returns the canonical, versioned target identity used in observations and codecs.
    pub const fn identity_tag(self) -> &'static str {
        TargetIdentityWireV2::from_target(self).as_str()
    }

    /// Selects an exact admitted production device target without guessing a default.
    pub const fn amdgpu_from_device_target(target: &str) -> Option<Self> {
        match fe2o3_amd_target::ProductionAmdTargetProfileV1::from_device_target(target) {
            Some(profile) => Some(Self::amdgpu_profile(profile)),
            None => None,
        }
    }

    pub(crate) fn hash_profile_identity(self, hash: &mut sha2::Sha256) {
        use sha2::Digest;
        if self.amd_profile.is_some() {
            hash.update(b"FE2O3/KIR-SIM/TARGET-PROFILE/V2\0");
            let tag = self.identity_tag().as_bytes();
            hash.update((tag.len() as u64).to_le_bytes());
            hash.update(tag);
        }
    }
}
