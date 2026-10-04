# Retained local-provenance component — 2026-09-28

The private analysis component now retains the actual carrier owner, all three
provenance results, all three edge tables and three distinct FIFO worklists.
Earlier results and partial allocations stay attached when a later operation
fails. The original return APIs and their production behavior remain unchanged.

The complete original provenance algorithm is preserved with explicit ownership
adaptations. Definition/escape shape validation still precedes carrier analysis;
carrier errors precede result initialization. Stable arguments, allocation
origins and allocation contracts remain separate. Pointer offsets do not
manufacture private storage or range authority. Each original FIFO call remains
at its original location and preserves successor order.

The component adds a 32-work-unit prefix and 25 typed logical-accounting rows.
Carrier and worklist frames are charged separately. The three new queue calls
explicitly account for both caller vector references and coerced slice
references. This is conservative logical accounting, not native stack, allocator
capacity or RSS measurement.

All 11 controls passed, including independent original-data/debit comparison,
nonexclusive ABIs, raw/private/pointer-offset distinctions, original refusal
priority, exact/short resource limits, partially populated tables and edges,
callback error/panic custody, wrong source/ledger refusal and second-result
allocation denial. Full regression passed 356 model and 3,256 backend tests
(197 ignored), plus backend and extractor builds.

Full regression receipt: `b1c52c4e69e42b67ede190a08d83123ab1743a3a1011b40dd5b8628d46b1e12f`.
Independent source review:
`aefb0a0919290c6ba90ed4099e2f2afad15eca0bfc1c86a75fc8d27662568d80`.

This is an isolated retained-data component, not an authenticated compilation
checkpoint. Its lexical address/ledger checks do not establish durable source
authority. A genuine factory must preserve original source loans and all pending
owners across checked postflight, drop them, then refund accepted credits.

The genuine BeforeProvenance boundary is earlier: Option → enum → scalar.
This component supports a later provenance-to-BeforeAllocation continuation.
Neither connection is established by these component tests. Allocation and
capability integration, argument writers, joint bounds and production admission
remain open. No GPU or native debugger execution was performed here.
Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).
