//! Checked shape and launch planning for tiled GEMM V1.

use core::fmt;
use core::mem::size_of;

use fe2o3_amd_target::AmdTargetId;

/// Exact gfx942 target admitted by this contract.
pub const TARGET_V1: &str = "gfx942:xnack-";
/// Output tile row count.
pub const TILE_M_V1: u32 = 16;
/// Output tile column count.
pub const TILE_N_V1: u32 = 16;
/// Reduction tile width.
pub const TILE_K_V1: u32 = 16;
/// One complete wave executes one output tile.
pub const WAVE_LANES_V1: u32 = 64;

/// Unforgeable admission token for the exact V1 target declaration.
///
/// This token proves only that a parsed target declaration equals
/// `gfx942:xnack-`. It does not attest an observed device or executable bytes.
///
/// ```compile_fail
/// use fe2o3_amd_target::AmdTargetId;
/// use fe2o3_tiled_gemm_v1::AdmittedTargetV1;
/// let _forged = AdmittedTargetV1 {
///     target: AmdTargetId::parse("gfx942:xnack+").unwrap(),
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedTargetV1 {
    target: AmdTargetId,
}

impl AdmittedTargetV1 {
    /// Returns the exact parsed target declaration retained by this token.
    pub const fn target_id(self) -> AmdTargetId {
        self.target
    }
}

/// A parsed target declaration did not exactly match the V1 target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetAdmissionErrorV1 {
    candidate: AmdTargetId,
}

impl TargetAdmissionErrorV1 {
    /// Returns the rejected parsed target declaration.
    pub const fn candidate(self) -> AmdTargetId {
        self.candidate
    }

    /// Returns the exact target declaration required by V1.
    pub fn required(&self) -> AmdTargetId {
        exact_target_v1()
    }
}

impl fmt::Display for TargetAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "tiled GEMM V1 requires exact target `{TARGET_V1}`, found `{}`",
            self.candidate
        )
    }
}

impl std::error::Error for TargetAdmissionErrorV1 {}

/// Returns the repository's canonical parsed V1 target declaration.
pub fn exact_target_v1() -> AmdTargetId {
    AmdTargetId::parse(TARGET_V1).expect("fixed tiled GEMM V1 target must remain canonical")
}

/// Admits only the exact canonical `gfx942:xnack-` target declaration.
///
/// Generic `gfx942`, XNACK-enabled, SRAM-ECC-qualified, and other processor
/// declarations fail closed even when they may describe related hardware.
pub fn admit_target_v1(candidate: AmdTargetId) -> Result<AdmittedTargetV1, TargetAdmissionErrorV1> {
    if candidate != exact_target_v1() {
        return Err(TargetAdmissionErrorV1 { candidate });
    }
    Ok(AdmittedTargetV1 { target: candidate })
}

/// A checked row-major GEMM shape and the extents accessed by its host action.
///
/// Fields are private so every value passes through [`Self::checked`]. Empty
/// output requires no operand or output storage, so all three accessed extents
/// are zero even when an unused mathematical operand would have a large shape.
///
/// ```compile_fail
/// use fe2o3_tiled_gemm_v1::ShapeV1;
/// let _forged = ShapeV1 {
///     m: 16,
///     n: 16,
///     k: 16,
///     a_elements: 0,
///     b_elements: 0,
///     c_elements: 0,
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShapeV1 {
    m: u32,
    n: u32,
    k: u32,
    a_elements: usize,
    b_elements: usize,
    c_elements: usize,
}

/// Shape construction failed before any allocation or launch decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeErrorV1 {
    /// A byte extent does not fit the contract's 64-bit accounting domain.
    ByteCountOverflow(&'static str),
    /// An element extent does not fit the host `usize` domain.
    HostLengthOverflow(&'static str),
}

impl fmt::Display for ShapeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteCountOverflow(buffer) => {
                write!(formatter, "{buffer} byte count overflows u64")
            }
            Self::HostLengthOverflow(buffer) => {
                write!(formatter, "{buffer} element count does not fit usize")
            }
        }
    }
}

