const { test, expect } = require('@playwright/test');

test('all nine scenarios render real decisions without page errors or overflow', async ({ page }, testInfo) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('./');
  await expect(page.locator('#engine-status')).toContainText('Rust runtime ready');
  for (const [id, reason, executions] of [
    ['bounds', 'Bound', 0], ['stale', 'Stale', 0], ['duplicate', 'Duplicate', 1],
    ['stream', 'StreamExpired', 1], ['sensors', 'StateStale', 1], ['watchdog', 'Watchdog', 1],
    ['estop', 'Estop', 1], ['feedback', 'Verification', 1], ['valid', 'Ok', 1],
  ]) {
    await page.locator(`[data-scenario="${id}"]`).click();
    await expect(page.locator('#receipt-body tr').first()).toContainText(reason);
    await expect(page.locator('#executions')).toHaveText(String(executions));
    await expect(page.locator(`[data-scenario="${id}"]`)).toHaveAttribute('aria-pressed', 'true');
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  expect(errors).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath('playground.png'), fullPage: true });
});

test('custom commands and e-stop recovery obey the runtime contract', async ({ page }) => {
  await page.goto('./');
  await page.getByRole('button', { name: 'Reset sandbox' }).click();
  await page.locator('#command-velocity').fill('1500');
  await page.getByRole('button', { name: 'Submit command' }).click();
  await expect(page.locator('#operation')).toContainText('Bound');
  await expect(page.locator('#executions')).toHaveText('0');
  await page.locator('#command-velocity').fill('-500');
  await page.getByRole('button', { name: 'Submit command' }).click();
  await expect(page.locator('#velocity')).toHaveText('-500');
  await page.getByRole('button', { name: 'Emergency stop', exact: true }).click();
  await expect(page.locator('#state')).toHaveText('E-stop latched');
  await expect(page.locator('#velocity')).toHaveText('0');
  await page.locator('.local-controls summary').click();
  await page.getByRole('button', { name: 'Recover locally' }).click();
  await expect(page.locator('#operation')).toContainText('Estop');
  await page.getByRole('button', { name: 'Clear local inputs' }).click();
  await expect(page.locator('#state')).toHaveText('E-stop latched');
  await page.getByRole('button', { name: 'Recover locally' }).click();
  await page.getByRole('button', { name: 'Grant new lease' }).click();
  await page.getByRole('button', { name: 'Submit command' }).click();
  await expect(page.locator('#velocity')).toHaveText('-500');
  await page.getByRole('button', { name: 'Advance 100 ms' }).click();
  await expect(page.locator('#velocity')).toHaveText('0');
  await expect(page.locator('#receipt-body tr').first()).toContainText('StreamExpired');
});

test('shared scenario links and downloaded JSON preserve decision evidence', async ({ page }) => {
  await page.goto('./?scenario=duplicate');
  await expect(page.locator('#scenario-title')).toHaveText('Replay an action');
  await expect(page.locator('#receipt-body tr').first()).toContainText('Duplicate');
  await page.locator('#receipt-body tr').first().locator('summary').click();
  await expect(page.locator('#receipt-body tr').first().locator('pre')).toContainText('"original_receipt": 2');
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export JSON' }).click();
  const download = await downloadPromise;
  const stream = await download.createReadStream();
  let content = '';
  for await (const chunk of stream) content += chunk.toString();
  const exported = JSON.parse(content);
  expect(exported.snapshot.executions).toBe(1);
  expect(exported.snapshot.receipts.at(-1).original_receipt).toBe(2);
  expect(exported.snapshot.hardware_validated).toBe(false);
});

test('a failed WASM download leaves a visible error and disabled controls', async ({ page }) => {
  await page.route('**/pxr_playground.wasm', route => route.abort());
  await page.goto('./');
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Submit command' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Export JSON' })).toBeDisabled();
});
