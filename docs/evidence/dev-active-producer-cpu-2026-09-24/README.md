# Active-Producer Typed Launch CPU Qualification

The corrected `cpu2` campaign passes all thirteen stages. GNU and musl each
pass 35 focused producer-launch tests and all 1,400 runtime tests, with the same
twenty hardware-only ignores. All 46 runtime doctests, strict Clippy, formatting,
default-feature checks and tool-identity brackets pass. The 3,960 source inputs
are unchanged across the campaign, with an exact two-path delta from the
authenticated baseline. `audit1` and `audit2` pass replay and all ten evidence-test
groups. The owned cache is removed: 1,041,485,824 path-accounted allocated bytes,
with absence checked again after removal. Later numbered audits bracket the
final documentation separately. This is CPU qualification, not native, formal
or performance acceptance.

## Scope

This extends the ordinary producer-aware launch in signed `ad8ad561d`, not
protected Worker or generated execution. Only exact authenticated producer/event
pairs may defer readiness of the two Read inputs of the admitted three-binding
DeviceLocal R/R/W path. Each deferred owner must be the exact active producer,
with matching prepared/published execution and retained allocation custody.
The output still requires ready backing. No synthetic ready receipt, new queue,
pending registry, allocation table or materialization fallback is introduced.

After explicit dependencies and stream ordering settle, the original ready
admission is recomputed before staging. Missing ready backing fails the
unpublished child; ordinary early-successor publication cannot bypass this
check. Final artifact authorization, snapshot checks and owner extraction remain
in the existing publication path.

Eight added tests cover same-/cross-stream admission, released public events,
consumer-first observation and explicit flush, cancellation, rejected identity/
extent/custody shapes, failed restoration, final authority denial, terminal
parent custody, and failed/quiescent receipt handling. Scripted owners are not
native GPU receipts. Injected completion-record outcomes are contract tests,
not native device-fault evidence.

## Qualification Policy

The thirteen-stage runner uses pinned nightly 2026-04-03, two build jobs and four
test threads. Each GNU/musl target must pass default-feature all-target checking,
35 producer-launch tests and the full runtime suite: 1,400 passed with the same
twenty hardware-only ignores. Formatting, strict all-feature/all-target Clippy,
46 runtime doctests and opening/closing compiler/Cargo identities are required.

Source qualification requires an unchanged input bracket and exactly two changed
runtime files relative to the authenticated preceding CPU packet. KFD and
runtime-model production sources are unchanged. Their previous full-library
results are historical baseline evidence, not suites rerun by this campaign.

The initial `cpu1` attempt remains failed: its sixth command exits 101 after
the test process aborts. The separately recorded serial `repro1` exits by
SIGABRT and exposes the incorrect destination-storage assertion. The original
runner and source patch against `ad8ad561d` are retained with the failed attempt.
Only after both attempts terminated was the assertion corrected to preserve
`H2dReady`, with the existing R57 gate replacing a needless unsafe test authority.
The runner's unexecuted incorrect doctest expectation was also corrected from
54 to 46, baseline inputs were authenticated before parsing, and focused-roster
equality was made explicit. `cpu2` is a fresh warm-cache run, not a cold build.

## Evidence And Cleanup

Replay checks the exact stage and file roster, command/environment/time controls,
reaped dispositions, output hashes, unchanged source/tool identities, inherited
named test outcomes, eight exact new tests, and GNU/musl equality. Ten rejection
test groups use disposable copies only. They exercise helper authentication
before compilation, baseline authentication before parsing, source substitutions,
receipt controls, missing/aliased trees, exact named rosters, doctest package
counts, canonical CLI roots and failed-command cleanup receipts.

`finalize.py --cleanup` first records successful replay and rejection tests,
then validates the exact failed, diagnostic and successful command rosters.
The seven original failed/diagnostic receipts are hash-pinned. All 22 campaign,
diagnostic and final-audit command groups must be absent before deletion.
Only the literal owned private `target` is eligible: every descendant must be
ordinary, owned, same-device and not a symlink or mount. The corrected inherited
remover writes distinct exclusive before/after receipts. This assumes a stable
private workspace, not a hostile-concurrent-filesystem or hermetic attestation.
No file or process on `mi300x` is created or removed by this CPU packet.

`raw/cleanup-before.json` and `raw/cleanup-after.json` record the distinct
initial and successful final observations. Raw artifacts retain the original
failure, diagnostic reproduction and corrected campaign without replacing any
failed receipt. `artifacts.json` inventories raw bytes; `SHA256SUMS` closes the
packet, including recorded audits.

Replay from the qualified source checkout:

```sh
python3 -I -B docs/evidence/dev-active-producer-cpu-2026-09-24/qualify.py --output docs/evidence/dev-active-producer-cpu-2026-09-24/raw/cpu2 --verify
python3 -I -B docs/evidence/dev-active-producer-cpu-2026-09-24/test_qualify.py
```

## Remaining Gates

Fresh native witnesses must exercise both a genuinely queued producer and a
producer with actual native published custody, using the unchanged two-launch
R57 authority. Exact event release, producer-first logical settlement, every
byte of A/B/C/D, and explicit logical/native teardown must be checked.

Production/model correspondence for deferred admission and mixed Context
transactions remains open. This packet does not prove physical overlap,
general pending-input compute, generated graphs, aggregate memory, Worker V3,
GPU memory ordering, fault tolerance, performance or A1/A2 closure.
