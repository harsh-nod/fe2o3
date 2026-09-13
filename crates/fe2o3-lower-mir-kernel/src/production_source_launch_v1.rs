//! Source-only launch agreement shared by ranked projection and neutral import.
//!
//! Inputs are detached source facts, not artifact, allocation, launch or proof
//! authority. The backend must retain the authenticated source LaunchContract;
//! these checks do not replace that contract's own construction/validation.
//! Semantic MIR authenticates the root binding and required workgroup, not an
//! independent copy of the caller's max_grid. Rank/grid geometry comes from
//! the caller-retained validated source LaunchContract custody.

use std::{collections::BTreeSet, error::Error, fmt};

use dialect_kernel::DYNAMIC_EXTENT;
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticFunctionDeclV1, SemanticFunctionIdV1,
    SemanticFunctionIdentityV1, SemanticFunctionRoleV1, SemanticTargetArchitectureV1,
};

/// Detached fields of an already-validated source launch contract.
///
/// `None` represents every non-exact block-size mode. Construction retains
/// inputs only; agreement with semantic MIR is checked separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceLaunchInputV1 {
    rank: u8,
    exact_workgroup: Option<[u32; 3]>,
    max_grid: [u32; 3],
}

impl ProductionSourceLaunchInputV1 {
    /// Retains source fields without granting authentication or launch authority.
    pub const fn new(rank: u8, exact_workgroup: Option<[u32; 3]>, max_grid: [u32; 3]) -> Self {
        Self {
            rank,
            exact_workgroup,
            max_grid,
        }
    }
}

/// One borrowed logical root and its detached source launch input.
#[derive(Clone, Copy, Debug)]
pub struct ProductionSourceLaunchRootInputV1<'name> {
    logical_name: &'name str,
    kernel_binding: [u8; 32],
    launch: ProductionSourceLaunchInputV1,
}

impl<'name> ProductionSourceLaunchRootInputV1<'name> {
    /// Retains the exact source association; no identity is inferred from a name.
    pub const fn new(
        logical_name: &'name str,
        kernel_binding: [u8; 32],
        launch: ProductionSourceLaunchInputV1,
    ) -> Self {
        Self {
            logical_name,
            kernel_binding,
            launch,
        }
    }
}

/// Existing source-projection diagnostic categories, without backend types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceLaunchErrorV1 {
    /// A source association or launch layout is outside the admitted surface.
    Unsupported(&'static str),
    /// Required exact source information is unavailable.
    Incomplete(&'static str),
}

impl fmt::Display for ProductionSourceLaunchErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(detail) | Self::Incomplete(detail) => formatter.write_str(detail),
        }
    }
}
impl Error for ProductionSourceLaunchErrorV1 {}

/// Geometry derived from caller-retained validated source launch fields, after
/// checking the semantic root's required-workgroup agreement.
///
/// Dynamic global extents retain the existing zero sentinel. Full workgroups
/// describe source launch geometry, not any observed device invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceExecutionLayoutV1 {
    grid_identity: u64,
    global_extents: [u64; 3],
    workgroup_extents: [u64; 3],
    subgroup_size: u64,
    full_physical_workgroups: bool,
}

