# Runtime Request Witness Development Evidence

Date: 2026-09-26. Base: `0d6ec62f0bef983ba8c0257ab565cfe2c9490829`.
This packet is single-device development evidence, not an accepted milestone,
whole-runtime formal verification or HIP/HSA parity claim.

## Implementation

- A composed KFD backend retains its request binding independently of one-shot
  native admission. Both legacy allocation entry points reject before handles,
  staging or native effects. All three startup switches consume composed intake.
- Context validates a complete device-keyed roster before exposure, rejecting
  missing/extra keys, duplicate keys, aliased request leaves and unhealthy
  sessions. It adopts the existing leaf without adding a fifth domain level.
- Private credit variants preserve typed reservations, retained custody and the
  owning batch iterator. No raw-account/token extraction is introduced.
  The composed-only binding is boxed so legacy account metadata does not inherit
  its 256-byte inline size; a layout regression test enforces that bound.
- A private, borrowed, non-Clone witness authenticates the Context device,
  independently held backend binding, exact leaf, model admission and byte
  extent. The default hook returns Unsupported without legacy dispatch.
  Rejection/settlement refunds and ambiguous-outcome quarantine are preserved.
- Generated shell registration retains and authenticates the complete request
  roster before journal/metadata commit. A move-only bound plan gates commit.
  Binder/journal rejection refunds unissued requests; unwind quarantines them.
  Exact retained-credit checks precede later generated plan use and retirement.
  DATA materialization consumes native backing, not a second logical request.
- Wrong native/request policy combinations seal startup. Multi-device
  construction rejects required-profile children until forwarding is implemented.

The stored generated marker attests registration, not independent native-layer
reauthentication of each token. Session health is observed at checked boundaries;
it does not exclude a concurrent external clone from quarantining during an
already-started operation. Corrupt-accounting refund failure is not atomic
rollback: already-refunded members stay refunded and remaining custody quarantines.

## Typed CPU Qualification

The shipped public KFD mint requires a real checked device. Tests do not weaken
that API or fabricate one. `qualification/overlay.patch` and `files.tsv` create
an isolated source copy with accounting-only private-admit fixtures, descriptive
model bindings, synthetic KFD storage, MockBackend outcome hooks and one journal
corruption injector. The KFD fixture returns only typed request handles and a
weak root observer; unused N1/N2 admissions are dropped. It is not native
execution or simultaneous three-class pressure qualification.

`check-overlay.sh` checks every unchanged file in the four package trees, the
complete file roster, exact fixture bytes and the complete overlay patch.
Production Context/wrapper/KFD allocation function bodies remain unchanged.
`raw/overlay-sources.sha256` pins the actual tested source; the final test ELF is
pinned separately. No fixture helper is added to the shipped crate interfaces.

All **14 test groups** pass in the final overlay run, including:

- Complete reordered rosters; independently exercised alias, duplicate-key,
  unknown-key and cardinality rejection; original backend custody on failed open.
- Foreign root/sibling/extent and Context/device-brand rejection; session sealing.
- Actual Context/KFD synthetic allocations and disposal for both memory kinds;
  immutable request admission and direct allocation rejection after shutdown.
- Cold Context and returned-backend root lifetime with no external owning clone.
- Unsupported, allocated, settled-no-owner, rejected, quiescent, terminal and
  panic outcomes, journal settlement and subsequent no-reentry checks.
- Generated installation/retirement; binder and post-bind journal rejection;
  panic quarantine; missing/extent/order/leaf/brand/tail witness rejection;
  exact-credit rejection at pre-adoption-plan and direct retirement boundaries.

Removing duplicate-leaf rejection in the isolated production body makes the
roster assertion fail (exit 101). `raw/negative-control.patch` records the
mutation; the production-body digest was restored before the final 14/0 run.
This is a semantic negative control, not a Verus proof or a native replay.

Reproduce with an unused absolute destination path:

```sh
bash docs/evidence/dev-runtime-request-witness-2026-09-26/qualify.sh /absolute/new/source-copy
bash docs/evidence/dev-runtime-request-witness-2026-09-26/validate.sh /absolute/owned/target
```

The scripts retain their caller-owned outputs for inspection and cleanup.

## Production CPU Results

The final focused run passes 65 tests (15 KFD, 50 Runtime). Broad libraries
report 1,347 KFD, 70 accounting, 1,487 Runtime and 1,034 model passes, for
3,938 total passes. The same four failures as the preceding development packet
remain: KFD's credential-bound telemetry channel reports `SocketAdmission`;
Runtime's cooperative telemetry, session-end and pre-native telemetry cases
report `InspectSocket` with EPERM. These are recorded failures, not passes or
successful qualification of those paths.

The broad run excludes 296 primary construction tests and ignores 19 model and
28 Runtime tests. Coverage groups overlap; this is not full CPU qualification.
The first validation run caught an oversized account enum in strict Clippy.
Its output is retained in `raw/pre-layout/`; the final source boxes only the
composed binding and adds the layout bound test. Raw statuses and source hashes
distinguish the pre-fix run from the final gates.

All 69 selected construction tests (67 KFD, 2 Runtime), 108 doctests
(31 KFD, 3 accounting, 47 Runtime, 27 model), strict all-feature/all-target
Clippy, no-default-feature checks and workspace formatting pass. The final
validation driver exits 1 solely because the broad library command exits 101
for the four recorded socket failures. No test process remains running.

Four final production test ELFs and the separate overlay test ELF are hashed.
Production and overlay source digests were checked after execution. The owned
668,784 KiB production build target and 512,836 KiB isolated source/build copy
were removed after terminal results and binary verification; absence was checked.
The unrelated owner-inspection evidence directory was left untouched.
Production/documentation whitespace checks pass. Aggregate staged whitespace
warnings are confined to preserved raw output and unified-diff context lines;
those evidence bytes are intentionally not normalized.

## Remaining Gates

The model and resource-accounting package sources are unchanged; this packet
does not rerun or extend Verus. Context/backend/ledger conservation refinement,
concurrent sealing and native layout/disposal correspondence remain open.
Native checked-device construction, SDMA-first/compute-first/generated-first
startup, DATA adoption/disposal, pool reuse and teardown still require hardware
qualification. The pre-adoption test does not execute native materialization.

Composed XGMI/multi-device forwarding is not implemented. Worker proxies do not
transport borrowed witnesses; wrapping this profile fails closed at the server's
legacy allocation call and is unsupported, not transparent admission transport.
Whole-process memory closure and broader language/collective/distributed/runtime
parity remain separate unfinished work.

MI300X access failed with hostname resolution before any remote command ran.
No remote artifacts were created and no performance measurements are claimed.