impl std::error::Error for ShapeErrorV1 {}

impl ShapeV1 {
    /// Checks all row-major element and byte extents that the action accesses.
    ///
    /// `M=0` or `N=0` returns an empty-output shape before unused input extent
    /// arithmetic. Consequently every `u32` dimension tuple with empty output
    /// is representable and requires no operand buffers.
    pub fn checked(m: u32, n: u32, k: u32) -> Result<Self, ShapeErrorV1> {
        fn extent(
            buffer: &'static str,
            rows: u32,
            columns: u32,
            element_bytes: usize,
        ) -> Result<usize, ShapeErrorV1> {
            let elements = u64::from(rows) * u64::from(columns);
            elements
                .checked_mul(element_bytes as u64)
                .ok_or(ShapeErrorV1::ByteCountOverflow(buffer))?;
            usize::try_from(elements).map_err(|_| ShapeErrorV1::HostLengthOverflow(buffer))
        }

        if m == 0 || n == 0 {
            return Ok(Self {
                m,
                n,
                k,
                a_elements: 0,
                b_elements: 0,
                c_elements: 0,
            });
        }

        Ok(Self {
            m,
            n,
            k,
            a_elements: extent("A", m, k, size_of::<u16>())?,
            b_elements: extent("B", k, n, size_of::<u16>())?,
            c_elements: extent("C", m, n, size_of::<f32>())?,
        })
    }

    /// Returns `[M, N, K]`.
    pub const fn dimensions(self) -> [u32; 3] {
        [self.m, self.n, self.k]
    }

    /// Returns the output row count.
    pub const fn m(self) -> u32 {
        self.m
    }

    /// Returns the output column count.
    pub const fn n(self) -> u32 {
        self.n
    }

    /// Returns the reduction extent.
    pub const fn k(self) -> u32 {
        self.k
    }

    /// Returns whether the shape has no output elements.
    pub const fn is_empty_output(self) -> bool {
        self.m == 0 || self.n == 0
    }

    /// Returns the number of BF16 `A` elements accessed by the host action.
    pub const fn a_elements(self) -> usize {
        self.a_elements
    }

    /// Returns the number of BF16 `B` elements accessed by the host action.
    pub const fn b_elements(self) -> usize {
        self.b_elements
    }

    /// Returns the number of FP32 `C` elements produced by the host action.
    pub const fn c_elements(self) -> usize {
        self.c_elements
    }

    /// Returns the row-major index in accessed `A[M,K]` storage, if valid.
    pub fn a_index(self, row: u32, depth: u32) -> Option<usize> {
        if self.is_empty_output() || row >= self.m || depth >= self.k {
            return None;
        }
        Some((row as usize) * (self.k as usize) + depth as usize)
    }

    /// Returns the row-major index in accessed `B[K,N]` storage, if valid.
    pub fn b_index(self, depth: u32, column: u32) -> Option<usize> {
        if self.is_empty_output() || depth >= self.k || column >= self.n {
            return None;
        }
        Some((depth as usize) * (self.n as usize) + column as usize)
    }

    /// Returns the row-major index in produced `C[M,N]` storage, if valid.
    pub fn c_index(self, row: u32, column: u32) -> Option<usize> {
        if row >= self.m || column >= self.n {
            return None;
        }
        Some((row as usize) * (self.n as usize) + column as usize)
    }
}

/// Launch admission failed closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanErrorV1 {
    /// A nonempty output has an `M` tail.
    MNotMultipleOf16(u32),
    /// A nonempty output has an `N` tail.
    NNotMultipleOf16(u32),
    /// A positive reduction extent has a `K` tail.
    KNotMultipleOf16(u32),
    /// The HSA x-grid dimension cannot be represented as `u32`.
    GridXOverflow,
}

impl fmt::Display for PlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MNotMultipleOf16(value) => write!(formatter, "M={value} has a 16-row tail"),
            Self::NNotMultipleOf16(value) => write!(formatter, "N={value} has a 16-column tail"),
            Self::KNotMultipleOf16(value) => write!(formatter, "K={value} has a 16-value tail"),
            Self::GridXOverflow => formatter.write_str("x-grid dimension overflows u32"),
        }
    }
}

