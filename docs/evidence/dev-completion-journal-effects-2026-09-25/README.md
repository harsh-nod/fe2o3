# Completion Journal Effects Qualification

Qualified source: signed `957c5ce64b9a8e7cfd43fa4a5172b58ba3aa86c2`.
Production integration: signed `024f2f78fbacdb1ccfb5169bb9bb316f203fdd38`.
The 38-stage source-bound campaign passes at the shared control/journal-effect
boundary. This is not a proof of production Context maps, quarantine, callbacks,
backend quiescence or native execution. A1/A2 and HIP/HSA parity remain open.

## Qualified Campaign

The checker authenticates 477 exact source inputs, including independently
authenticated historical owner and control closures. All 22 nested control
mutations flatten to their exact historical defects. Eight new shared-body
mutations leave the independent logical oracle unchanged. No historical checker
or evidence pin is changed.

| Check | Result |
| --- | --- |
| Opening and closing extended whole roots | 1,282 obligations each, zero errors, no diagnostics |
| Opening and closing control roots | Eight obligations each, zero errors, no diagnostics |
| Historical control mutations | All 22 compile-clean logical rejections |
| Journal-effect mutations | All eight compile-clean logical rejections |
| Frozen owner regression | 1,271 obligations, zero errors, no diagnostics |
| Tool/source brackets | Passed; pinned 190-file Verus closure |
| Checker calibration | Eight groups passed |
| Packet calibration | Six groups passed, including strict JSON types and exact campaign identity |
| Inherited cleanup calibration | Five tests passed |
| Retained campaign replay | Passed from the copied packet |

The only inherited proof-source change is a local
`hide(logical::producer_invariant_v1)` in the constructor/disposal caller. The
callee already provides the invariant-preservation fact. The directive prevents
unneeded expansion; all assertions, contracts and solver limits remain unchanged.
This is an explicitly qualified new source delta, not a repin of old evidence.

## Earlier Development

These logs are under `raw/development`; the later proof gate, 30 development
mutations and authenticated campaign are under `raw/qualification`. Runtime
production code is unchanged between the two signed sources above.

| Check | Result |
| --- | --- |
| Completion module, `module-6` | 11 obligations, zero errors; selected module only |
| Completion control, `control-4` | Eight obligations, zero errors; whole control root |
| Whole journal root, `whole-7` | Refused: 1,281 obligations passed, one inherited witness exceeded the solver resource limit |
| Canonical-name diagnostic, `identity-8` | Refused: the same inherited witness exhausted the resource limit |
| GNU runtime, `gnu-3` | 1,407 passed, 22 hardware-only ignored |
| musl runtime, `musl-5` | 1,407 passed, 22 hardware-only ignored |
| Runtime doctests, `doc-6` | 46 passed: four merged and 42 separate |
| Strict all-target Clippy, `clippy-4` | Passed |
| Workspace formatting, `fmt-6` | Passed |

The new Context regression runs 40 cases: every combination of stable-reader,
producer-reader and writer presence, across successful completion, definite
backend failure, quiescence without a result, terminal ambiguity and confirmed
pre-publication cancellation. It checks exact public status/quiescence, input
markers, reader counts, dependency retention and complete destination allocation
state. The eight terminal cases bypass settlement and test quarantine retention.
Post-prevalidation journal-failure injection in Context is still absent.

The two no-premise constructor-origin proof witnesses cover all four stable and
producer capacity combinations, and writer-only Success/NoEffect/Unknown with
both writer-capacity observations. The generic composition proves journal
result/state correspondence; supplied presence and capacity observations are
not authenticated Context-map or allocator facts.

Verus is the existing pinned `0.2026.08.09.92f466f` installation, with four solver
threads, `--no-cheating`, default SMT resource limits, JSON output and silent
triggers. The completion module command explicitly selects
`--verify-only-module production::completion`; the JSON's
`is-verifying-entire-crate` field is not relied upon for scope. The whole root
uses the established owner-lifecycle 1,200-second wall bound, and scoped
development runs use 600 seconds. No solver resource limit was increased.

Cargo uses locked/offline dependencies, four jobs, disabled incremental
compilation, `-p fe2o3-runtime --all-features`, and the disposable target path in
`build-cleanup.json`. Runtime tests use `--lib`, musl additionally selects
`x86_64-unknown-linux-musl`, doctests use `--doc`, and Clippy uses
`--all-targets -- -D warnings`.

## Retained Failures

- `composition-2` reports the missing no-producer intermediate-state proof and
  subsequently reaches its initially selected 600-second outer timeout. It is
  not a successful whole-root run or an accepted logical-negative control.
- `module-4` fails the writer witness assertions; `module-5` fails the concrete
  Unknown projection and exhausts resources in the mixed witness. Explicit
  recursive unfolding, a round-trip projection lemma and scoped hiding of large
  invariants fix these without deleting assertions or adding assumptions.
- `whole-7` fails the unchanged
  `owner_constructor_unknown_disposal_witness_v1` resource bound. Passing the
  completion module does not override this whole-root refusal. The canonical-name
  diagnostic also fails. `raw/qualification/opacity-1.patch` records the later
  one-line fix; its scoped check and full 1,282-obligation development run pass
  before the new signed-source campaign. Earlier failures remain rejected.
- `roots-1` incorrectly expected failed/quiescent writers to be removed. The
  runtime correctly retains Unknown. `roots-2` corrects the oracle and adds
  confirmed cancellation; `gnu-3` and `musl-5` additionally check exact status,
  quiescence and full unaffected allocation fields.

Earlier writer-only, selected-function and 1,406-test logs are intermediate
development checks, not evidence for the final whole composition. Failed source
iterations were not independently snapshotted; their logs are retained as
diagnostics, not replayable authenticated campaigns.

## Scope And Cleanup

Error-prefix states are pre-quarantine. Context maps/root removal, quarantine,
callbacks, dependency/status effects and producer-first reconciliation remain
unproved here. The campaign qualifies the shared journal effects and control
flow, not these additional production boundaries.
A1/A2, #182, native/resource gates and HIP/HSA parity remain open.

Both exact owned local scratch trees were copied byte-for-byte before removal,
retaining all 1,227 files. Retained replay, 68 recorded terminal proof-command
groups, final snapshot rechecks and an independent absence command pass.
Cleanup removes 24,326,144 path-accounted allocated bytes. Collection holds the
campaign lock and rejects changed files, symlinks, hard links, foreign owners,
mounts, live recorded groups and substituted campaign source/location.

The earlier task-owned CPU cache was removed after its jobs completed, reclaiming
1,908,985,856 path-accounted allocated bytes; an independent absence check
passed. The initial force-removal command was refused by the tool before
execution; non-force recursive removal succeeded. No MI300X resources were used.
This record does not claim a global process census or isolated hardware results.

Authenticate the enclosing signed commit, then replay from a checkout retaining
the qualified source inputs:

```sh
python3 -I -B docs/evidence/dev-completion-journal-effects-2026-09-25/audit.py
```
