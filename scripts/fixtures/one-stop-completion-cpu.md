# One-stop completion CPU regression

Run from a checkout with its `rust-toolchain.toml` nightly already installed:

```sh
python3 scripts/tests/one_stop_completion_cpu.py
python3 scripts/one_stop_completion_cpu.py
```

The runner never installs a toolchain or resolves Cargo dependencies. It uses
`rustup toolchain list` and `rustup which --toolchain <installed pinned channel>
rustc`, then rechecks that selection and compiler bytes after the run. A custom
installed rustup can be selected with `--rustup /path/to/rustup`. Python 3.9+
and an `x86_64-unknown-linux-gnu` Rust host are required. Compilation uses the
system C driver selected through the closed `/usr/bin:/bin` PATH and the pinned
nightly's bundled LLD. Its worker count is fixed to one (`--threads=1`) to bound
thread fan-out on large hosts; the 2-GiB address-space limit is unchanged.
Other or duplicated Rust host records refuse before compilation. This is a CPU
regression, not a full toolchain custody or deployment qualification.

The six generated include files contain unmodified slices from the **current
checkout**: strict `completion`, polling `completion_poll`, both deadline
functions, the `ObserveCompletion` match-arm expression, two AQL constants, and
the three named public predicate tests. Missing or duplicated delimiters fail
closed. Ordinary whitespace/body edits do not require updating a fixed source
hash. Syntax changes outside the extractor's closed shape require an explicit
extractor/test update; they must not silently fall back to an old fixture.

Eight additional tests in `one-stop-completion-cpu.rs` mock only the clock,
currentness check, backend reads, and sleep. Together the eleven named tests
cover lag-then-completion, permanent lag at the **original** deadline, positive
observation at expiry, pre-read expiry, invalid tuples, repeated completion,
invalid owner frontier, currentness failure, both observation orders, and strict
retirement. The compiled test list must contain all eleven names exactly once.
No real owner is constructed, no KFD device or GPU is used, and no native cleanup,
eventual frontier progress, hardware acceptance, or launch authority is proven.

Each child owns a separate process group. Cleanup covers the entire lifetime,
including selector construction and registration after spawn. Scoped SIGTERM
and SIGINT handlers record cancellation before spawn and throughout cleanup;
the bounded loop raises into cleanup at its next check. The caller's handlers
are restored. Cancellation during cleanup cannot return success. Uncatchable
signals such as SIGKILL still require the CI/job supervisor's containment.
Timeout, output overflow, or other failure kills the owned group and attempts
to reap its direct child; the leader stays waitable until group cleanup to avoid
PID reuse. Cleanup failures refuse success and retain a categorical cleanup log.
Pipe closure and both log writes are attempted even if the bounded wait fails;
an original refusal remains the primary error. Compile is limited to 60 seconds; tool
queries and test/list execution each have 10 seconds. Per-stream output is 1 MiB,
individual child files 32 MiB, child address space 2 GiB, and CPU time 60 seconds.
The private scratch tree is checked during each child for at most 256 entries
and 128 MiB logical contents. This polling check is not an atomic filesystem
quota or a proof of peak allocation. CI additionally has a five-minute job limit.

The thirty-one Python controls include selector/setup failures, SIGTERM/SIGINT to
the parent, cancellation during cleanup, a descendant holding output after its
direct parent exits, and injected wait failure. These are CPU-only subprocess
tests, not native cgroup qualification. A compile-command control fixes the
linker worker argument and rejects missing, foreign or duplicate host records.
Dynamic import disables bytecode writes
before loading the runner, so it does not create checkout `__pycache__` files.

The runner creates a private temporary directory or accepts an existing empty
owned mode-0700 directory via `--output`. It retains sources, bounded child logs,
test executable, and the success report; it never recursively deletes them.
The printed path identifies local diagnostics. CI uploads them on failure for
seven days. Source, generated fixture, executable, rustup, and compiler bytes are
checked before/after. Successful results are still **mocked CPU coverage**, not a
replacement for crate integration tests or a new native experiment.
