# Source instruction edits and helper replay: 2026-09-22

This bounded diagnostic increment advances #280 M2/M5, #281 V2 and #282 U2.
It does not complete these milestones or grant production/proof/hardware authority.
M1, V1 and U1 remain accepted (3/18); see the
[contract review](../assembly-authoring-contract-review-20260922.md).

## Implemented surfaces

- The normal ordered-program diagnostic exporter reports semantic MIR and canonical
  KIR identities from its actual current owner, alongside existing source preflight
  and inventory observations. These are inert identities, not authenticated source.
- Public-seeded source tests inspect two current owners, reject one exact old
  inspection identity, recover on the same accounting ledger, and preserve owner
  bytes/storage floors. Separate debugger checks refuse three source/capture joins.
- [The instruction-edit runner](../../scripts/source-promotion-instruction-edit-smoke.mjs)
  publishes ordinary Rust, changes the final XOR to OR, recompiles default/edit/repeat
  and independently validates every backing byte, initialized bit and both canaries.
- [The complete native-payload join](../../scripts/instruction-edit-native-join.mjs)
  independently rereads raw source stages, source/LLVM identities, two native reports
  and four whole HSACOs with exact instruction/descriptor offsets. It is a read-only
  observation validator, not a native compiler, ELF decoder or production receipt.
- [The helper capture runner](../../scripts/resource-helper-source-values-v2-smoke.mjs)
  captures a real two-frame CPU session with complete named-value pages, distinct
  caller/helper SSA rows and reverse/repeat. Missing locals are not filled from SSA.

The separate native observer used for this qualification is a retained task-private
C++ executable built against the unchanged retained worker and pinned LLVM inputs;
it is not a new shipped production worker route. The join consumes its exact report
profile. Reproducing native evidence still requires those reviewed observer/build
inputs, not merely the Node validator.

## Qualified source and compiler checks

Bases: harsh-nod `dc48d876cd5434e5d8e409a53177f8d43fe5a839`;
powderluv `4a4426669aa73414c7197e7849474b293aebc32c`.
Rust source was pinned-rustfmt formatted before compilation.

Canonical compiler regression: 1,711 passed, 110 ignored, zero failed, 179.76 s.
Mirror: 1,710 passed, 110 ignored, zero failed, 182.16 s. The pre-existing fork
difference is preserved; ignored tests are not counted as success.
The mirror also passed all 65 source-capture/instruction-edit/headless Node controls.
The complete-payload join has 18 separately passed pure controls.

The canonical Rust run used source census 6,542 files / 99,469,985 bytes.
A subsequent two-test JS inventory correction changed no Rust:
the source/capture runs used 6,542 files / 99,471,766 bytes, SHA256
`5d495fd39a493ca96453d3eb16dab926bc14ed7b48ed1c1ad84b982a8985ff31`.
Mirror Rust and actual-source gates used 6,535 files / 99,407,367 bytes, SHA256
`dc2777945b16672581190cad5b70b1d272aaf29c4f45df2f1f005d6bcd13ed1a`.
Final documentation and the byte-identical published join scripts were added
after these full runs; their final checks are separate.

Strict normal-library Clippy is **not green**: 31 errors, exactly matching the
retained Phase18 baseline's code, message and primary source text; zero additional
diagnostics. The comparison passed under receipt SHA256
`fe297e280a5c127870438db47bd319d350c0b5eee5869ed32362fbb8ff1762fd`.
This is not an all-workspace or cfg(test) Clippy pass, nor a new baseline rerun.

Both forks independently passed their normal-client smoke, actual helper capture,
public-seeded debug/current-inspection run, fresh instruction preparation and fresh
instruction-edit source run. Normal standalone clients were built without cfg(test)
against each fork's final matching backend and IR library; incompatible earlier
build attempts remain retained rather than counted.

## Actual evidence