impl ProductionSourceExecutionLayoutV1 {
    /// Checks the same exact source-workgroup/rank/grid rules as ranked projection.
    ///
    /// The caller remains responsible for custody and validity of the originating
    /// source launch contract. Semantic MIR supplies the required workgroup;
    /// max_grid comes from that caller-retained contract, not an independently
    /// authenticated semantic-MIR grid limit. This function does not authenticate
    /// raw rank/grid inputs or replace LaunchContract validation.
    pub fn try_from_source(
        architecture: SemanticTargetArchitectureV1,
        function: &SemanticFunctionDeclV1,
        source_launch: ProductionSourceLaunchInputV1,
    ) -> Result<Self, ProductionSourceLaunchErrorV1> {
        let entry = function
            .kernel_entry()
            .ok_or(ProductionSourceLaunchErrorV1::Unsupported(
                "a semantic kernel root is missing its authenticated entry contract",
            ))?;
        let required = entry
            .source_contract()
            .launch()
            .and_then(|launch| launch.required())
            .ok_or(ProductionSourceLaunchErrorV1::Incomplete(
                "concurrency verification requires exact source workgroup dimensions",
            ))?
            .as_array();
        let source_workgroup = source_launch.exact_workgroup.ok_or(
            ProductionSourceLaunchErrorV1::Incomplete(
                "concurrency verification requires an exact authenticated LaunchContract workgroup",
            ),
        )?;
        if source_workgroup != required {
            return Err(ProductionSourceLaunchErrorV1::Unsupported(
                "authenticated LaunchContract workgroup disagrees with semantic source workgroup",
            ));
        }
        let source_rank = source_launch.rank;
        match source_rank {
            1 if required[1] == 1 && required[2] == 1 => {}
            2 if required[2] == 1 => {}
            3 => {}
            _ => {
                return Err(ProductionSourceLaunchErrorV1::Unsupported(
                    "authenticated launch rank disagrees with source workgroup axes",
                ));
            }
        }
        let workgroup_extents = required.map(u64::from);
        let max_grid = source_launch.max_grid.map(u64::from);
        let dynamic_grid_limits = match (architecture, source_rank) {
            (SemanticTargetArchitectureV1::AmdGpuGfx942, 1) => [u32::MAX, 1, 1],
            (SemanticTargetArchitectureV1::AmdGpuGfx942, 2) => [u32::MAX, u32::MAX, 1],
            (SemanticTargetArchitectureV1::AmdGpuGfx942, 3) => {
                [u32::MAX, u32::from(u16::MAX), u32::from(u16::MAX)]
            }
            _ => {
                return Err(ProductionSourceLaunchErrorV1::Unsupported(
                    "authenticated launch rank has no production grid-limit profile",
                ));
            }
        };
        let mut global_extents = [1_u64; 3];
        for axis in 0..usize::from(source_rank) {
            global_extents[axis] = checked_global_extent_v1(
                max_grid[axis],
                workgroup_extents[axis],
                u64::from(dynamic_grid_limits[axis]),
            )?;
        }
        let subgroup_size = match architecture {
            SemanticTargetArchitectureV1::AmdGpuGfx942 => 64,
        };
        let grid_identity = entry.kernel_binding_identity().as_bytes()[..8]
            .try_into()
            .map(u64::from_le_bytes)
            .map_err(|_| {
                ProductionSourceLaunchErrorV1::Unsupported(
                    "the authenticated kernel identity cannot form a grid identity",
                )
            })?;
        Ok(Self {
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
            full_physical_workgroups: true,
        })
    }

    /// Returns the existing truncated diagnostic grid identity, not authority.
    pub const fn grid_identity(self) -> u64 {
        self.grid_identity
    }
    /// Returns global extents, preserving the zero dynamic sentinel.
    pub const fn global_extents(self) -> [u64; 3] {
        self.global_extents
    }
    /// Returns the exact source workgroup dimensions.
    pub const fn workgroup_extents(self) -> [u64; 3] {
        self.workgroup_extents
    }
    /// Returns the source architecture's subgroup size.
    pub const fn subgroup_size(self) -> u64 {
        self.subgroup_size
    }
    /// Reports the source full-workgroup geometry contract, not device execution.
    pub const fn full_physical_workgroups(self) -> bool {
        self.full_physical_workgroups
    }
}

/// One source root/layout pair validated in complete semantic-roster order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceLaunchRootV1 {
    selected_root: SemanticFunctionIdV1,
    semantic_root_identity: SemanticFunctionIdentityV1,
    kernel_binding: [u8; 32],
    source_launch: ProductionSourceLaunchInputV1,
    layout: ProductionSourceExecutionLayoutV1,
}

