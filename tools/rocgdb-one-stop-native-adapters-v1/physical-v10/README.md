# Disabled owned/legacy query separation — physical-v10

SPDX-License-Identifier: GPL-3.0-or-later

This additive source-only layer follows physical-v9. It changes one WAVE_STOP
routing span in amd-dbgapi-target.c: when the owned profile is selected, reject
the simultaneous legacy stopped-wave profile as unsupported before its native
query pair. The unselected legacy route is unchanged. No cache, synthetic
observation, provider status waiver, query-limit increase or paired-completion
edge change is introduced. The private provider's ten-query lifetime limit is
retained; the five owned pairs remain real same-client calls.

All public activation, capture and publication gates remain false. The legacy
record may move earlier and uses unsupported_profile (23), not stop_changed
(24). It supplies no owned proof. This source layer does not claim that a native
completion has succeeded, nor that the historical refusal had no concurrent
taint cause. A consumed attempt remains consumed.

Apply predecessor layers through physical-v9 first, then the single patch in
patches/series to a separate exact source tree. The closed selected source has
63 files (2264960 bytes), within the unchanged 2293760-byte cap.
All 62 unchanged selected pins and all bounded reader/accounting caps are kept.
The verifier reads only this package and the pinned immediate predecessor
manifest; it does not recurse into predecessor validators.

Source controls (no debugger or GPU invocation):

    node --test tests/*-tests.mjs
    node verify-source.mjs /absolute/v10-source physical-owned-legacy-query-separation-disabled-v10
    node verify-api-header.mjs /absolute/amd-dbgapi.h

The strict inert fixture is tests/query-routing.cc (C++17, -Wall -Wextra -Werror
-pedantic, then separately UBSan). It compiles the exact routing span against
frozen legacy/provider ledger headers, not GDB. It models the observed old
10-then-11 exhaustion and the new five-owned-pair schedule, including duplicate,
prior-invalidation, unselected/non-wave and sticky-taint cases. Tests and source
checks are not permission to enable this package or execute a native attempt.
