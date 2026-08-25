#[cfg(any(test, feature = "qualification-oracles-test-only"))]
use crate::CompilerGeneratedArgumentLayoutV1;
use crate::{
    AliasAdmissionError, ArtifactKernelIdentityV1, AuthenticatedKernelArtifactV1,
    CompilerGeneratedKernelContractV1, GeneratedAdmittedLaunch,
    GeneratedArtifactAuthenticationError, GeneratedReadDeviceSlice, GeneratedWriteDeviceSlice,
    LoadedKernel, LoadedKernelLoadError, LoadedKernelMatchError, LoadedLaunchError,
    ObservedContext, PrepareLaunchError, RegionError, UntrustedLaunchRequest,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
use fe2o3_artifacts::{AbiField, Name};
use fe2o3_artifacts::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    LaunchContract, Mutability, PointerWidth,
};
use fe2o3_core::{
    BorrowedDeviceOperation, DeviceBuffer, Error as CoreError, GpuContext, KernelParams, Stream,
};
use std::fmt;
use std::sync::Arc;

const VECADD_BLOCK_SIZE: u32 = 256;
const VECADD_ABI_SIZE: u64 = 48;
const VECADD_ABI_ALIGNMENT: u32 = 8;
const VECADD_FIELD_SIZE: u64 = 16;
const VECADD_FIELD_ALIGNMENT: u32 = 8;

type GeneratedVecAddResources<'allocation> = (
    GeneratedReadDeviceSlice<'allocation, f32>,
    GeneratedReadDeviceSlice<'allocation, f32>,
    GeneratedWriteDeviceSlice<'allocation, f32>,
);

/// Loaded authority for the exact generated `f32` vecadd profile.
///
/// This is compiler-generated host-binding infrastructure, not an application
/// extension point. Construction authenticates only the artifact embedded for
/// `K`, checks the complete profile represented by this adapter, and retains the
/// exact observed context with the loaded module.
#[doc(hidden)]
pub struct GeneratedVecAddKernelV1<K: CompilerGeneratedKernelContractV1> {
    loaded: LoadedKernel<K>,
    observed: ObservedContext,
}

impl<K: CompilerGeneratedKernelContractV1> fmt::Debug for GeneratedVecAddKernelV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedVecAddKernelV1")
            .field("identity", self.loaded.identity())
            .field("observed", &self.observed)
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelContractV1> GeneratedVecAddKernelV1<K> {
    /// Authenticates, profile-checks, and loads the artifact embedded for `K`.
    pub fn load(context: &Arc<GpuContext>) -> Result<Self, GeneratedVecAddLoadError> {
        let observed =
            ObservedContext::observe(context).map_err(GeneratedVecAddLoadError::Observe)?;
        let authenticated = AuthenticatedKernelArtifactV1::<K>::authenticate(&observed)
            .map_err(GeneratedVecAddLoadError::Authenticate)?;
        validate_vecadd_profile(authenticated.identity())
            .map_err(GeneratedVecAddLoadError::Profile)?;
        let loaded = authenticated
            .load(context)
            .map_err(GeneratedVecAddLoadError::Load)?;
        Ok(Self { loaded, observed })
    }

    /// Prepares one exact read/read/write vecadd launch.
    ///
    /// The returned value retains the shared input borrows, exclusive output
    /// borrow, alias admission, loaded authority, and six physical parameter
    /// values until a synchronous or scoped launch establishes quiescence.
    pub fn prepare<'allocation>(
        &self,
        a: &'allocation DeviceBuffer<f32>,
        b: &'allocation DeviceBuffer<f32>,
        c: &'allocation mut DeviceBuffer<f32>,
    ) -> Result<GeneratedVecAddPreparedV1<'_, 'allocation, K>, GeneratedVecAddPrepareError> {
        let grid_x = checked_vecadd_grid(a.len(), b.len(), c.len())?;

        let a = GeneratedReadDeviceSlice::new(&self.observed, a)
            .map_err(GeneratedVecAddPrepareError::Region)?;
        let b = GeneratedReadDeviceSlice::new(&self.observed, b)
            .map_err(GeneratedVecAddPrepareError::Region)?;
        let c = GeneratedWriteDeviceSlice::new(&self.observed, c)
            .map_err(GeneratedVecAddPrepareError::Region)?;

        let request = UntrustedLaunchRequest::<K>::new(
            self.loaded.identity().kernel_id(),
            1,
            [grid_x, 1, 1],
            [VECADD_BLOCK_SIZE, 1, 1],
            0,
        );
        let prepared = self
            .loaded
            .prepare(&self.observed, request)
            .map_err(GeneratedVecAddPrepareError::Geometry)?;

        let admitted = prepared
            .admit_arguments([
                a.argument_access(),
                b.argument_access(),
                c.argument_access(),
            ])
            .map_err(GeneratedVecAddPrepareError::Admission)?;
        let admitted = self
            .loaded
            .bind_admitted(admitted)
            .map_err(GeneratedVecAddPrepareError::Binding)?;

        let mut params = KernelParams::new();
        a.push_pointer_and_len(&mut params);
        b.push_pointer_and_len(&mut params);
        c.push_pointer_and_len(&mut params);
        debug_assert_eq!(params.len(), 6);

        // SAFETY: `CompilerGeneratedKernelContractV1` requires the trusted
        // backend to bind `K` to the exact executable, complete physical ABI,
        // opaque Rust type/layout identities, and all executable effects. The
        // authenticated token admitted only those embedded bytes. Before load,
        // `validate_vecadd_profile` independently required exactly three
        // 16-byte, 8-aligned global f32 slices in a 48-byte 64-bit ABI with
        // shared-read/shared-read/unique-write-only effects, plus the exact
        // 1D/256/no-shared-memory launch contract. The capabilities above own
        // the same three buffers used to pack these six pointer/length values;
        // their access descriptors are admitted in matching order and mode.
        // Equal nonzero u32-domain lengths and checked geometry ensure every
        // packed length and reachable linear element lies in the adapter's
        // represented regions.
        let launch = unsafe {
            GeneratedAdmittedLaunch::from_generated_unchecked(admitted, params, (a, b, c))
        };

        Ok(GeneratedVecAddPreparedV1 { launch })
    }
}