impl ProductionSourceLaunchRootV1 {
    /// Returns the exact root selected from the admitted semantic module.
    pub const fn selected_root(self) -> SemanticFunctionIdV1 {
        self.selected_root
    }
    /// Returns that root's complete semantic identity.
    pub const fn semantic_root_identity(self) -> SemanticFunctionIdentityV1 {
        self.semantic_root_identity
    }
    /// Returns the full binding, not the truncated diagnostic grid identity.
    pub const fn kernel_binding(self) -> [u8; 32] {
        self.kernel_binding
    }
    /// Returns the exact source launch rank, including degenerate higher ranks.
    pub const fn source_rank(self) -> u8 {
        self.source_launch.rank
    }
    /// Returns the exact detached source fields whose geometry was checked.
    pub const fn source_launch(self) -> ProductionSourceLaunchInputV1 {
        self.source_launch
    }
    /// Returns immutable validated source geometry.
    pub const fn layout(self) -> ProductionSourceExecutionLayoutV1 {
        self.layout
    }
}

/// Move-only complete launch roster checked against one admitted semantic source.
///
/// This source-only agreement grants no artifact, launch, execution or proof
/// authority. Names are checked for uniqueness but are not equated with export
/// symbols. The caller retains the originating validated source launch contracts;
/// their rank/max_grid fields are inputs, not independently authenticated copies
/// in semantic MIR. Future cross-stage use must retain that custody and bind
/// the aggregate semantic source identity, not just a copied row's function ID.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1;
/// fn needs_clone<T: Clone>() {}
/// needs_clone::<ProductionSourceLaunchRosterV1>();
/// ```
#[derive(Debug, Eq, PartialEq)]
#[must_use = "dropping the roster abandons its source launch agreement"]
pub struct ProductionSourceLaunchRosterV1 {
    semantic_sha256: [u8; 32],
    roots: Box<[ProductionSourceLaunchRootV1]>,
}