impl std::error::Error for PlanErrorV1 {}

/// Origin of one `16x16` output tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileOriginV1 {
    /// First output row owned by the tile.
    pub row: u32,
    /// First output column owned by the tile.
    pub column: u32,
}

/// Unforgeable checked HSA geometry for one wave64 per output tile.
///
/// ```compile_fail
/// use fe2o3_tiled_gemm_v1::LaunchGeometryV1;
/// let _forged = LaunchGeometryV1 {
///     target: panic!(),
///     grid: [64, 1, 1],
///     workgroup: [64, 1, 1],
///     tile_rows: 1,
///     tile_columns: 1,
///     reduction_tiles: 1,
///     total_work_items: 64,
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchGeometryV1 {
    target: AdmittedTargetV1,
    grid: [u32; 3],
    workgroup: [u32; 3],
    tile_rows: u32,
    tile_columns: u32,
    reduction_tiles: u32,
    total_work_items: u64,
}

impl LaunchGeometryV1 {
    /// Returns the exact target admission retained by this geometry.
    pub const fn target(self) -> AdmittedTargetV1 {
        self.target
    }

    /// Returns global work-item dimensions.
    pub const fn grid(self) -> [u32; 3] {
        self.grid
    }

    /// Returns workgroup dimensions, fixed to one wave64 by V1.
    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }

    /// Returns the number of output-tile rows.
    pub const fn tile_rows(self) -> u32 {
        self.tile_rows
    }

    /// Returns the number of output-tile columns.
    pub const fn tile_columns(self) -> u32 {
        self.tile_columns
    }

    /// Returns the number of `K=16` reduction steps per output tile.
    pub const fn reduction_tiles(self) -> u32 {
        self.reduction_tiles
    }

    /// Returns the total work-items in the checked grid.
    pub const fn total_work_items(self) -> u64 {
        self.total_work_items
    }

    /// Maps a workgroup coordinate to its output-tile origin.
    pub fn tile_origin(self, group_x: u32, group_y: u32) -> Option<TileOriginV1> {
        if group_x >= self.tile_columns || group_y >= self.tile_rows {
            return None;
        }
        Some(TileOriginV1 {
            row: group_y.checked_mul(TILE_M_V1)?,
            column: group_x.checked_mul(TILE_N_V1)?,
        })
    }
}

/// Checked host action for a shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchDecisionV1 {
    /// `M=0` or `N=0`: there are no output elements and no work is launched.
    NoDispatchEmptyOutput,
    /// `K=0` with nonempty tiled output: host writes FP32 positive zero.
    HostFillPositiveZero {
        /// Number of output values to fill.
        output_elements: usize,
    },
    /// A nonempty, positive, exact-tile kernel launch.
    Dispatch(LaunchGeometryV1),
}

/// Produces a checked V1 host action.
///
/// Empty output takes precedence over tail checks because no input or output
/// memory is accessed. For nonempty output, `M` and `N` are checked before the
/// explicit `K=0` host-fill case; all positive dimensions dispatched to the
/// GPU are therefore exact multiples of 16. Dispatch geometry retains
/// `target`, preserving the exact typed target admission through planning.
pub fn plan_v1(target: AdmittedTargetV1, shape: ShapeV1) -> Result<LaunchDecisionV1, PlanErrorV1> {
    if shape.m == 0 || shape.n == 0 {
        return Ok(LaunchDecisionV1::NoDispatchEmptyOutput);
    }
    if !shape.m.is_multiple_of(TILE_M_V1) {
        return Err(PlanErrorV1::MNotMultipleOf16(shape.m));
    }
    if !shape.n.is_multiple_of(TILE_N_V1) {
        return Err(PlanErrorV1::NNotMultipleOf16(shape.n));
    }
    if shape.k == 0 {
        return Ok(LaunchDecisionV1::HostFillPositiveZero {
            output_elements: shape.c_elements,
        });
    }
    if !shape.k.is_multiple_of(TILE_K_V1) {
        return Err(PlanErrorV1::KNotMultipleOf16(shape.k));
    }

    let tile_rows = shape.m / TILE_M_V1;
    let tile_columns = shape.n / TILE_N_V1;
    let reduction_tiles = shape.k / TILE_K_V1;
    let grid_x = tile_columns
        .checked_mul(WAVE_LANES_V1)
        .ok_or(PlanErrorV1::GridXOverflow)?;
    let total_work_items = u64::from(grid_x) * u64::from(tile_rows);

    Ok(LaunchDecisionV1::Dispatch(LaunchGeometryV1 {
        target,
        grid: [grid_x, tile_rows, 1],
        workgroup: [WAVE_LANES_V1, 1, 1],
        tile_rows,
        tile_columns,
        reduction_tiles,
        total_work_items,
    }))
}

