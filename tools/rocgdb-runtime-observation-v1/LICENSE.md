# License and separation

SPDX-License-Identifier: GPL-3.0-or-later

This tools/rocgdb-runtime-observation-v1 subtree, including producer additions,
patches, test adaptations and source-checking utilities, is supplied under the
GNU General Public License, version 3 or (at your option) any later version.
The fe2o3 repository's MIT/Apache defaults do not relicense these GPL materials.

COPYING is the exact COPYING3 text from ROCm/ROCgdb revision
48b1d324e389d2ed5e19822d377ff9050770233d; its identity is recorded in
source-manifest.json. Existing upstream copyright and license notices remain
in the patched source. New producer files carry GPL-3.0-or-later SPDX notices;
no assignment of new contributions to the Free Software Foundation is asserted.

This package modifies a separately obtained GDB checkout. It is not linked into
fe2o3 Rust crates and must not be copied into an MIT/Apache library. Numeric
diagnostic wire observations do not convey the producer's native handles or
live authority. No binary is distributed by this package.

The patches and tools are provided without warranty; see COPYING. A distributor
of a resulting debugger remains responsible for the applicable license terms
and corresponding-source requirements. This source package does not authorize
relicensing upstream GDB.
