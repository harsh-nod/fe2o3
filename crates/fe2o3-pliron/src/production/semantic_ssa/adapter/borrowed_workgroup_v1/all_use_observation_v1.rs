//! Bounded observations of the existing pass; no replay, graph or proof result.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticExecutionCapabilityOperationV1;
use std::ffi::OsStr;
use std::io::{self, Write};
use super::super::storage_observation_v1::Observation as StorageObservation;

#[path = "all_use_observation_v1/callable_abi_v1.rs"]
mod callable_abi_v1;
#[path = "all_use_observation_v1/workgroup_carrier_v1.rs"]
mod workgroup_carrier_v1;
mod carrier_site_v1;
pub(super) use carrier_site_v1::Observation as CarrierSite;

const TRACE_ENV: &str = "FE2O3_TRACE_CAPABILITY_CUSTODY";
const ROLE_ENV: &str = "FE2O3_TRACE_CAPABILITY_CUSTODY_ROLE";
const MAX_OWNED: usize = 16;
const MAX_TRACKED: usize = 64;
const MAX_STEPS: usize = 65_536;
const MAX_OUTPUT: usize = 32_768;
type Site = SemanticTransparentBorrowSiteV1;

// Query either completed output roster without allocating a diagnostic copy.
pub(super) trait AcceptedSites {
    fn contains_site(&self, site: &Site) -> bool;
}
impl AcceptedSites for BTreeSet<Site> {
    fn contains_site(&self, site: &Site) -> bool { self.contains(site) }
}
impl<T> AcceptedSites for BTreeMap<Site, T> {
    fn contains_site(&self, site: &Site) -> bool { self.contains_key(site) }
}

#[derive(Clone, Copy)]
struct Failure {
    site: Site,
    mode: &'static str,
}

#[derive(Clone, Copy)]
struct Tracked {
    index: usize,
    initial: bool,
    previous: bool,
    failure: Option<Failure>,
    component: Option<(bool, Option<usize>)>,
}

pub(super) struct Observation<'a> {
    scope: &'static str,
    view: Option<&'a SemanticExpandedRootV1>,
    callables: &'a [SemanticCallableDeclV1],
    owned: [u32; MAX_OWNED],
    owned_len: usize,
    tracked: [Option<Tracked>; MAX_TRACKED],
    len: usize,
    remaining: usize,
    truncated: bool,
    selected_site: Option<Site>,
}

impl<'a> Observation<'a> {
    pub(super) fn new(
        view: Option<&'a SemanticExpandedRootV1>,
        owned: impl Iterator<Item = u32>,
        subgroups: impl Iterator<Item = u32>,
        callables: &'a [SemanticCallableDeclV1],
        candidates: &[SemanticBorrowCandidateV1],
    ) -> Self {
        let setting = std::env::var_os(TRACE_ENV);
        let role = std::env::var_os(ROLE_ENV);
        let mut result = if role.as_deref() == Some(OsStr::new("site")) {
            let selection = view
                .filter(|_| setting.as_deref() == Some(OsStr::new("1")))
                .and_then(|view| StorageObservation::from_env(view.body()));
            Self::with_site_selection(view, candidates, setting.as_deref(), selection.as_ref())
        } else if role.as_deref() == Some(OsStr::new("workgroup")) {
            Self::with_workgroup_role(view, callables, candidates, setting.as_deref(), role.as_deref())
        } else if role.as_deref() == Some(OsStr::new("policy")) {
            Self::with_policy_role(view, callables, candidates, setting.as_deref(), role.as_deref())
        } else {
            Self::with_role(view, owned, subgroups, candidates,
                setting.as_deref(), role.as_deref())
        };
        if result.view.is_some() { result.callables = callables; }
        result
    }

    fn with_site_selection(
        view: Option<&'a SemanticExpandedRootV1>,
        candidates: &[SemanticBorrowCandidateV1],
        setting: Option<&OsStr>,
        selection: Option<&StorageObservation>,
    ) -> Self {
        let mut result = Self::with_setting(None, std::iter::empty(), &[], None);
        result.scope = "site-owned-type";
        if setting != Some(OsStr::new("1")) { return result; }
        let (Some(view), Some(selection)) = (view, selection) else { return result; };
        // Fixed body/coordinate checks precede the existing bounded candidate scan.
        if !result.step(16) { return result; }
        let Some((local, site)) = selection.selected_site(view.body()) else { return result; };
        let Some(SemanticStatementKindV1::Assign(assignment)) = view.body().blocks()
            .get(site.block as usize)
            .and_then(|block| block.statements().get(site.statement as usize))
            .map(|statement| statement.kind())
        else { return result; };
        let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind()
        else { return result; };
        if place.local().index() != local { return result; }
        result.view = Some(view);
        result.selected_site = Some(site);
        // The whole owned-type family retains aliases and component blockers;
        // selecting this diagnostic family grants no address transparency.
        if result.add_owned(place.ty().index()) {
            result.track_candidates(candidates);
        }
        result
    }

