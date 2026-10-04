//! Target-neutral declarations attached to a kernel before target lowering.
//!
//! These values describe source intent. They do not authorize assembly,
//! establish target compatibility, or prove that an executable honors them.

use core::fmt;

/// Structural upper bound accepted by the V1 workgroup contract.
pub const MAX_WORKGROUP_THREADS_V1: u32 = 1_024;
/// Structural upper bound for the V1 occupancy hint.
pub const MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1: u16 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelFrontendContractErrorV1 {
    EmptyContract,
    ZeroWorkgroupDimension,
    WorkgroupVolumeOverflow,
    WorkgroupVolumeTooLarge { actual: u64, max: u32 },
    RequiredExceedsMaximum,
    OccupancyRequiresMaximum,
    InvalidOccupancy { actual: u16, max: u16 },
    EmptyAssemblyOperands,
    UnsupportedAssemblyOperandBits(u16),
    UnsupportedAssemblyOptionBits(u16),
    UnsupportedAssemblyEffectBits(u16),
    ConflictingAssemblyOptions,
    AssemblyEffectsConflictWithOptions,
}

impl fmt::Display for KernelFrontendContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyContract => formatter.write_str("kernel frontend contract is empty"),
            Self::ZeroWorkgroupDimension => {
                formatter.write_str("workgroup dimensions must be nonzero")
            }
            Self::WorkgroupVolumeOverflow => {
                formatter.write_str("workgroup dimension product overflows")
            }
            Self::WorkgroupVolumeTooLarge { actual, max } => {
                write!(formatter, "workgroup volume {actual} exceeds {max}")
            }
            Self::RequiredExceedsMaximum => {
                formatter.write_str("required workgroup dimensions exceed the declared maximum")
            }
            Self::OccupancyRequiresMaximum => formatter
                .write_str("minimum resident workgroups requires maximum workgroup dimensions"),
            Self::InvalidOccupancy { actual, max } => write!(
                formatter,
                "minimum resident workgroups {actual} is outside 1..={max}"
            ),
            Self::EmptyAssemblyOperands => {
                formatter.write_str("unsafe assembly must declare at least one operand kind")
            }
            Self::UnsupportedAssemblyOperandBits(bits) => {
                write!(
                    formatter,
                    "unsupported unsafe assembly operand bits {bits:#x}"
                )
            }
            Self::UnsupportedAssemblyOptionBits(bits) => {
                write!(
                    formatter,
                    "unsupported unsafe assembly option bits {bits:#x}"
                )
            }
            Self::UnsupportedAssemblyEffectBits(bits) => {
                write!(
                    formatter,
                    "unsupported unsafe assembly effect bits {bits:#x}"
                )
            }
            Self::ConflictingAssemblyOptions => {
                formatter.write_str("unsafe assembly options conflict")
            }
            Self::AssemblyEffectsConflictWithOptions => {
                formatter.write_str("unsafe assembly effects conflict with its options")
            }
        }
    }
}

/// Nonzero three-dimensional workgroup dimensions with a bounded volume.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkgroupDimensionsV1 {
    x: u32,
    y: u32,
    z: u32,
}

impl WorkgroupDimensionsV1 {
    pub const fn new(x: u32, y: u32, z: u32) -> Result<Self, KernelFrontendContractErrorV1> {
        if x == 0 || y == 0 || z == 0 {
            return Err(KernelFrontendContractErrorV1::ZeroWorkgroupDimension);
        }
        let Some(volume) = (x as u64).checked_mul(y as u64) else {
            return Err(KernelFrontendContractErrorV1::WorkgroupVolumeOverflow);
        };
        let Some(volume) = volume.checked_mul(z as u64) else {
            return Err(KernelFrontendContractErrorV1::WorkgroupVolumeOverflow);
        };
        if volume > MAX_WORKGROUP_THREADS_V1 as u64 {
            return Err(KernelFrontendContractErrorV1::WorkgroupVolumeTooLarge {
                actual: volume,
                max: MAX_WORKGROUP_THREADS_V1,
            });
        }
        Ok(Self { x, y, z })
    }

