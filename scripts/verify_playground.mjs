// Execute the shipped WASM and the same scenarios used by the browser UI.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createPlayground, runScenario } from '../web/engine.mjs';

const binary = await readFile(new URL('../dist/playground/pxr_playground.wasm', import.meta.url));
const expected = {
  valid: ['Armed', 'Executed', 'Ok', 1, 400],
  bounds: ['Armed', 'Rejected', 'Bound', 0, 0],
  stale: ['Armed', 'Rejected', 'Stale', 0, 0],
  duplicate: ['Armed', 'Rejected', 'Duplicate', 1, 400],
  stream: ['SafeIdle', 'Fallback', 'StreamExpired', 1, 0],
  sensors: ['Faulted', 'Fallback', 'StateStale', 1, 0],
  watchdog: ['Faulted', 'Fallback', 'Watchdog', 1, 0],
  estop: ['Estopped', 'Fallback', 'Estop', 1, 0],
  feedback: ['Faulted', 'Fallback', 'Verification', 1, 0],
};

for (const [id, [state, decision, reason, executions, velocity]] of Object.entries(expected)) {
  test(`WASM scenario: ${id}`, async () => {
    const p = await createPlayground(binary);
    const result = runScenario(p, id);
    assert.equal(result.state, state);
    assert.equal(result.receipts.at(-1).decision, decision);
    assert.equal(result.receipts.at(-1).reason, reason);
    assert.equal(result.executions, executions);
    assert.equal(result.velocity, velocity);
    assert.equal(result.virtual_time, true);
    assert.equal(result.hardware_validated, false);
    if (id === 'duplicate') assert.equal(result.receipts.at(-1).original_receipt, result.receipts[1].id);
    if (id === 'feedback') {
      assert.equal(result.receipts[1].decision, 'Failed');
      assert.equal(result.receipts[1].observed_valid, true);
    }
  });
}

test('e-stop recovery requires cleared input, local recovery, then new authority', async () => {
  const p = await createPlayground(binary);
  runScenario(p, 'estop');
  assert.equal(p.recover(), 11);
  p.flags(0);
  assert.equal(p.snapshot().state, 'Estopped');
  assert.equal(p.send(400, 0, 100, 0), 11);
  assert.equal(p.recover(), 0);
  assert.equal(p.snapshot().state, 'SafeIdle');
  assert.equal(p.send(400, 0, 100, 0), 3);
  assert.equal(p.grant(), 0);
  assert.equal(p.send(400, 0, 100, 0), 0);
  assert.equal(p.snapshot().velocity, 400);
});

test('local obstacle stops an already moving motor and blocks new drive commands', async () => {
  const p = await createPlayground(binary);
  runScenario(p, 'valid');
  p.flags(2);
  assert.equal(p.snapshot().velocity, 0);
  assert.equal(p.snapshot().receipts.at(-1).reason, 'Precondition');
  p.grant();
  assert.equal(p.send(400, 0, 100, 0), 5);
});

test('receipt ring remains bounded after repeated dispatches', async () => {
  const p = await createPlayground(binary);
  for (let i = 0; i < 150; i++) assert.equal(p.send(i, 0, 100, 0), 0);
  const state = p.snapshot();
  assert.equal(state.receipts.length, 64);
  assert.equal(state.executions, 150);
  assert.equal(state.receipts[0].id, 88);
  assert.equal(state.receipts.at(-1).id, 151);
  assert.equal(state.next_receipt_id, 152);
});

test('browser wrapper bounds expensive work and rejects unsupported input modes', async () => {
  const p = await createPlayground(binary);
  assert.equal(p.advance(0xffffffff), 10);
  assert.equal(p.advance(20, 3), 10);
  assert.equal(p.flags(4), 10);
  assert.equal(p.send(400, 0, 1001, 0), 10);
  assert.equal(p.send(400, 0, 100, 0xffffffff), 10);
  assert.equal(p.snapshot().now, 0);
  assert.equal(p.snapshot().executions, 0);
});

test('snapshots survive linear-memory growth and resets', async () => {
  const p = await createPlayground(binary);
  for (let i = 0; i < 80; i++) { p.send(i, 0, 100, 0); p.snapshot(); }
  p.reset();
  const state = p.snapshot();
  assert.equal(state.receipts.length, 1);
  assert.equal(state.executions, 0);
  assert.equal(state.now, 0);
});