    pub(super) fn with_workgroup_role(
        view: Option<&'a SemanticExpandedRootV1>,
        callables: &'a [SemanticCallableDeclV1],
        candidates: &[SemanticBorrowCandidateV1],
        setting: Option<&OsStr>,
        role: Option<&OsStr>,
    ) -> Self {
        let mut result = Self::with_setting(None, std::iter::empty(), &[], None);
        result.scope = "workgroup";
        if setting != Some(OsStr::new("1")) || role != Some(OsStr::new("workgroup"))
            || view.is_none()
        { return result }
        result.view = view;
        result.callables = callables;
        for callable in callables {
            if !result.step(1) { break }
            if let SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
            } = callable {
                if let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                    workgroup, ..
                } = contract.operation() {
                    // Diagnostic selection only, not the production pair check.
                    if !result.add_owned(workgroup.index()) { break }
                }
            }
        }
        // An empty selected roster is itself observable. Do not suppress the
        // retained Borrow/type census when no Workgroup seed was selected.
        result.track_candidates(candidates);
        result
    }

    fn with_policy_role(
        view: Option<&'a SemanticExpandedRootV1>,
        callables: &'a [SemanticCallableDeclV1],
        candidates: &[SemanticBorrowCandidateV1],
        setting: Option<&OsStr>,
        role: Option<&OsStr>,
    ) -> Self {
        let mut result = Self::with_setting(None, std::iter::empty(), &[], None);
        result.scope = "policy";
        if setting != Some(OsStr::new("1")) || role != Some(OsStr::new("policy"))
            || view.is_none()
        { return result }
        result.view = view;
        result.callables = callables;
        // Charge every roster entry, including non-Policy callables. No hidden
        // unbounded filter scan and no separate/reset diagnostic allowance.
        for callable in callables {
            if !result.step(1) { break }
            if let SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
            } = callable {
                if let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { capability, .. } = contract.operation() {
                    if !result.add_owned(capability.index()) { break }
                }
            }
        }
        if result.owned_len == 0 { result.view = None; return result }
        result.track_candidates(candidates);
        result
    }

    pub(super) fn for_lanes(
        view: Option<&'a SemanticExpandedRootV1>,
        lanes: impl Iterator<Item = u32>,
        callables: &'a [SemanticCallableDeclV1],
        candidates: &[SemanticBorrowCandidateV1],
    ) -> Self {
        let setting = std::env::var_os(TRACE_ENV);
        let role = std::env::var_os(ROLE_ENV);
        let mut result = Self::with_lane_role(view, lanes, candidates,
            setting.as_deref(), role.as_deref());
        if result.view.is_some() { result.callables = callables; }
        result
    }

    fn with_lane_role(
        view: Option<&'a SemanticExpandedRootV1>,
        lanes: impl Iterator<Item = u32>,
        candidates: &[SemanticBorrowCandidateV1],
        setting: Option<&OsStr>,
        role: Option<&OsStr>,
    ) -> Self {
        // The two graph scopes are mutually exclusive under the existing flag.
        // Disabled scopes return before consuming the owned-type iterator.
        let view = view.filter(|_| role == Some(OsStr::new("lane")));
        let mut result = Self::with_setting(view, lanes, candidates, setting);
        result.scope = "lane";
        result
    }

    fn with_role(
        view: Option<&'a SemanticExpandedRootV1>,
        matrix: impl Iterator<Item = u32>,
        subgroups: impl Iterator<Item = u32>,
        candidates: &[SemanticBorrowCandidateV1],
        setting: Option<&OsStr>,
        role: Option<&OsStr>,
    ) -> Self {
        if setting != Some(OsStr::new("1")) {
            return Self::with_setting(None, std::iter::empty(), candidates, None);
        }
        match role {
            None => Self::with_setting(view, matrix, candidates, setting),
            Some(role) if role == OsStr::new("matrix") =>
                Self::with_setting(view, matrix, candidates, setting),
            Some(role) if role == OsStr::new("subgroup") =>
                Self::with_setting(view, subgroups, candidates, setting),
            Some(_) => Self::with_setting(None, std::iter::empty(), candidates, None),
        }
    }

    fn with_setting(
        view: Option<&'a SemanticExpandedRootV1>,
        owned: impl Iterator<Item = u32>,
        candidates: &[SemanticBorrowCandidateV1],
        setting: Option<&OsStr>,
    ) -> Self {
        // Temporary opt-in diagnostics: disabled before any inventory or use scan.
        let view = view.filter(|_| setting == Some(OsStr::new("1")));
        let mut this = Self {
            scope: "capability",
            view,
            callables: &[],
            owned: [0; MAX_OWNED],
            owned_len: 0,
            tracked: [None; MAX_TRACKED],
            len: 0,
            remaining: MAX_STEPS,
            truncated: false,
            selected_site: None,
        };
        if view.is_none() {
            return this;
        }
        for ty in owned {
            if !this.add_owned(ty) { break }
        }
        if this.owned_len == 0 {
            this.view = None;
            return this;
        }
        this.track_candidates(candidates);
        this
    }

    fn add_owned(&mut self, ty: u32) -> bool {
        if !self.step(MAX_OWNED + 1) { return false }
        if self.owned[..self.owned_len].contains(&ty) { return true }
        if self.owned_len == MAX_OWNED {
            self.truncated = true;
            return false;
        }
        self.owned[self.owned_len] = ty;
        self.owned_len += 1;
        true
    }

    fn track_candidates(&mut self, candidates: &[SemanticBorrowCandidateV1]) {
        for (index, candidate) in candidates.iter().enumerate() {
            if !self.step(MAX_OWNED + 1) { break }
            if !self.owned[..self.owned_len].contains(&candidate.source_type.index()) { continue }
            if self.len == MAX_TRACKED {
                self.truncated = true;
                break;
            }
            self.tracked[self.len] = Some(Tracked {
                index, initial: candidate.valid, previous: candidate.valid,
                failure: None, component: None,
            });
            self.len += 1;
        }
    }

    fn step(&mut self, amount: usize) -> bool {
        if let Some(remaining) = self.remaining.checked_sub(amount) {
            self.remaining = remaining;
            true
        } else {
            self.remaining = 0;
            self.truncated = true;
            false
        }
    }

    pub(super) fn after(
        &mut self,
        site: Site,
        mode: &'static str,
        candidates: &[SemanticBorrowCandidateV1],
    ) {
        if self.view.is_none() || !self.step(1) {
            return;
        }
        for slot in 0..self.len {
            if !self.step(1) {
                return;
            }
            let tracked = self.tracked[slot].as_mut().unwrap();
            let Some(candidate) = candidates.get(tracked.index) else {
                self.truncated = true;
                return;
            };
            if tracked.previous && !candidate.valid && tracked.failure.is_none() {
                tracked.failure = Some(Failure { site, mode });
            }
            tracked.previous = candidate.valid;
        }
    }

    pub(super) fn component(
        &mut self,
        root: usize,
        blocker: Option<usize>,
        valid: bool,
        candidates: &[SemanticBorrowCandidateV1],
    ) {
        if self.view.is_none() || !self.step(1) {
            return;
        }
        if candidates.get(root).is_none() {
            self.truncated = true;
            return;
        }
        for slot in 0..self.len {
            if !self.step(1) {
                return;
            }
            let tracked = self.tracked[slot].as_mut().unwrap();
            if tracked.index == root {
                let first_blocker = tracked.component.and_then(|(_, old)| old).or(blocker);
                tracked.component = Some((valid, first_blocker));
                return;
            }
        }
    }

    pub(super) fn emit(
        &self,
        function: &SemanticFunctionDeclV1,
        candidates: &[SemanticBorrowCandidateV1],
        accepted: &impl AcceptedSites,
        route_owner: impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
    ) {
        self.emit_inner(function, candidates, accepted, route_owner, None);
    }

    pub(super) fn emit_with_routes(
        &self,
        function: &SemanticFunctionDeclV1,
        candidates: &[SemanticBorrowCandidateV1],
        accepted: &impl AcceptedSites,
        route_owner: impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
        routes: &math_capture_flow_v1::Routes<'_>,
        references: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    ) {
        self.emit_inner(function, candidates, accepted, route_owner, Some((routes, references)));
    }

    fn emit_inner(
        &self,
        function: &SemanticFunctionDeclV1,
        candidates: &[SemanticBorrowCandidateV1],
        accepted: &impl AcceptedSites,
        route_owner: impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
        routes: Option<(&math_capture_flow_v1::Routes<'_>, &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>)>,
    ) {
        let Some(view) = self.view else {
            return;
        };
        if !std::ptr::eq(view.body(), function) {
            return;
        }
        let mut buffer = Output {
            bytes: [0; MAX_OUTPUT],
            len: 0,
            truncated: false,
        };
        let _ = self.write_with_routes(&mut buffer, view, candidates, accepted, route_owner, routes);
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(&buffer.bytes[..buffer.len]);
        let _ = writeln!(
            stderr,
            "matrix-all-use-end root={} payload_truncated={} diagnostic_only=true",
            view.root().index(),
            buffer.truncated
        );
    }

    #[cfg(test)]
    fn write(
        &self,
        out: &mut impl Write,
        view: &SemanticExpandedRootV1,
        candidates: &[SemanticBorrowCandidateV1],
        accepted: &impl AcceptedSites,
        route_owner: impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
    ) -> io::Result<()> {
        self.write_with_routes(out, view, candidates, accepted, route_owner, None)
    }

    pub(super) fn write_with_routes(
        &self,
        out: &mut impl Write,
        view: &SemanticExpandedRootV1,
        candidates: &[SemanticBorrowCandidateV1],
        accepted: &impl AcceptedSites,
        mut route_owner: impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
        routes: Option<(&math_capture_flow_v1::Routes<'_>, &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>)>,
    ) -> io::Result<()> {
        write!(
            out,
            "matrix-all-use root={} expanded_root_identity=",
            view.root().index()
        )?;
        for byte in view.identity() {
            write!(out, "{byte:02x}")?;
        }
        writeln!(
            out,
            " pass_complete=true scope={} candidates={} selected={} owned_types={:?} observation_truncated={} diagnostic_remaining={} diagnostic_limit={} diagnostic_only=true",
            self.scope,
            candidates.len(),
            self.len,
            &self.owned[..self.owned_len],
            self.truncated,
            self.remaining,
            MAX_STEPS
        )?;
        // Supplemental ABI inspection shares the unused diagnostic work; it
        // never borrows, resets or refunds the production proof budget.
        let mut abi_remaining = self.remaining;
        if let Some(site) = self.selected_site {
            writeln!(out, "selected_borrow_site=({}, {})", site.block, site.statement)?;
            write_site(out, view, site, self.callables, &mut abi_remaining, &mut route_owner)?;
        }
        if self.scope == "workgroup" && self.view.is_some_and(|owned| std::ptr::eq(owned, view)) {
            if let Some((routes, references)) = routes {
                workgroup_carrier_v1::write(out, view, candidates, routes, references, &mut abi_remaining)?;
            }
        }
        for tracked in self.tracked[..self.len].iter().flatten() {
            let Some(c) = candidates.get(tracked.index) else {
                continue;
            };
            let source_local = view.local_origins().get(c.source_local as usize).map(|o| {
                (
                    o.instance().index(),
                    o.function().index(),
                    o.local().index(),
                )
            });
            let reference = view
                .body()
                .blocks()
                .get(c.site.block as usize)
                .and_then(|b| b.statements().get(c.site.statement as usize))
                .and_then(|s| match s.kind() {
                    SemanticStatementKindV1::Assign(a) => Some((
                        a.destination().local().index(),
                        a.destination().ty().index(),
                    )),
                    _ => None,
                });
            writeln!(
                out,
                "candidate={} owner={} owned_ty={} owner_origin={source_local:?} reference={reference:?} parent_reference={:?} alias={} initial_valid={} final_valid={} consumers={} intrinsic={} borrow_site=({}, {}) accepted_borrow={} component={:?}",
                tracked.index,
                c.source_local,
                c.source_type.index(),
                c.source_reference,
                c.value_alias,
                tracked.initial,
                c.valid,
                c.consumers,
                c.intrinsic_consumer,
                c.site.block,
                c.site.statement,
                accepted.contains_site(&c.site),
                tracked.component
            )?;
            if let Some(failure) = tracked.failure {
                writeln!(
                    out,
                    "first_observed_invalidation candidate={} mode={}",
                    tracked.index, failure.mode
                )?;
                write_site(out, view, failure.site, self.callables, &mut abi_remaining, &mut route_owner)?;
            }
        }
        Ok(())
    }
}

fn write_site(
    out: &mut impl Write,
    view: &SemanticExpandedRootV1,
    site: Site,
    callables: &[SemanticCallableDeclV1],
    abi_remaining: &mut usize,
    route_owner: &mut impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
) -> io::Result<()> {
    let origin = view.block_origins().get(site.block as usize);
    writeln!(
        out,
        "point=({}, {}) source_block={:?} source_statement={:?}",
        site.block,
        site.statement,
        origin.map(|o| (
            o.instance().index(),
            o.function().index(),
            o.block().index()
        )),
        origin.and_then(|o| o.statements().get(site.statement as usize))
    )?;
    let Some(block) = view.body().blocks().get(site.block as usize) else {
        return Ok(());
    };
    if let Some(statement) = block.statements().get(site.statement as usize) {
        writeln!(
            out,
            "statement_kind={:?}",
            std::mem::discriminant(statement.kind())
        )?;
        if let SemanticStatementKindV1::Assign(a) = statement.kind() {
            write_place(out, "destination", view, a.destination(), route_owner)?;
            writeln!(
                out,
                "rvalue_kind={:?} result_ty={}",
                std::mem::discriminant(a.value().kind()),
                a.value().result_type().index()
            )?;
            let mut count = 0;
            let complete = a
                .value()
                .kind()
                .try_visit_operands(|operand| {
                    if count == 4 {
                        return Err(io::ErrorKind::Interrupted.into());
                    }
                    count += 1;
                    write_operand(out, view, operand, route_owner)
                })
                .is_ok();
            writeln!(out, "operand_prefix_truncated={}", !complete)?;
            if let SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. } = a.value().kind()
            {
                write_place(out, "borrow-place", view, place, route_owner)?;
            }
        }
    } else {
        writeln!(
            out,
            "terminator_kind={:?} origin={:?}",
            std::mem::discriminant(block.terminator().kind()),
            origin.map(|o| o.terminator())
        )?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            writeln!(
                out,
                "callee={} argument_count={}",
                call.callee().index(),
                call.arguments().len()
            )?;
            callable_abi_v1::write(out, call.callee().index(), callables, abi_remaining)?;
            for operand in call.arguments().iter().take(4) {
                write_operand(out, view, operand, route_owner)?;
            }
            writeln!(
                out,
                "operand_prefix_truncated={}",
                call.arguments().len() > 4
            )?;
        }
    }
    Ok(())
}