impl ProductionSourceLaunchRosterV1 {
    /// Checks a nonempty ordered bijection before deriving any ranked operations.
    pub fn try_new(
        semantic: &AdmittedInertSemanticMirV1,
        inputs: &[ProductionSourceLaunchRootInputV1<'_>],
    ) -> Result<Self, ProductionSourceLaunchErrorV1> {
        let mut semantic_roots = Vec::with_capacity(semantic.roots().len());
        for root in semantic.roots() {
            let function = semantic.functions().get(root.index() as usize).ok_or(
                ProductionSourceLaunchErrorV1::Unsupported("an out-of-range semantic kernel root"),
            )?;
            if function.role() != SemanticFunctionRoleV1::KernelRoot {
                return Err(ProductionSourceLaunchErrorV1::Unsupported(
                    "a rooted semantic function without the KernelRoot role",
                ));
            }
            let entry =
                function
                    .kernel_entry()
                    .ok_or(ProductionSourceLaunchErrorV1::Unsupported(
                        "a semantic KernelRoot without an authenticated kernel entry",
                    ))?;
            semantic_roots.push((*entry.kernel_binding_identity().as_bytes(), *root));
        }
        let matched = match_production_source_launch_root_bindings_v1(inputs, &semantic_roots)?;
        let mut roots = Vec::with_capacity(matched.len());
        for (input, selected_root) in inputs.iter().zip(matched) {
            let function = &semantic.functions()[selected_root.index() as usize];
            let layout = ProductionSourceExecutionLayoutV1::try_from_source(
                semantic.target().architecture(),
                function,
                input.launch,
            )?;
            roots.push(ProductionSourceLaunchRootV1 {
                selected_root,
                semantic_root_identity: function.identity(),
                kernel_binding: input.kernel_binding,
                source_launch: input.launch,
                layout,
            });
        }
        Ok(Self {
            semantic_sha256: *semantic.semantic_sha256().as_bytes(),
            roots: roots.into_boxed_slice(),
        })
    }

    /// Returns the source identity whose ordered roots were checked.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    /// Borrows the complete immutable root/layout roster.
    pub fn roots(&self) -> &[ProductionSourceLaunchRootV1] {
        &self.roots
    }
    /// Source agreement alone never authorizes an artifact or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Checks only the ordered logical-name/full-binding association.
///
/// This reusable structural check does not authenticate the supplied numeric
/// semantic-root inventory or derive source geometry. Production uses the full
/// roster constructor above with admitted semantic MIR.
pub fn match_production_source_launch_root_bindings_v1(
    inputs: &[ProductionSourceLaunchRootInputV1<'_>],
    semantic_roots: &[([u8; 32], SemanticFunctionIdV1)],
) -> Result<Vec<SemanticFunctionIdV1>, ProductionSourceLaunchErrorV1> {
    if inputs.is_empty() || inputs.len() != semantic_roots.len() {
        return Err(ProductionSourceLaunchErrorV1::Unsupported(
            "an incomplete typed/semantic ranked root roster",
        ));
    }
    let mut logical_names = BTreeSet::new();
    if inputs
        .iter()
        .any(|input| !logical_names.insert(input.logical_name))
    {
        return Err(ProductionSourceLaunchErrorV1::Unsupported(
            "duplicate typed logical roots in the ranked roster",
        ));
    }
    let mut semantic_bindings = BTreeSet::new();
    for (binding, _) in semantic_roots {
        if !semantic_bindings.insert(*binding) {
            return Err(ProductionSourceLaunchErrorV1::Unsupported(
                "duplicate semantic kernel bindings in the ranked roster",
            ));
        }
    }
    let mut typed_bindings = BTreeSet::new();
    for input in inputs {
        if !typed_bindings.insert(input.kernel_binding) {
            return Err(ProductionSourceLaunchErrorV1::Unsupported(
                "duplicate typed kernel bindings in the ranked roster",
            ));
        }
    }
    inputs
        .iter()
        .zip(semantic_roots)
        .map(|(input, (binding, root))| {
            if input.kernel_binding != *binding {
                return Err(ProductionSourceLaunchErrorV1::Unsupported(
                    "a reordered or substituted typed/semantic kernel binding in the ranked roster",
                ));
            }
            Ok(*root)
        })
        .collect()
}

fn checked_global_extent_v1(
    max_grid: u64,
    required_workgroup: u64,
    dynamic_grid_limit: u64,
) -> Result<u64, ProductionSourceLaunchErrorV1> {
    if max_grid == dynamic_grid_limit {
        return Ok(DYNAMIC_EXTENT);
    }
    max_grid
        .checked_mul(required_workgroup)
        .ok_or(ProductionSourceLaunchErrorV1::Unsupported(
            "authenticated finite grid extent overflows u64",
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extent_product_checks_overflow_after_dynamic_sentinel() {
        assert_eq!(
            checked_global_extent_v1(3, 64, u64::from(u32::MAX)),
            Ok(192)
        );
        assert_eq!(
            checked_global_extent_v1(u64::from(u32::MAX), u64::MAX, u64::from(u32::MAX)),
            Ok(DYNAMIC_EXTENT)
        );
        assert_eq!(
            checked_global_extent_v1(u64::from(u16::MAX), 16, u64::from(u16::MAX)),
            Ok(DYNAMIC_EXTENT)
        );
        assert_eq!(
            checked_global_extent_v1(u64::from(u16::MAX) - 1, 16, u64::from(u16::MAX)),
            Ok(1_048_544)
        );
        assert_eq!(
            checked_global_extent_v1(u64::MAX, 2, u64::from(u32::MAX)),
            Err(ProductionSourceLaunchErrorV1::Unsupported(
                "authenticated finite grid extent overflows u64"
            ))
        );
    }
}
