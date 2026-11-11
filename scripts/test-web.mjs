// Run after cargo web and cargo build. Tests the actual Wasm artifact.
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { initSync, run_script } from '../web/pkg/scripturec.js';

const root = fileURLToPath(new URL('../', import.meta.url));
const binary = `${root}target/debug/scripturec${process.platform === 'win32' ? '.exe' : ''}`;
initSync({ module: readFileSync(new URL('../web/pkg/scripturec_bg.wasm', import.meta.url)) });
let count = 0;
for (const name of readdirSync(`${root}examples`).filter(name => name.endsWith('.scripture'))) {
  const path = `${root}examples/${name}`;
  const browser = JSON.parse(await run_script(readFileSync(path, 'utf8')));
  const native = spawnSync(binary, [path], { encoding: 'utf8' });
  assert.equal(browser.error, null, `${name}: ${browser.error}`);
  assert.equal(native.status, 0, `${name}: ${native.stderr}`);
  assert.equal(browser.output.length ? browser.output.join('\n') + '\n' : '', native.stdout, name);
  count++;
}
for (const [source, message] of [
  ['genesis: praise 1 / 0 amen', 'division by zero'],
  ['genesis: pray -1 amen', 'non-negative seconds'],
  ['genesis: pray 18446744073709551616 amen', 'supported duration range'],
  ['genesis: pray "1" amen', 'expected a number'],
  ['genesis: pray true amen', 'expected a number'],
  ['genesis: praise "before"; confess "oops" amen', 'uncaught sin: oops'],
  ['commandment recurse(): rejoice recurse() amen\ngenesis: recurse() amen', 'call depth limit'],
  ['let there be x = 0\nprophecy x == 1: x = 2 amen\nprophecy x == 2: x = 1 amen\ngenesis: x = 1 amen', 'execution limit'],
  ['#!/usr/bin/env scripturec\ngenesis:\npraise 1 / 0\namen', 'line 3'],
  [`commandment f(): ${'if true: '.repeat(450)}f() ${'amen '.repeat(451)}\ngenesis: f() amen`, 'block nesting limit'],
  [`commandment f(): ${'if true: '.repeat(31)}f() ${'amen '.repeat(32)}\ngenesis: f() amen`, 'execution nesting limit'],
  [`commandment f(): rejoice ${'not '.repeat(4000)}true amen`, 'expression nesting limit'],
  [`genesis: let there be ${Array.from({ length: 1000 }, (_, i) => `v${i}`).join(',')} = "${'x'.repeat(100000)}" amen`, 'storage limit'],
  ['é'.repeat(65537), 'source limit'],
  [`let there be x = 0\n${'prophecy x == 1: x = 0; x = 1 amen\n'.repeat(32)}genesis: x = 1 amen`, 'pending event limit'],
]) {
  const result = JSON.parse(await run_script(source));
  assert.ok(result.error?.includes(message), JSON.stringify(result));
}
const partial = JSON.parse(await run_script('genesis: praise "before"; confess "oops" amen'));
assert.deepEqual(partial.output, ['before']);
// A positive delay yields to the host event loop and resumes nested execution.
let timerFired = false;
const started = performance.now();
const praying = run_script('let there be x = 0\ncommandment pause(): pray 0.02; rejoice 7 amen\nprophecy x == 1: pray 0.01; praise x amen\ngenesis: praise pause(); x = 1; pray 0; praise 2 amen');
assert.ok(praying instanceof Promise);
setTimeout(() => { timerFired = true; }, 0);
const resumed = JSON.parse(await praying);
assert.equal(resumed.error, null);
assert.deepEqual(resumed.output, ['7', '1', '2']);
assert.ok(timerFired, 'pray must yield to the event loop');
assert.ok(performance.now() - started >= 30, 'pray must wait for its duration');
const failedPrayer = JSON.parse(await run_script('genesis: praise "before"; pray -1; praise "after" amen'));
assert.deepEqual(failedPrayer.output, ['before']);
assert.match(failedPrayer.error, /non-negative seconds/);
// Each browser call is a new interpreter, even when the same Wasm instance is reused.
assert.equal(JSON.parse(await run_script('let there be x = 1')).error, null);
assert.match(JSON.parse(await run_script('genesis: praise x amen')).error, /undefined name/);
console.log(`Wasm/native parity passed for ${count} example programs; errors, limits, and fresh state passed.`);
