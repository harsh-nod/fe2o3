# Source-Bound Conditional Policy Roster

Date: 2026-09-28. Parent: `2fde965fd13fc80edc11fae32f0acae71f3437ab`.
This continues the [native integration checkpoint](native-integration-resume-20260928.md).
It does not complete an issue #272 M0-M7 milestone, enable the conditional
production wrapper, or establish a protected Worker/publication or GPU run.

## Implemented Boundary

The compiler captures the actual effect and formula policies borrowed by the
existing retained-source callback. It does not reconstruct successful proof
owners from signatures or manufacture policies from untrusted packet fields.
The owned roster survives both original account postchecks alongside the source.

The canonical inert roster binds the raw SHA-256 and length of the exact source
packet and its complete source-ordered root list. Each row carries semantic root,
kernel binding, sorted unique effect signer identities, effect toolchain, formula
verification key, formula toolchain and formula boundary. Root and signer counts,
source length, aggregate length, reserved bytes, schema and digest are bounded
and checked. Keys in this encoding are claims, not a trust origin.

Conditional metadata V2 pairs the unchanged six-field V1 metadata with this
roster using the existing bounded-pair framing. Conditional capsule V5 requires
V2, rejects a V1-only downgrade and checks the roster's exact source identity
against its carrier. The overall 160 MiB capsule ceiling is unchanged. Shared
receipt offsets account for the new enclosing frame; the original backing is
retained without copying its existing receipts. The FFI decode quote includes
the added framing, hashing, signer comparisons and bounded root uniqueness scan.

The verifier reconstructs inert policy values on the original bounded account,
validates the formula key cryptographically, decodes the source independently,
and matches every root and kernel binding in source order. Its move-only owner
keeps the complete policy charge and original ledger/address. Borrowed views
cannot escape their callback. Account substitution, floor loss and unwind do
not refund terminal debt or return a successful provisional payload.

Cargo's unselected native continuation reads the roster from the same current
publication token after receipt validation. Every policy field must equal the
independently supplied recovery policy, then currentness is checked again before
source recovery. Parent custody retains the roster through finalization and
persistence, where the supplied policy is compared again. No embedded key is
accepted solely because its enclosing handoff is signed.

## Trust Work Still Required

The V3 fixed client-profile reader authenticates a root-owned file, but its
public result type is also constructible from caller-selected values. The
profile pins the issuer, runtime and anchor identities, not an approved compiler
closure. Public compiler-closure construction and equality with the parent
invocation establish consistency, not independent approval.

The protected issuer verifies its own runtime and observes the child's rustc
executable, backend descriptor, arguments, environment and working directory.
These checks do not prove that an independently approved complete compiler
runtime executed throughout the invocation. The selected release/broker path
also remains V1. Loader libraries and proc-macro execution need an explicit
runtime policy and enforcement, not an inference from a matching receipt.

The existing `/etc/fe2o3/build-authority/policy-v1` launcher port is a protected
pre-exec input, not an implemented post-exec compiler-policy admission service.
Required next work is a typed root-policy approval owner binding the accepted
profile, compiler closure and runtime assumptions, delivery through the V3
release/broker path, and observation or enforcement of those assumptions during
the actual compiler execution. There must be no public positive constructor
that merely wraps caller-provided hashes.

The policy schema must bind all six `CompilerClosureV2` pins (Cargo, trampoline,
cargo-fe2o3 wrapper, rustc, runtime tree and backend), transition version 1, the
exact V3 client-profile identity, and an approved runtime manifest/enforcement
profile. The fixed-path owner must load the production profile internally and
enter before release closure observation. The runtime guard is a separate owner
covering pre-exec through completion, including loader, proc-macro, descendant,
mapping and executable-memory rules. Reuse existing image/ELF and mediation
components, but do not treat them as already integrated compiler enforcement.
Approved immutable compiler bytes need not originate in root-owned source files.

Only that independently approved compiler may nominate its retained proof keys
for the exact invocation and subject. Restart additionally needs fresh
challenge-bound Worker/external-anchor `VerifyCurrent` admission. Filesystem
currentness, decoded carriage and historical journal equality cannot replace
those gates. Production activation remains refused until the required owners
and the existing publication/load/launch authority path are integrated.

