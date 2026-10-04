use std::{collections::BTreeSet, fmt, marker::PhantomData};

use sha2::{Digest, Sha256};

use crate::{
    CanonicalCodeObjectDigest, DeviceTargetV1, KernelDescriptorDigest, KernelDescriptorV1,
    KernelId, ValidName,
};

pub const GFX942_XNACK_MINUS_TARGET_V1: &str = "gfx942:xnack-";
pub const GFX942_MAX_FLAT_WORKGROUP_SIZE_V1: u32 = 1_024;
pub const GFX942_MAX_WAVES_PER_EXECUTION_UNIT_V1: u8 = 8;
pub const GFX942_MAX_KERNEL_FAMILY_VARIANTS_V1: usize = 16;

const KERNEL_LAUNCH_POLICY_DOMAIN_V1: &[u8] = b"FE2O3/GFX942-KERNEL-LAUNCH-POLICY/V1\0";

macro_rules! opaque_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_opaque_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

opaque_identity!(KernelFamilyIdentityV1);
opaque_identity!(KernelInterfaceIdentityV1);
opaque_identity!(KernelLaunchPolicyIdentityV1);

/// Exact AMD LLVM launch metadata admitted by the bounded gfx942 profile.
///
/// The waves range is an AMD execution-unit occupancy constraint. It is not a
/// translation of CUDA's minimum-blocks-per-SM hint.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942LaunchBoundsV1 {
    minimum_flat_workgroup_size: u32,
    maximum_flat_workgroup_size: u32,
    minimum_waves_per_execution_unit: u8,
    maximum_waves_per_execution_unit: u8,
}

impl Gfx942LaunchBoundsV1 {
    pub fn new(
        minimum_flat_workgroup_size: u32,
        maximum_flat_workgroup_size: u32,
        minimum_waves_per_execution_unit: u8,
        maximum_waves_per_execution_unit: u8,
    ) -> Result<Self, KernelFamilyPolicyErrorV1> {
        if minimum_flat_workgroup_size == 0
            || minimum_flat_workgroup_size > maximum_flat_workgroup_size
        {
            return Err(KernelFamilyPolicyErrorV1::InvalidFlatWorkgroupRange);
        }
        if maximum_flat_workgroup_size > GFX942_MAX_FLAT_WORKGROUP_SIZE_V1 {
            return Err(KernelFamilyPolicyErrorV1::FlatWorkgroupLimitExceeded {
                actual: maximum_flat_workgroup_size,
                maximum: GFX942_MAX_FLAT_WORKGROUP_SIZE_V1,
            });
        }
        if minimum_waves_per_execution_unit == 0
            || minimum_waves_per_execution_unit > maximum_waves_per_execution_unit
        {
            return Err(KernelFamilyPolicyErrorV1::InvalidWavesPerExecutionUnitRange);
        }
        if maximum_waves_per_execution_unit > GFX942_MAX_WAVES_PER_EXECUTION_UNIT_V1 {
            return Err(
                KernelFamilyPolicyErrorV1::WavesPerExecutionUnitLimitExceeded {
                    actual: maximum_waves_per_execution_unit,
                    maximum: GFX942_MAX_WAVES_PER_EXECUTION_UNIT_V1,
                },
            );
        }
        Ok(Self {
            minimum_flat_workgroup_size,
            maximum_flat_workgroup_size,
            minimum_waves_per_execution_unit,
            maximum_waves_per_execution_unit,
        })
    }

    pub const fn minimum_flat_workgroup_size(self) -> u32 {
        self.minimum_flat_workgroup_size
    }

    pub const fn maximum_flat_workgroup_size(self) -> u32 {
        self.maximum_flat_workgroup_size
    }

    pub const fn minimum_waves_per_execution_unit(self) -> u8 {
        self.minimum_waves_per_execution_unit
    }

    pub const fn maximum_waves_per_execution_unit(self) -> u8 {
        self.maximum_waves_per_execution_unit
    }

    pub const fn admits_flat_workgroup_size(self, size: u32) -> bool {
        size >= self.minimum_flat_workgroup_size && size <= self.maximum_flat_workgroup_size
    }
}

