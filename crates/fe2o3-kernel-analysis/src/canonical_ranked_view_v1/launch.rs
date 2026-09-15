use super::*;
use fe2o3_kernel_ir::{LaunchExtent, Module, WorkgroupSize};

/// Facts from every canonical entry naming this function, never a target default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CanonicalLaunch {
    global: [u64; 3],
    workgroup: Option<WorkgroupSize>,
}

impl CanonicalLaunch {
    pub(super) fn for_function(module: &Module, function: &FunctionId) -> Option<Self> {
        let mut entries = module
            .kernels
            .iter()
            .filter(|kernel| &kernel.entry == function);
        let facts = |kernel: &fe2o3_kernel_ir::Kernel| {
            let mut global = [1; 3];
            for (axis, extent) in kernel.domain.extents().enumerate() {
                global[axis] = match extent {
                    LaunchExtent::Static(value) => u64::from(value),
                    LaunchExtent::Dynamic => 0,
                };
            }
            Self {
                global,
                workgroup: kernel.workgroup_size,
            }
        };
        let first = facts(entries.next()?);
        entries
            .all(|kernel| facts(kernel) == first)
            .then_some(first)
    }

    fn workgroup(self, axis: usize) -> Option<u64> {
        let size = self.workgroup?;
        let value = u64::from([size.x, size.y, size.z][axis]);
        (value != 0).then_some(value)
    }
}

impl Planner {
    pub(super) fn project_intrinsic(
        &mut self,
        current: usize,
        block: BlockId,
        operation: usize,
        intrinsic: IntrinsicKind,
    ) -> Result<Fact, CanonicalRankedViewErrorV1> {
        let missing = || CanonicalRankedViewErrorV1::UnsupportedIntrinsic {
            block,
            operation,
            intrinsic,
        };
        let axis = match intrinsic.axis() {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        };
        let extent = self.launch.map_or(0, |launch| launch.global[axis]);
        let kind = match intrinsic {
            IntrinsicKind::InvocationIndex { kind, .. } => kind,
            IntrinsicKind::LaunchExtent { .. } => {
                return if extent != 0 {
                    Ok(self.constant_index(current, extent))
                } else {
                    Err(missing())
                };
            }
        };
        if kind == IndexKind::Global {
            return Ok(Fact::Index {
                node: self.global_index(current, axis, extent),
                constant: None,
            });
        }
        let size = self
            .launch
            .and_then(|launch| launch.workgroup(axis))
            .ok_or_else(missing)?;
        match kind {
            IndexKind::WorkgroupSize => Ok(self.constant_index(current, size)),
            IndexKind::WorkgroupCount if extent != 0 => {
                Ok(self.constant_index(current, extent.div_ceil(size)))
            }
            IndexKind::Local | IndexKind::Workgroup => {
                // Euclidean decomposition of a global coordinate. The divisor
                // is authenticated, positive and static; neither operation can
                // overflow, including at u64::MAX or in a partial workgroup.
                let lhs = self.global_index(current, axis, extent);
                let Fact::Index { node: rhs, .. } = self.constant_index(current, size) else {
                    unreachable!()
                };
                let result = self.node();
                self.emit(
                    current,
                    PlannedOp::IndexBinary {
                        result,
                        kind: if kind == IndexKind::Local {
                            IndexBinaryKindAttr::Remainder
                        } else {
                            IndexBinaryKindAttr::Divide
                        },
                        lhs,
                        rhs,
                    },
                );
                Ok(Fact::Index {
                    node: result,
                    constant: None,
                })
            }
            _ => Err(missing()),
        }
    }

    fn constant_index(&mut self, current: usize, bits: u64) -> Fact {
        let node = self.node();
        self.emit(current, PlannedOp::Index { result: node, bits });
        Fact::Index {
            node,
            constant: Some(bits),
        }
    }

    fn global_index(&mut self, current: usize, axis: usize, extent: u64) -> Node {
        let result = self.node();
        self.emit(
            current,
            PlannedOp::Invocation {
                result,
                axis: axis as u32,
                extent,
            },
        );
        result
    }
}
