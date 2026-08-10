use core::fmt;
use fe2o3_amd_target::{AmdTargetId, FeatureState};
use fe2o3_core::{
    BorrowedDeviceOperation, DeviceBuffer, DeviceBufferIdentity, DevicePtr, Error as CoreError,
    Stream, StreamIdentity,
};
use fe2o3_device::RowMajorXor4;

pub const GFX942_XOR4_BF16_TILE_ROWS_V1: usize = 16;
pub const GFX942_XOR4_BF16_TILE_COLUMNS_V1: usize = 16;
pub const GFX942_XOR4_BF16_TILE_ELEMENTS_V1: usize =
    GFX942_XOR4_BF16_TILE_ROWS_V1 * GFX942_XOR4_BF16_TILE_COLUMNS_V1;
pub const GFX942_XOR4_BF16_TILE_WAVE_LANES_V1: usize = 64;

#[derive(Debug)]
pub struct Gfx942Xor4Bf16TileAllocationV1 {
    storage: DeviceBuffer<u16>,
    creation_stream: StreamIdentity,
    target: AmdTargetId,
}

pub struct Gfx942Xor4Bf16TileLeaseV1<'allocation, 'stream> {
    storage: &'allocation mut DeviceBuffer<u16>,
    stream: &'stream Stream,
    allocation_identity: DeviceBufferIdentity,
    stream_identity: StreamIdentity,
}

#[derive(Debug)]
pub enum Gfx942TileInteropErrorV1 {
    Runtime(CoreError),
    UnsupportedTarget(AmdTargetId),
    StreamSubstitution {
        expected: StreamIdentity,
        actual: StreamIdentity,
    },
    ContextSubstitution,
}

impl fmt::Display for Gfx942TileInteropErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::UnsupportedTarget(target) => write!(
                formatter,
                "gfx942 XOR4 tile interop requires gfx942:xnack-, observed {target}"
            ),
            Self::StreamSubstitution { expected, actual } => write!(
                formatter,
                "tile stream identity substitution: expected {expected:?}, got {actual:?}"
            ),
            Self::ContextSubstitution => {
                formatter.write_str("tile allocation and stream contexts differ")
            }
        }
    }
}

impl std::error::Error for Gfx942TileInteropErrorV1 {}

impl From<CoreError> for Gfx942TileInteropErrorV1 {
    fn from(error: CoreError) -> Self {
        Self::Runtime(error)
    }
}

impl Gfx942Xor4Bf16TileAllocationV1 {
    pub fn zeroed(stream: &Stream) -> Result<Self, Gfx942TileInteropErrorV1> {
        let target = require_gfx942_xnack_off(stream)?;
        let storage = DeviceBuffer::zeroed(stream, GFX942_XOR4_BF16_TILE_ELEMENTS_V1)?;
        Ok(Self {
            storage,
            creation_stream: stream.identity(),
            target,
        })
    }

    pub fn from_logical_bits(
        stream: &Stream,
        logical: &[[u16; GFX942_XOR4_BF16_TILE_COLUMNS_V1]; GFX942_XOR4_BF16_TILE_ROWS_V1],
    ) -> Result<Self, Gfx942TileInteropErrorV1> {
        let target = require_gfx942_xnack_off(stream)?;
        let mut physical = [0_u16; GFX942_XOR4_BF16_TILE_ELEMENTS_V1];
        for (row, values) in logical.iter().enumerate() {
            for (column, value) in values.iter().copied().enumerate() {
                let physical_index = RowMajorXor4::physical_index(row, column)
                    .expect("fixed tile coordinates are in bounds");
                physical[physical_index] = value;
            }
        }
        let storage = DeviceBuffer::from_host(stream, &physical)?;
        Ok(Self {
            storage,
            creation_stream: stream.identity(),
            target,
        })
    }

    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    pub fn allocation_identity(&self) -> DeviceBufferIdentity {
        self.storage.allocation_identity()
    }

    pub const fn stream_identity(&self) -> StreamIdentity {
        self.creation_stream
    }