## Validation

Commands use pinned `nightly-2026-04-03`, locked offline resolution, one Cargo
job, no incremental/debug-info output, a 12 GiB virtual-memory limit and a
1,200-second outer deadline. Test threads are serial and GPU visibility is empty.
Logs are retained in the sibling
`fe2o3-issue272-production-next-evidence-20260921` directory.

| Check | Result | Log |
| --- | --- | --- |
| Final six-package all-target integration check | Passed with warnings | `policy-roster-check-20260928-r4.log` |
| Roster and conditional metadata/capsule | 23 passed | `policy-roster-lineage-tests-20260928.log` |
| Conditional FFI V5 | 7 passed | `policy-roster-ffi-transaction-tests-20260928.log` |
| Conditional transaction V5 | 13 passed | `policy-roster-transaction-tests-20260928-r2.log` |
| Policy reconstruction and hostile callbacks | 14 passed, including 30 expected-policy mutations | `policy-roster-verifier-tests-20260928.log` |
| Compiler policy capture and V2 packing | 14 passed | `policy-roster-backend-tests-20260928.log` |
| Cargo native boundary and retained replacement accounting | 20 passed | `policy-roster-cargo-tests-20260928-r2.log` |
| Non-clone owner compile-fail doctest | 1 passed | `policy-roster-doctest-20260928.log` |

The first two check attempts found missing error-type annotations in new test
closures; the third passed after explicit `u8` annotations. The final rerun
passed after the export-formatting adjustment. The initial combined
FFI/transaction filter selected zero transaction tests and establishes no
transaction coverage; the separately selected 13-test rerun is the pass above.
The initial Cargo filename-based filter also selected zero tests. Enumeration
identified `compiler_execution_boundary::native`; the separate 20-test rerun
uses that actual Rust module path. The sole formatting follow-up reorders
verifier exports without changing APIs. Scoped formatting and whitespace checks
pass; source-hygiene policy unit tests passed all eight cases.

These 92 tests are local inert framing and hostile-input tests, not fresh
protected proof execution, GPU qualification or a universal machine-refinement
proof. The compiler's existing
genuine F-prefix test now asserts captured policies against retained owners, but
compilation of that assertion alone is not evidence of its execution.

Read-only SSH reached MI350-2 and confirmed the retained runtime image. The
retained r11 source-proof controller and its pinned 102-package base are reusable,
but the controller's fixed remote nightly rustc/driver path is absent and its
current deployment is unverified. A fresh run needs guarded JSON artifact
records and preparation from this exact harness, followed by isolated deployment
checks. No remote runtime, service or host security setting was modified here.
Those source-proof tests use extraction-only compiler custody: even a passing
F-prefix replay would establish genuine policy capture, not native V2 packing,
production admission or a protected Worker-to-safe-launch roundtrip.

SHA-256 identities of the retained passing logs:

```text
763b1570d88bc9850b6742b7675aceecdf18ec43427b79e701768358a78b749d  policy-roster-check-20260928-r4.log
706385b232189d37f85d816159f37bf7d2a9b9c345dcbab4446c032a4407fd95  policy-roster-lineage-tests-20260928.log
51d0b16046673edf8e2f66be56b88c8ed372a45753ce2d8f80d61b4ee4c74d0d  policy-roster-ffi-transaction-tests-20260928.log
d4e91d67f87d448017b509baa4a718cdb11d303ffa333ac6f848a49e1bb28d6d  policy-roster-transaction-tests-20260928-r2.log
b8fb8379907e1dcc4cee77bd16e43379826e1fbb9de11d6dc717cbb662b62027  policy-roster-verifier-tests-20260928.log
58af98552a0d60a55b1531141398ca5f8423196e3403b07d4f8b297bd39cacbb  policy-roster-backend-tests-20260928.log
a169e5f0d7ebc000e74db614fc336bd9ab34595a1b3d686d3e5b26329add2ef7  policy-roster-cargo-tests-20260928-r2.log
0e500b5b251d5abe188c78a18432dcd51f15ba8f52d40304e3a6957ca42fb4c7  policy-roster-doctest-20260928.log
```
