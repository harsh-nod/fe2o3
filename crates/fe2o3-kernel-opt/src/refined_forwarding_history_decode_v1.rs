//! Move-only independently admitted graph/row backing; no self-borrowed receipt.
use crate::{
    CanonicalPolicy5SemanticInputsV1 as P5, CanonicalPolicy6ContinuationClaimsV1 as P6Claims,
    CanonicalPolicy6SemanticInputsV1 as P6, CanonicalPolicy7SemanticInputsV1 as P7,
    CanonicalPolicy8ContinuationClaimsV1 as P8Claims, CanonicalPolicy8SemanticInputsV1 as P8,
    CanonicalRefinedForwardingHistoryErrorV1 as SemanticError,
    CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    CheckedCanonicalRefinedForwardingHistoryV1 as Checked, DecodedCanonicalPolicy7RowsV1 as P7Rows,
    POLICY8_COMMUTATIVE_PASS_NAME_V1 as PASS, check_canonical_refined_forwarding_history_v1,
    decode_canonical_policy7_rows_v1, private_cell_promotion_resources_v1 as resources,
    refined_forwarding_history_rows_v1 as rows,
    refined_forwarding_history_wire_v1::{
        Error, InertRefinedForwardingHistoryRefV1 as Frame, RefinedForwardingHistoryRoleV1 as Role,
        RefinedForwardingHistoryWireStorageV1 as Storage, add, configured,
    },
};
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingOriginV1 as Forwarding,
    CanonicalKirInductionRefinementOriginV1 as Refinement, CanonicalKirLicmOriginV1 as Licm,
    CanonicalKirLoadForwardingRowV1 as Load, CanonicalKirLoopPreheaderV1 as Preheader,
    CanonicalKirPrivateCellOriginV1 as Promotion,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayStorageV12, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Site,
    InertCanonicalKirTransitionGraphIdentityV1 as Identity,
    InertOwnedCanonicalKirOccurrenceRowsV1 as TailRows,
    VerifiedCanonicalKernelIrModuleV12 as Owner, materialize_canonical_kir_occurrence_rows_v1,
    read_canonical_kir_occurrence_row_bytes_v1,
};
use std::mem::size_of;

const ROLES: [Role; 12] = [
    Role::B,
    Role::C,
    Role::S,
    Role::O,
    Role::I,
    Role::J,
    Role::K,
    Role::P,
    Role::H,
    Role::L,
    Role::R,
    Role::F,
];

