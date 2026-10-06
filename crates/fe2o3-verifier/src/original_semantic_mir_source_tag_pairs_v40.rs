//! Complete source/physical tag pairs from retained original object endpoints.
//! Geometry checks refine that provenance; they never create nominal type or
//! reference-validity authority from equal bytes.
use super::super::super::tile_target::TileTargetV176;
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::target_view_contracts_v38::{
    TargetByteTagClassV38 as TargetClass, TargetByteViewContractsV38 as TargetContracts,
};
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKirOperationCoordinateV1 as Operation, OperationKind,
    ScalarType as PhysicalScalar, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind,
    StorageLayoutV1 as Layout, StorageOperationV1 as Storage, StorageProjectionV1 as Projection,
    StorageVariantEncodingV1 as PhysicalEncoding, Type as PhysicalType,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceObjectEndpointRoleV39 as EndpointRole,
    ProductionSourceObjectRecipeV39 as ObjectRecipe, ProductionSourceObjectSchemaV42 as SchemaRole,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Pair {
    source: TypeId,
    physical: Id,
    selected_space: Option<AddressSpace>,
}

struct TagOperation {
    coordinate: Operation,
    ordinal: usize,
    physical: Id,
    source: Option<TypeId>,
}

pub(in super::super::super) struct SourceTagPairsV40<'a, 'view, 'source, 'inventory, 'owner> {
    slots: &'a SourceSlots<'view, 'source>,
    contracts: &'a TargetContracts<'inventory, 'owner>,
    pairs: Vec<Pair>,
    operations: Vec<TagOperation>,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original source tag and canonical storage locator differ")
}

fn unsupported() -> Error {
    Error::Statement("original source tag pair is not modeled")
}

fn integer_primitive(scalar: PhysicalScalar) -> Option<(bool, u16)> {
    Some(match scalar {
        PhysicalScalar::I8 => (true, 8),
        PhysicalScalar::I16 => (true, 16),
        PhysicalScalar::I32 => (true, 32),
        PhysicalScalar::I64 => (true, 64),
        PhysicalScalar::I128 => (true, 128),
        PhysicalScalar::U8 => (false, 8),
        PhysicalScalar::U16 => (false, 16),
        PhysicalScalar::U32 => (false, 32),
        PhysicalScalar::U64 => (false, 64),
        PhysicalScalar::U128 => (false, 128),
        _ => return None,
    })
}

fn primitive_matches(
    primitive: Primitive,
    physical: &Layout,
    selected_space: Option<AddressSpace>,
) -> bool {
    if primitive.size_bytes() != Some(physical.size)
        || primitive.alignment_bytes() != u64::from(physical.alignment)
    {
        return false;
    }
    match (primitive, &physical.kind) {
        (Primitive::Integer { signed, bits, .. }, Kind::Scalar(scalar)) => {
            integer_primitive(*scalar) == Some((signed, bits))
        }
        (
            Primitive::Pointer {
                address_space,
                size_bytes,
                ..
            },
            Kind::Pointer(pointer),
        ) => {
            let space = match address_space {
                0 => AddressSpace::Generic,
                1 => AddressSpace::Global,
                3 => AddressSpace::Workgroup,
                4 => AddressSpace::Constant,
                5 => AddressSpace::Private,
                _ => return false,
            };
            size_bytes.checked_mul(8) == Some(u64::from(pointer.stored_bits))
                && pointer.encoded_space == space
                && pointer.value_space == selected_space.unwrap_or(space)
                && (space == AddressSpace::Generic || pointer.value_space == space)
        }
        _ => false,
    }
}

