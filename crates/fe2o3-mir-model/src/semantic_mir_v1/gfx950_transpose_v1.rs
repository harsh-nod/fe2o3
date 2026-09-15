//! Source-shaped, epoch-bound gfx950 transpose terminals. Not legacy Current.
use super::*;

/// Actual source types, including borrowed pointees and both publish results.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticGfx950TransposeOperationV1 {
    /// Issues an uninitialized tile from the actual shared partition receiver.
    Issue {
        /// Shared source receiver type.
        partition_reference: SemanticTypeIdV1,
        /// Owned partition type reached through that reference.
        partition: SemanticTypeIdV1,
        /// Uninitialized result tile type.
        tile: SemanticTypeIdV1,
    },
    /// Consumes an uninitialized tile and stages from a Global-backed A view.
    Stage {
        /// Consumed uninitialized tile type.
        input_tile: SemanticTypeIdV1,
        /// Staged result tile type.
        output_tile: SemanticTypeIdV1,
        /// Shared source A-view reference type.
        view_reference: SemanticTypeIdV1,
        /// Exact A-view pointee type.
        view: SemanticTypeIdV1,
        /// Both source base indices' unsigned pointer-sized integer type.
        index: SemanticTypeIdV1,
        /// The view's retained Global reference field type.
        global_reference: SemanticTypeIdV1,
        /// Exact retained Global pointee type.
        global: SemanticTypeIdV1,
    },
    /// Consumes both owners and returns their exact next-epoch tuple.
    Publish {
        /// Consumed staged tile type.
        input_tile: SemanticTypeIdV1,
        /// Consumed current-epoch workgroup type.
        input_workgroup: SemanticTypeIdV1,
        /// Tuple result type, workgroup first and tile second.
        transition: SemanticTypeIdV1,
        /// Next-epoch workgroup type.
        output_workgroup: SemanticTypeIdV1,
        /// Next-epoch published tile type.
        output_tile: SemanticTypeIdV1,
    },
    /// Consumes the published tile using its exact next-epoch lane borrow.
    Read {
        /// Consumed published tile type.
        tile: SemanticTypeIdV1,
        /// Shared source lane reference type.
        lane_reference: SemanticTypeIdV1,
        /// Exact lane pointee type.
        lane: SemanticTypeIdV1,
        /// Borrow-bound B-fragment result type.
        fragment: SemanticTypeIdV1,
        /// The fragment's eight-word register array type.
        registers: SemanticTypeIdV1,
        /// Exact unsigned 32-bit register element type.
        word: SemanticTypeIdV1,
    },
}

/// Static source contract. The enclosing execution contract retains root, E,
/// epochs and source identity. No field discharges a dynamic safety obligation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGfx950TransposeContractV1 {
    operation: SemanticGfx950TransposeOperationV1,
    format: SemanticGfx950LdsTransposeFormatV1,
    subgroup_brand: SemanticTypeIdentityV1,
    next_subgroup_brand: Option<SemanticTypeIdentityV1>,
}

