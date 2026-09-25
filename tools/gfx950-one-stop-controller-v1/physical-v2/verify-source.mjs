// SPDX-License-Identifier: MIT OR Apache-2.0
import {readPackage} from './tests/package-files.mjs';
if(process.argv.length!==3)throw Error('usage: node verify-source.mjs <absolute-package-directory>');
console.log(JSON.stringify(readPackage(process.argv[2])));
