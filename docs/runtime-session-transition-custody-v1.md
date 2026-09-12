# Session Transition Custody V1

R88 implements CONTROL-1 above R87 pending allocation custody. It closes the
session allocation/projection and consuming seal/map ownership gaps, not the
outer dispatch preparation or queue constructor owner.
The [local evidence](evidence/local-r88-session-transitions-2026-09-11/README.md)
records acceptance and the exact remaining proof/native boundaries.

R106 extends this adapter to [owning coherent initialization](runtime-borrowed-coherent-initialization-v1.md#owning-copy-extension).
Its CPU token now remains rooted through the copy stage before the existing map
transition takes over. Its [R106 local evidence](evidence/local-r106-coherent-initialization-custody-2026-09-12/README.md)
qualifies the named CPU/shared-sequence matrix; R88's historical evidence does
not qualify that extension.

## Ownership

The production session methods now use one private generic adapter over the
existing memory engine and queue foundation. Typed input and output slots live
outside its unwind boundary. Newly returned CPU tokens are stored before
evidence, allocation projection and foundation replacement. Seal and map native
bodies borrow the input; only successful native completion retags it, with no
fallible work between the move and output installation. Mapped successors remain
rooted through projection and replacement. Retain validates the borrowed token
and native record before consuming it into the existing queue-resource authority.

On error or unwind after a local input is admitted, or after allocation returns
a token, the adapter closes the session and moves the actual token fields into
one inline terminal owner. It preserves session/allocation/generation, complete
layout, exact profile and state types, flags/userptr classification, transition
stage and any observed seal/map outcome. Returned map prefix and syscall success
are untrusted observations; neither grants completion, cleanup or retry authority.
An input retained after an uncertain native transition carries its precursor
type only as historical custody, not as a usable writable/unmapped capability.

Terminal custody has no reconstruction, retag, extraction, disposal, Clone or
Copy operation. Its storage is fixed and cannot be overwritten by another
attempt. No native cleanup, currentness callback or allocation runs while
transferring failure ownership or resuming the original panic. Existing native
records and backing charges stay in the engine; no debit is refunded or moved
from a retained record merely because its token became terminal custody.

## Admission And Compatibility

Public method signatures, admitted profiles, allocation layout, native
validation and low-level seal/map ordering are unchanged. Private queue-resource
retainers now borrow the session mutably for terminal settlement. This does not
create a new allocator, mapper, loader, planner, queue or public authority type.

The early active/occupied-slot guard rejects already-terminal sessions without
another revision/currentness observation. A validated local consuming input is
retained on preflight failure; nonwrapping certificate exhaustion still poisons
the process gate before native effects. Pure invalid/foreign input rejection
retains the existing consuming-API behavior and grants no output authority. The
adapter does not promise unbounded storage for arbitrary rejected foreign or
already-terminal inputs. Pure allocation rejection before a native attempt has
no new token to retain. Failure inside allocation before a completed token exists
continues to use R87's pending native owner, not fabricated token custody.

Executable and kernarg mutable-byte access also quarantine on opening/closing
currentness or callback panic. An original materialization/backend-access panic
is resumed without a later currentness check replacing it. The actual token is
borrowed from the caller throughout this operation; the outer preparation owner
must still keep that caller-owned token across unwind. Other unconfigured byte
access profiles retain their existing public borrowed-access behavior. R106's
private owning coherent copy opts into first-panic preservation even without a
configured backing account; the outer transition retains the token on failure.

## Validation Boundary

CPU tests exercise the production adapter with actual fake-native records,
backing accounts and the queue foundation. Natural native/model aperture
divergence and conflicting model mapping state force post-native rejection.
Additional test-only stage faults exercise evidence, projection and replacement
error/panic, including deliberately induced late revision exhaustion. Late
exhaustion is not claimed reachable after correct preflight in ordinary execution.

The acceptance matrix includes exact revision headroom, every supported shared
allocation profile, seal failure/currentness, map errno and partial prefixes,
original panic payloads, precursor versus successor retention, original N1/N2
debits, borrowed and consuming retain rejection, terminal-slot nonreplacement,
materialization and a successful certified foundation-loan round trip. Scripted
bytes are not authenticated machine code or evidence of GPU execution.

## Still Open

- CONTROL-2 must root packets, plan, generation, original data/premises, complete
  code prefixes, current control stages and final dispatch outside preparation.
- CONTROL-3 must settle both bind paths through operation/retake/validation
  failure or panic, including the three-binding terminal-versus-retryable case.
- NATIVE-2 constructor custody and DATA-ADOPT/ISSUE/COMPLETE remain separate.
- Unreturned backend mappings, teardown/unmap/release adapter custody and
  process-aborting allocation failure are not repaired by these construction
  transitions. Existing native descriptors still have no implicit Drop cleanup.
- Kernarg/executable/control budgets, aggregate terminal storage accounting,
  protected compiler acceptance, executable refinement, Linux qualification
  and matched performance remain open. Historical model proofs do not prove
  this new adapter or the complete runtime.
