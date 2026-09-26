// Checked source/occurrence transport only. Global/collective/launch safety stays pending.
pub use scoped_tile_transport_v29::*;
mod scoped_tile_transport_v29 {
    use super::*;
    use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as BlockCoordinate,
        CanonicalKirDefinitionCoordinateV1 as DefinitionCoordinate,
        CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgumentCoordinate,
        CanonicalKirEdgeCoordinateV1 as EdgeCoordinate,
        CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
        CanonicalKirOperationCoordinateV1 as OperationCoordinate,
        CanonicalKirUseCoordinateV1 as UseCoordinate,
    };
    use std::{
        cell::Cell,
        ops::Range,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    type R<T> = Result<T, ProductionTileScalarTransportErrorV29>;

    /// Candidate layout only, not a compiler policy or execution permission.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionTileScalarOrderV29 {
        /// Exact blocked tag; no independent safety authority.
        Blocked,
        /// Exact striped tag; no independent safety authority.
        Striped,
    }

    /// Where the unchanged candidate producer rejected its input.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionTileScalarPhaseV29 {
        /// Exact preparation tag; no independent safety authority.
        Preparation,
        /// Exact replay tag; no independent safety authority.
        Replay,
        /// Exact materialization tag; no independent safety authority.
        Materialization,
        /// Exact correspondence tag; no independent safety authority.
        Correspondence,
    }

    /// A pre-verification rejection; no variant carries a safety grant.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionTileScalarTransportErrorV29 {
        /// Exact missing donor tag; no independent safety authority.
        MissingDonor,
        /// Exact resource tag; no independent safety authority.
        Resource(ArgumentResourceV1),
        /// Exact candidate tag; no independent safety authority.
        Candidate {
            /// Exact phase component.
            phase: ProductionTileScalarPhaseV29,
            /// Exact reason component.
            reason: &'static str,
        },
        /// Exact inventory tag; no independent safety authority.
        Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
        /// Exact binding tag; no independent safety authority.
        Binding(&'static str),
        /// Exact panicked tag; no independent safety authority.
        Panicked,
    }
    impl From<ArgumentResourceV1> for ProductionTileScalarTransportErrorV29 {
        fn from(error: ArgumentResourceV1) -> Self {
            Self::Resource(error)
        }
    }
    impl From<fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1>
        for ProductionTileScalarTransportErrorV29
    {
        fn from(error: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1) -> Self {
            Self::Inventory(error)
        }
    }
    impl std::fmt::Display for ProductionTileScalarTransportErrorV29 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Resource(e) => e.fmt(f),
                Self::Inventory(e) => e.fmt(f),
                other => write!(f, "pending tile transport: {other:?}"),
            }
        }
    }
    impl std::error::Error for ProductionTileScalarTransportErrorV29 {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Resource(e) => Some(e),
                Self::Inventory(e) => Some(e),
                _ => None,
            }
        }
    }
    fn invalid(reason: &'static str) -> ProductionTileScalarTransportErrorV29 {
        ProductionTileScalarTransportErrorV29::Binding(reason)
    }
    fn candidate_error(
        phase: ScopedTileFailurePhaseV29,
        kind: ScopedTileFailureKindV29,
    ) -> ProductionTileScalarTransportErrorV29 {
        use ProductionTileScalarTransportErrorV29 as E;
        let phase = match phase {
            ScopedTileFailurePhaseV29::Preparation => ProductionTileScalarPhaseV29::Preparation,
            ScopedTileFailurePhaseV29::Replay => ProductionTileScalarPhaseV29::Replay,
            ScopedTileFailurePhaseV29::Materialization => {
                ProductionTileScalarPhaseV29::Materialization
            }
            ScopedTileFailurePhaseV29::Correspondence => {
                ProductionTileScalarPhaseV29::Correspondence
            }
        };
        let reason = match kind {
            ScopedTileFailureKindV29::MissingDonor => return E::MissingDonor,
            ScopedTileFailureKindV29::Resource(e) => return e.into(),
            ScopedTileFailureKindV29::Source => "source",
            ScopedTileFailureKindV29::SemanticSsa => "semantic SSA",
            ScopedTileFailureKindV29::SourceLaunch => "source launch",
            ScopedTileFailureKindV29::Canonical => "canonical",
            ScopedTileFailureKindV29::Occurrences => "source occurrences",
            ScopedTileFailureKindV29::Census => "census",
            ScopedTileFailureKindV29::Geometry => "geometry",
            ScopedTileFailureKindV29::NoTileOccurrences => "no tile occurrences",
            ScopedTileFailureKindV29::ReplayMismatch => "independent replay",
        };
        E::Candidate { phase, reason }
    }

    /// Inert exact ancestor locator, not a value-equivalence assertion.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionTileSourceCoordinateV29 {
        /// Exact function parameter tag; no independent safety authority.
        FunctionParameter {
            /// Exact function component.
            function: usize,
            /// Exact parameter component.
            parameter: usize,
        },
        /// Exact block tag; no independent safety authority.
        Block {
            /// Exact function component.
            function: usize,
            /// Exact block component.
            block: usize,
        },
        /// Exact block parameter tag; no independent safety authority.
        BlockParameter {
            /// Exact function component.
            function: usize,
            /// Exact block component.
            block: usize,
            /// Exact parameter component.
            parameter: usize,
        },
        /// Exact operation tag; no independent safety authority.
        Operation(OperationCoordinate),
        /// Exact result tag; no independent safety authority.
        Result {
            /// Exact operation component.
            operation: OperationCoordinate,
            /// Exact result component.
            result: usize,
        },
        /// Exact terminator tag; no independent safety authority.
        Terminator(BlockCoordinate),
        /// Exact edge tag; no independent safety authority.
        Edge(EdgeCoordinate),
    }
    /// A complete expansion piece, including logical erasure and zero-operation gaps.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionTileTargetCoordinateV29 {
        /// Exact function parameter tag; no independent safety authority.
        FunctionParameter {
            /// Exact function component.
            function: usize,
            /// Exact parameter component.
            parameter: usize,
        },
        /// Exact block tag; no independent safety authority.
        Block(BlockCoordinate),
        /// Exact block parameter tag; no independent safety authority.
        BlockParameter {
            /// Exact block component.
            block: BlockCoordinate,
            /// Exact parameter component.
            parameter: usize,
        },
        /// Exact operation tag; no independent safety authority.
        Operation(OperationCoordinate),
        /// Exact result tag; no independent safety authority.
        Result {
            /// Exact operation component.
            operation: OperationCoordinate,
            /// Exact result component.
            result: usize,
        },
        /// Exact terminator tag; no independent safety authority.
        Terminator(BlockCoordinate),
        /// Exact edge tag; no independent safety authority.
        Edge(EdgeCoordinate),
        /// Exact anchor tag; no independent safety authority.
        Anchor {
            /// Exact block component.
            block: BlockCoordinate,
            /// Exact operation gap component.
            operation_gap: usize,
        },
        /// Exact erased value tag; no independent safety authority.
        ErasedValue(ValueId),
    }
    /// Exact independently replayed fragment role, not a safety classification.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionTileExpansionStageV29 {
        /// Exact preserved tag; no independent safety authority.
        Preserved,
        /// Exact erased tag; no independent safety authority.
        Erased,
        /// Exact prelude tag; no independent safety authority.
        Prelude,
        /// Exact predicate tag; no independent safety authority.
        Predicate,
        /// Exact read tag; no independent safety authority.
        Read,
        /// Exact read block tag; no independent safety authority.
        ReadBlock,
        /// Exact join tag; no independent safety authority.
        Join,
        /// Exact conditional tag; no independent safety authority.
        Conditional,
        /// Exact read branch tag; no independent safety authority.
        ReadBranch,
        /// Exact parts tag; no independent safety authority.
        Parts,
    }
    #[derive(Debug, Eq, PartialEq)]
    /// Borrowed-query row for exact origin, not a safety grant.
    pub struct ProductionTileOriginV29 {
        source: ProductionTileSourceCoordinateV29,
        pieces: Range<usize>,
    }
    impl ProductionTileOriginV29 {
        /// Returns the retained source.
        pub const fn source(&self) -> ProductionTileSourceCoordinateV29 {
            self.source
        }
        /// Returns the retained pieces.
        pub fn pieces(&self) -> Range<usize> {
            self.pieces.clone()
        }
    }
    #[derive(Debug, Eq, PartialEq)]
    /// Borrowed-query row for exact piece, not a safety grant.
    pub struct ProductionTilePieceV29 {
        target: ProductionTileTargetCoordinateV29,
        origin: usize,
        stage: ProductionTileExpansionStageV29,
        component: Option<u32>,
    }
    impl ProductionTilePieceV29 {
        /// Returns the retained target.
        pub const fn target(&self) -> ProductionTileTargetCoordinateV29 {
            self.target
        }
        /// Returns the retained origin.
        pub const fn origin(&self) -> usize {
            self.origin
        }
        /// Returns the retained stage.
        pub const fn stage(&self) -> ProductionTileExpansionStageV29 {
            self.stage
        }
        /// Returns the retained component.
        pub const fn component(&self) -> Option<u32> {
            self.component
        }
    }

    /// Structural parent, separate from the complete source/result/attachment alias roster.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionTileProvenanceV29 {
        /// Exact piece tag; no independent safety authority.
        Piece(usize),
        /// Exact declaration argument tag; no independent safety authority.
        DeclarationArgument {
            /// Exact function component.
            function: FunctionCoordinate,
            /// Exact argument component.
            argument: u32,
        },
    }
    /// Actual inventory coordinate. Definitions and values stay typed by that inventory.
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum ProductionTileOccurrenceCoordinateV29 {
        /// Exact operation tag; no independent safety authority.
        Operation(OperationCoordinate),
        /// Exact definition tag; no independent safety authority.
        Definition(DefinitionCoordinate),
        /// Exact use tag; no independent safety authority.
        Use(UseCoordinate),
        /// Exact block tag; no independent safety authority.
        Block(BlockCoordinate),
        /// Exact terminator tag; no independent safety authority.
        Terminator(BlockCoordinate),
        /// Exact edge tag; no independent safety authority.
        Edge(EdgeCoordinate),
        /// Exact edge argument tag; no independent safety authority.
        EdgeArgument(EdgeArgumentCoordinate),
    }
    #[derive(Debug, Eq, PartialEq)]
    /// Borrowed-query row for exact occurrence, not a safety grant.
    pub struct ProductionTileOccurrenceV29 {
        coordinate: ProductionTileOccurrenceCoordinateV29,
        provenance: ProductionTileProvenanceV29,
        definition: Option<usize>,
        target_definition: Option<usize>,
        value: Option<ValueId>,
        piece_aliases: Range<usize>,
    }
    impl ProductionTileOccurrenceV29 {
        /// Returns the retained coordinate.
        pub const fn coordinate(&self) -> ProductionTileOccurrenceCoordinateV29 {
            self.coordinate
        }
        /// Returns the retained provenance.
        pub const fn provenance(&self) -> ProductionTileProvenanceV29 {
            self.provenance
        }
        /// Returns the retained definition.
        pub const fn definition(&self) -> Option<usize> {
            self.definition
        }
        /// Returns the retained target definition.
        pub const fn target_definition(&self) -> Option<usize> {
            self.target_definition
        }
        /// Returns the retained value.
        pub const fn value(&self) -> Option<ValueId> {
            self.value
        }
        /// Returns the retained piece aliases.
        pub fn piece_aliases(&self) -> Range<usize> {
            self.piece_aliases.clone()
        }
    }

    /// Owns one current candidate and complete immutable source ancestry.
    /// This cannot be converted to ranked safety, a launch receipt or an executable.
    pub struct ProductionTileScalarTransportOwnerV29 {
        candidate: ScopedTileScalarCandidateV29,
        tables: Tables,
        retained: usize,
    }

    #[cfg(test)]
    impl ProductionTileScalarTransportOwnerV29 {
        pub(super) fn candidate_for_test_v29(&self) -> &ScopedTileScalarCandidateV29 {
            &self.candidate
        }
    }

    include!("production_scoped_tile_transport_resources_v29.rs");
    include!("production_scoped_tile_transport_index_v29.rs");
    include!("production_scoped_tile_transport_source_v29.rs");
    include!("production_scoped_tile_transport_queries_v29.rs");
}
