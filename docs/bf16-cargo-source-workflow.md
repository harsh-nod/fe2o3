# BF16 source authoring through Cargo

This Linux workflow uses actual Cargo rustc-wrapper invocations. You do not
capture or reconstruct rustc arguments, set Cargo's primary-package marker, or
supply a crate-binding identity. It is deliberately limited to the unchanged
direct BF16 fixture in this repository, gfx942 with XNACK disabled and Wave64.

The five actions inspect the original source, publish separate Identity and
Swap01 helpers, then admit each candidate in a fresh compiler invocation. The
candidate admission command requires the existing
`BF16 nominal source-ranked projection` refusal. Success here means diagnostic
source admission, not a normal artifact, simulator result, complete GEMM, or GPU
execution. Identity preserves the returned components; Swap01 exchanges the
first two. Neither operation selects physical registers or an MFMA encoding.

## Build and provision

Start in a current `fe2o3` checkout with Linux, Node.js 18 or newer, and the
components from [rust-toolchain.toml](../rust-toolchain.toml) installed. The
current pinned nightly is `nightly-2026-04-03`. Cargo may need network access
during these explicit provisioning commands. The workflow itself is offline;
missing dependencies cause a retained failure, not an automatic download or
retry.

~~~bash
cargo fetch --locked
cargo fetch --locked \
  --manifest-path crates/rustc-codegen-fe2o3/tests/fixtures/tiled-region-inspection-v1/Cargo.toml

cargo build --locked --release \
  -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract
~~~

Provision the build-std dependencies with a normal Cargo check, without any
extractor or source-publication opt-in. This is ordinary Rust checking, not a
fe2o3 artifact or BF16 production qualification:

~~~bash
RUSTFLAGS='-Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Cpanic=abort -Copt-level=3 -Cembed-bitcode=no -Cdebug-assertions=off -Coverflow-checks=on -Zalways-encode-mir -Zunstable-options' \
cargo check --locked --release --lib \
  --manifest-path crates/rustc-codegen-fe2o3/tests/fixtures/tiled-region-inspection-v1/Cargo.toml \
  --target amdgcn-amd-amdhsa -Zbuild-std=core
~~~

Do not put another `RUSTC_WRAPPER`, workspace wrapper, or fe2o3 output mode around
these setup commands. Resolve any ordinary setup failure before starting the
authoring workflow. The package manager and dependency/runtime setup are
outside the workflow's task-directory limits.

## Run the public workflow

The extractor and its dynamic backend must come from the same build. Set the
rustc runtime library path to the matching pinned toolchain. These Bash commands
use real toolchain paths; they do not manufacture a compiler invocation:

~~~bash
BF16_REPO="$(pwd -P)"
BF16_RUSTC="$(rustup which --toolchain nightly-2026-04-03 rustc)"
BF16_CARGO="$(rustup which --toolchain nightly-2026-04-03 cargo)"
BF16_SYSROOT="$("$BF16_RUSTC" --print sysroot)"
BF16_EXTRACTOR="$BF16_REPO/target/release/fe2o3-rustc-extract"
BF16_RUN_PARENT="$(mktemp -d)"
BF16_DEADLINE="$(date -u -d '+25 minutes' +%Y-%m-%dT%H:%M:%S.000Z)"

LD_LIBRARY_PATH="$BF16_REPO/target/release:$BF16_SYSROOT/lib" \
node scripts/bf16-source-workflow.mjs \
  --repo "$BF16_REPO" \
  --extractor "$BF16_EXTRACTOR" \
  --cargo "$BF16_CARGO" \
  --rustc "$BF16_RUSTC" \
  --work "$BF16_RUN_PARENT/run" \
  --deadline "$BF16_DEADLINE"
~~~

The work directory must not exist. Its parent and repository path must be
canonical, and the script accepts no arbitrary source, feature, target, request
or rustc argument. It copies the fixture and lock into a new standalone
`original` package; the only manifest changes replace the two repository-relative
device/host dependencies with actual absolute checkout paths. Both nested
candidate packages retain identical manifest bytes and their own locks.

One fresh `target` directory is shared across the five serial Cargo actions.
Dependencies and probes use the existing passthrough path. Each selected action
must create its own previously absent report directory and finish with a clean
Cargo exit and closed output streams. If Cargo skips an action, the missing new
report makes the workflow fail: there is no stale-report fallback, source touch,
metadata-salt trick, cleanup/retry, or saved compiler owner.

Inspect the results only after the command exits zero:

| Retained path | Meaning |
| --- | --- |
| `inspect/observation.json` | Original source selection from a real compiler callback |
| `identity.request.json`, `swap01.request.json` | Inert bounded selectors submitted to the existing publisher |
| `original/identity/src/lib.rs`, `original/swap01/src/lib.rs` | Create-new published Rust candidates |
| `publish-*/observation.json` | Publication effects and exact source file facts |
| `admit-*/observation.json` | Fresh nominal source admission and exact normal-ranked refusal |
| `*.started.json`, `*.terminal.json`, `*.stdout`, `*.stderr` | Original Cargo attempt and bounded output custody |
| `PASSED.json` | Joined five-action result, subject to the command's final exit/postflight |

The script compares each candidate's complete bytes, hash, length, device and
inode with publication facts and checks the fresh admission's source hash and
requested permutation. Compiler canonical identity is a different digest domain
from SHA-256 of serialized IR. The fixed source file is never overwritten.

## Failure and bounds

Stop on any nonzero exit and retain the entire work directory. A failed
publication may already have created a candidate. A failed admission report's
default false/null completion fields do not prove that all earlier stages were
absent. An absent report is not rollback evidence. A `PASSED.json` followed by a
final postflight failure remains historical only; `FAILED.json` and the command
exit take precedence. Choose a new work directory for a deliberately authorized
later attempt; the script never retries or deletes anything.

The script admits at most five direct Cargo launches, 300 seconds per stage,
30 minutes total and the supplied absolute deadline. It retains at most 8 MiB
per stdout/stderr stream, with a 128 MiB non-target evidence limit and a separate
500 MiB target-tree limit. Report/source/request limits remain 16 KiB/64 KiB/8
KiB. File-tree checks are bounded and sampled while Cargo runs, then repeated
at stage/final boundaries; they are not a filesystem quota or allocator-memory
limit. Output overflow retains the bounded prefix and rejects success.

Cargo can launch rustc, build scripts and other descendants, so five direct
launches is not a claim of five total processes. Cargo inherits the parent's process group; the script never creates a detached
child group. On failure it signals only the original direct Cargo PID after
checking its captured process start time, and retains exit/close/drain facts. It
does not claim control of an escaped process family, complete descendant
quiescence, cgroup memory limits, or a complete dependency/runtime input census.
Run qualification inside an appropriately bounded outer supervisor that owns
and cleans up the inherited process group, including on forced script termination.
A bare invocation cannot guarantee descendant cleanup after parent death or a
hang. Source and runtime qualification receipts remain separate.

## Controls and scope

~~~bash
node --test scripts/bf16-source-workflow.test.mjs scripts/bf16-source-workflow-io.test.mjs
~~~

These controls use inert transport fixtures and source checks; they do not
authenticate a compiler owner or qualify real Cargo execution. Actual
end-to-end evidence must come from the current compiler and all five completed
actions. This workflow does not add normal BF16 admission, numerical replay,
hardware evidence, full compiler memory coverage, or broad milestone closure.

For the lower-level public interface and historical qualification boundaries,
see [BF16 source authoring](bf16-source-authoring.md).