impl SemanticGfx950TransposeContractV1 {
    /// Checks the fixed type roster and full nominal subgroup identities.
    pub fn new(
        operation: SemanticGfx950TransposeOperationV1,
        format: SemanticGfx950LdsTransposeFormatV1,
        subgroup_brand: SemanticTypeIdentityV1,
        next_subgroup_brand: Option<SemanticTypeIdentityV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        let result = Self {
            operation,
            format,
            subgroup_brand,
            next_subgroup_brand,
        };
        if !result.is_well_formed() {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(result)
    }

    /// Returns the source-shaped operation and retained type edges.
    pub const fn operation(self) -> SemanticGfx950TransposeOperationV1 {
        self.operation
    }
    /// Returns the exact B4 or B8 format.
    pub const fn format(self) -> SemanticGfx950LdsTransposeFormatV1 {
        self.format
    }
    /// Returns the full input subgroup identity, including epoch.
    pub const fn subgroup_brand(self) -> SemanticTypeIdentityV1 {
        self.subgroup_brand
    }
    /// Returns the publish result's subgroup identity, absent otherwise.
    pub const fn next_subgroup_brand(self) -> Option<SemanticTypeIdentityV1> {
        self.next_subgroup_brand
    }
    /// Whether the operation consumes and advances the workgroup epoch.
    pub const fn advances_epoch(self) -> bool {
        matches!(
            self.operation,
            SemanticGfx950TransposeOperationV1::Publish { .. }
        )
    }

    pub(super) fn is_well_formed(self) -> bool {
        let ids = self.type_references();
        ids.iter().enumerate().all(|(i, id)| {
            id.is_none_or(|id| {
                u64::from(id.index()) < HARD_MAX_TYPES_V1 && !ids[..i].contains(&Some(id))
            })
        }) && self.subgroup_brand.as_bytes() != &[0; 32]
            && self.next_subgroup_brand.is_some() == self.advances_epoch()
            && self
                .next_subgroup_brand
                .is_none_or(|next| next.as_bytes() != &[0; 32] && next != self.subgroup_brand)
    }

    pub(super) fn signature_matches(
        self,
        signature: SemanticExecutionCapabilitySignatureV1,
    ) -> bool {
        use SemanticGfx950TransposeOperationV1 as T;
        match self.operation {
            T::Issue {
                partition_reference,
                tile,
                ..
            } => signature.matches(&[partition_reference], tile),
            T::Stage {
                input_tile,
                output_tile,
                view_reference,
                index,
                ..
            } => signature.matches(&[input_tile, view_reference, index, index], output_tile),
            T::Publish {
                input_tile,
                input_workgroup,
                transition,
                ..
            } => signature.matches(&[input_tile, input_workgroup], transition),
            T::Read {
                tile,
                lane_reference,
                fragment,
                ..
            } => signature.matches(&[tile, lane_reference], fragment),
        }
    }

    /// An address-transparent edge only. The SSA adapter still proves the actual
    /// reference definition, dominating owner, transfers and lifetime separately.
    pub const fn shared_reference_pair(
        self,
        argument: usize,
    ) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
        use SemanticGfx950TransposeOperationV1 as T;
        match (self.operation, argument) {
            (
                T::Issue {
                    partition_reference,
                    partition,
                    ..
                },
                0,
            ) => Some((partition_reference, partition)),
            (
                T::Stage {
                    view_reference,
                    view,
                    ..
                },
                1,
            ) => Some((view_reference, view)),
            (
                T::Read {
                    lane_reference,
                    lane,
                    ..
                },
                1,
            ) => Some((lane_reference, lane)),
            _ => None,
        }
    }

    pub(super) fn type_references(self) -> [Option<SemanticTypeIdV1>; 8] {
        use SemanticGfx950TransposeOperationV1 as T;
        let mut result = [None; 8];
        let ids: &[SemanticTypeIdV1] = match &self.operation {
            T::Issue {
                partition_reference,
                partition,
                tile,
            } => &[*partition_reference, *partition, *tile],
            T::Stage {
                input_tile,
                output_tile,
                view_reference,
                view,
                index,
                global_reference,
                global,
            } => &[
                *input_tile,
                *output_tile,
                *view_reference,
                *view,
                *index,
                *global_reference,
                *global,
            ],
            T::Publish {
                input_tile,
                input_workgroup,
                transition,
                output_workgroup,
                output_tile,
            } => &[
                *input_tile,
                *input_workgroup,
                *transition,
                *output_workgroup,
                *output_tile,
            ],
            T::Read {
                tile,
                lane_reference,
                lane,
                fragment,
                registers,
                word,
            } => &[*tile, *lane_reference, *lane, *fragment, *registers, *word],
        };
        for (slot, id) in result.iter_mut().zip(ids) {
            *slot = Some(*id);
        }
        result
    }

