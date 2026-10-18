import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const result = spawnSync(
  'cargo',
  ['run', '--locked', '--quiet', '-p', 'converter-app', '--example', 'export_contract_fixtures'],
  {
    cwd: fileURLToPath(new URL('../', import.meta.url)),
    encoding: 'utf8',
    maxBuffer: 1024 * 1024,
  },
);
if (result.status !== 0) {
  process.stderr.write(result.stderr);
  process.exit(result.status ?? 1);
}
const expected = JSON.parse(
  readFileSync(new URL('../tests/ui/fixtures/contracts.json', import.meta.url), 'utf8'),
);
assert.deepEqual(JSON.parse(result.stdout), expected, 'Rust DTO и frontend fixtures разошлись');
console.log('Rust DTO и frontend fixtures совпадают');