    pub const fn as_array(self) -> [u32; 3] {
        [self.x, self.y, self.z]
    }

    pub const fn volume(self) -> u32 {
        self.x * self.y * self.z
    }

    const fn contains(self, required: Self) -> bool {
        required.x <= self.x && required.y <= self.y && required.z <= self.z
    }
}

/// Source-level launch bounds. Target admission must validate them again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchBoundsV1 {
    required: Option<WorkgroupDimensionsV1>,
    maximum: Option<WorkgroupDimensionsV1>,
    min_workgroups_per_compute_unit: Option<u16>,
}

impl LaunchBoundsV1 {
    pub const fn new(
        required: Option<WorkgroupDimensionsV1>,
        maximum: Option<WorkgroupDimensionsV1>,
        min_workgroups_per_compute_unit: Option<u16>,
    ) -> Result<Self, KernelFrontendContractErrorV1> {
        if required.is_none() && maximum.is_none() {
            return Err(KernelFrontendContractErrorV1::EmptyContract);
        }
        if let (Some(required), Some(maximum)) = (required, maximum)
            && !maximum.contains(required)
        {
            return Err(KernelFrontendContractErrorV1::RequiredExceedsMaximum);
        }
        if let Some(actual) = min_workgroups_per_compute_unit {
            if maximum.is_none() {
                return Err(KernelFrontendContractErrorV1::OccupancyRequiresMaximum);
            }
            if actual == 0 || actual > MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1 {
                return Err(KernelFrontendContractErrorV1::InvalidOccupancy {
                    actual,
                    max: MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1,
                });
            }
        }
        Ok(Self {
            required,
            maximum,
            min_workgroups_per_compute_unit,
        })
    }

    pub const fn required(self) -> Option<WorkgroupDimensionsV1> {
        self.required
    }

    pub const fn maximum(self) -> Option<WorkgroupDimensionsV1> {
        self.maximum
    }

    pub const fn min_workgroups_per_compute_unit(self) -> Option<u16> {
        self.min_workgroups_per_compute_unit
    }
}

/// Assembly syntax and register model selected by the declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum UnsafeAssemblyTargetV1 {
    AmdGpuGfx942 = 1,
}

impl UnsafeAssemblyTargetV1 {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::AmdGpuGfx942 => "gfx942",
        }
    }
}

macro_rules! bit_set {
    ($name:ident, $allowed:expr, $error:ident, {$($constant:ident = $value:expr),+ $(,)?}) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[repr(transparent)]
        pub struct $name(u16);

        impl $name {
            $(pub const $constant: Self = Self($value);)+

            pub const fn from_bits(bits: u16) -> Result<Self, KernelFrontendContractErrorV1> {
                if bits & !$allowed != 0 {
                    return Err(KernelFrontendContractErrorV1::$error(bits));
                }
                Ok(Self(bits))
            }

            pub const fn bits(self) -> u16 {
                self.0
            }

            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }
        }
    };
}

bit_set!(AssemblyOperandSetV1, 0x000f, UnsupportedAssemblyOperandBits, {
    SGPR = 0x0001,
    VGPR = 0x0002,
    IMMEDIATE = 0x0004,
    ADDRESS = 0x0008,
});

bit_set!(AssemblyOptionSetV1, 0x001f, UnsupportedAssemblyOptionBits, {
    NOMEM = 0x0001,
    READONLY = 0x0002,
    PURE = 0x0004,
    PRESERVES_FLAGS = 0x0008,
    NOSTACK = 0x0010,
});

bit_set!(AssemblyEffectSetV1, 0x007f, UnsupportedAssemblyEffectBits, {
    READ_GLOBAL = 0x0001,
    WRITE_GLOBAL = 0x0002,
    READ_WORKGROUP = 0x0004,
    WRITE_WORKGROUP = 0x0008,
    ATOMIC = 0x0010,
    BARRIER = 0x0020,
    CONTROL_FLOW = 0x0040,
});

/// Explicit unsafe boundary for target-specific assembly reachable from a kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsafeAssemblyDeclarationV1 {
    target: UnsafeAssemblyTargetV1,
    operands: AssemblyOperandSetV1,
    options: AssemblyOptionSetV1,
    effects: AssemblyEffectSetV1,
}

