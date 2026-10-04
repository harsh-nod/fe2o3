//! Separate new-family source/SSA/N query; the legacy certificate path is fixed.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{InertSemanticMirSha256V1, SemanticFunctionIdentityV1};
use fe2o3_mir_model::{
    SemanticU32InductionBlockSiteV1 as BlockSite,
    SemanticU32InductionBoundSnapshotCertificateV1 as SnapshotCertificate,
    SemanticU32InductionPlaceBindingV1 as PlaceBinding,
    SemanticU32InductionStatementSiteV1 as StatementSite,
};

// Private closed dispatch shares only common recurrence checks. No caller mode,
// converted legacy certificate/report or publicly selectable relation exists.
#[derive(Clone, Copy)]
pub(super) enum CertificateSubject {
    Legacy(Certificate),
    Snapshot(SnapshotCertificate),
}
macro_rules! getter {
    ($name:ident, $ty:ty) => {
        pub(super) fn $name(self) -> $ty {
            match self {
                Self::Legacy(fact) => fact.$name(),
                Self::Snapshot(fact) => fact.$name(),
            }
        }
    };
}
impl CertificateSubject {
    getter!(semantic_mir_sha256, InertSemanticMirSha256V1);
    getter!(function, SemanticFunctionIdV1);
    getter!(function_identity, SemanticFunctionIdentityV1);
    getter!(induction, PlaceBinding);
    getter!(guard_induction, PlaceBinding);
    getter!(bound, PlaceBinding);
    getter!(predicate, PlaceBinding);
    getter!(checked_result, PlaceBinding);
    getter!(preheader, BlockSite);
    getter!(header, BlockSite);
    getter!(body_entry, BlockSite);
    getter!(exit, BlockSite);
    getter!(initialization, StatementSite);
    getter!(guard_induction_snapshot, Option<StatementSite>);
    getter!(guard, StatementSite);
    getter!(checked_addition, StatementSite);
    getter!(update, StatementSite);
    getter!(grants_authority, bool);
    getter!(authorizes_compiler_transform, bool);
}

/// Inert source/SSA/N recurrence fact retaining the new certificate family.
/// The actual RHS Statement is never relabeled as an Entry definition.
#[derive(Clone, Copy, Debug)]
pub struct ProductionU32BoundSnapshotRecurrenceFactV1<'source> {
    source: &'source ProductionPreRankedKirOwnerV1,
    root: SemanticFunctionIdV1,
    certificate: SnapshotCertificate,
    recurrence: Recurrence,
}
impl ProductionU32BoundSnapshotRecurrenceFactV1<'_> {
    /// Borrows the one actual original source/N owner used by the query.
    pub fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.source
    }
    /// Actual semantic root, independently joined to the selected body alias.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    /// New-family fact preserving separate Entry and snapshot bindings.
    pub const fn certificate(&self) -> SnapshotCertificate {
        self.certificate
    }
    /// Independently checked actual N recurrence; not a rewrite permission.
    pub const fn recurrence(&self) -> Recurrence {
        self.recurrence
    }
    /// No source, optimization, native or final-output authority.
    pub const fn authorizes_compiler_transform(&self) -> bool {
        false
    }
}

/// Explicit unsupported mapping or a new-family source/SSA/N join.
#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "Borrowed fixed-coordinate facts remain allocation-free"
)]
pub enum ProductionU32BoundSnapshotRecurrenceV1<'source> {
    /// Existing strict C transport/recurrence exclusions remain unchanged.
    Unavailable(ProductionScalarSsaEmissionUnavailableV1),
    /// Actual snapshot recipe, immutable Entry and N recurrence agree.
    Joined(ProductionU32BoundSnapshotRecurrenceFactV1<'source>),
}

impl<'source> ProductionScalarSsaEmissionQueryV1<'_, 'source> {
    /// Joins the separately derived new-family fact to actual source SSA and N.
    /// This does not rerun the source analyzer, convert old evidence, infer
    /// flattened header transport, or authorize general production admission.
    /// The enclosing original C scope retains ledger/floor and failure rules.
    pub fn check_u32_bound_snapshot_certificate_v1(
        &mut self,
        root: SemanticFunctionIdV1,
        certificate: &SnapshotCertificate,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionU32BoundSnapshotRecurrenceV1<'source>> {
        let result = (|| {
            if let Some(error) = &self.failure {
                return Err(copy_query_error(error));
            }
            if budget as *mut _ as usize != self.slot
                || budget.work_ledger_identity_v1() != self.ledger
                || budget.storage() < self.floor
            {
                *self.corrupted = true;
                return Err(Resource::Accounting.into());
            }
            if budget.storage() != self.floor {
                return Err(Resource::Accounting.into());
            }
            Ok(
                match self.check(root, CertificateSubject::Snapshot(*certificate), budget)? {
                    RecurrenceParts::Unavailable(reason) => {
                        ProductionU32BoundSnapshotRecurrenceV1::Unavailable(reason)
                    }
                    RecurrenceParts::Joined(recurrence) => {
                        ProductionU32BoundSnapshotRecurrenceV1::Joined(
                            ProductionU32BoundSnapshotRecurrenceFactV1 {
                                source: &self.owner.original,
                                root,
                                certificate: *certificate,
                                recurrence,
                            },
                        )
                    }
                },
            )
        })();
        if let Err(error) = &result {
            self.failure = Some(copy_query_error(error));
        }
        result
    }
}

