// All state transitions and decisions execute inside the compiled Rust runtime.
export async function createPlayground(bytes) {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const api = instance.exports;
  const decoder = new TextDecoder();
  const snapshot = () => {
    const pointer = api.pxr_demo_snapshot();
    const length = api.pxr_demo_snapshot_len();
    return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, pointer, length)));
  };
  return Object.freeze({
    snapshot,
    reset: () => api.pxr_demo_reset(),
    send: (velocity, turn, ttl, delay) => api.pxr_demo_send(velocity, turn, ttl, delay),
    repeat: () => api.pxr_demo_repeat(),
    advance: (ms, mode = 0) => api.pxr_demo_advance(ms, mode),
    grant: () => api.pxr_demo_grant(),
    estop: () => api.pxr_demo_estop(),
    flags: (value) => api.pxr_demo_flags(value),
    recover: () => api.pxr_demo_recover(),
    feedback: (wrong) => api.pxr_demo_feedback(Number(wrong)),
  });
}

export const scenarios = [
  { id: 'valid', title: 'A valid command', category: 'Admission',
    description: 'The lease is live, the command is fresh, and 400 mm/s is within the motor limit. The driver observes the requested output.',
    run: (p) => p.send(400, 0, 100, 0) },
  { id: 'bounds', title: 'Exceed the limit', category: 'Admission',
    description: 'Request 1,500 mm/s. The reference motor capability allows at most 1,000 mm/s. Rejection happens before any driver dispatch.',
    run: (p) => p.send(1500, 0, 100, 0) },
  { id: 'stale', title: 'Deliver it late', category: 'Admission',
    description: 'Hold a command for 120 ms when its validity window is 100 ms. Sensors and supervision stay active during the delay.',
    run: (p) => p.send(400, 0, 100, 120) },
  { id: 'duplicate', title: 'Replay an action', category: 'Admission',
    description: 'Deliver the same action twice. The first dispatch succeeds; the duplicate points back to its original receipt and never reaches the driver.',
    run: (p) => { p.send(400, 0, 100, 0); p.repeat(); } },
  { id: 'stream', title: 'Lose the command stream', category: 'Supervision',
    description: 'Execute a command, then advance 100 ms without another one. The local supervisor continues running and invokes fallback at stream expiry.',
    run: (p) => { p.send(400, 0, 100, 0); p.advance(100); } },
  { id: 'sensors', title: 'Lose the sensor feed', category: 'Supervision',
    description: 'Use a 1,000 ms command window but stop sensor updates. Supervision still runs every 10 ms; at 250 ms the stale sensor feed triggers a fault.',
    run: (p) => { p.send(400, 0, 1000, 0); p.advance(250, 1); } },
  { id: 'watchdog', title: 'Miss supervisor service', category: 'Supervision',
    description: 'Leave a 51 ms gap between supervisor calls. On the next call, PXR detects the missed 50 ms budget and invokes fallback. A hardware watchdog is needed if service never resumes.',
    run: (p) => { p.send(400, 0, 1000, 0); p.advance(51, 2); } },
  { id: 'estop', title: 'Latch an emergency stop', category: 'Local state',
    description: 'Execute a command, then trigger a local e-stop. PXR invokes fallback and revokes authority. Recovery requires clearing the local input and an explicit recovery call.',
    run: (p) => { p.send(400, 0, 100, 0); p.estop(); } },
  { id: 'feedback', title: 'Return incorrect feedback', category: 'Driver',
    description: 'The simulated driver dispatches the command but reports a different output. PXR records failed verification, faults the runtime, and invokes fallback.',
    run: (p) => { p.feedback(true); p.send(400, 0, 100, 0); } },
];

export function runScenario(playground, id) {
  const scenario = scenarios.find((item) => item.id === id);
  if (!scenario) throw new Error(`Unknown scenario: ${id}`);
  playground.reset();
  scenario.run(playground);
  return playground.snapshot();
}