Canonical instruction source: three exports, 100 stages, 90 whole-kernel CPU
simulations. Receipt 248,601 bytes, SHA256
`cee8b86f0d4c7f22cb0e868ebd0ca7dadf6f1f2740ad4a1ba7befc2239b8f6e9`;
external supervisor
`2a5324746ef8cd82460b874c18cdd1a94a096e6bc1f38a98950cf709e11cfdfc`.
Mirror independently repeated three exports / 100 stages / 90 simulations:
receipt 252,786 bytes,
`79d8c21ca58a7e8c9930b26cc3ce60edaae008d047ff6930ffebd1c9102236e7`;
supervisor
`d879995e75b3f5afb55c60f7696c82e7b89440ef8de37a0276da342e83c1ed43`.

Canonical default and edited LLVM were 1,994 and 1,993 bytes respectively,
SHA256 `f6c5e9363aeb66b39c2bae53bc289683fe2be039537e0ca97ded534857912155`
and `123495688260bc4ad38215b59b991bf5bbda421ad36c45d84d6da751a0572b8f`.
The separate native run emitted and inspected default/edited O0/O3.
The strict join checked 294 retained pins / 136,893,241 bytes and all four full
payloads; report 140,747 bytes,
`5230415719fa0c7c81473d5fea338d5f3a85c7a3a9a91fd55c3900e20165d162`;
supervisor
`9e3c77907c37abce49febe91db7c6b20f67967b27d8f600ad7ed41c6c8e766f5`.
No mirror-native or repeat-native execution is claimed.

Canonical helper capture: 29 complete pairs; retained browser excerpt 15 pairs /
29,813 bytes. Capture receipt 12,567 bytes,
`aae1d08b4666a7d516afdb94d08edf6421d1de569333a3a9d9b1e7f7786f2350`.
Mirror independently passed; receipt 12,618 bytes,
`6bb6af828057961ef53b588a9fae8a8c71e692d276dcee09ed15cb893238249e`.
Events/revisions 3/3, 2/4, 3/5 show caller frame 1 versus helper frame 2,
helper SSA rows 2/1/2 and exact parameter bits `0x3f800000`.
Five expected refusals preserve state. Allocation reuse, dynamic helper activation
and a source-to-SSA map remain unrepresented.

## Tutorials and remaining acceptance

The site publishes the
[helper walkthrough](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/resource-helper-source-values-v2.md),
[instruction-edit lab](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/source-promotion-instruction-edit-lab-v1.md)
and [site qualification](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/helper-instruction-qualification-20260922.md).
Its tested code passed 909 Vitest tests, 21 Node controls, lint/typecheck/build,
evidence validation and all 142 desktop/mobile browser tests without skips/flakes.
The shared tiled curriculum is still pending, not replaced by these scalar lessons.

All old failures are preserved. The instruction driver's first attempt incorrectly
required body-insensitive inventory hashes to change; the corrected guard requires
exact inventory equality and still requires all body-sensitive identity changes.
A fresh successful run, not relabeling the failed receipt, supports the result.

Fresh diagnostic import/typing/register-initialization observations and stale
inspection/capture rejection are not proof-cache invalidation. U2 still needs the
applicable owner-reviewed freshness/proof boundary; V2 retains watch/fault,
repeated activation and genuine allocation-reuse gaps. Physical helper/control/
memory/matrix contracts, checked production scheduling and tiled integration remain
with existing owners. LLVM IR remains in this pipeline: the ordered region is
constrained inline assembly inside an ordinarily compiled kernel.

Task-local records are below the mi350 authoring workspace's logs/phase19* paths.
Selected hashes are evidence locators, not source authentication or transitive
toolchain attestations. The explicit Phase19b envelope is 96 GiB retained task
storage, at least 40 GiB disk free / 64 GiB available RAM, jobs=2 and bounded
deadlines. It supersedes only the old self-declared 80 GiB storage ceiling;
sampled guards are not hard process-RSS enforcement.
