import { createPlayground, scenarios, runScenario } from './engine.mjs';

const $ = (id) => document.getElementById(id);
let playground;
let current;
let revision;
let activeScenario = 'valid';
const labels = { SafeIdle: 'Idle', Armed: 'Armed', Faulted: 'Faulted', Estopped: 'E-stop latched' };

function message(state) {
  if (state.operation_reason !== 'Ok') return `Operation result: ${state.operation_reason}. Inspect the receipts below.`;
  const receipt = state.receipts.at(-1);
  return receipt ? `${receipt.decision} · ${receipt.reason} · receipt #${receipt.id}` : 'Ready for a command.';
}

function render(state) {
  current = state;
  $('state').textContent = labels[state.state] ?? state.state;
  $('state').dataset.state = state.state;
  $('velocity').textContent = state.velocity.toLocaleString();
  $('turn-output').textContent = state.turn.toLocaleString();
  $('clock').textContent = state.now.toLocaleString();
  $('executions').textContent = state.executions;
  $('lease').textContent = state.lease_expires ? `${Math.max(0, state.lease_expires - state.now)} ms` : 'None';
  $('epoch').textContent = state.epoch;
  // A static orientation indicates output; virtual time never follows animation frames.
  $('rotor').setAttribute('transform', `rotate(${state.velocity / 20} 121 75)`);
  $('operation').textContent = message(state);
  $('receipt-body').replaceChildren();
  for (const receipt of [...state.receipts].reverse()) {
    const row = document.createElement('tr');
    const values = [`#${receipt.id}`, `${receipt.time} ms`, receipt.decision, receipt.reason, receipt.dispatched ? 'Yes' : 'No'];
    values.forEach((value, i) => {
      const cell = document.createElement('td');
      if (i === 2) {
        const badge = document.createElement('span');
        badge.className = 'decision';
        badge.dataset.decision = receipt.decision;
        badge.textContent = value;
        cell.append(badge);
      } else cell.textContent = value;
      row.append(cell);
    });
    const cell = document.createElement('td');
    const details = document.createElement('details');
    const summary = document.createElement('summary');
    summary.textContent = `Inspect #${receipt.id}`;
    const pre = document.createElement('pre');
    pre.textContent = JSON.stringify(receipt, null, 2);
    details.append(summary, pre);
    cell.append(details);
    row.append(cell);
    $('receipt-body').append(row);
  }
}

function fail(error) {
  console.error(error);
  $('error').hidden = false;
  $('error').textContent = 'The runtime could not run. Refresh this page in a browser with WebAssembly support, or use the CLI demo linked below.';
  $('engine-status').textContent = 'Runtime unavailable';
  $('engine-dot').classList.add('pending');
  document.querySelectorAll('.lab button, .lab input, #export').forEach((el) => { el.disabled = true; });
}

function act(operation) {
  try { operation(); render(playground.snapshot()); } catch (error) { fail(error); }
}

function selectScenario(id, updateUrl = true) {
  const scenario = scenarios.find((item) => item.id === id);
  if (!scenario) return;
  activeScenario = id;
  $('scenario-title').textContent = scenario.title;
  $('scenario-category').textContent = scenario.category.toUpperCase();
  $('scenario-description').textContent = scenario.description;
  document.querySelectorAll('.scenario-button').forEach((el) => el.setAttribute('aria-pressed', String(el.dataset.scenario === id)));
  if (updateUrl) history.replaceState(null, '', `?scenario=${encodeURIComponent(id)}#playground`);
  act(() => runScenario(playground, id));
}

for (const [index, scenario] of scenarios.entries()) {
  const button = document.createElement('button');
  button.className = 'scenario-button';
  button.dataset.scenario = scenario.id;
  button.disabled = true;
  button.setAttribute('aria-pressed', 'false');
  const number = document.createElement('span');
  number.textContent = String(index + 1).padStart(2, '0');
  button.append(number, document.createTextNode(scenario.title));
  button.addEventListener('click', () => selectScenario(scenario.id));
  $('scenario-list').append(button);
}

$('command-form').addEventListener('submit', (event) => {
  event.preventDefault();
  const inputs = ['velocity', 'turn', 'ttl', 'delay'].map((key) => $(`command-${key}`));
  if (inputs.some((input) => !input.reportValidity())) return;
  const values = inputs.map((input) => input.valueAsNumber);
  if (!values.every(Number.isSafeInteger)) return;
  act(() => playground.send(...values));
});

const actions = {
  reset: () => {
    playground.reset();
    activeScenario = 'sandbox';
    $('scenario-title').textContent = 'Your command sandbox';
    $('scenario-category').textContent = 'CUSTOM INPUT';
    $('scenario-description').textContent = 'Fresh sandbox. A 2,000 ms motor lease is ready; submit a command or choose a scenario.';
    document.querySelectorAll('.scenario-button').forEach((el) => el.setAttribute('aria-pressed', 'false'));
    history.replaceState(null, '', location.pathname + '#playground');
  },
  repeat: () => playground.repeat(), advance: () => playground.advance(100),
  grant: () => playground.grant(), estop: () => playground.estop(),
  obstacle: () => playground.flags(current.flags | 2),
  'clear-inputs': () => playground.flags(0),
  'correct-feedback': () => playground.feedback(false), recover: () => playground.recover(),
};
for (const [id, operation] of Object.entries(actions)) $(id).addEventListener('click', () => act(operation));
$('export').addEventListener('click', () => {
  const data = { scenario: activeScenario, build: revision ?? null, snapshot: current };
  const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2) + '\n'], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = `pxr-${activeScenario}-${current.now}ms.json`;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});

try {
  const response = await fetch('./pxr_playground.wasm');
  if (!response.ok) throw new Error(`Runtime download failed: ${response.status}`);
  playground = await createPlayground(await response.arrayBuffer());
  document.querySelectorAll('.lab button, .lab input, #export').forEach((el) => { el.disabled = false; });
  $('engine-status').textContent = 'Rust runtime ready · WebAssembly';
  $('engine-dot').classList.remove('pending');
  const requested = new URL(location.href).searchParams.get('scenario');
  selectScenario(scenarios.some((item) => item.id === requested) ? requested : 'valid', false);
  try {
    const response = await fetch('./build.json');
    if (response.ok) {
      revision = await response.json();
      if (/^[0-9a-f]{40}$/.test(revision.commit)) {
        $('revision').textContent = `Source ${revision.commit.slice(0, 7)}`;
        $('revision').href = `https://github.com/ashutosh-rath02/pxr/tree/${revision.commit}`;
      }
    }
  } catch { /* Provenance is optional offline; runtime loading errors remain visible. */ }
} catch (error) { fail(error); }
