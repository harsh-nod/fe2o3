# Conditional Native Handoff Checkpoint

This continues [CPU-bound conditional integration](cpu-bound-conditional-integration-20260926.md).
The later [execution custody checkpoint](conditional-execution-custody-20260926.md)
records V3 sealed policy/key and anchor/currentness component integration.
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

Preparation now returns the original prefix/native owner only after its original
target-account postchecks. The existing finalizer refusal consumes that owner
afterwards with a read-only account borrow. Opaque errors and panics keep their
terminal charges; only a completed unchanged refusal retires the successful
preparation's storage. No new readiness marker or publication effect is introduced.

## Execution Receipt Components

V3 receipt transport uses the existing transaction, currentness and shared native
receipt engine. It binds the full SubjectV3 before publication or recovery and
rejects occurrence substitution, old families and conflicting sidecars. Locked
recovery accepts only the concrete raw V5 token, before verifier-owner mapping;
it does not decode the payload a second time. Postcommit validation is prepaid.
Consumed recovery can validate stored content after payload deletion but cannot
restore compiler/proof ownership. Receipt bytes remain opaque and inert.

The V3 attestation policy, challenge, request and signed receipt now have typed
publication, acknowledgment and complete carriage owners. Private implementation
bodies are shared with V2; V1/V2 wire, validation order and resource contracts are
preserved. SubjectV3 still uses the existing CompilerClosureV2 and ModuleV2;
these component versions are independent, not interchangeable labels.

Carriage checks all internal policy/request/receipt/acknowledgment relationships
on one account, returning unreserved storage quotes. Consumers borrow the subject
through the carriage-owned request rather than recreating it. Signature validity
and internal agreement do not establish protected policy pinning, a live compiler
occurrence, an authentic Worker journal record or durable currentness. The active backend still acquires
SubjectV1; V3 service/client custody and production publication remain unwired.

## Local Verification

Each result applies only to its named snapshot. Counts overlap and must not be
added. The package sets also differ from the preceding checkpoint. All runs used
pinned nightly 2026-04-03, locked offline dependencies, one Cargo build job and one test thread,
hidden GPUs, the original limits and a private writable `XDG_RUNTIME_DIR`.

| Snapshot | Scope | Result |
| --- | --- | --- |
| `7fce566fd` | lineage, FFI and transaction libraries/integration tests | 707 passed; two IPC tests explicitly excluded |
| `afb6d7e0f` | filtered backend, verifier, lineage, FFI and transaction libraries | 601 passed, 35 ignored, zero failures |
| `088c5aecb` | lineage conditional V5 integration tests | 13 passed |
| `d60a14961` | transaction library, including all 16 SubjectV3 cases | 294 passed; two IPC tests explicitly excluded |
| `d60a14961` | all five packages' documentation tests | 126 compile-fail and two positive controls passed |
| `510c10814` | SubjectV3 follow-up | 16 library and 26 transaction compile-fail documentation tests passed |
| `19494f59f` | transaction and execution-protocol libraries/integration tests | 642 passed; two IPC tests explicitly excluded |
| `19494f59f` | both packages' documentation tests | 81 compile-fail and one positive example passed |
| `19494f59f` | execution-client, closure-capability, cargo-fe2o3 and rustc backend, all Cargo target kinds | offline `cargo check` passed |
| `d9fd4468b` | backend library, `conditional_` filter | 206 passed, 29 ignored, zero failures |
| `ec5887b65` | V3 receipt transport, including underpaid locked recovery | 19 passed |
| `ec5887b65` | complete execution-protocol package | 180 library/integration tests, 64 compile-fail and three positive documentation examples passed |
| `fffdeb72f` | combined backend library, `conditional_` filter | 207 passed, 29 ignored, zero failures |
| `fffdeb72f` | same four downstream packages, all Cargo target kinds | offline `cargo check` passed |

The 601-pass run includes all four production packing tests, the normal-entry
wiring test and nine recovery component tests. Ignored capture/proof tests supply
no execution evidence. Full-workspace and unfiltered transport success are not
claimed.

The 642-pass run includes 18 new transport and 12 new attestation tests. The
later protocol run adds all 15 new carriage tests and retains the V1/V2 golden
and consumer regressions. The 19-test transport follow-up covers both targets'
locked recovery with an underpaid input floor: entry-only work, no scratch or
filesystem mutation, and the original ready token still usable afterwards.
Independent source reviews found no production correctness defect in the shared
transport, V3 core/carriage or preparation-order refactor. Review and component
test results do not replace protected execution or whole-workspace validation.
The final backend run includes four inert postcheck/ownership tests, covering
both preparation and consuming-continuation errors and panics without refunding
terminal charges. The 29 ignored source/proof tests remain unexecuted.

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

Later log SHA256s, respectively: 642-test run; both-package docs; downstream
check; 206-test backend run; 19-test transport run; complete protocol run;
207-test combined backend run; final downstream check:

```text
48a61c920476901bcbfb6b48bd3e7051edcdd0825e9478cd3b530d70de27c795
9620cf6eed1ed6419bba05a841fbdfceb67c4188b0b25c3a9377f74a6ce6111b
c4b70713968780fed11a9326ede81408cd010b1a6191f080ac276199e7a101e5
4ee33b39f9ed0bb375fbd673aa635e34d4ffcb7212d300c33141d7468c25ad73
6b30bd784f3364efaeffd7a1cb2988bf75269a9c121a012be8e19b4ec96c4fb0
b0685286d78ed875f76feedb3c30a85fb55f4e882a8b4237dd6caedb75170df7
bcffe23a02087e518d1fce2b97539b9fc95c06553505c0ee32ca0a6b09df2fae
ce9c8db0ea5ad26dcaeeddec684337189925d123a60a34f6067972d8afea2c56
```

## Remaining Production Work

1. Connect the postchecked prepared owner to the original publication continuation,
   retaining the actual prefix and immediately revalidating the original protected
   invocation before publication. No extra replay marker or conversion to an
   ordinary roster receipt is sufficient or needed.
2. Extend genuinely admitted execution policy/client/service custody to SubjectV3
   and its typed carriage. The live backend still acquires SubjectV1. New transport,
   signature and carriage codecs alone cannot replace protected acquisition.
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

Both public mains were most recently observed at `2aaa79c6b` during this local work.
Normal Git fetch and GPU-host SSH name resolution fail in the current environment.
The new local commits have not been merged with that concurrent source or pushed;
neither main may be overwritten. No new remote job or scratch directory was created.
