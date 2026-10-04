//! Exact, owner-held semantic MIR representation for the production pipeline.
//!
//! Pliron stores typed locators into the retained admitted semantic object. It
//! does not duplicate semantic payloads, so there is exactly one source of
//! truth for types, ABI, places, rvalues, constants, provenance, and CFG roles.

use std::{error::Error, fmt};

use dialect_mir::pliron::{
    MirProductionBlockLocatorV1, MirProductionFunctionLocatorV1, MirProductionLocatorErrorV1,
    MirProductionModuleHandleV1, MirProductionModuleLocatorV1, MirProductionPlironLimitsV1,
    MirProductionSemanticSha256V1, MirProductionStatementLocatorV1, MirProductionSuccessorArcV1,
    MirProductionTerminatorLocatorV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticBasicBlockV1, SemanticBlockIdV1, SemanticFunctionDeclV1,
    SemanticFunctionIdV1, SemanticStatementV1, SemanticTerminatorV1,
};

use crate::{ContextBuildError, NameError, PlironSession, ShellLimits};

/// Independent limits for one exact semantic middle-end owner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductionSemanticMirLimitsV1 {
    shell: ShellLimits,
    pliron: MirProductionPlironLimitsV1,
}

impl ProductionSemanticMirLimitsV1 {
    /// Combines validated shell and semantic-scale Pliron limits.
    pub const fn new(shell: ShellLimits, pliron: MirProductionPlironLimitsV1) -> Self {
        Self { shell, pliron }
    }

    pub const fn shell(self) -> ShellLimits {
        self.shell
    }

    pub const fn pliron(self) -> MirProductionPlironLimitsV1 {
        self.pliron
    }
}

/// Fail-closed errors from exact semantic middle-end construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSemanticMirErrorV1 {
    DialectRegistration(NameError),
    Context(ContextBuildError),
    Locator(MirProductionLocatorErrorV1),
    EquivalenceMismatch,
}

impl fmt::Display for ProductionSemanticMirErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DialectRegistration(_) => {
                formatter.write_str("production MIR dialect registration is invalid")
            }
            Self::Context(_) => {
                formatter.write_str("production semantic Pliron context construction failed")
            }
            Self::Locator(error) => {
                write!(formatter, "production semantic locator failed: {error}")
            }
            Self::EquivalenceMismatch => formatter.write_str(
                "production semantic Pliron graph is not exactly equivalent to admitted MIR",
            ),
        }
    }
}

impl Error for ProductionSemanticMirErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Locator(error) => Some(error),
            Self::DialectRegistration(_) | Self::Context(_) | Self::EquivalenceMismatch => None,
        }
    }
}

/// Move-only owner of admitted semantic MIR and its exact verified Pliron graph.
///
/// Semantic payload remains solely in the admitted object. The Pliron graph
/// contains only typed locators and the source digest, and is inaccessible
/// without this owner-held context.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionSemanticMirOwnerV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionSemanticMirOwnerV1>();
/// ```
#[must_use = "dropping the semantic owner abandons the exact production middle end"]
pub struct ProductionSemanticMirOwnerV1 {
    semantic: AdmittedInertSemanticMirV1,
    expected: MirProductionModuleLocatorV1,
    graph: MirProductionModuleHandleV1,
    session: PlironSession,
}

impl fmt::Debug for ProductionSemanticMirOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSemanticMirOwnerV1")
            .field("semantic_sha256", &self.expected.semantic_sha256())
            .field("function_count", &self.expected.functions().len())
            .finish_non_exhaustive()
    }
}

impl ProductionSemanticMirOwnerV1 {
    /// Constructs and recursively verifies the exact locator graph before
    /// returning an owner. All schema and tree-work checks happen before the
    /// result can enter a downstream production stage.
    pub fn try_new(
        semantic: AdmittedInertSemanticMirV1,
        limits: ProductionSemanticMirLimitsV1,
    ) -> Result<Self, ProductionSemanticMirErrorV1> {
        let expected = semantic_locator_v1(&semantic)?;
        let registration = dialect_mir::pliron::mir_dialect_registration()
            .map_err(ProductionSemanticMirErrorV1::DialectRegistration)?;
        let mut session = PlironSession::new(limits.shell(), [registration])
            .map_err(ProductionSemanticMirErrorV1::Context)?;
        let graph = MirProductionModuleHandleV1::try_new(
            &mut session.context,
            expected.clone(),
            limits.pliron(),
        )
        .map_err(ProductionSemanticMirErrorV1::Locator)?;
        let actual = graph
            .snapshot(&session.context)
            .map_err(ProductionSemanticMirErrorV1::Locator)?;
        if actual != expected {
            return Err(ProductionSemanticMirErrorV1::EquivalenceMismatch);
        }
        Ok(Self {
            semantic,
            expected,
            graph,
            session,
        })
    }