/// One immutable variant policy tied to a concrete kernel descriptor and code
/// object. This is descriptive data and grants no load or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelFamilyVariantDescriptorV1 {
    family_identity: KernelFamilyIdentityV1,
    interface_identity: KernelInterfaceIdentityV1,
    variant_name: ValidName,
    logical_name: ValidName,
    entry_name: ValidName,
    target: DeviceTargetV1,
    kernel_id: KernelId,
    descriptor_digest: KernelDescriptorDigest,
    artifact_digest: CanonicalCodeObjectDigest,
    launch_bounds: Gfx942LaunchBoundsV1,
    policy_identity: KernelLaunchPolicyIdentityV1,
}

impl KernelFamilyVariantDescriptorV1 {
    pub fn new(
        family_identity: KernelFamilyIdentityV1,
        interface_identity: KernelInterfaceIdentityV1,
        variant_name: ValidName,
        target: DeviceTargetV1,
        kernel: &KernelDescriptorV1,
        artifact_digest: CanonicalCodeObjectDigest,
        launch_bounds: Gfx942LaunchBoundsV1,
    ) -> Result<Self, KernelFamilyPolicyErrorV1> {
        validate_gfx942_target(target)?;
        let descriptor_digest = KernelDescriptorDigest::calculate(kernel);
        let logical_name = kernel.logical_name().clone();
        let entry_name = kernel.entry_name().clone();
        let kernel_id = kernel.kernel_id();
        let policy_identity = derive_policy_identity(
            family_identity,
            interface_identity,
            &variant_name,
            &logical_name,
            &entry_name,
            target,
            kernel_id,
            descriptor_digest,
            artifact_digest,
            launch_bounds,
        );
        Ok(Self {
            family_identity,
            interface_identity,
            variant_name,
            logical_name,
            entry_name,
            target,
            kernel_id,
            descriptor_digest,
            artifact_digest,
            launch_bounds,
            policy_identity,
        })
    }

    pub const fn family_identity(&self) -> KernelFamilyIdentityV1 {
        self.family_identity
    }

    pub const fn interface_identity(&self) -> KernelInterfaceIdentityV1 {
        self.interface_identity
    }

    pub const fn variant_name(&self) -> &ValidName {
        &self.variant_name
    }

    pub const fn logical_name(&self) -> &ValidName {
        &self.logical_name
    }

    pub const fn entry_name(&self) -> &ValidName {
        &self.entry_name
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }

    pub const fn descriptor_digest(&self) -> KernelDescriptorDigest {
        self.descriptor_digest
    }

    pub const fn artifact_digest(&self) -> CanonicalCodeObjectDigest {
        self.artifact_digest
    }

    pub const fn launch_bounds(&self) -> Gfx942LaunchBoundsV1 {
        self.launch_bounds
    }

    pub const fn policy_identity(&self) -> KernelLaunchPolicyIdentityV1 {
        self.policy_identity
    }
}

/// Canonical bounded family of monomorphized variants sharing one logical ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942KernelFamilyBundleV1 {
    family_identity: KernelFamilyIdentityV1,
    interface_identity: KernelInterfaceIdentityV1,
    logical_name: ValidName,
    variants: Vec<KernelFamilyVariantDescriptorV1>,
}

