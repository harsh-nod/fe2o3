//! Closed, compiler-owned Pliron session for the production pipeline.
//!
//! It supports a closed builtin-module recipe and a bounded target-neutral
//! ranked-kernel recipe. Ranked graphs are constructed inside the owned context
//! and must pass the whole-function bounds transition before a move-only
//! lowering input exists. Callers cannot inject callbacks, arbitrary passes,
//! text, raw contexts, or contextless pointers.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    marker::PhantomData,
    num::NonZeroU64,
};

use fe2o3_pliron_owner_core::{ContextIdentity, DialectRegistration, NameError};

use super::{
    ContextBuildError, ContextManifest, NameKind, OperationHandle, OperationHandleError,
    OperationShapeV1, PlironSession, ShellLimits, validate_name,
};

mod middle_end_evidence_v4;
mod middle_end_evidence_v5;
mod mir_pliron_semantic_contract_derivation_v1;
mod mir_pliron_semantic_contract_v1;
mod parallel_reference_contract_v1;
mod ranked;
mod semantic_expression_v2;
mod semantic_mir;
mod total_output_refinement_v2;

pub use middle_end_evidence_v4::*;
pub use middle_end_evidence_v5::*;
pub use mir_pliron_semantic_contract_derivation_v1::*;
pub use mir_pliron_semantic_contract_v1::*;
pub use parallel_reference_contract_v1::*;
pub use ranked::*;
pub use semantic_expression_v2::*;
pub use semantic_mir::*;
pub use total_output_refinement_v2::*;

/// Hard cap for construction recipes registered during one production session.
pub const HARD_MAX_PRODUCTION_CONSTRUCTIONS: usize = 4_096;

/// Resource limits for one closed production Pliron session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSessionLimitsV1 {
    shell: ShellLimits,
    max_constructions: usize,
}

impl ProductionSessionLimitsV1 {
    /// Creates non-zero limits bounded by implementation hard caps.
    pub fn new(
        shell: ShellLimits,
        max_constructions: usize,
    ) -> Result<Self, ProductionSessionLimitErrorV1> {
        if max_constructions == 0 {
            return Err(ProductionSessionLimitErrorV1::ZeroConstructions);
        }
        if max_constructions > HARD_MAX_PRODUCTION_CONSTRUCTIONS {
            return Err(ProductionSessionLimitErrorV1::TooManyConstructions);
        }
        Ok(Self {
            shell,
            max_constructions,
        })
    }

    pub const fn shell(self) -> ShellLimits {
        self.shell
    }

    pub const fn max_constructions(self) -> usize {
        self.max_constructions
    }
}

impl Default for ProductionSessionLimitsV1 {
    fn default() -> Self {
        Self {
            shell: ShellLimits::default(),
            max_constructions: 64,
        }
    }
}

/// Invalid production-session resource configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSessionLimitErrorV1 {
    ZeroConstructions,
    TooManyConstructions,
}

impl fmt::Display for ProductionSessionLimitErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroConstructions => {
                formatter.write_str("production construction limit must be non-zero")
            }
            Self::TooManyConstructions => {
                formatter.write_str("production construction limit exceeds the hard cap")
            }
        }
    }
}

impl Error for ProductionSessionLimitErrorV1 {}

/// A closed, bounded construction recipe admitted by the production session.
///
/// The private recipe representation prevents callers from adding a constructor
/// callback or smuggling a raw Pliron capability into the session.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionConstructionV1 {
    kind: ProductionConstructionKindV1,
}

#[derive(Debug, Eq, PartialEq)]
enum ProductionConstructionKindV1 {
    BuiltinModule {
        root_name: String,
    },
    RankedKernel {
        root_name: String,
        kernel: ProductionRankedKernelV1,
        authenticated_functional_refinement: Vec<ProductionFunctionalRefinementEvidenceV2>,
    },
}

