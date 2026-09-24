# Fixed gfx950 one-stop controller source — disabled

This separate `publish = false` Cargo workspace packages the fixed MI2 protocol
library and its launch-owned native transport. It is not a member of the main
fe2o3 workspace, a public/general-purpose debugger frontend, or a launch recipe.
The source is MIT OR Apache-2.0; see the unchanged repository license texts
[LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

**The compiled runtime profile is `None`.** The binary refuses before spawning
GDB. There is no CLI/environment/configuration override for the profile, target,
artifact, debugger path, PID, command stream or startup prerequisite. Building
this package does not make a hardware debugger available.

## Scope and provenance

The generic library evaluates the bounded fixed protocol through a caller-owned
`Peer`. A successful result is a historical observation, not native custody.
Mocks cannot prove an actual process, queue, wave, ACK or resource lifetime.

The separate binary contains a real launch-owned adapter: an actual GDB child,
pidfd-backed process observations, selected target entry checks, current
scope/parent/executable observations, bounded streams, exact fixed commands,
and one cleanup path. Parent ELF identity is observed from the actual retained
parent; the outer launcher must independently pin both final executables. This
avoids a controller/parent binary-hash cycle. No caller-supplied report, digest,
PID or boolean constructs those owners.

The exact fixed command sequence has 17 MI2 commands and no read, attach or
retry entry. Asynchronous stops are consumed without requiring an invented
post-stop prompt. Failure cleanup requires both stream EOFs to be consumed,
even if the reader threads have already finished; failed readers remain
incomplete within the original five-second cleanup deadline.

Hardcoded target and artifact paths, entry/checkpoint symbols and scope shape
are **private qualification-fixture identities** inherited unchanged from the
historical source. They are not portable installation locations or user
configuration. The existing target is not shipped or invoked here. A future
enabled source successor needs its own reviewed static bindings; this README
does not describe how to enable one.

No GPL GDB source, patch, header or native producer implementation is imported,
compiled or linked into this package. The separate GPL producer must be
reviewed, built and startup-qualified under its own license and ownership
boundary. This Rust source does not grant that producer a launch permit.

## Limits and meaning of a result

Protocol limits remain 192 KiB per MI line, 8 MiB combined transcript payload,
8,192 records, 64 commands and 60 seconds from the original pre-spawn start.
The fixed output report is capped at 18 MiB to retain exact transcript hex;
failure cleanup has a separate five-second bound. Those logical payload
ceilings are not a compiler/process RSS claim or whole-family isolation.

Successful observations still say independent native tuple replay, physical
register capture, physical memory capture, process/source/launch authority,
whole-family cleanup, general host exclusion and operational qualification are
false. GDB direct-child reap, target pidfd exit, EOF and reader joins are
separate facts. A separately reviewed owning family supervisor, runtime/trap
isolation, selected queue/packet provenance and actual stop qualification
remain mandatory before any native use.

## Read-only source check

Set `package_dir` to this package's canonical absolute directory. This command
only reads the selected source files:

```sh
node "$package_dir/verify-source.mjs" "$package_dir"
```

[source-manifest.json](source-manifest.json) pins all 18 Rust leaves, exact
Cargo.toml/Cargo.lock and two license texts (22 selected files). The reader
requires exact roster/order, canonical JSON, strict UTF-8, stable regular files
and matching lengths/SHA256. It rejects symlink/relative/escaping paths and
checks directory/file identity before and after reads. Limits are 32 KiB
manifest, 128 KiB per file and 512 KiB aggregate selected bytes. A single
bounded growth/EOF probe follows each read.

This verifies consistency against the checked-in manifest, not authentication
of a caller-edited manifest, a complete tree, build provenance, race-free
filesystem isolation, startup, native currentness or execution authority.
The verifier never spawns a subprocess, loads the Rust binary or reads the
private target paths. Documentation and helper sources are governed by the
repository commit, not included in that selected Rust-source manifest.

## Explicit CPU-only checks

The following are package checks/builds, **not a controller invocation**:

```sh
node --test "$package_dir/tests/package-files-tests.mjs"
cargo test --manifest-path "$package_dir/Cargo.toml" --locked --offline --all-targets
cargo clippy --manifest-path "$package_dir/Cargo.toml" --locked --offline --all-targets -- -D warnings
cargo build --manifest-path "$package_dir/Cargo.toml" --locked --offline --bin fe2o3-private-one-stop-controller
```

They require the pinned dependencies already available to the chosen toolchain;
there is no acquisition, install, startup or runtime automation here. Package
tests use inert source-shaped temporary files or read checked-in source; they
do not launch the controller, GDB or a GPU workload.

## Historical qualification, not transferred acceptance

The exact private predecessor was CPU-qualified on 2026-09-24: **43 library
tests + 29 native-binary CPU tests = 72**, followed by strict Clippy and a static
binary build. Receipt SHA256:
`359e6c322b0a73470d1d77a7e4c9c73d37bcf3171a5f6841b46b1045d01013a8`.
[historical-evidence.json](historical-evidence.json) retains exact receipt,
stdout/stderr and merged-source delta pins.

All 20 Rust-project files are preserved; only stale library prose was corrected
to acknowledge the separate native binary. The 20 new Node package controls
and relocated source require their own root-owned checks; they do not inherit
a pass from those 72 tests. No native target/debugger invocation, GPU dispatch,
GPU stop, register/memory sample, enabled-family qualification or V4 milestone
completion is claimed.

## Package qualification update

Root qualified this relocated package on 2026-09-24: 72 Rust CPU tests,
20 Node package checks, strict Clippy and build passed. The compiled runtime
profile remains None and no target/debugger was invoked. The exact gate and
remaining boundaries are in the [qualification record](../../docs/source-transport-tiled-debugger-qualification-20260924.md).
