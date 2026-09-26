# Threaded Runtime Release Profile

The release-policy gate for the actual `gfx942-runtime-vecadd-benchmark` is:

```sh
scripts/ci-local.sh runtime-threaded-release
```

It also runs through `runtime-policy` and hosted `generic-core`. It requires
the repository-pinned Rust toolchain and musl target, a populated offline Cargo
cache, Python 3.11+, GCC with GNU bfd, and binutils. It does not access a GPU.
Ambient Cargo configuration is rejected rather than silently overriding the
selected profile. The command creates fresh owned build directories under the
CI log directory, retains binaries and evidence, and removes completed build
caches. Partial or interrupted runs are not qualification receipts.

## Profiles

Both candidates build the same runtime example with `--release`,
`--no-default-features` and `--features fe2o3-runtime/hardware-qualification`.
Cargo metadata uses the corresponding target and the identical feature set.

| Host target | Required result |
| --- | --- |
| `x86_64-unknown-linux-musl` | Strict policy acceptance plus static PIE, full-symbol, retained-thread and link-input checks |
| `x86_64-unknown-linux-gnu` | Explicit strict-policy rejection for the existing thread-startup `dlsym` import; never an accepted release artifact |

The musl profile selects `/usr/bin/cc`, `-fuse-ld=bfd` and the target's static
PIE CRT. It changes neither shared-memory helpers nor the production ELF
policy. GNU rejection remains visible; there is no `dlsym` exception, renamed
import, simulated policy, external-main smoke, disabled-feature stub or reduced
source/readback check.

## Evidence

The gate measures source files, selected tools (including the actual Python
interpreter), target standard libraries and CRT inputs before and after the
build. It checks the final musl link command, exact CRT order, resolved external
link-map inputs, ELF layout, dynamic dependencies and complete symbol table.
The audited and usage-tested binary identities must remain identical. Pinned
inherited validator/process helpers are captured as ordinary files and their
captured bytes are executed; policy parsing also uses captured bytes.

Retained executable symbols and disassembly must include the worker body,
scoped-thread implementation and spawn implementation for source copying,
complete verification and repeated-byte filling. Closure destructors, data
symbols, zero-size bodies and ambiguous/aliased matches do not satisfy these
checks. A no-argument invocation must return the enabled program's exact usage
diagnostic before topology discovery; the disabled feature message is rejected.

Compiled negative controls require a real GNU `dlsym` import to fail the dynamic
audit, and a static `dlsym` candidate to pass that audit but fail the full-symbol
guard. Build failures or unrelated audit errors do not qualify as these expected
rejections. Independent CI dispatch checks require the actual release command,
not only its parser tests.

## Boundaries

These are measured release-artifact checks, not a hermetic compiler execution
proof, machine-code refinement, native execution or performance qualification.
The output-size limit is an evidence-acceptance bound after process capture,
not a bound on subprocess output memory. Native correctness, teardown and
performance require separately guarded hardware runs and matched baselines.

The benchmark requests HostVisible allocations. Its usage check and normal GPU
smoke do not exercise the retained device-local threaded initialization branch.
That branch needs its own behavior/native qualification. The newer update on
[#277](https://github.com/harsh-nod/fe2o3/issues/277#issuecomment-5650895680)
reports that the earlier teardown defect was fixed and functionally tested;
this does not transfer native qualification to a newly built musl artifact.