impl ProductionConstructionV1 {
    /// Builds a validated builtin-module recipe without allocating Pliron IR.
    pub fn builtin_module(root_name: &str) -> Result<Self, NameError> {
        validate_name(root_name, NameKind::Dialect)?;
        Ok(Self {
            kind: ProductionConstructionKindV1::BuiltinModule {
                root_name: root_name.to_owned(),
            },
        })
    }

    fn root_name(&self) -> &str {
        match &self.kind {
            ProductionConstructionKindV1::BuiltinModule { root_name }
            | ProductionConstructionKindV1::RankedKernel { root_name, .. } => root_name,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StageIdentityV1(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RootIdentityV1(NonZeroU64);

/// Typestate for a recipe registered but not yet materialized in Pliron.
#[derive(Debug)]
pub struct ConstructionRegisteredStageV1 {
    _private: (),
}

/// Typestate for a recursively verified, session-owned Pliron graph.
#[derive(Debug)]
pub struct ConstructedGraphStageV1 {
    _private: (),
}

/// Typestate for a ranked function that passed the fixed generic verifier pipeline.
#[derive(Debug)]
pub struct KernelChecksVerifiedGraphStageV1 {
    _private: (),
}

/// An opaque, move-only stage capability owned by one production session.
///
/// It contains no Pliron pointer. Its owner and registry identity are private
/// and are authenticated before every operation.
#[must_use = "dropping a stage capability abandons its bounded production transition"]
pub struct ProductionStageHandleV1<Stage> {
    owner: ContextIdentity,
    identity: StageIdentityV1,
    _stage: PhantomData<fn() -> Stage>,
}

impl<Stage> fmt::Debug for ProductionStageHandleV1<Stage> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionStageHandleV1")
            .finish_non_exhaustive()
    }
}

/// An opaque, move-only root capability owned by one production session/stage.
///
/// Neither the private operation handle nor its upstream pointer can be
/// recovered by callers:
///
/// ```compile_fail
/// use fe2o3_pliron::{ConstructedGraphStageV1, ProductionRootHandleV1};
/// use pliron::{context::Ptr, operation::Operation};
///
/// fn escape(root: &ProductionRootHandleV1<ConstructedGraphStageV1>) -> Ptr<Operation> {
///     root.operation
/// }
/// ```
#[must_use = "dropping a root capability makes its production graph inaccessible"]
pub struct ProductionRootHandleV1<Stage> {
    owner: ContextIdentity,
    stage: StageIdentityV1,
    identity: RootIdentityV1,
    operation: OperationHandle,
    _stage: PhantomData<fn() -> Stage>,
}

impl<Stage> fmt::Debug for ProductionRootHandleV1<Stage> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionRootHandleV1")
            .finish_non_exhaustive()
    }
}

/// Fail-closed errors from the closed production session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSessionErrorV1 {
    SessionPoisoned,
    ConstructionLimitExceeded,
    DuplicateConstructionName(String),
    StageIdentitySpaceExhausted,
    RootIdentitySpaceExhausted,
    ForeignSession,
    StaleStage,
    StageRootMismatch,
    WrongConstructionKind,
    RankedGraphChanged,
    RankedRecipe(ProductionRankedKernelErrorV1),
    RankedTensorLayout(fe2o3_kernel_analysis::PlironTensorLayoutCheckErrorV1),
    RankedBounds(fe2o3_kernel_analysis::RankedBoundsCheckErrorV1),
    RankedAtomic(fe2o3_kernel_analysis::PlironAtomicLegalityCheckErrorV1),
    RankedRace(fe2o3_kernel_analysis::RankedRaceCheckErrorV1),
    RankedOwnership(fe2o3_kernel_analysis::HierarchicalOwnershipCheckErrorV1),
    RankedBarrier(fe2o3_kernel_analysis::PlironBarrierCheckErrorV1),
    RankedWorkgroup(fe2o3_kernel_analysis::PlironWorkgroupMemoryCheckErrorV1),
    RankedSemantic(fe2o3_kernel_analysis::PlironSemanticRefinementCheckErrorV1),
    Operation(OperationHandleError),
}