pub(super) fn check_extra_binding(
    original: &ProductionPreRankedKirOwnerV1,
    certificate: SnapshotCertificate,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(6)?;
    let source = original.semantic_ssa().source_semantic();
    let function = source
        .functions()
        .get(certificate.function().index() as usize)
        .ok_or(Error::Mismatch("snapshot certificate function"))?;
    let binding = certificate.guard_bound();
    let local = function
        .locals()
        .get(binding.local().index() as usize)
        .ok_or(Error::Mismatch("snapshot certificate local"))?;
    let ty = source
        .types()
        .get(binding.ty().index() as usize)
        .ok_or(Error::Mismatch("snapshot certificate type"))?;
    if local.identity() != binding.local_identity()
        || local.ty() != binding.ty()
        || ty.identity() != binding.type_identity()
    {
        return Err(Error::Mismatch("snapshot certificate typed RHS"));
    }
    if let Some(site) = certificate.bound_snapshot() {
        budget.charge_work(3)?;
        if function
            .blocks()
            .get(site.block().block().index() as usize)
            .filter(|block| block.identity() == site.block().identity())
            .and_then(|block| block.statements().get(site.statement() as usize))
            .is_none()
        {
            return Err(Error::Mismatch("snapshot certificate statement"));
        }
    }
    Ok(())
}

fn exact_place(operand: &SemanticOperandV1, binding: PlaceBinding) -> bool {
    let place = match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        _ => return false,
    };
    place.local() == binding.local() && place.ty() == binding.ty() && place.projections().is_empty()
}
fn source_site(site: StatementSite) -> SourceSite {
    SourceSite::Event(Site::Statement {
        block: SsaBlockIdV1::new(site.block().block().index()),
        statement: site.statement(),
    })
}

pub(super) fn check_transport(
    owner: &ProductionScalarSsaEmissionOwnerV1,
    function: &EmittedFunction,
    certificate: SnapshotCertificate,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let original = &owner.original;
    let sealed = &owner.emission;
    let source = original.semantic_ssa().source_semantic();
    let declaration = &source.functions()[certificate.function().index() as usize];
    let guard_site = certificate.guard();
    let guard = &declaration.blocks()[guard_site.block().block().index() as usize].statements()
        [guard_site.statement() as usize];
    budget.charge_work(8)?;
    let SemanticStatementKindV1::Assign(assignment) = guard.kind() else {
        return Err(Error::Mismatch("snapshot guard assignment"));
    };
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::LessThan,
        left,
        right,
    } = assignment.value().kind()
    else {
        return Err(Error::Mismatch("snapshot guard ordered LessThan"));
    };
    if !exact_place(left, certificate.guard_induction())
        || !exact_place(right, certificate.guard_bound())
    {
        return Err(Error::Mismatch("snapshot guard actual source operands"));
    }
    let rhs_variable = SsaVariableIdV1::new(certificate.guard_bound().local().index());
    let entry_variable = SsaVariableIdV1::new(certificate.bound().local().index());
    let rhs_value = guard_rhs_value(owner, certificate, left, budget)?;
    let rhs = sealed.definition(function, rhs_value, budget)?;
    let entry = if let Some(site) = certificate.bound_snapshot() {
        budget.charge_work(12)?;
        let statement = &declaration.blocks()[site.block().block().index() as usize].statements()
            [site.statement() as usize];
        let SemanticStatementKindV1::Assign(copy) = statement.kind() else {
            return Err(Error::Mismatch("snapshot actual assignment"));
        };
        let SemanticRvalueKindV1::Use(operand @ SemanticOperandV1::Copy(_)) = copy.value().kind()
        else {
            return Err(Error::Mismatch("snapshot actual Copy recipe"));
        };
        if site.block() != certificate.header()
            || site.statement() >= guard_site.statement()
            || copy.destination().local() != certificate.guard_bound().local()
            || copy.destination().ty() != certificate.bound().ty()
            || !copy.destination().projections().is_empty()
            || copy.value().result_type() != certificate.bound().ty()
            || !exact_place(operand, certificate.bound())
        {
            return Err(Error::Mismatch("snapshot actual source Copy relation"));
        }
        let snapshot = site_definition(sealed, function, site, rhs_variable, budget)?;
        let expected = &sealed.capture.expected[snapshot.expected];
        if expected.site != source_site(site)
            || expected.variable != rhs_variable
            || expected.value != rhs_value
            || expected.shape != Shape::Scalar(ScalarType::U32)
            || snapshot.expected != rhs.expected
        {
            return Err(Error::Mismatch("snapshot actual Statement SSA definition"));
        }
        let entry_value = sealed.first_operand(
            original,
            certificate.function(),
            site,
            entry_variable,
            budget,
        )?;
        sealed.definition(function, entry_value, budget)?
    } else {
        if certificate.guard_bound() != certificate.bound() {
            return Err(Error::Mismatch("snapshot absent RHS definition"));
        }
        rhs
    };
    budget.charge_work(8)?;
    let expected = &sealed.capture.expected[entry.expected];
    if expected.site != SourceSite::Entry
        || expected.variable != entry_variable
        || expected.shape != Shape::Scalar(ScalarType::U32)
        || !matches!(
            entry.definitions[0],
            Some(Definition::FunctionArgument { .. })
        )
        || entry.definitions[0] != rhs.definitions[0]
        || entry.values[0] != rhs.values[0]
        || !declaration.locals()[certificate.bound().local().index() as usize]
            .role()
            .is_entry_argument()
    {
        return Err(Error::Mismatch(
            "snapshot actual immutable Entry input and N value",
        ));
    }
    Ok(())
}

