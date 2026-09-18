# C5 Heterogeneous Typed Bundle Development

CPU qualification above source base
`8058fd785e9d73f80d704a9cf726250a11865264`, including the uncommitted source
identified by `raw/source-before.log` and `raw/source-after.log`. This is not
protected/native execution, formal Rust/native refinement, performance acceptance
or HIP/HSA parity. R125 Native CPU/test, R118B C1/C2/C3 and R116/V3 remain the
accepted checkpoints; A1/A2 and #182 remain incomplete.

## Change

The sealed tuple adapter covers 2 through 64 heterogeneous charged output
observers. It binds to the original completion and, after receiving its original
receipt, acquires every slot guard, validates every member and only then moves
all original results. Rejection preserves the entire tuple. Duplicate slots are
rejected before locking; late poison, readiness or identity failures cannot
partially consume a prefix. Each returned allocation retains its independent
credit until disposal. No new reply, decoder, gate, allocation or Context command
is introduced. The existing scalar adapter uses the same split validate/move
helpers without changing its public behavior.

Eleven new tests exercise actual charged buffers, late-slot failure matrices,
pointer/value preservation, exact refunds, three-member failures, the full
64-member ABI limit, Pending/panic/retry behavior, engine/readback failure and
observer loss. Six additional doctests check real public await/join type linkage,
sealed implementation, inaccessible arbitrary matching, move-only owners and
rejection of 65-member tuples. Compile-fail examples establish rejection, not
an independently checked compiler diagnostic code.

Host fixtures use inert runtime domain metadata, not fabricated protected
completion receipts. Genuine runtime receipt, Stop/drain and owner-thread
behavior are covered separately by existing runtime tests. Public protected
bundle execution remains a composition qualification gap; these fixtures do not
establish native execution or a formal proof of the Rust transaction.

## Qualification

`qualify.sh` records each exact command, start/end UTC timestamp, exit status and
complete stdout/stderr transcript. GNU and musl run the full all-feature library
harnesses serially, not selected filters:

| Harness | GNU | musl |
| --- | --- | --- |
| `fe2o3-host` | 282 passed, 4 ignored | 282 passed, 4 ignored |
| `fe2o3-runtime` | 1,067 passed, 17 ignored | 1,067 passed, 17 ignored |

Ignored tests are not counted as passes. Musl explicitly sets
`FE2O3_HIP_SYS_DISABLE=1`, excluding optional legacy HIP linkage, not direct KFD.
There are 67 passing GNU doctests: host 4 compile-only and 21 compile-fail;
runtime 1 compile-only and 41 compile-fail. No doctest executes a native kernel.
All-feature/all-target strict Clippy, no-default-feature checks and workspace
formatting pass. The unsafe-source policy harness passes five tests with one
explicit maintenance test ignored. The expected HIP-disabled warning is retained
in the musl host transcript and accepted only as that exact leading line.

`source.py` captures all 5,534 files in the stated Cargo source surface, including
untracked new Rust modules. `binaries.py` hashes the four actual `--no-run`
executables before and after full library execution. `verify.py` requires
identical before/after source maps, current source contents, identical executable
hashes and exact build/run paths. Documentation and evidence scripts are outside
that Cargo source surface and are finalized separately before sealing.

The verifier consumes whole library/doctest harnesses, compares complete GNU/musl
test rosters, checks the eleven new bundle tests and rejects sixteen corrupted
transcripts. Hash-pinned sealed baselines establish that all 275 prior host
outcomes remain unchanged with exactly eleven passing additions, and all 1,084
runtime outcomes are unchanged. It imports only the hash-pinned whole-libtest parser from the prior
native-wait CPU archive, preserving its exact known abort-child handling. Source
and executable identity checks require this qualified workspace's build artifacts;
the sealed checksum manifest alone can be checked in another checkout.

## Preserved Development Attempts

Three failed exploratory commands remain in `raw/` and do not count as final
qualification: an associated-type projection compile error; a focused run with
10 passing tests and one fixture-access failure after readiness; and Clippy's
`err-expect` finding in a test. All were corrected before the frozen qualification.
The intervening focused retry passed all eleven bundle tests. Its 154 filtered
tests are not represented as a complete host run.

The first evidence ShellCheck pass also reported SC2094 for the checksum
pipeline. That pipeline explicitly excludes its output manifest from its input;
a local suppression documents that fact. The failed receipt is preserved, and
the final check is separate from the Rust qualification.

The final evidence Python formatting/lint and ShellCheck gates pass. Independent
read-only source/API and evidence reviews found no blocker. The evidence review
also checked the source/binary identities and baseline test preservation; its
two seal/roster hardening suggestions are incorporated in the final verifier.

No MI300X job or remote scratch directory was created for this CPU-only packet.
