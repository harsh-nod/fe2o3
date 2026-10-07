# Repeated ordered-program validation measurements

This opt-in, test-build-only observer measures the existing diagnostic validation
transition for the exact one-, three- and sixteen-instruction Rust fixtures.
It is an implementation for collecting evidence, not a published performance
result or acceptance of the proposed authoring budgets.

One genuine rustc invocation reaches one real `after_analysis` callback. In that
callback, five calibration transactions precede thirty measured transactions.
Each iteration obtains a fresh authenticated source transaction through
`transaction_in_active_session_v1` and consumes it through the unchanged
`observe_ordered_program_v32`. It does not replay an exported KIR graph.
The source-collection and consuming validation intervals have separate clocks.
The returned original owner remains live for the complete baseline-byte,
descriptor, register and identity checks, then is dropped before the sample is
published or another transaction is created.

The original transition owns its original per-transaction Work and storage
budget. The observer does not construct or reset a Work account, release a
reservation, retry a refused transaction, or mutate production policy. Its
fixed 35 transactions are separate observations, not 35 retries of an exhausted
transaction. At most one observed owner is live at once. Rustc's session and
query cache remain live for the complete callback; their storage is not counted
by the two retained receipt sizes and is not claimed to be a complete peak.

The fixed callback stopping budget is 60 seconds, checked before every
transaction and after every completed observation, including final publication.
It cannot interrupt an in-flight compiler operation. An independently reviewed
external process deadline, source/tool currentness checks, resource floors and
stream limits remain mandatory. The old one-attempt observer and its schema,
limits and default behavior are unchanged.

Each finished sample is serialized under a separate 2-KiB limit, written as a
versioned line and flushed before later work. An aggregate report is separately
limited to 64 KiB. The parser caps its entire input at 256 KiB, covering both the
35 raw sample envelopes and final report; unrelated enclosing process limits
are not increased. A refusal stops the series and retains its completed prefix.
Started and completed transaction counts differ if a compiler fatal interrupts
an iteration. A panic, failed input recheck or final output refusal may leave no
aggregate; that is a failed observation, never a zero-duration or partial pass.

## What the numbers mean

The thirty retained values exclude all five calibration points. The parser
recomputes nearest-rank p50, p95 and maximum from the raw values, without dropping
outliers. Thirty samples use sorted indices 14, 28 and 29 respectively.
Both calibration and measured observations must be present, in order, and agree
exactly with the final aggregate. Every successful sample must have checked the
entire baseline while its owner was live and retained identical canonical,
semantic, inventory and preflight identities and receipt byte counts.

This is repeated validation in the same real compiler callback. Descriptor
generation happened earlier in macro expansion/CTFE. Therefore
`generation_ns: null` and `warm_stage: false` remain mandatory. The observer does
not claim warm CTFE, exclusive helper execution, whole-owner storage, a warm
end-to-end compiler, a performance service guarantee, production admission,
proof, artifact publication or GPU execution.

## Root-owned execution

The ignored test is
`production_rustc_driver_v1::ordered_program_stage_measurement_v1_tests::validation_series::actual_source_validation_series`.
It requires `FE2O3_ORDERED_VALIDATION_SERIES_V1_CONFIG` to name a retained exact
absolute JSON file with schema `fe2o3-ordered-validation-series-input-v1`.
Its other closed fields are the existing `steps`, `rustc_args`,
`baseline_path`, `baseline_bytes` and `baseline_sha256`.

Prepare a new real-source invocation and independent original-route baseline
using the existing operational adapter. Do not relabel an old binary, source
tree or artifact as the current one. Carry its exact crate binding, Cargo
observation, fixture environment and independently pinned dependency closure
into the new process. Preserve all failures and execute one profile per fresh
bounded process with `--exact --ignored --nocapture --test-threads=1`.
No compiler command, dependency installation or benchmark runs automatically
when the parser is imported.

Read-only consistency checking:

```sh
node scripts/ordered-program-validation-series-v1.mjs \
  /absolute/retained/series.stdout /absolute/retained/config.json \
  /absolute/retained/baseline-v17.bin
```

The parser requires the actual config and baseline digests, exact 1/3/16 selector,
complete 5/30 series, every clock and owner-drop marker, fixed stopping budget,
baseline and identity joins, raw-prefix/aggregate equality, recalculated
quantiles and explicit unavailable/authority fields. It rejects the historical
one-shot schema, partial series, dropped/duplicated/reordered samples, unknown
fields, duplicate JSON keys, altered selectors and fabricated generation or
authority claims. These checks establish report consistency, not producer
authentication; root retains and reviews the actual source, binary, invocation,
raw exit and currentness evidence separately.

Component controls use synthetic records only and do not constitute timing
results or genuine-source qualification. The original ignored probe remains
available independently. No milestone is completed by adding this observer.
