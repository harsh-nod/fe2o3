# Private Generated ISSUE

Status: I2 development above N5, not milestone acceptance. R125 (Native,
CPU/test), R118B C1/C2/C3 and R116/V3 remain accepted. A1/A2, #182, protected
Worker application authority and HIP/HSA parity remain open.

## Connected Path

The existing owned preparation driver now connects DATA adoption to private
submission and physical completion. Public preparation/reservation are still
nonexecuting; activation remains private. No alternate queue, loader, signal,
packet, ordinary launch recipe or completion reply is created.

Context pre-reserves its attempt and ordinary submission registries, mints one
Context submission ID, and retains the exact logical allocation plan plus the
original descriptive source/access roster before backend entry. The original
move-only hold, carrier and R80 completion consumer/producer stay in the owned
operation registry. Owned failure retains that registry and Context together;
transferable-engine activation remains rejected.

The backend pre-reserves one ID-to-shell entry and roots one backend ID under
the existing generated native owner. The bounded index counts once toward
submission capacity and provides direct lookup. Its entries do not also occupy
the ordinary backend completion map or compute completion reservations.

Authority-bracketed Context advancement invokes the original lane's classified
fixed-dispatch submission. Only its explicit RetryableBeforeSideEffect outcome
restores Ready, using the same IDs and permit. Generic backend poll/wait/cleanup
never submit a Ready generated entry. Backend events and ordinary launches do
not acquire generated authority or allocation handles.

## Custody And Settlement

The generated owner retains Ready, Published(batch), Completed(completed),
Recycled, or a precise consuming-lower-handoff marker. Returned publication,
pending and completed receipts are installed inside the lane callback, before
outer lane restoration/currentness. Recycle's explicitly returned retryable
completed receipt stays pending. Lower errors/panics without returned custody
retain the original queue and handoff marker; no receipt recovery is fabricated.

Opening/closing device identity and exact lane leases remain checked. Context
validates the stored token against both the existing submission registry and
the exact attempt ID/held stream/device before native progress or retirement.
Closing Worker-currentness failure or ambiguity makes the mutation attempt
sticky Unknown and terminalizes Context/backend. This is a first-run,
non-reusing mutation hook, not the production version journal, NoEffect recovery,
successful output settlement or permission for cross-run reuse.

Physical completion parks the async driver without repeated hashing or
currentness callbacks. Generic backend observation reports quiescence without
a generated result, never Succeeded. No typed output is decoded or delivered.
Generated entries participate in cold-SDMA mutation exclusion as well as the
existing lane/persistent exclusions.

## Drain And Stop

Normal drain is non-cancelling. It retains issued/physically settled generated
operations until C4 supplies output delivery; exhausted drain seals Context
under the existing policy. It cannot claim a successful generated drain.

Actual Stop first resolves observers as EngineStopped. Only then may the
generated driver request disposal. Definitely unpublished Ready work uses the
existing pristine abort without another issue attempt. Published work receives
at most two observation/recycle steps, without waiting or publication. Pending
or ambiguous work remains retained until process exit.

Recycled work uses original recycled detach, not pristine abort. Returned DATA
is rooted immediately and disposed in forward order. Closing native identity
precedes lane release; exact native disposal precedes backend submission removal,
DisposedWithoutResult, Context records, shell/credit disposal and final hold
release. An entered failure preserves the remaining custody and prohibits retry.

## Evidence And Remaining Work

See the [development receipt](evidence/dev-i2-generated-issue-2026-09-17/README.md)
for exact commands, results and source identities. CPU receipt tests use generic
drop-counted owners; Context tests use actual registration/admission code over
mock backend metadata; async tests use injected hooks and the real owned engine.
None of these fixtures proves protected Worker-to-native composition.

Exact-fixture native probes cover cold/bootstrap, primary/AUX and rebound using
the original lower APIs, check primary/AUX output bytes before detach, and retire
their resources. They bypass generated carrier/Context admission and establish
only the native behavior explicitly recorded in the receipt. Compilation is
not execution. Formal correspondence, native failure matrices, protected Worker
authority, C4 readback/decoding/reply, C5 public typed API, C6 generated graph/drain,
production journal/reuse, aggregate memory and matched performance remain open.
