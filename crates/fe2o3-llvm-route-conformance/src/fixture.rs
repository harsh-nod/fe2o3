use fe2o3_llvm_handoff::{
    AddressSpaceV1, DeviceLibraryInputV1, DeviceLibraryKindV1, FunctionAttributeV1,
    Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942TargetPolicyV1, HandoffDiagnosticV1, IdentityV1,
    KernelEntryV1, KernelParameterV1, KernelValueTypeV1, ModuleFlagV1, ModuleMetadataV1,
    NamedMetadataV1, ObligationKindV1, ObligationV1, OriginKindV1, OriginV1, ParameterAttributeV1,
    ScalarTypeV1, SourceSpanV1, StageIdentitiesV1, WavesPerEuV1, WorkgroupSizeRangeV1,
};
use fe2o3_llvm_worker_handoff::SUPPORTED_DEVICE_LIBRARY_CLOSURE_V1;

/// Every address space represented by the public handoff V1 model.
pub const GFX942_FIXTURE_ADDRESS_SPACES_V1: [AddressSpaceV1; 6] = [
    AddressSpaceV1::Flat,
    AddressSpaceV1::Global,
    AddressSpaceV1::Region,
    AddressSpaceV1::Local,
    AddressSpaceV1::Constant,
    AddressSpaceV1::Private,
];

/// Valid alignment values paired with the fixture address-space parameters.
pub const GFX942_FIXTURE_ALIGNMENTS_V1: [u16; 6] = [1, 2, 4, 8, 16, 256];

/// Every origin kind represented by the public handoff V1 model.
pub const GFX942_FIXTURE_ORIGINS_V1: [OriginKindV1; 5] = [
    OriginKindV1::RustSource,
    OriginKindV1::Mir,
    OriginKindV1::KernelIr,
    OriginKindV1::ScheduleIr,
    OriginKindV1::AmdgcnIr,
];

/// Every preservation-obligation kind represented by handoff V1.
pub const GFX942_FIXTURE_OBLIGATIONS_V1: [ObligationKindV1; 8] = [
    ObligationKindV1::PreserveKernelAbi,
    ObligationKindV1::PreserveAddressSpaces,
    ObligationKindV1::PreserveTargetFeatures,
    ObligationKindV1::PreserveCallingConvention,
    ObligationKindV1::PreserveFunctionAttributes,
    ObligationKindV1::PreserveModuleMetadata,
    ObligationKindV1::AuthenticateDeviceLibraries,
    ObligationKindV1::MaintainOriginCoverage,
];

/// Every device-library kind represented by the public handoff V1 model.
pub const GFX942_FIXTURE_DEVICE_LIBRARIES_V1: [DeviceLibraryKindV1; 9] = [
    DeviceLibraryKindV1::Ocml,
    DeviceLibraryKindV1::Ockl,
    DeviceLibraryKindV1::OpenCl,
    DeviceLibraryKindV1::OclcIsaVersion942,
    DeviceLibraryKindV1::OclcWavefrontSize64On,
    DeviceLibraryKindV1::OclcFiniteOnlyOff,
    DeviceLibraryKindV1::OclcUnsafeMathOff,
    DeviceLibraryKindV1::OclcCorrectlyRoundedSqrtOn,
    DeviceLibraryKindV1::OclcDazOff,
];

/// Input order used to verify canonicalization of unordered handoff collections.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FixtureCollectionOrderV1 {
    /// Construct collections in their declared fixture order.
    #[default]
    Declared,
    /// Reverse every semantically unordered collection before validation.
    Reversed,
}

/// Device-library set included by the deterministic fixture builder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FixtureDeviceLibrarySetV1 {
    /// Include every device-library kind represented by handoff V1.
    #[default]
    FullHandoffSurface,
    /// Include the complete closed set admitted by worker admission V1.
    WorkerSupportedClosure,
}

/// Reusable deterministic builder for the generic-CI gfx942 handoff fixture.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx942FixtureBuilderV1 {
    order: FixtureCollectionOrderV1,
    device_libraries: FixtureDeviceLibrarySetV1,
}

impl Gfx942FixtureBuilderV1 {
    /// Creates a builder using declared collection order.
    pub const fn new() -> Self {
        Self {
            order: FixtureCollectionOrderV1::Declared,
            device_libraries: FixtureDeviceLibrarySetV1::FullHandoffSurface,
        }
    }

    /// Selects the pre-validation order for semantically unordered collections.
    pub const fn with_collection_order(mut self, order: FixtureCollectionOrderV1) -> Self {
        self.order = order;
        self
    }

    /// Selects the deterministic device-library closure included in the fixture.
    pub const fn with_device_libraries(mut self, libraries: FixtureDeviceLibrarySetV1) -> Self {
        self.device_libraries = libraries;
        self
    }

    /// Builds deterministic, unchecked construction data for handoff V1.
    ///
    /// All nested values are still created through their public checked APIs.
    pub fn build_input(self) -> Result<Gfx942HandoffInputV1, HandoffDiagnosticV1> {
        let mut origins = self.origins()?;
        let rust_origin = origins[0].identity();
        let amdgcn_origin = origins[4].identity();

        let mut kernels = vec![
            self.kernel("address_space_probe", rust_origin, true)?,
            self.kernel("zeta_metadata_probe", amdgcn_origin, false)?,
        ];
        let mut obligations = self.obligations(&origins)?;
        self.maybe_reverse(&mut kernels);
        self.maybe_reverse(&mut origins);
        self.maybe_reverse(&mut obligations);

        Ok(Gfx942HandoffInputV1 {
            stage_identities: StageIdentitiesV1::new([0x01; 32], [0x02; 32], [0x03; 32])?,
            target: Gfx942TargetPolicyV1::canonical(),
            kernels,
            module: self.module_metadata()?,
            origins,
            obligations,
        })
    }