/// A sealed generated vecadd launch retaining all borrowed resources.
#[doc(hidden)]
#[must_use = "a prepared vecadd launch does no work until it is launched"]
pub struct GeneratedVecAddPreparedV1<'loaded, 'allocation, K> {
    launch: GeneratedAdmittedLaunch<'loaded, 'allocation, K, GeneratedVecAddResources<'allocation>>,
}

impl<'loaded, 'allocation, K> GeneratedVecAddPreparedV1<'loaded, 'allocation, K> {
    /// Enqueues the launch and waits for completion before releasing borrows.
    pub fn launch(self, stream: &Stream) -> Result<(), LoadedLaunchError> {
        self.launch.launch_generated(stream)
    }

    /// Runs a callback while the launch is in flight, then waits for completion.
    ///
    /// The higher-ranked callback can observe the operation but cannot return
    /// it or otherwise extend the operation lifetime beyond this call.
    pub fn launch_scoped<'stream, O>(
        self,
        stream: &'stream Stream,
        during: impl for<'operation> FnOnce(
            &'operation BorrowedDeviceOperation<'stream, 'allocation>,
        ) -> O,
    ) -> Result<O, LoadedLaunchError> {
        self.launch.launch_generated_scoped(stream, during)
    }
}

/// Failure while authenticating and loading a generated vecadd artifact.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedVecAddLoadError {
    Observe(CoreError),
    Authenticate(GeneratedArtifactAuthenticationError),
    Profile(GeneratedVecAddProfileError),
    Load(LoadedKernelLoadError),
}

impl fmt::Display for GeneratedVecAddLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observe(error) => write!(formatter, "failed to observe GPU context: {error}"),
            Self::Authenticate(error) => {
                write!(
                    formatter,
                    "failed to authenticate generated vecadd artifact: {error}"
                )
            }
            Self::Profile(error) => write!(formatter, "invalid generated vecadd profile: {error}"),
            Self::Load(error) => write!(formatter, "failed to load generated vecadd: {error}"),
        }
    }
}

impl std::error::Error for GeneratedVecAddLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Observe(error) => Some(error),
            Self::Authenticate(error) => Some(error),
            Self::Profile(error) => Some(error),
            Self::Load(error) => Some(error),
        }
    }
}

/// Failure while preparing exact generated vecadd arguments and geometry.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedVecAddPrepareError {
    EmptyInput,
    LengthMismatch { a: usize, b: usize, c: usize },
    LinearIndexDomainExceeded { length: usize, max: u32 },
    Region(RegionError),
    Geometry(PrepareLaunchError),
    Admission(AliasAdmissionError),
    Binding(LoadedKernelMatchError),
}

