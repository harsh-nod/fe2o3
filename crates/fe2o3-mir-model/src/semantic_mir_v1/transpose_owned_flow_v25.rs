//! Homogeneous V25 source-flow footer. These are inert records; the live
//! provider and all consuming ownership views must independently replay them.
use super::*;

#[path = "transpose_owned_flow_v25/validate.rs"]
mod validate;

impl InertSemanticMirRequestV1 {
    pub fn with_transpose_owned_flows_v25(
        mut self,
        rows: Vec<SemanticTransposeOwnedFlowV1>,
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if !self.transpose_owned_flows.is_empty() {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        let mut work = limits.limit(SemanticMirResourceV1::ValidationWork);
        validate::rows(
            &self.types,
            &self.functions,
            &self.callables,
            &rows,
            &mut work,
            limits,
        )?;
        self.transpose_owned_flows = rows.into_boxed_slice();
        Ok(self)
    }

    pub fn admit_exact_v25(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        self.admit_for_wire_version(SemanticMirWireVersionV1::V25, limits)
    }
}

impl AdmittedInertSemanticMirV1 {
    /// Consumes the preliminary inert owner. All source/expansion borrows must
    /// end first, and production must replay against the returned final owner.
    /// This operation grants structural admission only, never source authority.
    pub fn with_transpose_owned_flows_v25(
        self,
        rows: Vec<SemanticTransposeOwnedFlowV1>,
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        self.with_transpose_owned_flows_for_wire_version(rows, SemanticMirWireVersionV1::V25, limits)
    }

    /// Explicit-version consuming composition. The V25 facade remains exact;
    /// no old envelope can carry a newer defined contract. Source replay must
    /// still be repeated against the returned final owner.
    pub fn with_transpose_owned_flows_for_wire_version(
        self,
        rows: Vec<SemanticTransposeOwnedFlowV1>,
        wire_version: SemanticMirWireVersionV1,
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if wire_version < SemanticMirWireVersionV1::V25 {
            return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: wire_version, required: SemanticMirWireVersionV1::V25,
            });
        }
        if !self.request.transpose_owned_flows.is_empty() {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        enforce_hard(SemanticMirResourceV1::Blocks, rows.len())?;
        let Self {
            mut request,
            canonical,
            ..
        } = self;
        // Do not retain the obsolete bytes while allocating the final encoding.
        drop(canonical);
        request.transpose_owned_flows = rows.into_boxed_slice();
        // One complete request/row validation under the supplied limits, not a
        // fresh per-row budget followed by an unchecked owner reconstruction.
        request.admit_for_wire_version(wire_version, limits)
    }

    pub fn transpose_owned_flows(&self) -> &[SemanticTransposeOwnedFlowV1] {
        &self.request.transpose_owned_flows
    }
}

