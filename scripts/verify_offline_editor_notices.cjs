const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const root = path.resolve(__dirname, '..');
const frontend = path.join(root, 'apps/desktop');
const monaco = fs.realpathSync(path.join(frontend, 'node_modules/monaco-editor'));
const dompurify = path.dirname(path.dirname(createRequire(path.join(monaco, 'package.json')).resolve('dompurify')));
assert.equal(JSON.parse(fs.readFileSync(path.join(monaco, 'package.json'))).version, '0.57.0');
assert.equal(JSON.parse(fs.readFileSync(path.join(dompurify, 'package.json'))).version, '3.4.17');
const output = path.join(frontend, 'public/third-party');
const manifest = path.join(output, 'notice-inventory.json');
const files = [
  ['monaco-editor-0.57.0/LICENSE', path.join(monaco, 'LICENSE')],
  ['monaco-editor-0.57.0/ThirdPartyNotices.txt', path.join(monaco, 'ThirdPartyNotices.txt')],
  ['dompurify-3.4.17/LICENSE', path.join(dompurify, 'LICENSE')]
];
const expected = files.map(([file, source]) => {
  const bytes = fs.readFileSync(source);
  const sha256 = crypto.createHash('sha256').update(bytes).digest('hex');
  if (process.argv.includes('--write')) {
    fs.mkdirSync(path.dirname(path.join(output, file)), { recursive: true });
    fs.writeFileSync(path.join(output, file), bytes);
  }
  assert.deepEqual(fs.readFileSync(path.join(output, file)), bytes, file);
  if (process.argv.includes('--built')) assert.deepEqual(fs.readFileSync(path.join(frontend, 'dist/third-party', file)), bytes, file);
  return { path: `apps/desktop/public/third-party/${file}`, sha256 };
});
if (process.argv.includes('--write')) fs.writeFileSync(manifest, JSON.stringify(expected, null, 2) + '\n');
assert.deepEqual(JSON.parse(fs.readFileSync(manifest)), expected);
process.stdout.write('Verified three original upstream notice files and exact pinned runtime versions.\n');
