import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
const project = createRequire(resolve(process.cwd(), 'package.json'));
const routes = {
  eslint: ['eslint'],
  '@eslint/config-array': ['eslint', '@eslint/config-array'],
  '@typescript-eslint/typescript-estree': ['typescript-eslint', '@typescript-eslint/parser', '@typescript-eslint/typescript-estree'],
};
for (const name of ["@typescript-eslint/typescript-estree"]) {
  let consumer = project;
  for (const step of routes[name]) consumer = createRequire(consumer.resolve(step));
  const target = consumer.resolve('minimatch');
  execFileSync(process.execPath, ['-e', `
    const assert = require('node:assert/strict');
    const { minimatch, Minimatch } = require(process.argv[1]);
    assert.equal(typeof minimatch, 'function');
    assert.equal(typeof Minimatch, 'function');
    for (const [path, glob, expected] of [
      ['src/view.tsx', '**/*.{ts,tsx}', true],
      ['src/view.js', '**/*.{ts,tsx}', false],
      ['node_modules/pkg/x.ts', '**/node_modules/**', true],
      ['src/a/b.ts', 'src/**/b.ts', true],
      ['.hidden.ts', '*.ts', false],
    ]) assert.equal(minimatch(path, glob), expected);
    const matcher = new Minimatch('src/**/../**/../*.{ts,tsx}', {optimizationLevel: 2});
    assert.equal(typeof matcher.match('view.ts'), 'boolean');
  `, target], { timeout: 3000, stdio: 'inherit' });
  process.stdout.write(`Verified minimatch compatibility for ${name}: ${target}\n`);
}