fn write_operand(
    out: &mut impl Write,
    view: &SemanticExpandedRootV1,
    operand: &SemanticOperandV1,
    route_owner: &mut impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
) -> io::Result<()> {
    match operand {
        SemanticOperandV1::Copy(place) => write_place(out, "Copy", view, place, route_owner),
        SemanticOperandV1::Move(place) => write_place(out, "Move", view, place, route_owner),
        SemanticOperandV1::Constant(_) => writeln!(out, "Constant ty={}", operand.ty().index()),
    }
}

fn write_place(
    out: &mut impl Write,
    label: &str,
    view: &SemanticExpandedRootV1,
    place: &SemanticPlaceV1,
    route_owner: &mut impl FnMut(SemanticTypeIdV1) -> Option<SemanticTypeIdV1>,
) -> io::Result<()> {
    let root_type = view
        .body()
        .locals()
        .get(place.local().index() as usize)
        .map(|l| l.ty());
    writeln!(
        out,
        "{label} local={} ty={} root_ty={:?} root_route_owned={:?} projections={} prefix={:?} projection_prefix_truncated={}",
        place.local().index(),
        place.ty().index(),
        root_type.map(|t| t.index()),
        root_type.and_then(route_owner).map(|t| t.index()),
        place.projections().len(),
        &place.projections()[..place.projections().len().min(4)],
        place.projections().len() > 4
    )
}

struct Output {
    bytes: [u8; MAX_OUTPUT],
    len: usize,
    truncated: bool,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.bytes.len() - self.len {
            self.truncated = true;
            return Err(io::ErrorKind::WriteZero.into());
        }
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "all_use_observation_v1/tests.rs"]
mod tests;