pub(super) fn validate(context: &mut ValidationContextV1<'_>) -> Result<(), SemanticMirErrorV1> {
    let before = context
        .limits
        .limit(SemanticMirResourceV1::ValidationWork)
        .saturating_sub(context.work);
    let mut remaining = before;
    let result = validate::rows(
        &context.request.types,
        &context.request.functions,
        &context.request.callables,
        &context.request.transpose_owned_flows,
        &mut remaining,
        context.limits,
    );
    charge_validation_work(
        context,
        usize::try_from(before - remaining).map_err(|_| SemanticMirErrorV1::InvalidFunctionAbi)?,
    )?;
    result
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticOwnedSourceCallSiteV1 {
    pub function: SemanticFunctionIdV1,
    pub block: SemanticBlockIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticOwnedSourceStatementSiteV1 {
    pub function: SemanticFunctionIdV1,
    pub block: SemanticBlockIdV1,
    pub statement: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticTransposeOwnedFlowSitesV1 {
    pub issue: SemanticOwnedSourceCallSiteV1,
    pub capture: SemanticOwnedSourceStatementSiteV1,
    pub capture_field: u32,
    pub matrix_call: SemanticOwnedSourceCallSiteV1,
    pub closure_call: SemanticOwnedSourceCallSiteV1,
    pub stage: SemanticOwnedSourceCallSiteV1,
    pub publish: SemanticOwnedSourceCallSiteV1,
    pub workgroup_local: SemanticLocalIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticSourceBodyBindingV25 {
    function: SemanticFunctionIdV1,
    identity: [u8; 32],
}

impl SemanticSourceBodyBindingV25 {
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransposeOwnedFlowV1 {
    sites: SemanticTransposeOwnedFlowSitesV1,
    workgroup_borrows: Box<[SemanticOwnedSourceCallSiteV1]>,
    bodies: [SemanticSourceBodyBindingV25; 3],
    source_binding: [u8; 32],
}

impl SemanticTransposeOwnedFlowV1 {
    /// Builds inert source commitments from the exact canonical body table.
    /// This is not an authenticated attachment: request admission validates the
    /// full flow and production must compare the complete live provider roster.
    /// `max_fragment_bytes` is shared across all three bodies, not reset per row
    /// or body by the caller; the returned count is charged by that owner.
    pub fn for_retained_source(
        functions: &[SemanticFunctionDeclV1],
        sites: SemanticTransposeOwnedFlowSitesV1,
        workgroup_borrows: Vec<SemanticOwnedSourceCallSiteV1>,
        source_binding: [u8; 32],
        max_fragment_bytes: u64,
    ) -> Result<(Self, usize), SemanticMirErrorV1> {
        let roles = [
            sites.issue.function,
            sites.closure_call.function,
            sites.stage.function,
        ];
        let mut bodies = [(SemanticFunctionIdV1(0), [0; 32]); 3];
        let mut remaining = max_fragment_bytes.min(HARD_MAX_CANONICAL_BYTES_V1);
        let mut total = 0usize;
        for (slot, function) in roles.into_iter().enumerate() {
            let body = functions
                .get(function.index() as usize)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            let (identity, bytes) = canonical_semantic_source_body_sha256_v25(body, remaining)?;
            remaining = remaining
                .checked_sub(
                    u64::try_from(bytes).map_err(|_| SemanticMirErrorV1::InvalidFunctionAbi)?,
                )
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            total = total
                .checked_add(bytes)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            bodies[slot] = (function, identity);
        }
        Ok((
            Self::from_encoded_parts(sites, workgroup_borrows, bodies, source_binding)?,
            total,
        ))
    }

    pub const fn sites(&self) -> SemanticTransposeOwnedFlowSitesV1 {
        self.sites
    }
    pub fn workgroup_borrows(&self) -> &[SemanticOwnedSourceCallSiteV1] {
        &self.workgroup_borrows
    }
    pub const fn bodies(&self) -> &[SemanticSourceBodyBindingV25; 3] {
        &self.bodies
    }
    pub const fn source_binding(&self) -> &[u8; 32] {
        &self.source_binding
    }
    pub const fn canonical_key(&self) -> SemanticOwnedSourceCallSiteV1 {
        self.sites.issue
    }

    // Called by the closed source constructor/decoder before any attachment.
    // Structural admission and live source replay remain separate mandatory
    // steps; this constructor is deliberately not public source authority.
    pub(super) fn from_encoded_parts(
        sites: SemanticTransposeOwnedFlowSitesV1,
        workgroup_borrows: Vec<SemanticOwnedSourceCallSiteV1>,
        bodies: [(SemanticFunctionIdV1, [u8; 32]); 3],
        source_binding: [u8; 32],
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(
            SemanticMirResourceV1::CallArguments,
            workgroup_borrows.len(),
        )?;
        let functions = [
            sites.issue.function,
            sites.closure_call.function,
            sites.stage.function,
        ];
        let call_site = |s: SemanticOwnedSourceCallSiteV1| {
            u64::from(s.function.index()) < HARD_MAX_FUNCTIONS_V1
                && u64::from(s.block.index()) < HARD_MAX_BLOCKS_V1
        };
        let invalid = source_binding == [0; 32]
            || sites.capture.function != functions[0]
            || sites.matrix_call.function != functions[0]
            || sites.publish.function != functions[0]
            || u64::from(sites.capture.block.index()) >= HARD_MAX_BLOCKS_V1
            || u64::from(sites.capture.statement) >= HARD_MAX_STATEMENTS_V1
            || u64::from(sites.capture_field) >= HARD_MAX_TYPES_V1
            || u64::from(sites.workgroup_local.index()) >= HARD_MAX_LOCALS_V1
            || [
                sites.issue,
                sites.matrix_call,
                sites.closure_call,
                sites.stage,
                sites.publish,
            ]
            .into_iter()
            .any(|site| !call_site(site))
            || functions
                .iter()
                .enumerate()
                .any(|(i, f)| functions[..i].contains(f))
            || bodies
                .iter()
                .zip(functions)
                .any(|((f, hash), expected)| *f != expected || *hash == [0; 32])
            || workgroup_borrows
                .iter()
                .any(|site| !call_site(*site) || site.function != functions[0])
            || workgroup_borrows.windows(2).any(|pair| pair[0] >= pair[1]);
        if invalid {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            sites,
            workgroup_borrows: workgroup_borrows.into_boxed_slice(),
            bodies: bodies
                .map(|(function, identity)| SemanticSourceBodyBindingV25 { function, identity }),
            source_binding,
        })
    }

    pub(super) fn encoded_length(&self) -> Result<usize, SemanticMirErrorV1> {
        self.workgroup_borrows
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(204))
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::CanonicalBytes,
            })
    }

    pub(super) fn encode(&self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        let s = self.sites;
        encode_call_site(writer, s.issue)?;
        writer.u32(s.capture.function.index())?;
        writer.u32(s.capture.block.index())?;
        writer.u32(s.capture.statement)?;
        writer.u32(s.capture_field)?;
        for site in [s.matrix_call, s.closure_call, s.stage, s.publish] {
            encode_call_site(writer, site)?;
        }
        writer.u32(s.workgroup_local.index())?;
        writer.count(self.workgroup_borrows.len())?;
        for site in &self.workgroup_borrows {
            encode_call_site(writer, *site)?;
        }
        for body in self.bodies {
            writer.u32(body.function.index())?;
            writer.identity(body.identity)?;
        }
        writer.identity(self.source_binding)
    }

    /// Root-closure function edges. The full flow validator separately checks
    /// block/local sites; operand types/callees derive from retained bodies.
    pub(super) fn visit_functions<E>(
        &self,
        mut visit: impl FnMut(SemanticFunctionIdV1) -> Result<(), E>,
    ) -> Result<(), E> {
        for body in self.bodies {
            visit(body.function)?;
        }
        Ok(())
    }
}

fn encode_call_site(
    writer: &mut CanonicalWriterV1,
    site: SemanticOwnedSourceCallSiteV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u32(site.function.index())?;
    writer.u32(site.block.index())
}

pub(super) fn encode_footer(
    writer: &mut CanonicalWriterV1,
    version: SemanticMirWireVersionV1,
    rows: &[SemanticTransposeOwnedFlowV1],
) -> Result<(), SemanticMirErrorV1> {
    if version < SemanticMirWireVersionV1::V25 {
        if rows.is_empty() {
            return Ok(());
        }
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: version,
            required: SemanticMirWireVersionV1::V25,
        });
    }
    enforce_hard(SemanticMirResourceV1::Blocks, rows.len())?;
    if rows
        .windows(2)
        .any(|pair| pair[0].canonical_key() >= pair[1].canonical_key())
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    writer.count(rows.len())?;
    for row in rows {
        row.encode(writer)?;
    }
    Ok(())
}
