# Private Root-To-Issuer Launch Checkpoint

Date: 2026-09-29 UTC. Tested code: `8087ca6f06302194d3f287f5b893c27d8f4a50d9`.

Follow-up: [actual startup matrix and root observation](native-root-observation-20260929.md).

[Issue #272](https://github.com/harsh-nod/fe2o3/issues/272) and
[compiler occurrence #218](https://github.com/harsh-nod/fe2o3/issues/218) remain
incomplete. This checkpoint implements a private launch primitive and shared
transport mechanics. It does not establish an installed production compiler
transaction, protected proof execution, GPU execution or 47/47 qualification.

## Implemented

- One exact pipe framer now serves the existing V2/V3 supervisor waits and the
  private root issuer launch. Readiness requires the complete record plus EOF.
  Nonblocking read-end validation and an oversize sentinel reject truncation,
  trailing bytes and packet-mode writes that could otherwise hide a suffix.
  The 120-byte pipe limit is separate from the unchanged 88-byte SEQPACKET limit.
- The private V3 launch consumes genuine prepared root custody and the original
  confirmed compiler trace's one-use inputs. It stages the pinned issuer image
  and existing descriptor ABI, and retains the complete preparation, compiler
  backing dependency, fresh service-owned key and manifest before clone.
- Profile checks, gate release, exec-status EOF, readiness-plus-EOF, launch-record
  matching and liveness precede exec confirmation. Parent and staged writer
  aliases close before readiness EOF. Failure or unwind cancels the original
  compiler, including failure of the outer scope's final accounting check.
- Checked work/storage queries include overlapping dependency ownership,
  staging, finite waits and persistent cleanup funding. These are logical
  resource bounds, not time, stack or RSS guarantees. Readiness does not resume
  the compiler or authorize an occurrence.

Independent review found and corrected two integration defects. The compiler's
blocking service endpoint is now made exactly `RDWR | NONBLOCK` before staging,
without changing the opposite compiler endpoint or clearing unexpected flags.
Post-launch readiness validation now rejects substitution of either the original
work ledger or Budget address before accessing retained resources or native I/O.
Receiver admission and public handoff credential rules remain unchanged.

## Validation

Final serialized tests and compiler checks used one unchanged 9,275-file snapshot:
`5695189d495e3ac67b11cebf924f2a184a212e31533bcd73d62fca83ba1e335c`.

- Capability, client, coordinator, supervisor and spawn: **1,151 unit/integration
  tests and 430 doctests passed**; 55 tests ignored locally.
- `cargo-fe2o3` wrapper binary: **452 tests passed**, five ignored.
- Wrapper, codegen backend and coordinator all-target check passed with warnings.
- Unsafe-source policy: five passed, maintenance test ignored. Reviewed inventory:
  2,424 sites in 419 files. Scoped formatting, diff checks, eight hygiene-policy
  self-tests and the change-set hygiene check passed.

Counts exclude nested unit-test subprocess summaries and include both runnable
and compile-fail doctest groups. New tests cover real pipe framing, finite retries,
late success, resource refusal, descriptor mapping, endpoint flags, identity
substitution, retained-charge arithmetic and original-account continuity.

The final combined-build coordinator binary passed the nine-case native trace
diagnostic on `mi350-2`, including the exact issuer-input transfer quota and
backing-dependency charge. Binary SHA256:
`0d7aeac5678f4cf32e4497bdc86ac6ab9523ba402db96c30b7e999df1b4b3777`.
The diagnostic used a static fixture in a read-only, networkless, resource-limited
container, not an approved compiler. Its container, scratch files and private SSH
connection were removed. An earlier diagnostic also passed and was cleaned up;
only the final binary is identified here.

## Still Required

- Native positive and refusal tests for the actual root-to-issuer composition
  through Ready120, post-clone/deferred cleanup and complete launch-budget limits.
  Rootless tests and the compiler trace diagnostic do not establish those results.
- A connected root observer using actual original trace custody, authenticated
  issuer-only control transport, and live occurrence/publication-lock retention
  across Prepare, Issue and replay. Publication must retire that lock at the
  correct protocol boundary before the compiler writes its receipt transport.
- The owning production attempt, wrapper intake, runtime/helper association,
  descendant enforcement and installed activation. The installed coordinator
  does not yet call this private launch path for complete compiler attempts.
- Protected proof and target-matched GPU validation of the complete kernel
  manifest. No milestone or tutorial kernel qualification closes here.