impl Gfx942KernelFamilyBundleV1 {
    pub fn new(
        mut variants: Vec<KernelFamilyVariantDescriptorV1>,
    ) -> Result<Self, KernelFamilyPolicyErrorV1> {
        let first = variants
            .first()
            .ok_or(KernelFamilyPolicyErrorV1::EmptyFamily)?;
        if variants.len() > GFX942_MAX_KERNEL_FAMILY_VARIANTS_V1 {
            return Err(KernelFamilyPolicyErrorV1::TooManyVariants {
                actual: variants.len(),
                maximum: GFX942_MAX_KERNEL_FAMILY_VARIANTS_V1,
            });
        }
        let family_identity = first.family_identity;
        let interface_identity = first.interface_identity;
        let logical_name = first.logical_name.clone();
        for variant in &variants {
            if variant.family_identity != family_identity {
                return Err(KernelFamilyPolicyErrorV1::FamilySubstitution);
            }
            if variant.interface_identity != interface_identity {
                return Err(KernelFamilyPolicyErrorV1::InterfaceSubstitution);
            }
            if variant.logical_name != logical_name {
                return Err(KernelFamilyPolicyErrorV1::LogicalInterfaceSubstitution);
            }
            validate_gfx942_target(variant.target)?;
        }
        variants.sort_by(|left, right| left.variant_name.cmp(&right.variant_name));
        reject_duplicate(
            variants.iter().map(|variant| &variant.variant_name),
            KernelFamilyPolicyErrorV1::DuplicateVariant,
        )?;
        reject_duplicate(
            variants.iter().map(|variant| &variant.entry_name),
            KernelFamilyPolicyErrorV1::DuplicateEntry,
        )?;
        reject_duplicate(
            variants.iter().map(|variant| &variant.policy_identity),
            KernelFamilyPolicyErrorV1::DuplicatePolicy,
        )?;
        Ok(Self {
            family_identity,
            interface_identity,
            logical_name,
            variants,
        })
    }

    pub const fn family_identity(&self) -> KernelFamilyIdentityV1 {
        self.family_identity
    }

    pub const fn interface_identity(&self) -> KernelInterfaceIdentityV1 {
        self.interface_identity
    }

    pub const fn logical_name(&self) -> &ValidName {
        &self.logical_name
    }

    pub fn variants(&self) -> &[KernelFamilyVariantDescriptorV1] {
        &self.variants
    }

    pub fn admit_variant<Family, Interface, Variant>(
        &self,
        expectation: &TypedKernelFamilyVariantExpectationV1<Family, Interface, Variant>,
    ) -> Result<
        AdmittedKernelFamilyVariantV1<'_, Family, Interface, Variant>,
        KernelFamilyPolicyErrorV1,
    > {
        if self.family_identity != expectation.family_identity {
            return Err(KernelFamilyPolicyErrorV1::FamilySubstitution);
        }
        if self.interface_identity != expectation.interface_identity {
            return Err(KernelFamilyPolicyErrorV1::InterfaceSubstitution);
        }
        if self.logical_name != expectation.logical_name {
            return Err(KernelFamilyPolicyErrorV1::LogicalInterfaceSubstitution);
        }
        let descriptor = self
            .variants
            .binary_search_by(|variant| variant.variant_name.cmp(&expectation.variant_name))
            .ok()
            .map(|index| &self.variants[index])
            .ok_or(KernelFamilyPolicyErrorV1::VariantNotFound)?;
        compare_expectation(descriptor, expectation)?;
        Ok(AdmittedKernelFamilyVariantV1 {
            descriptor,
            _markers: PhantomData,
        })
    }
}

type FamilyVariantMarkers<Family, Interface, Variant> =
    PhantomData<fn() -> (Family, Interface, Variant)>;

/// Generated typed expectation for one variant of one logical interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedKernelFamilyVariantExpectationV1<Family, Interface, Variant> {
    family_identity: KernelFamilyIdentityV1,
    interface_identity: KernelInterfaceIdentityV1,
    variant_name: ValidName,
    logical_name: ValidName,
    entry_name: ValidName,
    target: DeviceTargetV1,
    kernel_id: KernelId,
    descriptor_digest: KernelDescriptorDigest,
    artifact_digest: CanonicalCodeObjectDigest,
    launch_bounds: Gfx942LaunchBoundsV1,
    policy_identity: KernelLaunchPolicyIdentityV1,
    _markers: FamilyVariantMarkers<Family, Interface, Variant>,
}

impl<Family, Interface, Variant> TypedKernelFamilyVariantExpectationV1<Family, Interface, Variant> {
    pub fn from_descriptor(descriptor: &KernelFamilyVariantDescriptorV1) -> Self {
        Self {
            family_identity: descriptor.family_identity,
            interface_identity: descriptor.interface_identity,
            variant_name: descriptor.variant_name.clone(),
            logical_name: descriptor.logical_name.clone(),
            entry_name: descriptor.entry_name.clone(),
            target: descriptor.target,
            kernel_id: descriptor.kernel_id,
            descriptor_digest: descriptor.descriptor_digest,
            artifact_digest: descriptor.artifact_digest,
            launch_bounds: descriptor.launch_bounds,
            policy_identity: descriptor.policy_identity,
            _markers: PhantomData,
        }
    }