    pub const fn len(&self) -> usize {
        GFX942_XOR4_BF16_TILE_ELEMENTS_V1
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    pub fn lease<'allocation, 'stream>(
        &'allocation mut self,
        stream: &'stream Stream,
    ) -> Result<Gfx942Xor4Bf16TileLeaseV1<'allocation, 'stream>, Gfx942TileInteropErrorV1> {
        self.validate_stream(stream)?;
        let allocation_identity = self.storage.allocation_identity();
        Ok(Gfx942Xor4Bf16TileLeaseV1 {
            storage: &mut self.storage,
            stream,
            allocation_identity,
            stream_identity: self.creation_stream,
        })
    }

    pub fn to_logical_bits(
        &self,
        stream: &Stream,
    ) -> Result<
        [[u16; GFX942_XOR4_BF16_TILE_COLUMNS_V1]; GFX942_XOR4_BF16_TILE_ROWS_V1],
        Gfx942TileInteropErrorV1,
    > {
        self.validate_stream(stream)?;
        let physical = self.storage.to_host_vec(stream)?;
        let mut logical =
            [[0_u16; GFX942_XOR4_BF16_TILE_COLUMNS_V1]; GFX942_XOR4_BF16_TILE_ROWS_V1];
        for (row, values) in logical.iter_mut().enumerate() {
            for (column, value) in values.iter_mut().enumerate() {
                let physical_index = RowMajorXor4::physical_index(row, column)
                    .expect("fixed tile coordinates are in bounds");
                *value = physical[physical_index];
            }
        }
        Ok(logical)
    }

    fn validate_stream(&self, stream: &Stream) -> Result<(), Gfx942TileInteropErrorV1> {
        if self.storage.context().identity() != stream.context().identity() {
            return Err(Gfx942TileInteropErrorV1::ContextSubstitution);
        }
        if self.creation_stream != stream.identity() {
            return Err(Gfx942TileInteropErrorV1::StreamSubstitution {
                expected: self.creation_stream,
                actual: stream.identity(),
            });
        }
        Ok(())
    }
}

impl Gfx942Xor4Bf16TileLeaseV1<'_, '_> {
    pub const fn len(&self) -> usize {
        GFX942_XOR4_BF16_TILE_ELEMENTS_V1
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    pub const fn allocation_identity(&self) -> DeviceBufferIdentity {
        self.allocation_identity
    }

    pub const fn stream_identity(&self) -> StreamIdentity {
        self.stream_identity
    }

    pub fn physical_index(&self, row: usize, column: usize) -> Option<usize> {
        RowMajorXor4::physical_index(row, column)
    }

    pub fn lane_fragment_indices(&self, lane: usize) -> Option<[usize; 4]> {
        RowMajorXor4::lane_fragment_indices(lane)
    }

    /// Runs one asynchronous tile operation while retaining the exact
    /// allocation and stream until completion is established.
    ///
    /// # Safety
    ///
    /// `enqueue` must submit only to this lease's stream and may access only
    /// the exact 256-element global BF16-storage allocation represented by the
    /// supplied pointer. The submitted operation must honor the XOR4 layout,
    /// bounds, aliasing, and initialization contracts.
    pub unsafe fn run_scoped_unchecked<O>(
        &mut self,
        enqueue: impl FnOnce(DevicePtr<u16>, usize) -> Result<(), CoreError>,
        during: impl for<'operation> FnOnce(&'operation BorrowedDeviceOperation<'_, '_>) -> O,
    ) -> Result<O, Gfx942TileInteropErrorV1> {
        let pointer = self.storage.as_device_ptr();
        let stream = self.stream;
        let storage = &mut *self.storage;
        // SAFETY: the lease retains the exact stream and allocation. The
        // caller supplies the operation-specific device safety contract.
        unsafe {
            BorrowedDeviceOperation::run_scoped_unchecked(
                stream,
                storage,
                |_| enqueue(pointer, GFX942_XOR4_BF16_TILE_ELEMENTS_V1),
                during,
            )
        }
        .map_err(Into::into)
    }
}

fn require_gfx942_xnack_off(stream: &Stream) -> Result<AmdTargetId, Gfx942TileInteropErrorV1> {
    let target = stream.context().observe_target()?.target_id();
    if target.processor() != "gfx942" || target.xnack() != Some(FeatureState::Disabled) {
        return Err(Gfx942TileInteropErrorV1::UnsupportedTarget(target));
    }
    Ok(target)
}