impl fmt::Display for ProductionSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionPoisoned => formatter.write_str("production Pliron session is poisoned"),
            Self::ConstructionLimitExceeded => {
                formatter.write_str("production construction registration limit exceeded")
            }
            Self::DuplicateConstructionName(_) => {
                formatter.write_str("duplicate production construction name")
            }
            Self::StageIdentitySpaceExhausted => {
                formatter.write_str("production stage identity space exhausted")
            }
            Self::RootIdentitySpaceExhausted => {
                formatter.write_str("production root identity space exhausted")
            }
            Self::ForeignSession => {
                formatter.write_str("production capability belongs to another session")
            }
            Self::StaleStage => formatter.write_str("production stage capability is stale"),
            Self::StageRootMismatch => {
                formatter.write_str("production root does not belong to the supplied stage")
            }
            Self::WrongConstructionKind => {
                formatter.write_str("production stage is not a ranked-kernel construction")
            }
            Self::RankedGraphChanged => {
                formatter.write_str("production ranked graph changed after safety verification")
            }
            Self::RankedRecipe(error) => {
                write!(formatter, "production ranked recipe failed: {error}")
            }
            Self::RankedTensorLayout(error) => error.fmt(formatter),
            Self::RankedBounds(error) => error.fmt(formatter),
            Self::RankedAtomic(error) => error.fmt(formatter),
            Self::RankedRace(error) => error.fmt(formatter),
            Self::RankedOwnership(error) => error.fmt(formatter),
            Self::RankedBarrier(error) => error.fmt(formatter),
            Self::RankedWorkgroup(error) => error.fmt(formatter),
            Self::RankedSemantic(error) => error.fmt(formatter),
            Self::Operation(_) => formatter.write_str("production Pliron operation failed"),
        }
    }
}

impl Error for ProductionSessionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Operation(error) => Some(error),
            Self::RankedRecipe(error) => Some(error),
            Self::RankedTensorLayout(error) => Some(error),
            Self::RankedBounds(error) => Some(error),
            Self::RankedAtomic(error) => Some(error),
            Self::RankedRace(error) => Some(error),
            Self::RankedOwnership(error) => Some(error),
            Self::RankedBarrier(error) => Some(error),
            Self::RankedWorkgroup(error) => Some(error),
            Self::RankedSemantic(error) => Some(error),
            _ => None,
        }
    }
}

/// Closed compiler-owned production session.
///
/// The private [`PlironSession`] transitively owns the only raw Pliron context.
/// This type intentionally exposes no generic callback, textual importer,
/// arbitrary pass, operation handle, context, or pointer surface:
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionPlironSessionV1;
/// use pliron::context::Context;
///
/// fn escape(session: &mut ProductionPlironSessionV1) -> &mut Context {
///     session.with_context_mut(|context| context)
/// }
/// ```
pub struct ProductionPlironSessionV1 {
    inner: PlironSession,
    limits: ProductionSessionLimitsV1,
    registered: BTreeMap<StageIdentityV1, ProductionConstructionV1>,
    construction_names: BTreeSet<String>,
    constructed_roots: BTreeMap<StageIdentityV1, ConstructedRootV1>,
    registration_count: usize,
    next_stage: Option<NonZeroU64>,
    next_root: Option<NonZeroU64>,
    poisoned: bool,
}

impl ProductionPlironSessionV1 {
    /// Creates a fresh production session using only bounded typed dialect registrations.
    pub fn new(
        limits: ProductionSessionLimitsV1,
        registrations: impl IntoIterator<Item = DialectRegistration>,
    ) -> Result<Self, ContextBuildError> {
        let inner = PlironSession::new(limits.shell(), registrations)?;
        Ok(Self {
            inner,
            limits,
            registered: BTreeMap::new(),
            construction_names: BTreeSet::new(),
            constructed_roots: BTreeMap::new(),
            registration_count: 0,
            next_stage: NonZeroU64::new(1),
            next_root: NonZeroU64::new(1),
            poisoned: false,
        })
    }

    pub const fn manifest(&self) -> &ContextManifest {
        self.inner.manifest()
    }

