// Shared SSA mechanics only; issuer contracts remain in their typed children.
mod capability_ssa_graph_01 {
    include!("capability_ssa_graph_01/graph.rs");
    pub(super) mod shared_source_path {
        include!("capability_ssa_graph_01/shared_source_path.rs");
    }
}
