# Conditional Native Handoff Checkpoint

This continues [CPU-bound conditional integration](cpu-bound-conditional-integration-20260926.md).
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7 remain
open. This is not 47/47 completion, protected compiler execution, machine-level
equivalence, or safe GPU launch. No new protected or hardware run was completed.

## Implemented Boundaries

The normal production lowering entry now dispatches original Direct/RawEmpty
conditional roots before the ordinary roster gate. It uses the existing source
replay, optimizer history and final content checks. The target account's native
256 MiB ceiling is selected before this work starts; the retained source account
is not replaced. Unsupported context materialization still rejects.

The continuation can assemble an internal V5 native handoff while retaining the
original compiler-owned prefix. It still ends at `ConditionalFinalizerRequired`
or an earlier error. Source wiring and packing component tests passed; a genuine
protected execution of this new route has not been observed.

The wire carries one conditional source packet, one history containing actual
final F, one contract catalog and descriptor, and the existing ModuleV2 envelope.
Invocation, original inventory/preflight, target layout, native lowering and final
module commitment remain bound together. Strict decoding establishes content
agreement, not original compiler custody. Existing V3/V4 wire families are unchanged.

Independent V5 recovery uses caller-accepted policies and history limits, replays
the source-through-F relation once, and moves the actual F allocation from the
decoded history. The existing transaction engine retains the original backing
and currentness lock while mapping to that recovered owner. Mapping does not
consume the journal occurrence or authorize artifact publication. Opaque proof
failures and unwinds retain terminal resource charges.

The distinct 690-byte execution SubjectV3 binds the exact V5 occurrence,
invocation/closure and seven cached content coordinates. Its final-module field
uses the lineage receipt identity, not the different internal FFI trailer digest.
Source-only substitutions and identical payloads from different attempts remain
distinguishable. The subject is move-only, version-separated and explicitly inert;
only the concrete raw consumed V5 owner enters its consumed-owner constructor.

The cached final receipt shares the original immutable backing. V5 decode work
now includes two additional commitment traversals: the allowance is V4 plus
`6*n`, or fourteen full-image visits plus the inherited bounded terms. Subject
construction charges 26,184 logical work units. Returned owner storage must be
reserved before retention; spare backing capacity and metadata stay prepaid.
These are logical bounds, not process RSS or exact instruction counts.

## Local Verification

Each result applies only to its named snapshot. Counts overlap and must not be
added. The package sets also differ from the preceding checkpoint. All runs used
pinned nightly 2026-04-03, locked offline dependencies, one build/test thread,
hidden GPUs, the original limits and a private writable `XDG_RUNTIME_DIR`.

| Snapshot | Scope | Result |
| --- | --- | --- |
| `7fce566fd` | lineage, FFI and transaction libraries/integration tests | 707 passed; two IPC tests explicitly excluded |
| `afb6d7e0f` | filtered backend, verifier, lineage, FFI and transaction libraries | 601 passed, 35 ignored, zero failures |
| `088c5aecb` | lineage conditional V5 integration tests | 13 passed |
| `d60a14961` | transaction library, including all 16 SubjectV3 cases | 294 passed; two IPC tests explicitly excluded |
| `d60a14961` | all five packages' documentation tests | 126 compile-fail and two positive controls passed |
| `510c10814` | SubjectV3 follow-up | 16 library and 26 transaction compile-fail documentation tests passed |

The 601-pass run includes all four production packing tests, the normal-entry
wiring test and nine recovery component tests. Ignored capture/proof tests supply
no execution evidence. Full-workspace and unfiltered transport success are not
claimed.

The excluded tests are `coordinated_fork_exec_does_not_leak_inherited_lock_aliases`
and `uncoordinated_fork_inherits_publication_lock_until_cloexec`. The isolated
coordinated test still fails because its IPC release write receives `EPERM` in
this environment. Its test-only pre-exec wait is now bounded, so the failure
terminates rather than hanging during cleanup. Production lock behavior is
unchanged. Earlier read-only runtime-directory failures, compile failures and
the interrupted unfiltered retry remain failed results, not passing evidence.

Log SHA256s for the 707-pass, 601-pass, 294-pass and five-package documentation
runs, respectively:

```text
45e10426705d2d41ec75d90e4c6457b55dc04fead5c1cf03a1870da50bf694d4
5f02111fdad0d672218d5b50b524b761e20fa974acf8684ef9d3ed79a2e01725
819dde6ff46008f98aa04bab37719f0adae051289a4c6942b25b03c5e2277b81
30fb59fbc9442b92c8e17bffa854002e326d9fd5b669c6ba102d3b85dfc73bb3
```

## Remaining Production Work

1. Finish resource-account postchecks before publication side effects, retaining
   the actual prepared prefix and original protected invocation. No extra replay
   marker or conversion to an ordinary roster receipt is sufficient or needed.
2. Extend genuinely admitted execution policy/client/service custody and exact
   receipt carriage to SubjectV3. The live backend still acquires SubjectV1.
   New transport or signature codecs alone cannot replace protected acquisition.
3. Supply independent conditional policies, history limits and profile to parent
   intake. Read the receipt under the raw token, recover once under currentness,
   perform real Worker preflight, then consume that same occurrence.
4. Integrate the conditional owner through the existing Worker, finalizer,
   restart recovery and generated safe host contract. Their current native or
   ordinary adapters cannot be relabeled as conditional V5 authority.
5. Complete machine semantic refinement and the target-matched tutorial matrix.
   Existing formula proofs use shared floating-point operators; this is not an
   IEEE/LLVM/ISA equivalence proof. The production machine-refinement backend is
   still required for both targets, including control flow, addresses, memory,
   execution masks and the explicit numerical contract. See
   [#214](https://github.com/harsh-nod/fe2o3/issues/214).

Both public mains were observed at `9a8407ccb` after this local work began.
Normal Git fetch and GPU-host SSH name resolution fail in the current environment.
The new local commits have not been merged with that concurrent source or pushed;
neither main may be overwritten. No new remote job or scratch directory was created.