impl fmt::Display for GeneratedVecAddPrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => formatter.write_str("vecadd length must be nonzero"),
            Self::LengthMismatch { a, b, c } => {
                write!(formatter, "vecadd lengths differ: a={a}, b={b}, c={c}")
            }
            Self::LinearIndexDomainExceeded { length, max } => write!(
                formatter,
                "vecadd length {length} exceeds the u32 linear-index limit {max}"
            ),
            Self::Region(error) => write!(formatter, "invalid vecadd buffer region: {error}"),
            Self::Geometry(error) => write!(formatter, "invalid vecadd launch geometry: {error}"),
            Self::Admission(error) => write!(formatter, "vecadd alias admission failed: {error}"),
            Self::Binding(error) => write!(formatter, "vecadd artifact binding failed: {error}"),
        }
    }
}

impl std::error::Error for GeneratedVecAddPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Region(error) => Some(error),
            Self::Geometry(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::EmptyInput
            | Self::LengthMismatch { .. }
            | Self::LinearIndexDomainExceeded { .. } => None,
        }
    }
}

/// Why an authenticated artifact is not the exact generated vecadd profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedVecAddProfileError {
    HostRustLayout,
    AbiSize {
        actual: u64,
    },
    AbiAlignment {
        actual: u32,
    },
    PointerWidth {
        actual: PointerWidth,
    },
    AbiFieldCount {
        actual: usize,
    },
    AbiFieldShape {
        index: usize,
        expected_offset: u64,
        actual_offset: u64,
        actual_size: u64,
        actual_alignment: u32,
        actual_kind: AbiKind,
    },
    AbiFieldContract {
        index: usize,
        mutability: Mutability,
        access: Access,
        address_space: AddressSpace,
        ownership: ArgumentOwnership,
        alias_class: AliasClass,
    },
    LaunchRank {
        actual: u8,
    },
    LaunchBlockSize {
        actual: BlockSize,
    },
    LaunchGridShape {
        actual: [u32; 3],
    },
    StaticSharedMemory {
        actual: u32,
    },
    DynamicSharedMemory {
        actual: u32,
    },
}

impl fmt::Display for GeneratedVecAddProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostRustLayout => formatter
                .write_str("host Rust layout does not match the generated 64-bit vecadd contract"),
            Self::AbiSize { actual } => write!(
                formatter,
                "ABI size {actual} does not match required size {VECADD_ABI_SIZE}"
            ),
            Self::AbiAlignment { actual } => write!(
                formatter,
                "ABI alignment {actual} does not match required alignment {VECADD_ABI_ALIGNMENT}"
            ),
            Self::PointerWidth { actual } => {
                write!(formatter, "ABI pointer width {actual:?} is not 64-bit")
            }
            Self::AbiFieldCount { actual } => {
                write!(formatter, "ABI has {actual} fields; vecadd requires three")
            }
            Self::AbiFieldShape {
                index,
                expected_offset,
                actual_offset,
                actual_size,
                actual_alignment,
                actual_kind,
            } => write!(
                formatter,
                "ABI field {index} has offset {actual_offset}, size {actual_size}, alignment \
                 {actual_alignment}, and kind {actual_kind:?}; expected a 16-byte, 8-aligned f32 \
                 slice at offset {expected_offset}"
            ),
            Self::AbiFieldContract {
                index,
                mutability,
                access,
                address_space,
                ownership,
                alias_class,
            } => write!(
                formatter,
                "ABI field {index} has incompatible vecadd reference contract \
                 ({mutability:?}, {access:?}, {address_space:?}, {ownership:?}, {alias_class:?})"
            ),
            Self::LaunchRank { actual } => {
                write!(
                    formatter,
                    "launch rank {actual} does not match required rank 1"
                )
            }
            Self::LaunchBlockSize { actual } => write!(
                formatter,
                "launch block size {actual:?} does not match exact [256, 1, 1]"
            ),
            Self::LaunchGridShape { actual } => write!(
                formatter,
                "launch maximum grid {actual:?} is not one-dimensional"
            ),
            Self::StaticSharedMemory { actual } => write!(
                formatter,
                "static shared memory {actual} does not match required zero"
            ),
            Self::DynamicSharedMemory { actual } => write!(
                formatter,
                "dynamic shared memory maximum {actual} does not match required zero"
            ),
        }
    }
}