    /// Obligations retained for independent downstream proof, not discharged here.
    pub const fn obligations(self) -> u32 {
        use SemanticExecutionSafetyObligationsV1 as O;
        use SemanticGfx950TransposeOperationV1 as T;
        let base = O::TARGET_SUPPORT | O::DYNAMIC_WORKGROUP_IDENTITY | O::LIFETIME_VALIDITY;
        match self.operation {
            T::Issue { .. } => base | O::DISJOINT_LDS_ALLOCATION,
            T::Stage { .. } => {
                base | O::BOUNDS
                    | O::INITIALIZATION
                    | O::RACE_FREEDOM
                    | O::SUBGROUP_CONVERGENCE
                    | O::EXACT_PARTICIPATION
            }
            T::Publish { .. } => {
                base | O::WORKGROUP_CONVERGENCE
                    | O::EXACT_PARTICIPATION
                    | O::INITIALIZATION
                    | O::MEMORY_MODEL
            }
            T::Read { .. } => {
                base | O::BOUNDS
                    | O::INITIALIZATION
                    | O::RACE_FREEDOM
                    | O::MEMORY_MODEL
                    | O::SUBGROUP_CONVERGENCE
                    | O::EXACT_PARTICIPATION
            }
        }
    }

    pub(super) fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        use SemanticGfx950TransposeOperationV1 as T;
        writer.u8(match self.operation {
            T::Issue { .. } => 0,
            T::Stage { .. } => 1,
            T::Publish { .. } => 2,
            T::Read { .. } => 3,
        })?;
        encode_gfx950_lds_transpose_format(writer, self.format)?;
        for id in self.type_references().into_iter().flatten() {
            writer.u32(id.index())?;
        }
        writer.identity(*self.subgroup_brand.as_bytes())?;
        if let Some(next) = self.next_subgroup_brand {
            writer.identity(*next.as_bytes())?;
        }
        Ok(())
    }
}

pub(super) fn abi_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
    contract: SemanticGfx950TransposeContractV1,
) -> bool {
    use SemanticGfx950TransposeOperationV1 as T;
    use SemanticSourceArgumentOwnershipV1 as Own;
    let ownership: &[Own] = match contract.operation {
        T::Issue { .. } => &[Own::SharedBorrow],
        T::Stage { .. } => &[Own::ByValue, Own::SharedBorrow, Own::ByValue, Own::ByValue],
        T::Publish { .. } => &[Own::ByValue, Own::ByValue],
        T::Read { .. } => &[Own::ByValue, Own::SharedBorrow],
    };
    if abi.source_argument_ownership() != ownership
        || abi.c_variadic()
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || !abi.hidden_arguments().is_empty()
    {
        return false;
    }
    let signature = SemanticExecutionCapabilitySignatureV1::new(
        abi.source_input_types(),
        abi.source_output_type(),
    );
    if !signature.is_ok_and(|signature| contract.signature_matches(signature)) {
        return false;
    }
    match contract.operation {
        T::Issue {
            partition_reference,
            partition,
            tile,
        } => {
            shared_reference_to(request, partition_reference, partition)
                && tile_matches(request, tile)
                && zst_fields(request, partition, 2)
        }
        T::Stage {
            input_tile,
            output_tile,
            view_reference,
            view,
            index,
            global_reference,
            global,
        } => {
            shared_reference_to(request, view_reference, view)
                && shared_reference_to(request, global_reference, global)
                && tile_matches(request, input_tile)
                && tile_matches(request, output_tile)
                && unsigned_integer(request, index, 64)
                && fields(request, view).is_some_and(|ids| {
                    ids.len() == 9
                        && ids[..5] == [global_reference, index, index, index, index]
                        && ids[5..].iter().all(|id| zst(request, *id))
                })
        }
        T::Publish {
            input_tile,
            input_workgroup,
            transition,
            output_workgroup,
            output_tile,
        } => {
            tile_matches(request, input_tile)
                && tile_matches(request, output_tile)
                && workgroup_borrow_v1::workgroup_fields(request, input_workgroup).is_some()
                && workgroup_borrow_v1::workgroup_fields(request, output_workgroup).is_some()
                && matches!(request.types.get(transition.index() as usize).map(|ty| ty.shape()),
                    Some(SemanticTypeShapeV1::Tuple(tuple))
                        if tuple.fields() == [output_workgroup, output_tile])
        }
        T::Read {
            tile,
            lane_reference,
            lane,
            fragment,
            registers,
            word,
        } => {
            tile_matches(request, tile)
                && shared_reference_to(request, lane_reference, lane)
                && unsigned_integer(request, word, 32)
                && matches!(request.types.get(registers.index() as usize).map(|ty| ty.shape()),
                    Some(SemanticTypeShapeV1::Array { element, length: 8 }) if *element == word)
                && fields(request, fragment).is_some_and(|ids| {
                    ids.len() == 3
                        && ids[0] == registers
                        && ids[1..].iter().all(|id| zst(request, *id))
                })
        }
    }
}

