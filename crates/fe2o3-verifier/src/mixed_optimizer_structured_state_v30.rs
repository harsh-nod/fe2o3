//! Shared generated physical-state interface; this text grants no proof authority.

pub(super) const STATE: &str = r#"
struct AggregateStateV30 {
    pc: int,
    values: Seq<int>,
    cells: Seq<int>,
    initialized: Seq<bool>,
    external: int,
}
// Values are exact typed logical payloads; scalar bits/Select use the common
// emitter. Selected private leaves have concrete state, while unchanged external
// operations retain their shared interpretation and explicit effect order.
"#;