/// Fresh graph admission and row syntax only; source proof is a separate input to
/// a later verifier. This cannot mint execution, ABI, descriptor or launch authority.
/// Receipts borrow this owner briefly and are never stored inside it.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedRefinedForwardingHistoryV1 as Owner;
/// fn copy(o: Owner<'_, '_>) { let _ = o.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedRefinedForwardingHistoryV1 as Owner,
///     CheckedCanonicalRefinedForwardingHistoryV1 as Receipt};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape<'a>(o: Owner<'_, '_>, b: &mut Budget<'_>) -> Receipt<'a> {
///     o.check_semantics(b).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedRefinedForwardingHistoryV1 as Owner;
/// fn detach<'a, 'w>(o: Owner<'a, 'w>) -> Owner<'static, 'w> { o }
/// ```
pub struct DecodedRefinedForwardingHistoryV1<'frame, 'wire> {
    frame: &'frame Frame<'wire>,
    graphs: Vec<Owner>,
    final_graph_storage: CanonicalKernelIrReplayStorageV12,
    loads: Vec<Load>,
    policy7: P7Rows<'wire>,
    tail: TailRows,
    selected: Vec<Site>,
    promotion: Vec<Promotion>,
    preheaders: Vec<Preheader>,
    licm: Vec<Licm>,
    refinement: Vec<Refinement>,
    forwarding: Vec<Forwarding>,
    storage: Storage,
}
impl<'frame, 'wire> DecodedRefinedForwardingHistoryV1<'frame, 'wire> {
    pub fn graph(&self, role: Role) -> &Owner {
        &self.graphs[role as usize]
    }
    /// Moves the actual freshly admitted F graph without re-admission or allocation.
    /// This alone establishes no history semantics, source relation or authority.
    ///
    /// After consumption drops all other graphs and rows, the caller transfers its
    /// existing reservation H by releasing H minus the returned graph receipt G.
    /// G remains reserved; do not reserve it a second time. Frame/wire reservations
    /// are separate and can be released when their own owners are dropped.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_opt::{DecodedRefinedForwardingHistoryV1 as History, RefinedForwardingHistoryRoleV1 as Role};
    /// fn borrowed(h: History<'_, '_>) {
    ///     let graph = h.graph(Role::F);
    ///     let _ = h.into_final_graph();
    ///     let _ = graph.canonical();
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_kernel_opt::DecodedRefinedForwardingHistoryV1 as History;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn receipt(h: History<'_, '_>, b: &mut Budget<'_>) {
    ///     let checked = h.check_semantics(b).unwrap();
    ///     let _ = h.into_final_graph();
    ///     let _ = checked.output();
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_kernel_opt::DecodedRefinedForwardingHistoryV1 as History;
    /// fn reused(h: History<'_, '_>) { let _ = h.into_final_graph(); let _ = h.storage(); }
    /// ```
    pub fn into_final_graph(mut self) -> (Owner, CanonicalKernelIrReplayStorageV12) {
        let graph = self
            .graphs
            .pop()
            .expect("fixed twelve-role history ends in F");
        (graph, self.final_graph_storage)
    }
    pub const fn frame(&self) -> &'frame Frame<'wire> {
        self.frame
    }
    pub const fn storage(&self) -> Storage {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }

    fn inputs(&self) -> Inputs<'_> {
        let fields = &self.frame.fields;
        Inputs {
            prefix: P8 {
                prefix: P7 {
                    prefix: P6 {
                        prefix: P5 {
                            input: self.graph(Role::B),
                            intermediate: self.graph(Role::C),
                            stored: self.graph(Role::S),
                            output: self.graph(Role::O),
                            policy4_wire: fields[12],
                            policy5_record: fields[13],
                            load_rows: &self.loads,
                        },
                        output: self.graph(Role::I),
                        continuation: P6Claims {
                            composition_record: fields[15],
                            integer_record: fields[16],
                            transition_wire: fields[17],
                        },
                    },
                    output: self.graph(Role::J),
                    continuation: self.policy7.claims(),
                },
                output: self.graph(Role::K),
                continuation: P8Claims {
                    pass_name: PASS,
                    input: Identity::from_verified(self.graph(Role::J).canonical().identity()),
                    output: Identity::from_verified(self.graph(Role::K).canonical().identity()),
                    occurrences: self.tail.candidate(),
                },
            },
            promoted: self.graph(Role::P),
            selected_allocations: &self.selected,
            promotion_origins: &self.promotion,
            preheaders: self.graph(Role::H),
            preheader_rows: &self.preheaders,
            licm: self.graph(Role::L),
            licm_origins: &self.licm,
            refined: self.graph(Role::R),
            refinement_origins: &self.refinement,
            output: self.graph(Role::F),
            forwarding_origins: &self.forwarding,
            limits: self.frame.limits,
        }
    }

    /// Full independently checked B-through-F relation with all original limits.
    /// The owner, wire and frame receipts must already remain reserved. The
    /// numeric floor check is necessary accounting, not reservation provenance.
    /// No new decoded object/graph is introduced here. Result storage is returned
    /// UNRESERVED under the unchanged semantic checker contract.
    pub fn check_semantics<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<Checked<'a>, SemanticError> {
        let required = self
            .storage
            .retained_storage()
            .checked_add(self.frame.storage().retained_storage())
            .and_then(|n| n.checked_add(self.frame.canonical_bytes().len()))
            .ok_or(Resource::Arithmetic)?;
        if budget.storage_limit()
            > super::refined_forwarding_history_wire_v1::MAX_REFINED_FORWARDING_HISTORY_STORAGE_V1
            || budget.storage() < required
        {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(256)?;
        check_canonical_refined_forwarding_history_v1(self.inputs(), budget)
    }
}