/// Expected category for one edge-case contract row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedDecisionV1 {
    /// Empty-output no-dispatch.
    NoDispatch,
    /// Nonempty zero-reduction host fill.
    HostFill,
    /// Exact-tile GPU dispatch.
    Dispatch,
    /// Tail or geometry rejection.
    Reject(PlanErrorV1),
}

/// One required shape/edge-case row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EdgeCaseV1 {
    /// Stable case name.
    pub name: &'static str,
    /// `M` extent.
    pub m: u32,
    /// `N` extent.
    pub n: u32,
    /// `K` extent.
    pub k: u32,
    /// Required planning outcome.
    pub expected: ExpectedDecisionV1,
}

/// Minimum host acceptance and rejection matrix for V1.
pub const EDGE_CASES_V1: &[EdgeCaseV1] = &[
    EdgeCaseV1 {
        name: "all-zero",
        m: 0,
        n: 0,
        k: 0,
        expected: ExpectedDecisionV1::NoDispatch,
    },
    EdgeCaseV1 {
        name: "zero-m-with-k-tail",
        m: 0,
        n: 17,
        k: 3,
        expected: ExpectedDecisionV1::NoDispatch,
    },
    EdgeCaseV1 {
        name: "zero-n",
        m: 32,
        n: 0,
        k: 16,
        expected: ExpectedDecisionV1::NoDispatch,
    },
    EdgeCaseV1 {
        name: "zero-k",
        m: 16,
        n: 32,
        k: 0,
        expected: ExpectedDecisionV1::HostFill,
    },
    EdgeCaseV1 {
        name: "single-tile",
        m: 16,
        n: 16,
        k: 16,
        expected: ExpectedDecisionV1::Dispatch,
    },
    EdgeCaseV1 {
        name: "rectangular-multi-tile",
        m: 32,
        n: 48,
        k: 32,
        expected: ExpectedDecisionV1::Dispatch,
    },
    EdgeCaseV1 {
        name: "three-reduction-tiles",
        m: 16,
        n: 16,
        k: 48,
        expected: ExpectedDecisionV1::Dispatch,
    },
    EdgeCaseV1 {
        name: "m-tail",
        m: 17,
        n: 16,
        k: 16,
        expected: ExpectedDecisionV1::Reject(PlanErrorV1::MNotMultipleOf16(17)),
    },
    EdgeCaseV1 {
        name: "n-tail",
        m: 16,
        n: 31,
        k: 16,
        expected: ExpectedDecisionV1::Reject(PlanErrorV1::NNotMultipleOf16(31)),
    },
    EdgeCaseV1 {
        name: "k-tail-below-one-tile",
        m: 16,
        n: 16,
        k: 15,
        expected: ExpectedDecisionV1::Reject(PlanErrorV1::KNotMultipleOf16(15)),
    },
    EdgeCaseV1 {
        name: "k-tail-after-one-tile",
        m: 16,
        n: 16,
        k: 17,
        expected: ExpectedDecisionV1::Reject(PlanErrorV1::KNotMultipleOf16(17)),
    },
];