    /// Re-verifies owner binding, recursive Pliron structure, and exact source
    /// equivalence. Downstream lowering should call this before a transition.
    pub fn verify_equivalence(&self) -> Result<(), ProductionSemanticMirErrorV1> {
        let actual = self
            .graph
            .snapshot(&self.session.context)
            .map_err(ProductionSemanticMirErrorV1::Locator)?;
        if actual == self.expected {
            Ok(())
        } else {
            Err(ProductionSemanticMirErrorV1::EquivalenceMismatch)
        }
    }

    /// Borrows the sole semantic payload owner. No raw Pliron capability is exposed.
    pub const fn semantic(&self) -> &AdmittedInertSemanticMirV1 {
        &self.semantic
    }

    /// Returns the verified, pointer-independent locator snapshot.
    pub const fn locator(&self) -> &MirProductionModuleLocatorV1 {
        &self.expected
    }

    pub fn resolve_function(
        &self,
        function: SemanticFunctionIdV1,
    ) -> Option<&SemanticFunctionDeclV1> {
        let index = usize::try_from(function.index()).ok()?;
        self.expected
            .functions()
            .get(index)
            .filter(|locator| locator.function_id() == function)?;
        self.semantic.functions().get(index)
    }

    pub fn resolve_block(
        &self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    ) -> Option<&SemanticBasicBlockV1> {
        let function_locator = self
            .expected
            .functions()
            .get(usize::try_from(function.index()).ok()?)?;
        let block_index = usize::try_from(block.index()).ok()?;
        function_locator
            .blocks()
            .get(block_index)
            .filter(|locator| locator.block_id() == block)?;
        self.resolve_function(function)?.blocks().get(block_index)
    }

    pub fn resolve_statement(
        &self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        ordinal: u32,
    ) -> Option<&SemanticStatementV1> {
        let ordinal = usize::try_from(ordinal).ok()?;
        self.resolve_block(function, block)?
            .statements()
            .get(ordinal)
    }

    pub fn resolve_terminator(
        &self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    ) -> Option<&SemanticTerminatorV1> {
        Some(self.resolve_block(function, block)?.terminator())
    }

    /// Exact semantic ownership is evidence, not proof or execution authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn semantic_locator_v1(
    semantic: &AdmittedInertSemanticMirV1,
) -> Result<MirProductionModuleLocatorV1, ProductionSemanticMirErrorV1> {
    let mut functions = Vec::with_capacity(semantic.functions().len());
    for (function_index, function) in semantic.functions().iter().enumerate() {
        let function_id = SemanticFunctionIdV1::from_index(function_index as u32);
        let mut blocks = Vec::with_capacity(function.blocks().len());
        for (block_index, block) in function.blocks().iter().enumerate() {
            let block_id = SemanticBlockIdV1::from_index(block_index as u32);
            let statements = (0..block.statements().len())
                .map(|ordinal| MirProductionStatementLocatorV1::new(ordinal as u32))
                .collect();
            let mut successors = Vec::with_capacity(block.terminator().kind().edge_count());
            block
                .terminator()
                .kind()
                .try_for_each_edge::<std::convert::Infallible>(|edge| {
                    successors.push(MirProductionSuccessorArcV1::new(edge.role(), edge.target()));
                    Ok(())
                })
                .expect("infallible semantic edge collection");
            blocks.push(
                MirProductionBlockLocatorV1::try_new(
                    block_id,
                    statements,
                    MirProductionTerminatorLocatorV1::try_new(successors)
                        .map_err(ProductionSemanticMirErrorV1::Locator)?,
                )
                .map_err(ProductionSemanticMirErrorV1::Locator)?,
            );
        }
        functions.push(
            MirProductionFunctionLocatorV1::try_new(function_id, function.entry(), blocks)
                .map_err(ProductionSemanticMirErrorV1::Locator)?,
        );
    }
    MirProductionModuleLocatorV1::try_new(
        MirProductionSemanticSha256V1::from_sha256(*semantic.semantic_sha256().as_bytes()),
        functions,
    )
    .map_err(ProductionSemanticMirErrorV1::Locator)
}
