// SPDX-License-Identifier: GPL-3.0-or-later
import {SourceSession, STAGE} from './tests/source-files.mjs';
if (process.argv.length !== 4 || process.argv[3] !== STAGE)
  throw Error('usage: node verify-source.mjs <canonical-absolute-source-root> <physical-owned-legacy-query-separation-disabled-v10>');
process.stdout.write(new SourceSession().verifyStage(process.argv[2], process.argv[3]).report());