    pub const fn limits(&self) -> ProductionSessionLimitsV1 {
        self.limits
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned || self.inner.is_poisoned()
    }

    /// Registers one prevalidated recipe and returns a move-only stage capability.
    ///
    /// The count is monotonic for the session lifetime. A rejected duplicate or
    /// over-limit recipe changes no registry state and does not poison the
    /// session because no Pliron allocation has started.
    pub fn register_construction(
        &mut self,
        construction: ProductionConstructionV1,
    ) -> Result<ProductionStageHandleV1<ConstructionRegisteredStageV1>, ProductionSessionErrorV1>
    {
        self.validate_live()?;
        if self.registration_count >= self.limits.max_constructions() {
            return Err(ProductionSessionErrorV1::ConstructionLimitExceeded);
        }
        if self.construction_names.contains(construction.root_name()) {
            return Err(ProductionSessionErrorV1::DuplicateConstructionName(
                construction.root_name().to_owned(),
            ));
        }
        let raw_identity = self
            .next_stage
            .ok_or(ProductionSessionErrorV1::StageIdentitySpaceExhausted)?;
        let identity = StageIdentityV1(raw_identity);
        let next_stage = raw_identity.get().checked_add(1).and_then(NonZeroU64::new);

        self.construction_names
            .insert(construction.root_name().to_owned());
        self.registered.insert(identity, construction);
        self.registration_count += 1;
        self.next_stage = next_stage;

        Ok(ProductionStageHandleV1 {
            owner: self.inner.identity,
            identity,
            _stage: PhantomData,
        })
    }

