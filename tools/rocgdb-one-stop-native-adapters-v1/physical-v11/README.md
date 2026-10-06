# Disabled physical v11: first budget-denial diagnostics

This source-only layer follows physical-v10 and changes three leaves. It records the first denied debit in the existing owner and adds fixed decimal fields to the existing refusal. It does not change any cap, charge, guard, successful transition, permit, or public false gate. It is not a behavioral fix and does not identify the counter in the prior failed run.

The exact 63 selected leaves occupy 2,266,924 bytes under the unchanged 2,293,760-byte cap. The old query-separation fixture and all bounded package/account/I/O/transform helpers remain. The earlier target transform is explicitly retained metadata, not a new selected delta. New three-leaf transforms contain five exact reversible spans.

Run the Node source tests with node --test tests/*-tests.mjs. The C++ fixtures are separate root-owned CPU controls: compile tests/query-routing.cc and tests/budget-diagnostic.cc with C++17, -Wall -Wextra -Werror -pedantic, then separately UBSan with no recovery. Source tests neither build nor execute these fixtures. Verify the disabled 63-leaf stage with verify-source.mjs ROOT physical-first-budget-denial-disabled-v11 and the exact API header with verify-api-header.mjs. Root must separately check/apply the patch only to exact staged v10 inputs.

The new note is bounded at32 bytes and separately prepaid scratch at320 bytes. On the selected ABI the predicted reservation is20,312 bytes, not an observed result. A new twenty-primary-type measurement, including the two diagnostic auxiliary definitions, must establish actual member/type/offset joins and total storage before any new startup or native scope. The failed one-use scopes remain consumed; this package grants no execution authority.
