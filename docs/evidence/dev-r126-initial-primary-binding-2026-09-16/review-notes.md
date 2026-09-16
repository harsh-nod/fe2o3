# Review And Tool Observations

These are primary-agent notes transcribed from review/tool messages, not raw
independently executed proof output.

## Source Reviews

The production reviewer found no blocking defect in the new primary binding
driver. It checked pre-reserved per-owner retention, borrowed initializer state,
model retake before ordinary error propagation, validation before installation,
and original-parent transport without fallible allocation. Review accepted using
borrowed primary owner validation rather than teardown admission, because SDMA
buffers must be allowed during initial binding.

The integration reviewer requested three stronger checks: exact ordinary-error
and retake precedence, authenticated loan state/aggregate device usage on failures,
and dispatch-to-stream attribution in the two-stream native profile. All were
implemented. Its subsequent read-only review reported no blocking finding and
confirmed the initializer destructor remains inside the guarded pre-commit path.

A separate legacy-path review found local-prefix authority loss in initial and
rebound materializers, missing NEW memory retention on panic, and lost resident
roster custody on overwrite failure. These findings are deliberately not treated
as repaired by the new primary driver.

A follow-up routing audit found no reachable live production workflow with an
all-unassigned compute-lane roster and a previously used primary. Ordinary and
persistent routes record their lane before publication/retryable prepared state;
cancellation, detach and stream destruction preserve the handles. The only bulk
clear follows final queue removal and backend retirement. The lower fresh-state
preflight still checks the invariant rather than trusting runtime bookkeeping.

The evidence reviewer independently matched the final six-file patch to the
worktree, counted 97 constructed cases and parsed the 22/41-event native traces.
It corrected the capture-lifetime wording: captures are retained until guarded
destruction, not after a panicking destructor has consumed them. Native owners,
preparation and the original parent remain rooted across that panic.

## Uploaded Binary

Command observed through the SSH tool before final execution:

```sh
ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x \
  sha256sum /tmp/fe2o3-r126-primary.tOhOZ1/runtime-native-qualified
```

Exit 0, output:

```text
620e448da2983d1a131e52d8fbe82a516bfbd7bb43cac50fb9a49849da386a8e  /tmp/fe2o3-r126-primary.tOhOZ1/runtime-native-qualified
```

This matches `executables.log`. The separately retained `native-upload-sha.log`
belongs to the preliminary binary, not this final one.

## Superseded CPU Processes

The first whole KFD GNU run completed: 1,313 passes and the documented test
assertion failure. Later preliminary GNU/musl whole-KFD processes, using binaries
named `kfd-gnu-final` and `kfd-musl-final`, were intentionally stopped after the
source changed to the named return-type alias and preallocated native test.
Only these private executable paths matched the cancellation query. Both session
handles completed with exit 143; a subsequent anchored process query returned
no matches. Their partial logs are history, not passing regression evidence.

The authoritative full KFD runs use `kfd-gnu-published` and `kfd-musl-published`.
The runtime binaries named `runtime-gnu-qualified` and `runtime-musl-qualified`
are byte-identical to the final rebuild after import formatting, as recorded in
`runtime-identity.log`.

## Source Reconstruction

The primary loaded base `d5cb4d3b375c5ce27b682541357335aa33389df4` into a private
temporary Git index and applied `source-published.patch` with
`git apply --cached --unidiff-zero`. Both commands returned exit 0. The resulting
zero-context six-file diff was byte-identical to the archived patch (`cmp`, exit
0). The source-only tree identity is
`35961c450c4beec1e890ae67b8a2ff69244f8c0a`; documentation additions are not included
in that tree. The temporary index was removed without changing the real index.