// This compares inert records only. Private construction below first rejoins
// each pair to its archived original object endpoint and complete operation.
fn check_records(
    types: &[Declaration],
    source: &Declaration,
    class: SourceTagClassV39,
    physical_rows: &[Layout],
    physical: &Layout,
    target_class: TargetClass,
    selected_space: Option<AddressSpace>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(10)?;
    let (
        Shape::Enum {
            discriminant,
            variants,
        },
        Variants::Multiple(layout),
    ) = (source.shape(), source.layout().variants())
    else {
        return Err(unsupported());
    };
    let Kind::Variants {
        encoding,
        variants: actual,
    } = &physical.kind
    else {
        return Err(mismatch());
    };
    if source.layout().size_bytes() != Some(physical.size)
        || source.layout().alignment_bytes() != u64::from(physical.alignment)
        || variants.len() != actual.len()
        || variants.len() != layout.variants().len()
    {
        return Err(mismatch());
    }
    let (offset, primitive) = match layout.encoding() {
        Encoding::Direct(tag) => (tag.tag_offset_bytes(), tag.tag().primitive()),
        Encoding::Niche(niche) => (
            niche.source().expected_offset_bytes(),
            niche.tag().primitive(),
        ),
    };
    let tag = physical_rows
        .get(encoding.tag().layout.0 as usize)
        .ok_or_else(mismatch)?;
    if selected_space.is_some() != matches!(class, SourceTagClassV39::PointerNullReference { .. }) {
        return Err(mismatch());
    }
    if offset != encoding.tag().offset || !primitive_matches(primitive, tag, selected_space) {
        return Err(mismatch());
    }
    if let SourceTagClassV39::PointerNullReference { terminal } = class {
        let Shape::Pointer(reference) = types
            .get(terminal.index() as usize)
            .ok_or_else(mismatch)?
            .shape()
        else {
            return Err(mismatch());
        };
        let Kind::Pointer(pointer) = tag.kind else {
            return Err(mismatch());
        };
        let access = match reference.mutability() {
            fe2o3_mir_model::semantic_mir_v1::SemanticMutabilityV1::Immutable => {
                fe2o3_kernel_ir::AccessMode::ReadOnly
            }
            fe2o3_mir_model::semantic_mir_v1::SemanticMutabilityV1::Mutable => {
                fe2o3_kernel_ir::AccessMode::ReadWrite
            }
        };
        if reference.kind() != PointerKind::Reference
            || reference.metadata() != Metadata::None
            || pointer.stored_bits != reference.pointer_width_bits()
            || pointer.access != access
        {
            return Err(mismatch());
        }
    }
    let direct = match (class, target_class, layout.encoding(), encoding) {
        (
            SourceTagClassV39::DirectScalar,
            TargetClass::DirectScalar,
            Encoding::Direct(_),
            PhysicalEncoding::Direct { .. },
        ) => {
            let Shape::Scalar(logical) = types
                .get(discriminant.index() as usize)
                .ok_or_else(mismatch)?
                .shape()
            else {
                return Err(mismatch());
            };
            Some(*logical)
        }
        (
            SourceTagClassV39::ScalarNiche { .. },
            TargetClass::ScalarNiche,
            Encoding::Niche(source),
            PhysicalEncoding::Niche {
                untagged_variant,
                first_niche_variant,
                last_niche_variant,
                niche_start,
                ..
            },
        )
        | (
            SourceTagClassV39::PointerNullReference { .. },
            TargetClass::PointerNullNiche,
            Encoding::Niche(source),
            PhysicalEncoding::Niche {
                untagged_variant,
                first_niche_variant,
                last_niche_variant,
                niche_start,
                ..
            },
        ) => {
            if source.untagged_variant() != *untagged_variant
                || source.niche_variant_range() != (*first_niche_variant, *last_niche_variant)
                || source.niche_start() != *niche_start
            {
                return Err(mismatch());
            }
            None
        }
        _ => return Err(unsupported()),
    };
    for ((source, layout), actual) in variants.iter().zip(layout.variants()).zip(actual) {
        out.budget.charge_work(7)?;
        let payload = physical_rows
            .get(actual.layout.0 as usize)
            .ok_or_else(mismatch)?;
        if source.discriminant() != actual.discriminant
            || source.is_uninhabited() != actual.uninhabited
            || layout.is_uninhabited() != actual.uninhabited
            || layout.rustc_size_bytes() != payload.size
            || layout.alignment_bytes() != u64::from(payload.alignment)
            || actual.direct_tag_bits
                != direct
                    .map(|logical| direct_tag(logical, primitive, source.discriminant()))
                    .transpose()?
        {
            return Err(mismatch());
        }
    }
    Ok(())
}

fn is_tag_operation(operation: Storage) -> bool {
    matches!(
        operation,
        Storage::ReadDiscriminant { .. }
            | Storage::SetDiscriminant { .. }
            | Storage::Project {
                step: Projection::Variant { .. } | Projection::VariantForWrite { .. },
                ..
            }
    )
}

