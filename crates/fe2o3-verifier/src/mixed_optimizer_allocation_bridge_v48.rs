//! Exact allocation-register transport through the checked prefix and LICM.
//! Event-time source maps read these registers even when they are not an
//! operation's payload. No pointer is recovered from final heap contents.
use super::byte_function_v30::{ByteAllocationResolverV30, ByteAllocationSiteV30};
use super::*;
use fe2o3_kernel_analysis::{
    CheckedCanonicalKirLicmV18 as Licm, CheckedCanonicalKirTransitionV18 as Prefix,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::cell::Cell;

const EVENT_PRELUDE: &str = include_str!("mixed_optimizer_typed_event_bridge_v48.vrs");

#[path = "mixed_optimizer_typed_prefix_v49.rs"]
mod prefix_v49;
pub(super) use prefix_v49::PrefixSegmentsV49;
#[path = "mixed_optimizer_typed_licm_bridge_v48.rs"]
mod relocation_v48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AllocationSideV48 {
    Original,
    Prefix,
    Relocated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationRowV48 {
    site: ByteAllocationSiteV30,
    definitions: [usize; 3],
}

pub(super) struct AllocationBridgeV48<'a, 'owner, 'rows, R> {
    prefix: &'a Prefix<'a, 'owner, 'owner, 'rows>,
    licm: &'a Licm<'owner>,
    output: &'a Inventory<'owner>,
    source: &'a R,
    rows: Vec<Option<AllocationRowV48>>,
    original_rows: Vec<usize>,
    prefix_rows: Vec<usize>,
    prefix_origins: Vec<usize>,
    relocated_origins: Vec<usize>,
    required: usize,
    slot: usize,
    ledger: Ledger,
    failure: Cell<Option<Resource>>,
}

pub(super) struct AllocationViewV48<'b, 'a, 'owner, 'rows, R> {
    bridge: &'b AllocationBridgeV48<'a, 'owner, 'rows, R>,
    side: AllocationSideV48,
}

fn mismatch() -> Error {
    Error::Statement("typed allocation environment differs from exact prefix/LICM owners")
}

impl<'a, 'owner, 'rows, R: ByteAllocationResolverV30> AllocationBridgeV48<'a, 'owner, 'rows, R> {
    pub(super) fn derive(
        prefix: &'a Prefix<'a, 'owner, 'owner, 'rows>,
        licm: &'a Licm<'owner>,
        output: &'a Inventory<'owner>,
        source: &'a R,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        source.check_owner(prefix.input().owner(), out)?;
        out.budget.charge_work(3)?;
        if !std::ptr::eq(prefix.output().owner(), licm.input()) || !output.belongs_to(licm.output())
        {
            return Err(mismatch());
        }
        out.budget.reserve_storage(
            size_of::<Self>()
                + size_of::<Result<Self>>()
                + 3 * (size_of::<AllocationViewV48<'_, '_, '_, '_, R>>()
                    + size_of::<Result<AllocationViewV48<'_, '_, '_, '_, R>>>())
                + size_of::<([usize; 8], [&(); 8], AllocationRowV48)>(),
        )?;
        let mut original_rows = allocate(prefix.input().operations().len(), out)?;
        let mut prefix_rows = allocate(prefix.output().operations().len(), out)?;
        let mut prefix_origins = allocate(prefix.output().operations().len(), out)?;
        let mut relocated_origins = allocate(output.operations().len(), out)?;
        let count = output.operations().len();
        out.budget.reserve_storage(
            count
                .checked_mul(size_of::<Option<AllocationRowV48>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        out.budget.reserve_storage(
            rows.capacity()
                .checked_sub(count)
                .ok_or(Resource::Accounting)?
                .checked_mul(size_of::<Option<AllocationRowV48>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let scratch = out.budget.storage();
        let built = (|| {
            let mut middle_operations = allocate(output.operations().len(), out)?;
            if licm.origins().len() != prefix.output().operations().len()
                || prefix.rows().operations.len() != prefix.output().operations().len()
            {
                return Err(mismatch());
            }
            for (ordinal, origin) in prefix.rows().operations.iter().enumerate() {
                out.budget.charge_work(4)?;
                let row = &prefix.output().operations()[ordinal];
                if origin.output != row.coordinate {
                    return Err(mismatch());
                }
                prefix_origins[ordinal] = match origin.origin {
                    Origin::Retained(site) => operation_index(prefix.input(), site)?,
                    Origin::ConstantFrom(definition) => {
                        definition_index(prefix.input(), definition)?;
                        if !matches!(row.operation.kind, OperationKind::Constant(_))
                            || row.results.len() != 1
                        {
                            return Err(mismatch());
                        }
                        NONE
                    }
                };
            }
            for (middle, origin) in licm.origins().iter().enumerate() {
                out.budget.charge_work(5)?;
                let target = operation_index(output, origin.output)?;
                if origin.input != prefix.output().operations()[middle].coordinate
                    || middle_operations[target] != NONE
                {
                    return Err(mismatch());
                }
                middle_operations[target] = middle;
                relocated_origins[target] = prefix_origins[middle];
            }
            out.budget.charge_work(middle_operations.len())?;
            if middle_operations.contains(&NONE) {
                return Err(mismatch());
            }
            let mut sites = Vec::new();
            out.budget.reserve_storage(
                size_of::<Vec<(usize, Operation)>>()
                    + count
                        .checked_mul(size_of::<(usize, Operation)>())
                        .ok_or(Resource::Arithmetic)?,
            )?;
            sites
                .try_reserve_exact(count)
                .map_err(|_| Resource::Allocation)?;
            out.budget.reserve_storage(
                sites
                    .capacity()
                    .checked_sub(count)
                    .ok_or(Resource::Accounting)?
                    .checked_mul(size_of::<(usize, Operation)>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            for (ordinal, final_row) in output.operations().iter().enumerate() {
                out.budget.charge_work(12)?;
                if !matches!(final_row.operation.kind, OperationKind::Alloca { .. }) {
                    rows.push(None);
                    continue;
                }
                let middle = *middle_operations.get(ordinal).ok_or_else(mismatch)?;
                let prefix_origin = prefix.rows().operations.get(middle).ok_or_else(mismatch)?;
                let Origin::Retained(original_site) = prefix_origin.origin else {
                    return Err(mismatch());
                };
                let original = operation_index(prefix.input(), original_site)?;
                let input_row = prefix
                    .input()
                    .operations()
                    .get(original)
                    .ok_or_else(mismatch)?;
                let prefix_row = prefix
                    .output()
                    .operations()
                    .get(middle)
                    .ok_or_else(mismatch)?;
                if input_row.operation.kind != final_row.operation.kind
                    || prefix_row.operation.kind != final_row.operation.kind
                    || input_row.results.len() != 1
                    || prefix_row.results.len() != 1
                    || final_row.results.len() != 1
                    || prefix_origin.output != prefix_row.coordinate
                    || licm.origins()[middle].hoist.is_some()
                    || prefix.input().definitions()[input_row.results.start].ty
                        != output.definitions()[final_row.results.start].ty
                    || prefix.output().definitions()[prefix_row.results.start].ty
                        != output.definitions()[final_row.results.start].ty
                    || original_rows[original] != NONE
                    || prefix_rows[middle] != NONE
                {
                    return Err(mismatch());
                }
                let descendants = descendants(prefix.rows(), input_row.results.start)?;
                let mut retained = 0usize;
                for descendant in descendants {
                    out.budget.charge_work(3)?;
                    if descendant.kind == Descendant::Retained {
                        if descendant.output
                            != prefix.output().definitions()[prefix_row.results.start].coordinate
                        {
                            return Err(mismatch());
                        }
                        retained = retained.checked_add(1).ok_or(Resource::Arithmetic)?;
                    }
                }
                if retained != 1 {
                    return Err(mismatch());
                }
                let site = source.site(input_row.coordinate, out)?;
                original_rows[original] = ordinal;
                prefix_rows[middle] = ordinal;
                sites.push((site.physical_root_owner, site.original));
                rows.push(Some(AllocationRowV48 {
                    site,
                    definitions: [
                        input_row.results.start,
                        prefix_row.results.start,
                        final_row.results.start,
                    ],
                }));
            }
            for (inventory, mapping) in [
                (prefix.input(), &original_rows),
                (prefix.output(), &prefix_rows),
            ] {
                for (row, mapped) in inventory.operations().iter().zip(mapping) {
                    out.budget.charge_work(2)?;
                    if matches!(row.operation.kind, OperationKind::Alloca { .. })
                        != (*mapped != NONE)
                    {
                        return Err(mismatch());
                    }
                }
            }
            out.budget.charge_work(
                sites
                    .len()
                    .checked_mul(sites.len().checked_ilog2().unwrap_or(0) as usize + 2)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            sites.sort_unstable();
            for adjacent in sites.windows(2) {
                out.budget.charge_work(1)?;
                if adjacent[0] == adjacent[1] {
                    return Err(mismatch());
                }
            }
            drop((sites, middle_operations));
            Ok(())
        })();
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(scratch)
                .ok_or(Resource::Accounting)?,
        )?;
        built?;
        source.check_owner(prefix.input().owner(), out)?;
        Ok(Self {
            prefix,
            licm,
            output,
            source,
            rows,
            original_rows,
            prefix_rows,
            prefix_origins,
            relocated_origins,
            required: out.budget.storage(),
            slot: std::ptr::from_ref(out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            failure: Cell::new(None),
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        if self.slot != std::ptr::from_ref(out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || out.budget.storage() < self.required
        {
            self.failure.set(Some(Resource::Accounting));
            return Err(Resource::Accounting.into());
        }
        self.source.check_owner(self.prefix.input().owner(), out)
    }

    fn charge(&self, work: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Err(error) = out.budget.charge_work(work) {
            self.failure.set(Some(error));
            return Err(error.into());
        }
        Ok(())
    }

    fn inventory(&self, side: AllocationSideV48) -> &Inventory<'owner> {
        match side {
            AllocationSideV48::Original => self.prefix.input(),
            AllocationSideV48::Prefix => self.prefix.output(),
            AllocationSideV48::Relocated => self.output,
        }
    }

    pub(super) fn view(
        &self,
        side: AllocationSideV48,
        out: &mut Writer<'_, '_>,
    ) -> Result<AllocationViewV48<'_, 'a, 'owner, 'rows, R>> {
        self.check(out)?;
        Ok(AllocationViewV48 { bridge: self, side })
    }

    // This environment includes Undefined values before an allocation executes.
    // It preserves the exact event-time register value, not a guessed live cell.
    fn emit_environment(
        &self,
        side: AllocationSideV48,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let side_index = match side {
            AllocationSideV48::Original => 0,
            AllocationSideV48::Prefix => 1,
            AllocationSideV48::Relocated => 2,
        };
        emit!(
            out,
            "spec fn typed_allocation_environment_{namespace}_v48(s: MemoryStateV30) -> Map<MemoryPrivateSiteV30, MemoryValueV30> {{\n let environment = Map::empty();\n"
        );
        for row in &self.rows {
            self.charge(2, out)?;
            let Some(row) = row else {
                continue;
            };
            let definition = row.definitions[side_index];
            let site = row.site;
            emit!(
                out,
                " let environment = environment.insert(MemoryPrivateSiteV30 {{ owner: {}, invocation: 0, site: MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }} }}, if {definition} < s.values.len() {{ s.values[{definition}] }} else {{ MemoryValueV30::Undefined }});\n",
                site.physical_root_owner,
                site.original.block.function.0,
                site.original.block.block,
                site.original.operation
            );
        }
        emit!(
            out,
            " environment\n}}\nspec fn typed_allocation_environment_shape_{namespace}_v48(s: MemoryStateV30) -> bool {{ s.values.len() == {} }}\n",
            self.inventory(side).definitions().len()
        );
        self.check(out)
    }

    fn emit_observation_projection(
        &self,
        side: AllocationSideV48,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        self.emit_environment(side, namespace, out)?;
        emit!(
            out,
            "spec fn typed_original_operation_{namespace}_v48(operation: MemorySourceOperationV30) -> Option<int> {{\n"
        );
        for (ordinal, row) in self.inventory(side).operations().iter().enumerate() {
            self.charge(2, out)?;
            let original = match side {
                AllocationSideV48::Original => ordinal,
                AllocationSideV48::Prefix => self.prefix_origins[ordinal],
                AllocationSideV48::Relocated => self.relocated_origins[ordinal],
            };
            if original == NONE {
                continue;
            }
            let coordinate = row.coordinate;
            emit!(
                out,
                " if operation == (MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}) {{ Some({original}) }} else\n",
                coordinate.block.function.0,
                coordinate.block.block,
                coordinate.operation
            );
        }
        emit!(out, " {{ None }}\n}}\n");
        emit!(
            out,
            "spec fn typed_event_state_{namespace}_v48(s: MemoryStateV30) -> TypedEventStateV48 {{ TypedEventStateV48 {{ memory: s.memory, generations: s.generations, frames: s.frames, allocations: typed_allocation_environment_{namespace}_v48(s), valid: s.valid }} }}\n"
        );
        emit!(
            out,
            "spec fn typed_observation_{namespace}_v48(observation: MemoryOperationObservationV30) -> Option<Seq<TypedMemoryObservationV48>> {{\n if !byte_observation_snapshots_valid_v39(observation) || !typed_allocation_environment_shape_{namespace}_v48(observation.before) || !typed_allocation_environment_shape_{namespace}_v48(observation.after) {{ None }} else {{\n let before = typed_event_state_{namespace}_v48(observation.before);\n let after = typed_event_state_{namespace}_v48(observation.after);\n if typed_observation_erases_pure_v48(observation, before, after) {{ Some(Seq::empty()) }} else {{ match typed_original_operation_{namespace}_v48(observation.operation) {{ Some(original) => Some(seq![TypedMemoryObservationV48 {{ original, before, after, effect: observation.effect }}]), None => None }} }}\n }}\n}}\n"
        );
        emit!(
            out,
            "spec fn typed_observations_{namespace}_v48(observations: Seq<MemoryOperationObservationV30>) -> Option<Seq<TypedMemoryObservationV48>>\n decreases observations.len(),\n{{ if observations.len() == 0 {{ Some(Seq::empty()) }} else {{ match (typed_observation_{namespace}_v48(observations[0]), typed_observations_{namespace}_v48(observations.drop_first())) {{ (Some(head), Some(tail)) => Some(head + tail), _ => None }} }} }}\n"
        );
        self.check(out)
    }

    fn emit_original_register_dependencies(
        &self,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        emit!(
            out,
            "proof fn typed_original_allocation_registers_{namespace}_v48(a: MemoryStateV30, b: MemoryStateV30)\n requires typed_allocation_environment_shape_{namespace}_v48(a), typed_allocation_environment_shape_{namespace}_v48(b), typed_allocation_environment_{namespace}_v48(a) == typed_allocation_environment_{namespace}_v48(b),\n ensures true"
        );
        for row in self.rows.iter().flatten() {
            self.charge(1, out)?;
            let definition = row.definitions[0];
            emit!(out, " && a.values[{definition}] == b.values[{definition}]");
        }
        emit!(out, ",\n{{\n");
        for row in self.rows.iter().flatten() {
            self.charge(2, out)?;
            let definition = row.definitions[0];
            let site = row.site;
            emit!(
                out,
                " let site = MemoryPrivateSiteV30 {{ owner: {}, invocation: 0, site: MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }} }};\n assert(typed_allocation_environment_{namespace}_v48(a).contains_key(site));\n assert(typed_allocation_environment_{namespace}_v48(a)[site] == a.values[{definition}]);\n assert(typed_allocation_environment_{namespace}_v48(b)[site] == b.values[{definition}]);\n",
                site.physical_root_owner,
                site.original.block.function.0,
                site.original.block.block,
                site.original.operation
            );
        }
        emit!(out, "}}\n");
        self.check(out)
    }

    // This projection is only an input to source-map predicates. It is not an
    // original execution state, a cut relation, or an initializedness witness.
    fn emit_original_map_view(
        &self,
        side: AllocationSideV48,
        namespace: usize,
        original_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let side_index = match side {
            AllocationSideV48::Original => 0,
            AllocationSideV48::Prefix => 1,
            AllocationSideV48::Relocated => 2,
        };
        emit!(
            out,
            "spec fn typed_original_map_view_{namespace}_v48(s: MemoryStateV30) -> MemoryStateV30 {{\n let values = Seq::new({}, |definition: int|\n",
            self.prefix.input().definitions().len()
        );
        for row in self.rows.iter().flatten() {
            self.charge(2, out)?;
            let original = row.definitions[0];
            let actual = row.definitions[side_index];
            emit!(
                out,
                " if definition == {original} && {actual} < s.values.len() {{ s.values[{actual}] }} else\n"
            );
        }
        emit!(
            out,
            " {{ MemoryValueV30::Undefined }});\n MemoryStateV30 {{ values, ..s }}\n}}\n"
        );
        emit!(
            out,
            "proof fn typed_original_map_view_exact_{namespace}_v48(s: MemoryStateV30)\n requires typed_allocation_environment_shape_{namespace}_v48(s),\n ensures typed_allocation_environment_shape_{original_namespace}_v48(typed_original_map_view_{namespace}_v48(s)),\n typed_allocation_environment_{original_namespace}_v48(typed_original_map_view_{namespace}_v48(s)) == typed_allocation_environment_{namespace}_v48(s),\n typed_original_map_view_{namespace}_v48(s).memory == s.memory,\n typed_original_map_view_{namespace}_v48(s).generations == s.generations,\n typed_original_map_view_{namespace}_v48(s).frames == s.frames,\n typed_original_map_view_{namespace}_v48(s).valid == s.valid,\n{{\n assert(typed_allocation_environment_{original_namespace}_v48(typed_original_map_view_{namespace}_v48(s)) =~= typed_allocation_environment_{namespace}_v48(s));\n}}\n"
        );
        self.check(out)
    }

    pub(super) fn emit_transport(
        &self,
        namespaces: [usize; 3],
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        self.charge(4, out)?;
        if namespaces[0] == namespaces[1]
            || namespaces[0] == namespaces[2]
            || namespaces[1] == namespaces[2]
        {
            return Err(mismatch());
        }
        emit!(out, "{EVENT_PRELUDE}\n");
        for (side, namespace) in [
            (AllocationSideV48::Original, namespaces[0]),
            (AllocationSideV48::Prefix, namespaces[1]),
            (AllocationSideV48::Relocated, namespaces[2]),
        ] {
            self.emit_observation_projection(side, namespace, out)?;
            self.emit_original_map_view(side, namespace, namespaces[0], out)?;
        }
        self.emit_original_register_dependencies(namespaces[0], out)?;
        self.check(out)
    }
}

impl<R: ByteAllocationResolverV30> ByteAllocationResolverV30
    for AllocationViewV48<'_, '_, '_, '_, R>
{
    fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        self.bridge.check(out)?;
        self.bridge.charge(3, out)?;
        if !std::ptr::eq(owner, self.bridge.inventory(self.side).owner())
            || !std::ptr::eq(
                self.bridge.prefix.output().owner(),
                self.bridge.licm.input(),
            )
            || !self.bridge.output.belongs_to(self.bridge.licm.output())
        {
            return Err(mismatch());
        }
        Ok(())
    }

    fn site(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<ByteAllocationSiteV30> {
        self.bridge.check(out)?;
        let inventory = self.bridge.inventory(self.side);
        self.bridge.charge(
            inventory.operations().len().checked_ilog2().unwrap_or(0) as usize + 3,
            out,
        )?;
        let ordinal = inventory
            .operations()
            .binary_search_by_key(&operation, |row| row.coordinate)
            .map_err(|_| mismatch())?;
        let output = match self.side {
            AllocationSideV48::Original => self.bridge.original_rows[ordinal],
            AllocationSideV48::Prefix => self.bridge.prefix_rows[ordinal],
            AllocationSideV48::Relocated => ordinal,
        };
        self.bridge
            .rows
            .get(output)
            .copied()
            .flatten()
            .map(|row| row.site)
            .ok_or_else(mismatch)
    }
}

#[cfg(test)]
#[path = "mixed_optimizer_allocation_bridge_v48_tests.rs"]
mod tests;
