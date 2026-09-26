// SPDX-License-Identifier: GPL-3.0-or-later
import {readApiHeader} from './tests/source-files.mjs';
if(process.argv.length!==3)throw Error('usage: node verify-api-header.mjs <canonical-absolute-api-header>');
const result=readApiHeader(process.argv[2]);
console.log(JSON.stringify({schema:'fe2o3-rocgdb-one-stop-physical-v3-api-header-check-v1',...result,library_verified:false,native_authority:false}));