fn pair_present(rows: &[Pair], key: (TypeId, Id), out: &mut Writer<'_, '_>) -> Result<bool> {
    let (mut lo, mut hi) = (0, rows.len());
    while lo < hi {
        out.budget.charge_work(1)?;
        let middle = lo + (hi - lo) / 2;
        if (rows[middle].source, rows[middle].physical) < key {
            lo = middle + 1;
        } else {
            hi = middle;
        }
    }
    out.budget.charge_work(1)?;
    Ok(rows
        .get(lo)
        .is_some_and(|row| (row.source, row.physical) == key))
}

fn tag_position(
    rows: &[TagOperation],
    coordinate: Operation,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let (mut lo, mut hi) = (0, rows.len());
    while lo < hi {
        out.budget.charge_work(1)?;
        let middle = lo + (hi - lo) / 2;
        if rows[middle].coordinate < coordinate {
            lo = middle + 1;
        } else {
            hi = middle;
        }
    }
    out.budget.charge_work(1)?;
    if rows.get(lo).is_some_and(|row| row.coordinate == coordinate) {
        Ok(lo)
    } else {
        Err(mismatch())
    }
}

fn sort_pairs(rows: &mut [Pair], out: &mut Writer<'_, '_>) -> Result<()> {
    fn sift(
        rows: &mut [Pair],
        mut root: usize,
        end: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        loop {
            out.budget.charge_work(1)?;
            let mut child = root
                .checked_mul(2)
                .and_then(|v| v.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if child >= end {
                return Ok(());
            }
            out.budget.charge_work(2)?;
            if child + 1 < end && rows[child] < rows[child + 1] {
                child += 1;
            }
            if rows[root] >= rows[child] {
                return Ok(());
            }
            out.budget.charge_work(1)?;
            rows.swap(root, child);
            root = child;
        }
    }
    for root in (0..rows.len() / 2).rev() {
        out.budget.charge_work(1)?;
        sift(rows, root, rows.len(), out)?;
    }
    for end in (1..rows.len()).rev() {
        out.budget.charge_work(1)?;
        rows.swap(0, end);
        sift(rows, 0, end, out)?;
    }
    Ok(())
}

impl<'a, 'view, 'source, 'inventory, 'owner>
    SourceTagPairsV40<'a, 'view, 'source, 'inventory, 'owner>
{
    pub(in super::super::super) fn derive(
        slots: &'a SourceSlots<'view, 'source>,
        contracts: &'a TargetContracts<'inventory, 'owner>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let result = Self::derive_inner(slots, contracts, out);
        if let Err(Error::Resource(resource)) = &result {
            slots.relation.retain_query_resource_error_v18(*resource);
        }
        result
    }

    fn derive_inner(
        slots: &'a SourceSlots<'view, 'source>,
        contracts: &'a TargetContracts<'inventory, 'owner>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.check_tag_query(out)?;
        let inventory = slots.relation.inventory(out.budget)?;
        contracts.check_owner(inventory.owner(), out)?;
        out.budget.reserve_storage(headers())?;
        let source = slots.relation.source(out.budget)?;
        let roots = source.root_count(out.budget)?;
        let mut capacity = 0usize;
        for root in 0..roots {
            out.budget.charge_work(1)?;
            for instance in 0..source.instance_count(root, out.budget)? {
                out.budget.charge_work(1)?;
                capacity = capacity
                    .checked_add(source.memory_anchor_count(root, instance, out.budget)?)
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        let mut pairs = vector(capacity.checked_mul(4).ok_or(Resource::Arithmetic)?, out)?;
        let mut count = 0usize;
        for actual in inventory.operations() {
            out.budget.charge_work(1)?;
            if matches!(actual.operation.kind, OperationKind::Storage(operation) if is_tag_operation(operation))
            {
                count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        let mut operations: Vec<TagOperation> = vector(count, out)?;
        for (ordinal, actual) in inventory.operations().iter().enumerate() {
            out.budget.charge_work(2)?;
            let OperationKind::Storage(operation) = actual.operation.kind else {
                continue;
            };
            if !is_tag_operation(operation) {
                continue;
            }
            let first = inventory
                .uses()
                .get(actual.operands.clone())
                .and_then(|uses| uses.first())
                .ok_or_else(mismatch)?;
            let PhysicalType::Pointer(pointer) = inventory
                .definitions()
                .get(first.definition)
                .ok_or_else(mismatch)?
                .ty
            else {
                return Err(mismatch());
            };
            let PhysicalType::StorageObject(physical) = pointer.pointee.as_ref() else {
                return Err(mismatch());
            };
            if operations
                .last()
                .is_some_and(|previous| previous.coordinate >= actual.coordinate)
            {
                return Err(mismatch());
            }
            operations.push(TagOperation {
                coordinate: actual.coordinate,
                ordinal,
                physical: *physical,
                source: None,
            });
        }
        for root in 0..roots {
            out.budget.charge_work(1)?;
            for instance in 0..source.instance_count(root, out.budget)? {
                out.budget.charge_work(1)?;
                for anchor in 0..source.memory_anchor_count(root, instance, out.budget)? {
                    out.budget.charge_work(1)?;
                    let Some(recipe) = slots
                        .relation
                        .memory_object_recipe_v39(root, instance, anchor, out.budget)?
                    else {
                        continue;
                    };
                    Self::record_recipe(slots, &recipe, &mut pairs, &mut operations, out)?;
                }
            }
        }
        sort_pairs(&mut pairs, out)?;
        // Compact in place, retaining the already paid allocation capacity.
        let mut unique = 0usize;
        for read in 0..pairs.len() {
            out.budget.charge_work(4)?;
            if unique != 0
                && (pairs[unique - 1].source, pairs[unique - 1].physical)
                    == (pairs[read].source, pairs[read].physical)
                && pairs[unique - 1].selected_space != pairs[read].selected_space
            {
                return Err(mismatch());
            }
            if unique == 0 || pairs[unique - 1] != pairs[read] {
                pairs[unique] = pairs[read];
                unique += 1;
            }
        }
        pairs.truncate(unique);
        let semantic = source.source_semantic(out.budget)?;
        let rows = &inventory.owner().module().storage_layouts;
        for pair in &pairs {
            out.budget.charge_work(2)?;
            let original = slots.tag_contract(pair.source, out)?;
            check_records(
                semantic.types(),
                original.declaration(out)?,
                original.class(out)?,
                rows,
                rows.get(pair.physical.0 as usize).ok_or_else(mismatch)?,
                contracts.row_class(pair.physical, out)?,
                pair.selected_space,
                out,
            )?;
        }
        Self::complete(&pairs, &operations, out)?;
        Ok(Self {
            slots,
            contracts,
            pairs,
            operations,
            required: out.budget.storage(),
        })
    }

    fn record_recipe(
        slots: &SourceSlots<'_, '_>,
        recipe: &ObjectRecipe<'_, '_>,
        pairs: &mut Vec<Pair>,
        operations: &mut [TagOperation],
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let inventory = slots.relation.inventory(out.budget)?;
        let rows = &inventory.owner().module().storage_layouts;
        let (operation, _) = recipe.physical_operation(out.budget)?;
        let coordinate = recipe.original_operation(out.budget)?;
        let tag = if is_tag_operation(operation) {
            Some(tag_position(operations, coordinate, out)?)
        } else {
            None
        };
        for ordinal in 0..recipe.endpoint_count(out.budget)? {
            out.budget.charge_work(3)?;
            let endpoint = recipe.endpoint(ordinal, out.budget)?;
            let source = endpoint.source_types(out.budget)?;
            let physical = endpoint.storage_layouts(out.budget)?;
            for (component, source, physical) in [
                (SchemaRole::Root, source.0, physical.0),
                (SchemaRole::Projected, source.1, physical.1),
            ] {
                out.budget.charge_work(2)?;
                let class = slots.tag_contract(source, out)?.class(out)?;
                let tagged = matches!(
                    rows.get(physical.0 as usize).ok_or_else(mismatch)?.kind,
                    Kind::Variants { .. }
                );
                match (class, tagged) {
                    // A downcast retains its original enum TypeId while its
                    // projected physical schema is the untagged payload row.
                    // Only whole tagged schemas need a registry correspondence.
                    (_, false) => {}
                    (SourceTagClassV39::NotTagged | SourceTagClassV39::Unsupported, true) => {
                        return Err(unsupported());
                    }
                    _ => {
                        let selected_space = if let SourceTagClassV39::PointerNullReference {
                            terminal,
                        } = class
                        {
                            let selected = recipe
                                .selected_pointer_niche_v42(ordinal, component, out.budget)?
                                .ok_or_else(mismatch)?;
                            selected.check_binding(recipe, ordinal, component, out.budget)?;
                            if selected.types_and_schema(out.budget)?
                                != (source, terminal, physical)
                            {
                                return Err(mismatch());
                            }
                            let pointer = selected.pointer(out.budget)?;
                            let Kind::Variants { encoding, .. } =
                                &rows.get(physical.0 as usize).ok_or_else(mismatch)?.kind
                            else {
                                return Err(mismatch());
                            };
                            if !matches!(rows.get(encoding.tag().layout.0 as usize).ok_or_else(mismatch)?.kind, Kind::Pointer(actual) if actual == pointer)
                            {
                                return Err(mismatch());
                            }
                            Some(pointer.value_space)
                        } else {
                            None
                        };
                        if pairs.len() == pairs.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        pairs.push(Pair {
                            source,
                            physical,
                            selected_space,
                        });
                    }
                }
            }
            if ordinal == 0
                && let Some(index) = tag
            {
                let expected = match operation {
                    Storage::Project { .. } => EndpointRole::ProjectionBase,
                    Storage::ReadDiscriminant { .. } => EndpointRole::DiscriminantRead,
                    Storage::SetDiscriminant { .. } => EndpointRole::DiscriminantWrite,
                    _ => return Err(mismatch()),
                };
                let row = &mut operations[index];
                if endpoint.role(out.budget)? != expected
                    || physical.1 != row.physical
                    || row.source.is_some()
                {
                    return Err(mismatch());
                }
                let actual = inventory
                    .operations()
                    .get(row.ordinal)
                    .ok_or_else(mismatch)?;
                if actual.coordinate != coordinate
                    || actual.operation.kind != OperationKind::Storage(operation)
                {
                    return Err(mismatch());
                }
                row.source = Some(source.1);
            }
        }
        Ok(())
    }

    fn complete(
        pairs: &[Pair],
        operations: &[TagOperation],
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        for operation in operations {
            out.budget.charge_work(1)?;
            let source = operation.source.ok_or(Error::Statement(
                "actual tag operation lacks an authenticated original object recipe",
            ))?;
            if !pair_present(pairs, (source, operation.physical), out)? {
                return Err(mismatch());
            }
        }
        Ok(())
    }

    pub(in super::super::super) fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        let result = (|| {
            self.slots.check_tag_query(out)?;
            let inventory = self.slots.relation.inventory(out.budget)?;
            self.contracts.check_owner(inventory.owner(), out)?;
            if out.budget.storage() < self.required {
                return Err(Resource::Accounting.into());
            }
            Ok(())
        })();
        if let Err(Error::Resource(resource)) = &result {
            self.slots
                .relation
                .retain_query_resource_error_v18(*resource);
        }
        result
    }

    pub(in super::super::super) fn require_pair(
        &self,
        source: TypeId,
        physical: Id,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let result = (|| {
            self.check(out)?;
            if !pair_present(&self.pairs, (source, physical), out)? {
                return Err(mismatch());
            }
            Ok(())
        })();
        if let Err(Error::Resource(resource)) = &result {
            self.slots
                .relation
                .retain_query_resource_error_v18(*resource);
        }
        result
    }

    pub(in super::super::super) fn emit(
        &self,
        source_namespace: usize,
        target_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let result = (|| {
            self.check(out)?;
            if source_namespace == target_namespace {
                return Err(mismatch());
            }
            emit!(
                out,
                "spec fn invocation_source_target_tag_pair_{source_namespace}_{target_namespace}_v40(source_type: int, physical_layout: int) -> bool {{ false"
            );
            for pair in &self.pairs {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    " || (source_type == {}int && physical_layout == {}int)",
                    pair.source.index(),
                    pair.physical.0
                );
            }
            emit!(out, " }}\n");
            Ok(())
        })();
        if let Err(Error::Resource(resource)) = &result {
            self.slots
                .relation
                .retain_query_resource_error_v18(*resource);
        }
        result
    }

    /// Rejoins the original nominal pairs to the actual expanded owner before
    /// emission. This supplies tag correspondence, not value/effect refinement.
    pub(in super::super::super) fn emit_expanded_v190(
        &self,
        target: &TileTargetV176<'_, '_, '_>,
        contracts: &TargetContracts<'_, '_>,
        width: fe2o3_kernel_ir::FormalIndexWidth,
        source_namespace: usize,
        target_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check_expanded_v190(target, contracts, width, out)?;
            self.emit(source_namespace, target_namespace, out)?;
            contracts.check_owner_width_v39(target.inventory(out)?.owner(), width, out)?;
            self.check(out)
        })
    }

    fn check_expanded_v190(
        &self,
        target: &TileTargetV176<'_, '_, '_>,
        contracts: &TargetContracts<'_, '_>,
        width: fe2o3_kernel_ir::FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        out.budget.reserve_storage(expanded_headers_v190())?;
        if !std::ptr::eq(self.slots, target.source_slots(out)?) {
            return Err(mismatch());
        }
        let output = target.inventory(out)?;
        let original = self.slots.correspondence(out)?.inventory(out.budget)?;
        self.contracts
            .check_owner_width_v39(original.owner(), width, out)?;
        contracts.check_owner_width_v39(output.owner(), width, out)?;
        same_tables_v190(
            &original.owner().module().storage_layouts,
            &output.owner().module().storage_layouts,
            out,
        )?;
        let semantic = self
            .slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let physical = &output.owner().module().storage_layouts;
        // Equal table bytes alone grant no nominal type authority. Check each
        // retained source contract against the independently classified target.
        for pair in &self.pairs {
            out.budget.charge_work(1)?;
            let source_contract = self.slots.tag_contract(pair.source, out)?;
            check_records(
                semantic.types(),
                source_contract.declaration(out)?,
                source_contract.class(out)?,
                physical,
                physical
                    .get(pair.physical.0 as usize)
                    .ok_or_else(mismatch)?,
                contracts.row_class(pair.physical, out)?,
                pair.selected_space,
                out,
            )?;
        }
        let tile = self.slots.tile_owner_v176(out)?;
        let neutral = tile
            .neutral_source_v162(out.budget)?
            .output_inventory(out.budget)?;
        let mut seen = vector(output.operations().len(), out)?;
        out.budget.charge_work(output.operations().len())?;
        seen.resize(output.operations().len(), false);
        for row in &self.operations {
            out.budget.charge_work(1)?;
            // A prefix rewrite is deliberately not an empty scalar span.
            let Some(span) = tile.operation_span(row.coordinate, out.budget)? else {
                continue;
            };
            let span = span.expansion;
            out.budget.charge_work(3)?;
            if span.end.checked_sub(span.first) != Some(1) {
                return Err(mismatch());
            }
            let coordinate = Operation {
                block: span.input.block,
                operation: span.first,
            };
            out.budget.charge_work(
                (usize::BITS - output.operations().len().leading_zeros()) as usize + 1,
            )?;
            let at = output
                .operations()
                .binary_search_by_key(&coordinate, |row| row.coordinate)
                .map_err(|_| mismatch())?;
            out.budget.charge_work(
                (usize::BITS - neutral.operations().len().leading_zeros()) as usize + 1,
            )?;
            let before = neutral
                .operations()
                .binary_search_by_key(&span.input, |row| row.coordinate)
                .map_err(|_| mismatch())?;
            let actual = &output.operations()[at];
            let predecessor = &neutral.operations()[before];
            out.budget.charge_work(6)?;
            let (OperationKind::Storage(left), OperationKind::Storage(right)) =
                (predecessor.operation.kind, actual.operation.kind)
            else {
                return Err(mismatch());
            };
            if !is_tag_operation(left) || left != right || seen[at] {
                return Err(mismatch());
            }
            let first = output
                .uses()
                .get(actual.operands.clone())
                .and_then(|uses| uses.first())
                .ok_or_else(mismatch)?;
            let PhysicalType::Pointer(pointer) = output
                .definitions()
                .get(first.definition)
                .ok_or_else(mismatch)?
                .ty
            else {
                return Err(mismatch());
            };
            if !matches!(pointer.pointee.as_ref(), PhysicalType::StorageObject(id) if *id == row.physical)
            {
                return Err(mismatch());
            }
            let source = row.source.ok_or_else(mismatch)?;
            if !pair_present(&self.pairs, (source, row.physical), out)? {
                return Err(mismatch());
            }
            seen[at] = true;
        }
        for (row, bound) in output.operations().iter().zip(&seen) {
            out.budget.charge_work(1)?;
            if *bound
                != matches!(row.operation.kind, OperationKind::Storage(operation) if is_tag_operation(operation))
            {
                return Err(mismatch());
            }
        }
        let temporary = seen
            .capacity()
            .checked_mul(size_of::<bool>())
            .ok_or(Resource::Arithmetic)?;
        drop(seen);
        out.budget.release_storage(temporary)?;
        self.check(out)?;
        contracts.check_owner_width_v39(target.inventory(out)?.owner(), width, out)
    }
}

fn same_tables_v190(left: &[Layout], right: &[Layout], out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(1)?;
    if left.len() != right.len() {
        return Err(mismatch());
    }
    for (left, right) in left.iter().zip(right) {
        // Layout children are table IDs, not recursive objects. Charge every
        // variable-length field before the complete structural comparison.
        for row in [left, right] {
            let fields = match &row.kind {
                Kind::Record(fields) | Kind::Union(fields) => fields.len(),
                Kind::Variants { variants, .. } => variants.len(),
                Kind::Scalar(_)
                | Kind::Vector(_)
                | Kind::Pointer(_)
                | Kind::Array { .. }
                | Kind::Slice { .. } => 0,
            };
            out.budget
                .charge_work(fields.checked_add(8).ok_or(Resource::Arithmetic)?)?;
        }
        if left != right {
            return Err(mismatch());
        }
    }
    Ok(())
}

fn expanded_headers_v190() -> usize {
    size_of::<Vec<bool>>()
        + 2 * size_of::<Result<Vec<bool>>>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>()
        + 2 * size_of::<Result<Option<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>>>(
        )
        + size_of::<(
            &SourceTagPairsV40<'_, '_, '_, '_, '_>,
            &TileTargetV176<'_, '_, '_>,
            &TargetContracts<'_, '_>,
            &mut Writer<'_, '_>,
            [&Layout; 2],
            &[Layout],
            &[Layout],
            std::slice::Iter<'static, TagOperation>,
            std::slice::Iter<'static, Pair>,
            [usize; 24],
            [Operation; 3],
            [Storage; 2],
            Option<TypeId>,
        )>()
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceTagPairsV40<'_, '_, '_, '_, '_>>()
        + h::<ObjectRecipe<'_, '_>>()
        + h::<Option<ObjectRecipe<'_, '_>>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectEndpointV39<'_, '_>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceSelectedPointerNicheV42<'_, '_>>()
        + h::<Option<fe2o3_lower_mir_kernel::ProductionSourceSelectedPointerNicheV42<'_, '_>>>()
        + h::<SchemaRole>()
        + h::<Option<AddressSpace>>()
        + h::<fe2o3_kernel_ir::StoragePointerV1>()
        + h::<(TypeId, TypeId, Id)>()
        + h::<Vec<Pair>>()
        + h::<Vec<TagOperation>>()
        + h::<Pair>()
        + h::<TagOperation>()
        + h::<SourceTagRecipeV39<'_, '_, '_>>()
        + h::<SourceTagClassV39>()
        + h::<TargetClass>()
        + h::<EndpointRole>()
        + h::<(TypeId, TypeId)>()
        + h::<(Id, Id)>()
        + h::<Operation>()
        + h::<(Storage, Option<fe2o3_kernel_ir::ValueId>)>()
        + h::<bool>()
        + h::<usize>()
        + h::<()>()
        + size_of::<(
            &SourceSlots<'_, '_>,
            &TargetContracts<'_, '_>,
            &mut Writer<'_, '_>,
            &crate::mixed_optimizer_refinement_v26::semantics::Inventory<'_>,
            &[Declaration],
            &[Layout],
            [&Layout; 4],
            &Declaration,
            &EnumLayout,
            std::slice::Iter<'static, Pair>,
            std::slice::Iter<'static, TagOperation>,
            std::iter::Zip<
                std::iter::Zip<
                    std::slice::Iter<'static, Variant>,
                    std::slice::Iter<
                        'static,
                        fe2o3_mir_model::semantic_mir_v1::SemanticEnumVariantLayoutV1,
                    >,
                >,
                std::slice::Iter<'static, fe2o3_kernel_ir::StorageVariantV1>,
            >,
            std::array::IntoIter<(SchemaRole, TypeId, Id), 2>,
            [usize; 24],
            [u64; 4],
            Option<Scalar>,
            PhysicalEncoding,
            Primitive,
        )>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_tag_pairs_v40_tests.rs"]
pub(in super::super::super) mod tests;