impl UnsafeAssemblyDeclarationV1 {
    pub const fn new(
        target: UnsafeAssemblyTargetV1,
        operands: AssemblyOperandSetV1,
        options: AssemblyOptionSetV1,
        effects: AssemblyEffectSetV1,
    ) -> Result<Self, KernelFrontendContractErrorV1> {
        if operands.bits() == 0 {
            return Err(KernelFrontendContractErrorV1::EmptyAssemblyOperands);
        }
        if options.contains(AssemblyOptionSetV1::NOMEM)
            && options.contains(AssemblyOptionSetV1::READONLY)
        {
            return Err(KernelFrontendContractErrorV1::ConflictingAssemblyOptions);
        }
        if options.contains(AssemblyOptionSetV1::PURE)
            && !options.contains(AssemblyOptionSetV1::NOMEM)
            && !options.contains(AssemblyOptionSetV1::READONLY)
        {
            return Err(KernelFrontendContractErrorV1::ConflictingAssemblyOptions);
        }

        let writes = AssemblyEffectSetV1::WRITE_GLOBAL
            .union(AssemblyEffectSetV1::WRITE_WORKGROUP)
            .union(AssemblyEffectSetV1::ATOMIC);
        let any_memory = AssemblyEffectSetV1::READ_GLOBAL
            .union(AssemblyEffectSetV1::WRITE_GLOBAL)
            .union(AssemblyEffectSetV1::READ_WORKGROUP)
            .union(AssemblyEffectSetV1::WRITE_WORKGROUP)
            .union(AssemblyEffectSetV1::ATOMIC)
            .union(AssemblyEffectSetV1::BARRIER);
        if options.contains(AssemblyOptionSetV1::NOMEM) && effects.bits() & any_memory.bits() != 0 {
            return Err(KernelFrontendContractErrorV1::AssemblyEffectsConflictWithOptions);
        }
        if options.contains(AssemblyOptionSetV1::READONLY) && effects.bits() & writes.bits() != 0 {
            return Err(KernelFrontendContractErrorV1::AssemblyEffectsConflictWithOptions);
        }
        if options.contains(AssemblyOptionSetV1::PURE)
            && effects.contains(AssemblyEffectSetV1::CONTROL_FLOW)
        {
            return Err(KernelFrontendContractErrorV1::AssemblyEffectsConflictWithOptions);
        }
        if effects.bits() == 0 && !options.contains(AssemblyOptionSetV1::NOMEM) {
            return Err(KernelFrontendContractErrorV1::AssemblyEffectsConflictWithOptions);
        }

        Ok(Self {
            target,
            operands,
            options,
            effects,
        })
    }

    pub const fn target(self) -> UnsafeAssemblyTargetV1 {
        self.target
    }

    pub const fn operands(self) -> AssemblyOperandSetV1 {
        self.operands
    }

    pub const fn options(self) -> AssemblyOptionSetV1 {
        self.options
    }

    pub const fn effects(self) -> AssemblyEffectSetV1 {
        self.effects
    }
}

/// Complete source declaration attached to one kernel entry point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelFrontendContractV1 {
    launch: Option<LaunchBoundsV1>,
    unsafe_assembly: Option<UnsafeAssemblyDeclarationV1>,
}

impl KernelFrontendContractV1 {
    pub const fn new(
        launch: Option<LaunchBoundsV1>,
        unsafe_assembly: Option<UnsafeAssemblyDeclarationV1>,
    ) -> Result<Self, KernelFrontendContractErrorV1> {
        if launch.is_none() && unsafe_assembly.is_none() {
            return Err(KernelFrontendContractErrorV1::EmptyContract);
        }
        Ok(Self {
            launch,
            unsafe_assembly,
        })
    }

    pub const fn launch(self) -> Option<LaunchBoundsV1> {
        self.launch
    }

    pub const fn unsafe_assembly(self) -> Option<UnsafeAssemblyDeclarationV1> {
        self.unsafe_assembly
    }
}