/// Materializes every complete graph once in fixed B..F order. Equal-byte roles
/// are not aliased or silently replaced by external F. The existing V12 decoder
/// and P7/P8 typed-row decoders preserve their domains and errors. New owning
/// headers, all observed Vec capacities and complete opaque nested receipts are
/// paid while they coexist; conservative embedded-header overlap grants no credit.
/// Borrowed frame/wire storage is caller-owned and prepaid once. No new allocator
/// exclusion or fresh ledger is introduced. Locals drop before same-ledger cleanup
/// on error/unwind. Success returns complete new storage UNRESERVED.
pub fn materialize_refined_forwarding_history_v1<'frame, 'wire>(
    frame: &'frame Frame<'wire>,
    budget: &mut Budget<'_>,
) -> Result<DecodedRefinedForwardingHistoryV1<'frame, 'wire>, Error> {
    configured(budget)?;
    if budget.storage()
        < add(
            frame.storage().retained_storage(),
            frame.canonical_bytes().len(),
        )?
    {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        let mut retained = size_of::<DecodedRefinedForwardingHistoryV1<'_, '_>>();
        meter.reserve(retained)?;
        let (mut graphs, capacity) = meter.table::<Owner>(12)?;
        retained = add(retained, capacity)?;
        let mut final_graph_storage = None;
        for role in ROLES {
            meter.work(1)?;
            let (graph, storage) = meter.derive(|b| {
                Owner::from_canonical_bytes_with_verification_budget_v12(frame.graph_bytes(role), b)
                    .map_err(|error| Error::Admission { role, error })
            })?;
            meter.reserve(storage.retained_storage())?;
            retained = add(retained, storage.retained_storage())?;
            if role == Role::F {
                final_graph_storage = Some(storage);
            }
            meter.push(&mut graphs, graph)?;
        }
        // Do not substitute actual identities for unchecked wire claims.
        meter.work(80)?;
        let mut cursor = rows::Reader {
            bytes: frame.fields[19],
            pos: 4 + PASS.len(),
        };
        for role in [Role::J, Role::K] {
            let id = graphs[role as usize].canonical().identity();
            if cursor.take(32)? != id.digest() || cursor.wide()? != id.canonical_length() {
                return Err(Error::TailIdentity);
            }
        }
        let (loads, capacity) = rows::decode(frame.fields[14], meter)?;
        retained = add(retained, capacity)?;
        let policy7 = meter.derive(|b| {
            decode_canonical_policy7_rows_v1(frame.fields[18], b).map_err(Error::Policy7)
        })?;
        let storage = policy7.storage().retained_storage();
        meter.reserve(storage)?;
        retained = add(retained, storage)?;
        let (tail, storage, view_storage) = {
            let tail_view = meter.derive(|b| {
                read_canonical_kir_occurrence_row_bytes_v1(frame.tail_body(), frame.tail_counts, b)
                    .map_err(Error::Tail)
            })?;
            let view_storage = tail_view.storage().retained_storage();
            meter.reserve(view_storage)?;
            let (tail, storage) = meter.derive(|b| {
                materialize_canonical_kir_occurrence_rows_v1(&tail_view, b).map_err(Error::Tail)
            })?;
            meter.reserve(storage.retained_storage())?;
            (tail, storage, view_storage)
        };
        retained = add(retained, storage.retained_storage())?;
        meter.release(view_storage)?;
        let (selected, capacity) = rows::decode(frame.fields[20], meter)?;
        retained = add(retained, capacity)?;
        let (promotion, capacity) = rows::decode(frame.fields[21], meter)?;
        retained = add(retained, capacity)?;
        let (preheaders, capacity) = rows::decode(frame.fields[22], meter)?;
        retained = add(retained, capacity)?;
        let (licm, capacity) = rows::decode(frame.fields[23], meter)?;
        retained = add(retained, capacity)?;
        let (refinement, capacity) = rows::decode(frame.fields[24], meter)?;
        retained = add(retained, capacity)?;
        let (forwarding, capacity) = rows::decode(frame.fields[25], meter)?;
        retained = add(retained, capacity)?;
        meter.work(1)?;
        Ok(DecodedRefinedForwardingHistoryV1 {
            frame,
            graphs,
            final_graph_storage: final_graph_storage.ok_or(Resource::Accounting)?,
            loads,
            policy7,
            tail,
            selected,
            promotion,
            preheaders,
            licm,
            refinement,
            forwarding,
            storage: Storage(retained),
        })
    })
}