    pub const fn policy_identity(&self) -> KernelLaunchPolicyIdentityV1 {
        self.policy_identity
    }
}

/// Inert exact-match result for one typed family variant.
///
/// Private fields prevent callers from manufacturing or retagging this token.
/// It grants neither load nor launch authority.
pub struct AdmittedKernelFamilyVariantV1<'bundle, Family, Interface, Variant> {
    descriptor: &'bundle KernelFamilyVariantDescriptorV1,
    _markers: FamilyVariantMarkers<Family, Interface, Variant>,
}

impl<Family, Interface, Variant> fmt::Debug
    for AdmittedKernelFamilyVariantV1<'_, Family, Interface, Variant>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedKernelFamilyVariantV1")
            .field("variant_name", self.descriptor.variant_name())
            .field("policy_identity", &self.descriptor.policy_identity())
            .finish_non_exhaustive()
    }
}

impl<Family, Interface, Variant> AdmittedKernelFamilyVariantV1<'_, Family, Interface, Variant> {
    pub const fn descriptor(&self) -> &KernelFamilyVariantDescriptorV1 {
        self.descriptor
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn compare_expectation<Family, Interface, Variant>(
    descriptor: &KernelFamilyVariantDescriptorV1,
    expectation: &TypedKernelFamilyVariantExpectationV1<Family, Interface, Variant>,
) -> Result<(), KernelFamilyPolicyErrorV1> {
    if descriptor.target != expectation.target {
        return Err(KernelFamilyPolicyErrorV1::TargetSubstitution);
    }
    if descriptor.kernel_id != expectation.kernel_id
        || descriptor.entry_name != expectation.entry_name
    {
        return Err(KernelFamilyPolicyErrorV1::KernelSubstitution);
    }
    if descriptor.descriptor_digest != expectation.descriptor_digest {
        return Err(KernelFamilyPolicyErrorV1::DescriptorSubstitution);
    }
    if descriptor.artifact_digest != expectation.artifact_digest {
        return Err(KernelFamilyPolicyErrorV1::ArtifactSubstitution);
    }
    if descriptor.launch_bounds != expectation.launch_bounds {
        return Err(KernelFamilyPolicyErrorV1::LaunchMetadataSubstitution);
    }
    if descriptor.policy_identity != expectation.policy_identity {
        return Err(KernelFamilyPolicyErrorV1::PolicyIdentitySubstitution);
    }
    Ok(())
}

fn validate_gfx942_target(target: DeviceTargetV1) -> Result<(), KernelFamilyPolicyErrorV1> {
    if target.to_string() != GFX942_XNACK_MINUS_TARGET_V1 {
        return Err(KernelFamilyPolicyErrorV1::UnsupportedTarget);
    }
    Ok(())
}

fn reject_duplicate<'a, T: 'a + Ord>(
    values: impl Iterator<Item = &'a T>,
    error: KernelFamilyPolicyErrorV1,
) -> Result<(), KernelFamilyPolicyErrorV1> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(error);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn derive_policy_identity(
    family_identity: KernelFamilyIdentityV1,
    interface_identity: KernelInterfaceIdentityV1,
    variant_name: &ValidName,
    logical_name: &ValidName,
    entry_name: &ValidName,
    target: DeviceTargetV1,
    kernel_id: KernelId,
    descriptor_digest: KernelDescriptorDigest,
    artifact_digest: CanonicalCodeObjectDigest,
    launch_bounds: Gfx942LaunchBoundsV1,
) -> KernelLaunchPolicyIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(KERNEL_LAUNCH_POLICY_DOMAIN_V1);
    hasher.update(family_identity.as_bytes());
    hasher.update(interface_identity.as_bytes());
    hash_text(&mut hasher, variant_name.as_str());
    hash_text(&mut hasher, logical_name.as_str());
    hash_text(&mut hasher, entry_name.as_str());
    hash_text(&mut hasher, &target.to_string());
    hasher.update(kernel_id.as_bytes());
    hasher.update(descriptor_digest.as_bytes());
    hasher.update(artifact_digest.as_bytes());
    hasher.update(launch_bounds.minimum_flat_workgroup_size.to_le_bytes());
    hasher.update(launch_bounds.maximum_flat_workgroup_size.to_le_bytes());
    hasher.update([launch_bounds.minimum_waves_per_execution_unit]);
    hasher.update([launch_bounds.maximum_waves_per_execution_unit]);
    KernelLaunchPolicyIdentityV1(hasher.finalize().into())
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum KernelFamilyPolicyErrorV1 {
    InvalidFlatWorkgroupRange,
    FlatWorkgroupLimitExceeded { actual: u32, maximum: u32 },
    InvalidWavesPerExecutionUnitRange,
    WavesPerExecutionUnitLimitExceeded { actual: u8, maximum: u8 },
    UnsupportedTarget,
    EmptyFamily,
    TooManyVariants { actual: usize, maximum: usize },
    FamilySubstitution,
    InterfaceSubstitution,
    LogicalInterfaceSubstitution,
    DuplicateVariant,
    DuplicateEntry,
    DuplicatePolicy,
    VariantNotFound,
    TargetSubstitution,
    KernelSubstitution,
    DescriptorSubstitution,
    ArtifactSubstitution,
    LaunchMetadataSubstitution,
    PolicyIdentitySubstitution,
}

impl fmt::Display for KernelFamilyPolicyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFlatWorkgroupRange => {
                formatter.write_str("flat workgroup range is empty or reversed")
            }
            Self::FlatWorkgroupLimitExceeded { actual, maximum } => write!(
                formatter,
                "flat workgroup limit {actual} exceeds gfx942 maximum {maximum}"
            ),
            Self::InvalidWavesPerExecutionUnitRange => {
                formatter.write_str("waves-per-execution-unit range is empty or reversed")
            }
            Self::WavesPerExecutionUnitLimitExceeded { actual, maximum } => write!(
                formatter,
                "waves-per-execution-unit limit {actual} exceeds gfx942 maximum {maximum}"
            ),
            Self::UnsupportedTarget => write!(
                formatter,
                "kernel family policy requires exact target {GFX942_XNACK_MINUS_TARGET_V1}"
            ),
            Self::EmptyFamily => formatter.write_str("kernel family has no variants"),
            Self::TooManyVariants { actual, maximum } => {
                write!(
                    formatter,
                    "kernel family has {actual} variants; maximum is {maximum}"
                )
            }
            Self::FamilySubstitution => {
                formatter.write_str("kernel family identity was substituted")
            }
            Self::InterfaceSubstitution => {
                formatter.write_str("kernel interface identity was substituted")
            }
            Self::LogicalInterfaceSubstitution => {
                formatter.write_str("kernel logical interface name was substituted")
            }
            Self::DuplicateVariant => formatter.write_str("kernel family variant is duplicated"),
            Self::DuplicateEntry => formatter.write_str("kernel family entry is duplicated"),
            Self::DuplicatePolicy => formatter.write_str("kernel launch policy is duplicated"),
            Self::VariantNotFound => formatter.write_str("kernel family variant is absent"),
            Self::TargetSubstitution => formatter.write_str("kernel target was substituted"),
            Self::KernelSubstitution => formatter.write_str("kernel identity was substituted"),
            Self::DescriptorSubstitution => {
                formatter.write_str("kernel descriptor was substituted")
            }
            Self::ArtifactSubstitution => formatter.write_str("kernel artifact was substituted"),
            Self::LaunchMetadataSubstitution => {
                formatter.write_str("kernel launch metadata was substituted")
            }
            Self::PolicyIdentitySubstitution => {
                formatter.write_str("kernel launch policy identity was substituted")
            }
        }
    }
}

impl std::error::Error for KernelFamilyPolicyErrorV1 {}