    /// Materializes one registered recipe and recursively verifies its root.
    ///
    /// This transition consumes the registered-stage capability. Any failure in
    /// the underlying operation construction is treated as a TCB failure and
    /// terminally poisons the production session.
    pub fn construct_registered(
        &mut self,
        stage: ProductionStageHandleV1<ConstructionRegisteredStageV1>,
    ) -> Result<
        (
            ProductionStageHandleV1<ConstructedGraphStageV1>,
            ProductionRootHandleV1<ConstructedGraphStageV1>,
        ),
        ProductionSessionErrorV1,
    > {
        self.validate_live()?;
        self.authenticate_owner(stage.owner)?;
        let construction = self
            .registered
            .remove(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        self.preflight_construction(&construction)?;
        let root_name = construction.root_name().to_owned();
        let raw_root = self
            .next_root
            .ok_or(ProductionSessionErrorV1::RootIdentitySpaceExhausted)?;
        let root_identity = RootIdentityV1(raw_root);
        let next_root = raw_root.get().checked_add(1).and_then(NonZeroU64::new);

        let is_builtin = matches!(
            &construction.kind,
            ProductionConstructionKindV1::BuiltinModule { .. }
        );
        let materialized = match self.materialize_construction(construction, &root_name) {
            Ok(materialized) => materialized,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        let operation = materialized.operation;
        if is_builtin {
            match self
                .inner
                .validate_production_module(&operation, &root_name)
            {
                Ok(_) => {}
                Err(error) => {
                    self.poisoned = true;
                    return Err(ProductionSessionErrorV1::Operation(error));
                }
            }
        }

        self.constructed_roots.insert(
            stage.identity,
            ConstructedRootV1 {
                identity: root_identity,
                ranked_function: materialized.ranked_function,
                ranked_kernel: materialized.ranked_kernel,
                ranked_view_names: materialized.ranked_view_names,
                authenticated_functional_refinement: materialized
                    .authenticated_functional_refinement,
                production_pipeline_report: None,
            },
        );
        self.next_root = next_root;

        Ok((
            ProductionStageHandleV1 {
                owner: stage.owner,
                identity: stage.identity,
                _stage: PhantomData,
            },
            ProductionRootHandleV1 {
                owner: stage.owner,
                stage: stage.identity,
                identity: root_identity,
                operation,
                _stage: PhantomData,
            },
        ))
    }

    /// Returns a bounded pointer-free root description after authenticating the session and stage.
    pub fn root_shape(
        &mut self,
        stage: &ProductionStageHandleV1<ConstructedGraphStageV1>,
        root: &ProductionRootHandleV1<ConstructedGraphStageV1>,
    ) -> Result<OperationShapeV1, ProductionSessionErrorV1> {
        self.validate_live()?;
        self.authenticate_owner(stage.owner)?;
        self.authenticate_owner(root.owner)?;
        let expected_root = self
            .constructed_roots
            .get(&stage.identity)
            .map(|record| record.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        if root.stage != stage.identity || root.identity != expected_root {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }

        match self.inner.operation_shape(&root.operation) {
            Ok(shape) => Ok(shape),
            Err(error) => {
                self.poisoned = true;
                Err(ProductionSessionErrorV1::Operation(error))
            }
        }
    }

    fn validate_live(&mut self) -> Result<(), ProductionSessionErrorV1> {
        if self.poisoned || self.inner.is_poisoned() {
            self.poisoned = true;
            return Err(ProductionSessionErrorV1::SessionPoisoned);
        }
        Ok(())
    }

    fn authenticate_owner(&self, owner: ContextIdentity) -> Result<(), ProductionSessionErrorV1> {
        if owner != self.inner.identity {
            return Err(ProductionSessionErrorV1::ForeignSession);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dialect_kernel::AccessKindAttr;

    fn session() -> ProductionPlironSessionV1 {
        ProductionPlironSessionV1::new(ProductionSessionLimitsV1::default(), [])
            .expect("fresh production session")
    }

    fn ranked_session() -> ProductionPlironSessionV1 {
        ProductionPlironSessionV1::new(
            ProductionSessionLimitsV1::default(),
            [
                dialect_gpu::dialect_registration().expect("gpu registration"),
                dialect_kernel::dialect_registration().expect("kernel registration"),
            ],
        )
        .expect("fresh ranked production session")
    }

    fn ranked_construction(name: &str) -> ProductionConstructionV1 {
        let view = ProductionRankedValueIdV1::new(0);
        let index = ProductionRankedValueIdV1::new(1);
        let kernel = ProductionRankedKernelV1::new(
            "checked",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [1, 1, 1],
                        workgroup_extents: [1, 1, 1],
                        subgroup_size: 1,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::View {
                        result: view,
                        element_width: 32,
                        writable: false,
                        shape: vec![1],
                        dynamic_extents: vec![],
                        allocation_origin: 1,
                        noalias_class: 1,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: index,
                        value: 0,
                    },
                    ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        view: ProductionRankedValueV1::Local(view),
                        indices: vec![ProductionRankedValueV1::Local(index)],
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .expect("ranked recipe");
        ProductionConstructionV1::ranked_kernel(name, kernel).expect("ranked construction")
    }

    #[test]
    fn stale_private_operation_poisons_the_production_session() {
        let mut session = session();
        let registered = session
            .register_construction(
                ProductionConstructionV1::builtin_module("root").expect("valid recipe"),
            )
            .expect("registered recipe");
        let (stage, root) = session
            .construct_registered(registered)
            .expect("constructed root");

        session
            .inner
            .erase_operation(&root.operation)
            .expect("simulate private registry corruption");
        assert_eq!(
            session.root_shape(&stage, &root),
            Err(ProductionSessionErrorV1::Operation(
                OperationHandleError::StaleHandle
            ))
        );
        assert!(session.is_poisoned());
        assert!(matches!(
            session.register_construction(
                ProductionConstructionV1::builtin_module("later").expect("valid recipe")
            ),
            Err(ProductionSessionErrorV1::SessionPoisoned)
        ));
    }

    #[test]
    fn recipe_cross_check_failure_poisons_after_allocation() {
        let mut session = session();
        let operation = session
            .inner
            .create_module("actual")
            .expect("private typed construction");

        assert_eq!(
            session
                .inner
                .validate_production_module(&operation, "expected"),
            Err(OperationHandleError::ConstructionRecipeMismatch)
        );
        assert!(session.is_poisoned());
        assert!(matches!(
            session.register_construction(
                ProductionConstructionV1::builtin_module("later").expect("valid recipe")
            ),
            Err(ProductionSessionErrorV1::SessionPoisoned)
        ));
    }

    #[test]
    fn exhausted_stage_identity_rejects_without_poisoning() {
        let mut session = session();
        session.next_stage = None;

        assert!(matches!(
            session.register_construction(
                ProductionConstructionV1::builtin_module("root").expect("valid recipe")
            ),
            Err(ProductionSessionErrorV1::StageIdentitySpaceExhausted)
        ));
        assert!(!session.is_poisoned());
        assert!(session.registered.is_empty());
        assert!(session.construction_names.is_empty());
        assert_eq!(session.registration_count, 0);
    }

    #[test]
    fn exhausted_root_identity_rejects_before_pliron_allocation() {
        let mut session = session();
        let registered = session
            .register_construction(
                ProductionConstructionV1::builtin_module("root").expect("valid recipe"),
            )
            .expect("registered recipe");
        session.next_root = None;

        assert!(matches!(
            session.construct_registered(registered),
            Err(ProductionSessionErrorV1::RootIdentitySpaceExhausted)
        ));
        assert!(!session.is_poisoned());
        assert!(session.inner.operations.is_empty());
    }

    #[test]
    fn stale_stage_identity_is_rejected_before_pliron_allocation() {
        let mut session = session();
        let stale = ProductionStageHandleV1 {
            owner: session.inner.identity,
            identity: StageIdentityV1(NonZeroU64::new(99).unwrap()),
            _stage: PhantomData,
        };

        assert!(matches!(
            session.construct_registered(stale),
            Err(ProductionSessionErrorV1::StaleStage)
        ));
        assert!(!session.is_poisoned());
        assert!(session.inner.operations.is_empty());
    }

    #[test]
    fn forged_verified_typestate_cannot_skip_the_safety_transitions() {
        let mut session = ranked_session();
        let registered = session
            .register_construction(ranked_construction("root"))
            .expect("registration");
        let (stage, root) = session
            .construct_registered(registered)
            .expect("construction");
        let forged_stage = ProductionStageHandleV1::<KernelChecksVerifiedGraphStageV1> {
            owner: stage.owner,
            identity: stage.identity,
            _stage: PhantomData,
        };
        let forged_root = ProductionRootHandleV1::<KernelChecksVerifiedGraphStageV1> {
            owner: root.owner,
            stage: root.stage,
            identity: root.identity,
            operation: root.operation,
            _stage: PhantomData,
        };

        assert!(matches!(
            session.prepare_ranked_lowering(forged_stage, forged_root),
            Err(ProductionSessionErrorV1::StageRootMismatch)
        ));
    }

    #[test]
    fn private_graph_erasure_is_rechecked_before_lowering_release() {
        let mut session = ranked_session();
        let registered = session
            .register_construction(ranked_construction("root"))
            .expect("registration");
        let (stage, root) = session
            .construct_registered(registered)
            .expect("construction");
        let (verified, root) = session
            .verify_production_ranked_kernel_pipeline(stage, root)
            .expect("generic kernel verification");
        session
            .inner
            .erase_operation(&root.operation)
            .expect("simulate private graph corruption");

        assert!(matches!(
            session.prepare_ranked_lowering(verified, root),
            Err(ProductionSessionErrorV1::Operation(
                OperationHandleError::StaleHandle
            ))
        ));
    }

    #[test]
    fn ranked_tree_capacity_rejects_before_allocation_without_poisoning() {
        let mut session = ranked_session();
        let registered = session
            .register_construction(ranked_construction("root"))
            .expect("registration");
        session.inner.operation_tree_work = crate::HARD_MAX_SESSION_OPERATION_TREE_ITEMS - 1;

        assert!(matches!(
            session.construct_registered(registered),
            Err(ProductionSessionErrorV1::Operation(
                OperationHandleError::SessionOperationTreeLimitExceeded
            ))
        ));
        assert!(!session.is_poisoned());
        assert!(session.inner.operations.is_empty());
    }
}
