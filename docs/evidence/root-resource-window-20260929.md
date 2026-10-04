# Original-Account Resource Windows

Checkpoint for [#272](https://github.com/harsh-nod/fe2o3/issues/272), following
[publication custody](root-publication-custody-20260929.md). M0 is accepted;
**M1-M7 remain incomplete and the strict production-to-safe-GPU matrix remains
0/47.** This checkpoint does not execute production rustc, Verus or a GPU.

## Implementation

`CanonicalKernelIrVerificationResourceBudgetV1::with_additional_storage_window_v1`
adds an allocation-free window to an original owned account. Its floor is the
actual entry storage, not a caller-selected exclusion. Reservations obey the
original total ceiling and every enclosing window. Releases cannot cross the
entry floor. Success, refusal, unwind and view replacement restore only window
control, never storage, work, peaks or denial history. Inline budgets cannot
create these windows; account coordinates are meaningful only during the
original borrow and grant no authority.

V5 custody composition binds one nonrenewable acquisition to the exact quoted
lease/token pair and original account. The caller retains the full prepaid
ownership charge. Each local operation additionally pays the complete quote as
overlapping storage; no existing reservation is subtracted. Acquisition, inert
subject construction and currentness revalidation reuse the existing engines.
The old public V5 entrypoints retain their strict whole-account 256 MiB limit.
The new composition enforces an artifact-local ceiling of at most 256 MiB on a
possibly larger, unchanged root account.

The real `RootPublicationCustodyV3` caller uses this composition. Lease and token
installation precedes fallible accounting; the independently funded cleanup
slot retains them on every exit. Subject joining is followed by exactly one
artifact-currentness check and the final native observation. The complete
observation quote includes recovery, acquisition, subject and post-join windows.

These are canonical-payload and counted-account bounds, not a universal RSS,
stack or filesystem-allocation bound. Ordinary-path opening inherits an
environment/path-guard allocation caveat. The native root's retained proc-FD
path bypasses that configuration. A future descriptor-only interface should
reuse `RetainedDurableDirectoryV1`, with exact admission and duplication charges,
not add a raw-path authority constructor.

## Local Validation

Implementation commit: `27d55c7c8`. Test corrections:
`e57ec184b08b35c4865b2169fda352a30ffd6d87`. The corrected guarded source snapshot is
`caf566b2601fc5306c1a3cfe23acd011b403e216ed84fcda3a9049e3c3105091`.
Builds used locked/offline nightly `2026-04-03`, one Cargo job, serial tests,
disabled GPU visibility, a 12 GiB process-memory ceiling and 1,200-second deadline.
All guards verified unchanged source and tool hashes.

The first run, r28, passed 345 artifact tests and failed one new exact-boundary
assertion. The test had used a conservative pre-acquisition quote instead of
the retained token's exact location footprint. r29 passed all 346 artifact tests,
then 390 broker tests; two broker assertions exposed the same work-quote mistake
and an old assumption that the whole request must fit the artifact-local limit.
Only tests changed: exact token work/scratch remain bounded by the advertised
quote, and the whole-request test now checks complete custody overlap rather
than conflating the two ceilings. r30 passed all three focused root-publication
tests. The other seven libraries passed together in r31.

These are distinct top-level tests across those runs, **not one all-green
combined invocation** or a sum of repeated subprocess tests:

| Library | Passed | Ignored |
| --- | ---: | ---: |
| Artifact transaction | 346 | 0 |
| Broker authority service | 392 | 20 |
| Compiler closure capability | 271 | 4 |
| Compiler execution coordinator | 254 | 6 |
| Compiler execution issuer | 31 | 2 |
| Compiler execution protocol | 110 | 0 |
| Kernel IR | 1,023 | 0 |
| Process identity | 16 | 0 |
| Protected service spawn | 258 | 7 |
| Total | 2,701 | 39 |

The new tests include ten window/account controls, five artifact composition
controls and the broker's acquisition/refusal/unwind lifecycle cases. r38 passed
248 documentation tests: artifact 32, broker 92, kernel IR 59 and spawn 65.
Formatting passed for all 16 changed Rust files. Diff checks, the hygiene delta
policy and all nine M0 consistency tests passed. Source-manifest validation still
reports 50 fixtures, `qualified=false`, and the existing unmatched
`gemm-proof-plan` curriculum path. No manifest status or selection changed.
Existing compiler warnings remain.

## MI350 Validation

Three fresh static images from the corrected source passed the secure-entrypoint
checker before and after symbol-table stripping. Original build outputs were
preserved. Packaged image identities:

| Image | Bytes | SHA-256 |
| --- | ---: | --- |
| Issuer | 11,892,160 | `bb89a919433f461a1a0ae79abab1ea95a97c1d6f4af517cf0d3f1ae163759d7a` |
| Helper | 1,532,264 | `d0300c1307d53107827e431208054ee53f4e01af10621f86532383fce2c154a7` |
| Daemon | 1,483,112 | `88f72619476a9d6826405f638afce00f5633542353dbd52a3f6a701c65b742ba` |

The coordinator test binary SHA-256 is
`5925938aa14b30311cc2812bcc27cf62315868e872a0a77fd35df9702d268142`.
Both runs used isolated image
`sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f`,
no network or GPU devices, a read-only root, one CPU, 2 GiB memory, 32 PIDs and
a 600-second inner deadline. No unconfirmed job was retried.

- **Publication: 7/7 passed.** Drop, issuer removal, unwind, outer accounting
  refusal, invocation mismatch and short handoff ceiling retain their original
  268,435,456-byte accounts. Their maximum measured peak was 218,881,538 bytes.
  The new larger-root case starts with a 352,457,184-byte logical external floor
  on one original 620,892,640-byte account. Its peak was 571,338,605 bytes and its
  surviving storage stayed above that floor through terminal cleanup. This is
  a synthetic ownership shape, not an allocated or approved compiler runtime.
  An earlier inert writer transaction funds its surviving fixture inputs on
  the root before transfer; the request account is never reset. Subject joining
  and post-join currentness execute in this case. Maximum independent cleanup
  peak across the publication cases was 80,015,439 bytes.
- **Startup: 10/10 passed.** The unchanged readiness, continuity, substitution,
  timeout and resource-refusal matrix keeps its separate 1 GiB account. Usual
  peak was 742,629,942 bytes; expected short-storage refusal peaked at
  1,073,657,761 bytes. These are not publication-fixture resource limits.
- All work stayed within the advertised quotes. Both containers exited without
  OOM. Container, private staged inputs and SSH control-directory removal were
  verified; both cleanup reports contain zero errors.

Local evidence remains in the sibling
`fe2o3-issue272-production-next-evidence-20260921` directory. Publication/startup
reports are `root-window-publication-r36/status.json` and
`root-window-startup-r37/status.json`, SHA-256 respectively
`13da1e3a58bb1d28f0a1995bdf037e27443981d12f35b1037a125b946a5ce751` and
`bd9ac472a6b7fe2aa019b92f131356cafbcfa04b7c25b6936d285587c58f294b`.
Packaging report `root-window-packaged-r35/packaging.json` is
`e373205d1d8fbc26a0d2e55c593566c8e2499e6737dcae29acd351c016bbbe5c`.
The seven-case driver preserves the earlier six-case driver and checks exact
account limits, retained floors, peaks, denial fields and work quotes in addition
to success markers; its SHA-256 is
`6c6edf4ca835f7398475ffe50d397c1753b304eaecb0614610e4fe8bc11ec97d`.

## Remaining Production Work

The normal wrapper still does not submit compilations through this root path.
Authenticated per-compilation intake, retained source/cwd/stdio association,
complete runtime enforcement, protected proof RPC, durable retirement, original
trace completion, generic finalization and safe launch remain required. In
particular, the old invocation-authority stream and child-created native channel
both reserve FD195: their parent/child ownership transition must be integrated,
not bypassed. `RuntimeEnforcementUnavailable` remains in place.

Read-only host inspection found the pinned isolated proof image and retained
proof volume on both MI350 hosts, but no fixed-path approved compiler inventory
or policy installation in their host namespaces. Image/volume metadata is not
proof of admitted contents, complete compiler isolation or proof execution.
Source-side checked executable continuation for context-bearing kernels also
remains separate from authenticated MIR admission and diagnostic observations.
No additional milestone, website deployment or kernel qualification is claimed.
