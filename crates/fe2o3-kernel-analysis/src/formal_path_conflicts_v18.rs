//! Nominal actual-owner composition. No admission or shared-ledger authority.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalOwnerFormalAnalysisV18, ExplicitLaunchExtent, VerifiedCanonicalKernelIrModuleV18,
};

/// Path observations consuming a fresh actual-owner formal result. The original
/// report, its exact root/entry/index width and every conflict remain untouched.
/// This owner cannot be assembled from a caller report or alleged identity.
///
/// The launch remains a descriptive analysis input, not authenticated runtime
/// state. No production ledger, discharge receipt or publication path accepts
/// these observations as admission. Original and optimized owners each require
/// their own fresh formal scope and this composition; equal bytes are not custody.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::FormalPathConflictsV18;
/// use fe2o3_kernel_ir::*;
/// fn escape(owner: VerifiedCanonicalKernelIrModuleV18) -> FormalPathConflictsV18<'static> {
///     let mut formal = CanonicalOwnerFormalScopeV18::new(&owner, Default::default()).unwrap();
///     FormalPathConflictsV18::from_formal(formal.derive(&KernelId::new("kernel"),
///         ExplicitLaunchExtent::Unknown, FormalIndexWidth::Bits64).unwrap(),
///         Default::default()).unwrap()
/// }
/// ```
#[derive(Debug)]
#[must_use = "path observations alone do not authorize formal admission"]
pub struct FormalPathConflictsV18<'owner> {
    formal: CanonicalOwnerFormalAnalysisV18<'owner>,
    decisions: Vec<Decision>,
    queries: usize,
    construction_steps: usize,
}

impl<'owner> FormalPathConflictsV18<'owner> {
    /// Consume the nominal original-owner report without re-verification,
    /// re-encoding or conversion to a V1 verification token. The predecessor
    /// formal scope performs the complete pre-extraction candidate-pair bound.
    pub fn from_formal(
        formal: CanonicalOwnerFormalAnalysisV18<'owner>,
        limits: Limits,
    ) -> Result<Self> {
        let limits = clamp(limits);
        let module = formal.owner().module();
        check_source_items(module, limits)?;
        let report = formal.analysis();
        let selected = module
            .kernels
            .iter()
            .find(|row| &row.id == report.obligations().kernel())
            .ok_or(Error::InconsistentReport)?;
        if &selected.entry != report.obligations().entry() {
            return Err(Error::InconsistentReport);
        }
        // Incomplete reports cannot discharge anything, even with no conflicts.
        // Preserve their complete reason set before interpreting supported rank.
        let extent = if report.is_complete() {
            match formal.launch() {
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [extent, 1, 1],
                } if extent != 0 && matches!(selected.domain, LaunchDomain::D1 { .. }) => extent,
                _ => return Err(Error::UnsupportedLaunch),
            }
        } else {
            0
        };
        let observed = analyze_fresh_report(
            module,
            report,
            extent,
            report.obligations().index_width(),
            limits,
        )?;
        Ok(Self {
            formal,
            decisions: observed.decisions,
            queries: observed.queries,
            construction_steps: observed.construction_steps,
        })
    }

    /// Pointer identity with the actual borrowed V18 owner, not byte equality.
    pub fn belongs_to(&self, owner: &VerifiedCanonicalKernelIrModuleV18) -> bool {
        self.formal.belongs_to(owner)
    }
    /// Genuine immutable owner used by both formal extraction and path analysis.
    pub const fn owner(&self) -> &'owner VerifiedCanonicalKernelIrModuleV18 {
        self.formal.owner()
    }
    /// Unchanged fresh formal wrapper, preserving its source/root/launch binding.
    pub const fn formal(&self) -> &CanonicalOwnerFormalAnalysisV18<'owner> {
        &self.formal
    }
    /// Decisions correspond one-for-one, in order, to the original conflict rows.
    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }
    /// Independently bounded solver calls; not shared-ledger work credit.
    pub const fn queries(&self) -> usize {
        self.queries
    }
    /// Local visits, excluding formal, CFG and solver work; not total resource use.
    pub const fn construction_steps(&self) -> usize {
        self.construction_steps
    }
}

#[cfg(test)]
#[path = "formal_path_conflicts_v18_tests.rs"]
mod tests;
