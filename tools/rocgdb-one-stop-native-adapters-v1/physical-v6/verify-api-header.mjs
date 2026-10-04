// SPDX-License-Identifier: GPL-3.0-or-later
import {SourceSession} from './tests/source-files.mjs';
if (process.argv.length !== 3)
  throw Error('usage: node verify-api-header.mjs <canonical-absolute-api-header>');
process.stdout.write(new SourceSession().verifyApi(process.argv[2]).report());