impl std::error::Error for GeneratedVecAddProfileError {}

pub(crate) fn validate_vecadd_profile(
    identity: &ArtifactKernelIdentityV1,
) -> Result<(), GeneratedVecAddProfileError> {
    validate_vecadd_artifact_profile(identity.abi(), identity.launch())
}

fn validate_vecadd_artifact_profile(
    abi: &AbiLayout,
    launch: &LaunchContract,
) -> Result<(), GeneratedVecAddProfileError> {
    if abi.size() != VECADD_ABI_SIZE {
        return Err(GeneratedVecAddProfileError::AbiSize { actual: abi.size() });
    }
    if abi.alignment() != VECADD_ABI_ALIGNMENT {
        return Err(GeneratedVecAddProfileError::AbiAlignment {
            actual: abi.alignment(),
        });
    }
    if abi.pointer_width() != PointerWidth::Bits64 {
        return Err(GeneratedVecAddProfileError::PointerWidth {
            actual: abi.pointer_width(),
        });
    }
    if abi.fields().len() != 3 {
        return Err(GeneratedVecAddProfileError::AbiFieldCount {
            actual: abi.fields().len(),
        });
    }

    for (index, field) in abi.fields().iter().enumerate() {
        let expected_offset = (index as u64) * VECADD_FIELD_SIZE;
        let expected_kind = AbiKind::Slice {
            element_size: 4,
            element_alignment: 4,
        };
        if field.offset() != expected_offset
            || field.size() != VECADD_FIELD_SIZE
            || field.alignment() != VECADD_FIELD_ALIGNMENT
            || field.kind() != expected_kind
        {
            return Err(GeneratedVecAddProfileError::AbiFieldShape {
                index,
                expected_offset,
                actual_offset: field.offset(),
                actual_size: field.size(),
                actual_alignment: field.alignment(),
                actual_kind: field.kind(),
            });
        }

        let expected_contract = if index < 2 {
            (
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            )
        } else {
            (
                Mutability::Mutable,
                Access::WriteOnly,
                AddressSpace::Global,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
        };
        let actual_contract = (
            field.mutability(),
            field.access(),
            field.address_space(),
            field.ownership(),
            field.alias_class(),
        );
        if actual_contract != expected_contract {
            return Err(GeneratedVecAddProfileError::AbiFieldContract {
                index,
                mutability: field.mutability(),
                access: field.access(),
                address_space: field.address_space(),
                ownership: field.ownership(),
                alias_class: field.alias_class(),
            });
        }
    }

    if launch.rank() != 1 {
        return Err(GeneratedVecAddProfileError::LaunchRank {
            actual: launch.rank(),
        });
    }
    let required_block = BlockSize::Exact(
        fe2o3_artifacts::Dimensions::new(VECADD_BLOCK_SIZE, 1, 1)
            .expect("the fixed vecadd block is valid"),
    );
    if launch.block_size() != required_block {
        return Err(GeneratedVecAddProfileError::LaunchBlockSize {
            actual: launch.block_size(),
        });
    }
    let max_grid = launch.max_grid();
    let max_grid = [max_grid.x(), max_grid.y(), max_grid.z()];
    if max_grid[1] != 1 || max_grid[2] != 1 {
        return Err(GeneratedVecAddProfileError::LaunchGridShape { actual: max_grid });
    }
    if launch.static_shared_memory_bytes() != 0 {
        return Err(GeneratedVecAddProfileError::StaticSharedMemory {
            actual: launch.static_shared_memory_bytes(),
        });
    }
    if launch.max_dynamic_shared_memory_bytes() != 0 {
        return Err(GeneratedVecAddProfileError::DynamicSharedMemory {
            actual: launch.max_dynamic_shared_memory_bytes(),
        });
    }
    Ok(())
}

pub(crate) fn checked_vecadd_grid(
    a: usize,
    b: usize,
    c: usize,
) -> Result<u32, GeneratedVecAddPrepareError> {
    if a == 0 || b == 0 || c == 0 {
        return Err(GeneratedVecAddPrepareError::EmptyInput);
    }
    if a != b || a != c {
        return Err(GeneratedVecAddPrepareError::LengthMismatch { a, b, c });
    }
    let length =
        u32::try_from(a).map_err(|_| GeneratedVecAddPrepareError::LinearIndexDomainExceeded {
            length: a,
            max: u32::MAX,
        })?;
    Ok(length.div_ceil(VECADD_BLOCK_SIZE))
}

#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn generated_vecadd_argument_layout_v2()
-> Result<CompilerGeneratedArgumentLayoutV1, GeneratedVecAddProfileError> {
    let abi = generated_vecadd_abi_v2()?;
    CompilerGeneratedArgumentLayoutV1::new(
        abi.size(),
        abi.alignment(),
        abi.pointer_width(),
        abi.fields().to_vec(),
    )
    .map_err(|_| GeneratedVecAddProfileError::HostRustLayout)
}

#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn generated_vecadd_abi_v2() -> Result<AbiLayout, GeneratedVecAddProfileError> {
    let type_identities = crate::artifact_binding::host_typed_vecadd_type_identities()
        .map_err(|_| GeneratedVecAddProfileError::HostRustLayout)?;
    let slice_kind = AbiKind::Slice {
        element_size: 4,
        element_alignment: 4,
    };
    let fields = [
        (
            "arg0",
            0,
            Mutability::Immutable,
            Access::ReadOnly,
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        ),
        (
            "arg1",
            16,
            Mutability::Immutable,
            Access::ReadOnly,
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        ),
        (
            "arg2",
            32,
            Mutability::Mutable,
            Access::WriteOnly,
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        ),
    ]
    .into_iter()
    .zip(type_identities)
    .map(
        |((name, offset, mutability, access, ownership, alias), type_identity)| {
            AbiField::new(
                Name::new(name).expect("fixed generated vecadd names are valid"),
                offset,
                VECADD_FIELD_SIZE,
                VECADD_FIELD_ALIGNMENT,
                slice_kind,
                mutability,
                access,
                AddressSpace::Global,
                type_identity,
                ownership,
                alias,
            )
            .expect("fixed generated vecadd fields are valid")
        },
    )
    .collect();
    AbiLayout::new(
        VECADD_ABI_SIZE,
        VECADD_ABI_ALIGNMENT,
        PointerWidth::Bits64,
        fields,
    )
    .map_err(|_| GeneratedVecAddProfileError::HostRustLayout)
}