fn fields(
    request: &InertSemanticMirRequestV1,
    id: SemanticTypeIdV1,
) -> Option<&[SemanticTypeIdV1]> {
    match request.types.get(id.index() as usize)?.shape() {
        SemanticTypeShapeV1::Aggregate(aggregate) => Some(aggregate.fields()),
        _ => None,
    }
}

fn zst(request: &InertSemanticMirRequestV1, id: SemanticTypeIdV1) -> bool {
    request.types.get(id.index() as usize).is_some_and(|ty| {
        ty.layout().size_bytes() == Some(0)
            && ty.layout().alignment_bytes() == 1
            && !ty.layout().is_uninhabited()
    })
}

fn zst_fields(request: &InertSemanticMirRequestV1, id: SemanticTypeIdV1, count: usize) -> bool {
    zst(request, id)
        && fields(request, id)
            .is_some_and(|ids| ids.len() == count && ids.iter().all(|id| zst(request, *id)))
}

fn tile_matches(request: &InertSemanticMirRequestV1, id: SemanticTypeIdV1) -> bool {
    zst_fields(request, id, 2)
}

fn unsigned_integer(request: &InertSemanticMirRequestV1, id: SemanticTypeIdV1, width: u16) -> bool {
    request.types.get(id.index() as usize).is_some_and(|ty|
        matches!(ty.shape(), SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits })
            if *bits == width))
}

impl InertSemanticMirRequestV1 {
    /// Admits exact additive V24 bytes without granting execution authority.
    pub fn admit_exact_v24(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        self.admit_for_wire_version(SemanticMirWireVersionV1::V24, limits)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TileClaim {
    provenance: SemanticKernelCapabilityProvenanceV1,
    execution: SemanticTypeIdentityV1,
    epoch: SemanticTypeIdentityV1,
    subgroup: SemanticTypeIdentityV1,
    format: SemanticGfx950LdsTransposeFormatV1,
    state: u8,
}

pub(super) fn record_claim(
    claims: &mut IntrinsicCapabilityClaimsV1,
    contract: SemanticExecutionCapabilityContractV1,
) -> bool {
    let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose) = contract.operation()
    else {
        return true;
    };
    let (Some(execution), Some(epoch)) = (contract.workgroup_brand(), contract.epoch_before())
    else {
        return false;
    };
    let before = TileClaim {
        provenance: contract.provenance(),
        execution,
        epoch,
        subgroup: transpose.subgroup_brand,
        format: transpose.format,
        state: 0,
    };
    use SemanticGfx950TransposeOperationV1 as T;
    let rows = match transpose.operation {
        T::Issue { tile, .. } => [Some((tile, before)), None],
        T::Stage {
            input_tile,
            output_tile,
            ..
        } => [
            Some((input_tile, before)),
            Some((output_tile, TileClaim { state: 1, ..before })),
        ],
        T::Publish {
            input_tile,
            input_workgroup,
            output_tile,
            output_workgroup,
            ..
        } => {
            let (Some(next_epoch), Some(subgroup)) =
                (contract.epoch_after(), transpose.next_subgroup_brand)
            else {
                return false;
            };
            if !claims.record_borrowed_workgroup(
                input_workgroup,
                contract.provenance(),
                execution,
                epoch,
            ) || !claims.record_borrowed_workgroup(
                output_workgroup,
                contract.provenance(),
                execution,
                next_epoch,
            ) {
                return false;
            }
            [
                Some((input_tile, TileClaim { state: 1, ..before })),
                Some((
                    output_tile,
                    TileClaim {
                        state: 2,
                        epoch: next_epoch,
                        subgroup,
                        ..before
                    },
                )),
            ]
        }
        T::Read { tile, .. } => [Some((tile, TileClaim { state: 2, ..before })), None],
    };
    rows.into_iter()
        .flatten()
        .all(|(ty, claim)| match claims.transpose_tiles.entry(ty) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(claim);
                true
            }
            std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == claim,
        })
}

#[cfg(test)]
#[path = "gfx950_transpose_v1/tests.rs"]
mod tests;