// The legacy index deliberately contains operand zero only. For this closed
// unprojected scalar recipe, operand one immediately follows its BaseUse and
// optional MoveKill. Check those actual rows instead of searching by variable.
fn guard_rhs_value(
    owner: &ProductionScalarSsaEmissionOwnerV1,
    certificate: SnapshotCertificate,
    left: &SemanticOperandV1,
    budget: &mut Budget<'_>,
) -> Result<SsaValueV1> {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    let sealed = &owner.emission;
    let site = certificate.guard();
    let key = (
        certificate.function().index(),
        site.block().block().index(),
        site.statement(),
    );
    charge_lookup(sealed.first_operands.len(), budget)?;
    let index = sealed
        .first_operands
        .binary_search_by_key(&key, |row| row.key)
        .map_err(|_| Error::Mismatch("snapshot guard first-operand occurrence"))?;
    // At most three actual rows; all checks are prepaid before examining them.
    budget.charge_work(24)?;
    let rows = owner
        .original()
        .semantic_ssa()
        .occurrences_v1()
        .and_then(|rows| rows.function(certificate.function()))
        .ok_or(Error::Mismatch("snapshot guard operand function"))?;
    let ordinal = sealed.first_operands[index].ordinal;
    let actual_site = Site::Statement {
        block: SsaBlockIdV1::new(site.block().block().index()),
        statement: site.statement(),
    };
    let lhs_variable = SsaVariableIdV1::new(certificate.guard_induction().local().index());
    let rhs_variable = SsaVariableIdV1::new(certificate.guard_bound().local().index());
    let valid_row = |row: &fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1, operand, role| {
        row.site() == actual_site
            && row.operand() == operand
            && row.role() == role
            && row.is_reachable()
            && row.is_promoted()
    };
    let lhs = rows
        .events()
        .get(ordinal)
        .filter(|row| valid_row(row, Operand::RvalueOperand(0), Role::BaseUse))
        .ok_or(Error::Mismatch("snapshot guard exact LHS occurrence"))?;
    let Some(SsaResolvedEventV1::Use {
        variable,
        value: lhs_value,
    }) = lhs.resolved()
    else {
        return Err(Error::Mismatch("snapshot guard LHS resolution"));
    };
    if variable != lhs_variable {
        return Err(Error::Mismatch("snapshot guard actual LHS variable"));
    }
    let offset = match left {
        SemanticOperandV1::Copy(place) if place.projections().is_empty() => 1,
        SemanticOperandV1::Move(place) if place.projections().is_empty() => {
            let kill = rows
                .events()
                .get(ordinal.checked_add(1).ok_or(Resource::Arithmetic)?)
                .filter(|row| valid_row(row, Operand::RvalueOperand(0), Role::MoveKill))
                .ok_or(Error::Mismatch("snapshot guard exact LHS MoveKill"))?;
            if kill.resolved()
                != Some(SsaResolvedEventV1::Kill {
                    variable: lhs_variable,
                    previous: Some(lhs_value),
                })
            {
                return Err(Error::Mismatch("snapshot guard LHS MoveKill resolution"));
            }
            2
        }
        _ => return Err(Error::Mismatch("snapshot guard scalar LHS recipe")),
    };
    let rhs = rows
        .events()
        .get(ordinal.checked_add(offset).ok_or(Resource::Arithmetic)?)
        .filter(|row| valid_row(row, Operand::RvalueOperand(1), Role::BaseUse))
        .ok_or(Error::Mismatch("snapshot guard exact RHS occurrence"))?;
    match rhs.resolved() {
        Some(SsaResolvedEventV1::Use { variable, value }) if variable == rhs_variable => Ok(value),
        _ => Err(Error::Mismatch("snapshot guard RHS resolution")),
    }
}