#[cfg(test)]
mod tests {
    use super::{
        GeneratedVecAddPrepareError, GeneratedVecAddProfileError, checked_vecadd_grid,
        validate_vecadd_artifact_profile,
    };
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        BlockSize, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, Dimensions,
        LaunchContract, Mutability, Name, PointerWidth, TypeIdentity,
    };

    fn type_identity(seed: u8) -> TypeIdentity {
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes([seed; 32])),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                [seed.wrapping_add(1); 32],
            )),
        )
    }

    fn field(index: usize, output_access: Access) -> AbiField {
        field_with_kind(
            index,
            output_access,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            16,
            8,
        )
    }

    fn field_with_kind(
        index: usize,
        output_access: Access,
        kind: AbiKind,
        size: u64,
        alignment: u32,
    ) -> AbiField {
        let output = index == 2;
        AbiField::new(
            Name::new(["a", "b", "c"][index]).unwrap(),
            (index as u64) * size,
            size,
            alignment,
            kind,
            if output {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            },
            if output {
                output_access
            } else {
                Access::ReadOnly
            },
            AddressSpace::Global,
            type_identity(index as u8),
            if output {
                ArgumentOwnership::UniqueBorrow
            } else {
                ArgumentOwnership::SharedBorrow
            },
            if output {
                AliasClass::Exclusive
            } else {
                AliasClass::SharedReadOnly
            },
        )
        .unwrap()
    }

    fn abi(output_access: Access) -> AbiLayout {
        AbiLayout::new(
            48,
            8,
            PointerWidth::Bits64,
            (0..3).map(|index| field(index, output_access)).collect(),
        )
        .unwrap()
    }

    fn launch(block_size: BlockSize, static_shared: u32, dynamic_shared: u32) -> LaunchContract {
        LaunchContract::new(
            1,
            block_size,
            Dimensions::new(u32::MAX, 1, 1).unwrap(),
            static_shared,
            dynamic_shared,
        )
        .unwrap()
    }

    fn exact_launch() -> LaunchContract {
        launch(BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()), 0, 0)
    }

    #[test]
    fn exact_vecadd_profile_is_accepted() {
        validate_vecadd_artifact_profile(&abi(Access::WriteOnly), &exact_launch()).unwrap();
    }

    #[test]
    fn output_must_be_unique_write_only_global_memory() {
        assert!(matches!(
            validate_vecadd_artifact_profile(&abi(Access::ReadWrite), &exact_launch()),
            Err(GeneratedVecAddProfileError::AbiFieldContract { index: 2, .. })
        ));
    }

    #[test]
    fn physical_abi_shape_is_exact() {
        let oversized = AbiLayout::new(
            56,
            8,
            PointerWidth::Bits64,
            (0..3)
                .map(|index| field(index, Access::WriteOnly))
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            validate_vecadd_artifact_profile(&oversized, &exact_launch()),
            Err(GeneratedVecAddProfileError::AbiSize { actual: 56 })
        ));

        let f64_fields = AbiLayout::new(
            48,
            8,
            PointerWidth::Bits64,
            (0..3)
                .map(|index| {
                    field_with_kind(
                        index,
                        Access::WriteOnly,
                        AbiKind::Slice {
                            element_size: 8,
                            element_alignment: 8,
                        },
                        16,
                        8,
                    )
                })
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            validate_vecadd_artifact_profile(&f64_fields, &exact_launch()),
            Err(GeneratedVecAddProfileError::AbiFieldShape { index: 0, .. })
        ));
    }

    #[test]
    fn profile_rejects_a_32_bit_slice_abi() {
        let abi = AbiLayout::new(
            48,
            8,
            PointerWidth::Bits32,
            (0..3)
                .map(|index| {
                    field_with_kind(
                        index,
                        Access::WriteOnly,
                        AbiKind::Slice {
                            element_size: 4,
                            element_alignment: 4,
                        },
                        8,
                        4,
                    )
                })
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            validate_vecadd_artifact_profile(&abi, &exact_launch()),
            Err(GeneratedVecAddProfileError::PointerWidth {
                actual: PointerWidth::Bits32,
            })
        ));
    }

    #[test]
    fn launch_requires_exact_block_and_no_shared_memory() {
        let any_block = launch(BlockSize::Any, 0, 0);
        assert!(matches!(
            validate_vecadd_artifact_profile(&abi(Access::WriteOnly), &any_block),
            Err(GeneratedVecAddProfileError::LaunchBlockSize { .. })
        ));

        let dynamic_shared = launch(BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()), 0, 4);
        assert!(matches!(
            validate_vecadd_artifact_profile(&abi(Access::WriteOnly), &dynamic_shared),
            Err(GeneratedVecAddProfileError::DynamicSharedMemory { actual: 4 })
        ));

        let static_shared = launch(BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()), 4, 0);
        assert!(matches!(
            validate_vecadd_artifact_profile(&abi(Access::WriteOnly), &static_shared),
            Err(GeneratedVecAddProfileError::StaticSharedMemory { actual: 4 })
        ));

        let rank_two = LaunchContract::new(
            2,
            BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
            Dimensions::new(u32::MAX, 2, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        assert!(matches!(
            validate_vecadd_artifact_profile(&abi(Access::WriteOnly), &rank_two),
            Err(GeneratedVecAddProfileError::LaunchRank { actual: 2 })
        ));
    }

    #[test]
    fn length_and_grid_checks_cover_boundaries() {
        assert!(matches!(
            checked_vecadd_grid(0, 0, 0),
            Err(GeneratedVecAddPrepareError::EmptyInput)
        ));
        assert!(matches!(
            checked_vecadd_grid(1, 2, 1),
            Err(GeneratedVecAddPrepareError::LengthMismatch { a: 1, b: 2, c: 1 })
        ));
        assert_eq!(checked_vecadd_grid(1, 1, 1).unwrap(), 1);
        assert_eq!(checked_vecadd_grid(256, 256, 256).unwrap(), 1);
        assert_eq!(checked_vecadd_grid(257, 257, 257).unwrap(), 2);
        assert_eq!(
            checked_vecadd_grid(u32::MAX as usize, u32::MAX as usize, u32::MAX as usize).unwrap(),
            u32::MAX.div_ceil(256)
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn length_above_u32_is_rejected() {
        let length = (u32::MAX as usize) + 1;
        assert!(matches!(
            checked_vecadd_grid(length, length, length),
            Err(GeneratedVecAddPrepareError::LinearIndexDomainExceeded {
                length: actual,
                max: u32::MAX,
            }) if actual == length
        ));
    }
}