    /// Builds and validates the deterministic gfx942 handoff fixture.
    pub fn build(self) -> Result<Gfx942HandoffV1, HandoffDiagnosticV1> {
        Gfx942HandoffV1::new(self.build_input()?)
    }

    fn kernel(
        self,
        symbol: &str,
        origin: fe2o3_llvm_handoff::OriginIdentityV1,
        with_parameters: bool,
    ) -> Result<KernelEntryV1, HandoffDiagnosticV1> {
        let parameters = if with_parameters {
            self.parameters()?
        } else {
            Vec::new()
        };
        let mut attributes =
            FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(64, 256)?);
        attributes.push(FunctionAttributeV1::WavesPerEu(WavesPerEuV1::new(2, 8)?));
        self.maybe_reverse(&mut attributes);
        KernelEntryV1::new(symbol, parameters, attributes, origin)
    }

    fn parameters(self) -> Result<Vec<KernelParameterV1>, HandoffDiagnosticV1> {
        const NAMES: [&str; 6] = [
            "flat_ptr",
            "global_ptr",
            "region_ptr",
            "local_ptr",
            "constant_ptr",
            "private_ptr",
        ];
        const POINTEES: [ScalarTypeV1; 6] = [
            ScalarTypeV1::I8,
            ScalarTypeV1::F32,
            ScalarTypeV1::I32,
            ScalarTypeV1::F16,
            ScalarTypeV1::Bf16,
            ScalarTypeV1::F64,
        ];

        let mut parameters = Vec::with_capacity(GFX942_FIXTURE_ADDRESS_SPACES_V1.len() + 1);
        for index in 0..GFX942_FIXTURE_ADDRESS_SPACES_V1.len() {
            let alignment = GFX942_FIXTURE_ALIGNMENTS_V1[index];
            let mut attributes = vec![
                ParameterAttributeV1::NoCapture,
                ParameterAttributeV1::NonNull,
                ParameterAttributeV1::Align(alignment),
                ParameterAttributeV1::Dereferenceable(u32::from(alignment) * 16),
            ];
            self.maybe_reverse(&mut attributes);
            parameters.push(KernelParameterV1::new(
                NAMES[index],
                KernelValueTypeV1::Pointer {
                    pointee: POINTEES[index],
                    address_space: GFX942_FIXTURE_ADDRESS_SPACES_V1[index],
                },
                attributes,
            )?);
        }
        parameters.push(KernelParameterV1::new(
            "element_count",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            Vec::new(),
        )?);
        Ok(parameters)
    }

    fn module_metadata(self) -> Result<ModuleMetadataV1, HandoffDiagnosticV1> {
        let mut flags = vec![
            ModuleFlagV1::CodeObjectVersion6,
            ModuleFlagV1::PicLevel2,
            ModuleFlagV1::WcharSize4,
        ];
        let mut named = vec![
            NamedMetadataV1::OpenClVersion2_0,
            NamedMetadataV1::OpenClSpirVersion2_0,
            NamedMetadataV1::ProducerIdentity(identity(0x31)?),
        ];
        let mut libraries = GFX942_FIXTURE_DEVICE_LIBRARIES_V1
            .into_iter()
            .enumerate()
            .filter(|(_, kind)| {
                self.device_libraries == FixtureDeviceLibrarySetV1::FullHandoffSurface
                    || SUPPORTED_DEVICE_LIBRARY_CLOSURE_V1.contains(kind)
            })
            .map(|(index, kind)| {
                let ordinal = u8::try_from(index).expect("fixed device-library count fits u8");
                DeviceLibraryInputV1::new(
                    kind,
                    [0x41 + ordinal; 32],
                    (u64::try_from(index).expect("fixed index fits u64") + 1) * 4_096,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.maybe_reverse(&mut flags);
        self.maybe_reverse(&mut named);
        self.maybe_reverse(&mut libraries);
        ModuleMetadataV1::new(flags, named, libraries)
    }

    fn origins(self) -> Result<Vec<OriginV1>, HandoffDiagnosticV1> {
        GFX942_FIXTURE_ORIGINS_V1
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let ordinal = u8::try_from(index).expect("fixed origin count fits u8");
                let span = if kind == OriginKindV1::RustSource {
                    Some(SourceSpanV1::new(
                        "crates/conformance/src/kernel.rs",
                        10,
                        1,
                        42,
                        2,
                    )?)
                } else {
                    None
                };
                Ok(OriginV1::new(kind, identity(0x11 + ordinal)?, span))
            })
            .collect()
    }

    fn obligations(self, origins: &[OriginV1]) -> Result<Vec<ObligationV1>, HandoffDiagnosticV1> {
        GFX942_FIXTURE_OBLIGATIONS_V1
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let ordinal = u8::try_from(index).expect("fixed obligation count fits u8");
                Ok(ObligationV1::new(
                    kind,
                    identity(0x71 + ordinal)?,
                    origins[index % origins.len()].identity(),
                ))
            })
            .collect()
    }

    fn maybe_reverse<T>(self, values: &mut [T]) {
        if self.order == FixtureCollectionOrderV1::Reversed {
            values.reverse();
        }
    }
}

/// Builds the declared-order deterministic gfx942 handoff fixture.
pub fn gfx942_fixture_v1() -> Result<Gfx942HandoffV1, HandoffDiagnosticV1> {
    Gfx942FixtureBuilderV1::new().build()
}

fn identity(byte: u8) -> Result<IdentityV1, HandoffDiagnosticV1> {
    IdentityV1::new([byte; 32])
}
