#!/usr/bin/env node
/**
 * AISec Discovery — Playwright runner (JSON-lines protocol over stdin/stdout).
 *
 * Launches real Chromium, navigates a target, waits for single-page-app
 * rendering, and captures all network traffic (with emphasis on XHR/fetch API
 * calls) so the Rust Discovery Engine can export them as endpoints.
 *
 * Commands: navigate_capture, close
 */
import { chromium } from 'playwright';
import readline from 'node:readline';

/** @type {import('playwright').Browser | null} */
let browser = null;
/** @type {import('playwright').BrowserContext | null} */
let context = null;
/** @type {import('playwright').Page | null} */
let page = null;

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

rl.on('line', async (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;
  let request;
  try {
    request = JSON.parse(trimmed);
    const result = await handleCommand(request);
    emit({ id: request.id, ok: true, result });
  } catch (err) {
    emit({
      id: request?.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
});

function emit(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

async function handleCommand(req) {
  switch (req.cmd) {
    case 'navigate_capture':
      return cmdNavigateCapture(req);
    case 'close':
      return cmdClose();
    default:
      throw new Error(`unknown command: ${req.cmd}`);
  }
}

async function ensureBrowser(options) {
  if (!browser) {
    browser = await chromium.launch({ headless: options.headless ?? true });
  }
  if (!context) {
    const ctxOptions = {
      userAgent: options.user_agent || undefined,
      ignoreHTTPSErrors: true,
    };
    if (options.storage_state_path) {
      ctxOptions.storageState = options.storage_state_path;
    }
    context = await browser.newContext(ctxOptions);
    page = await context.newPage();
  }
}

async function cmdNavigateCapture(req) {
  const options = req.options ?? {};
  await ensureBrowser(options);
  if (!page) throw new Error('page not initialized');

  const maxRequests = options.max_requests ?? 1000;
  /** @type {Map<import('playwright').Request, object>} */
  const entries = new Map();

  const onRequest = (request) => {
    if (entries.size >= maxRequests) return;
    let fromMain = false;
    try {
      fromMain = request.frame() === page.mainFrame();
    } catch {
      // frame detached during navigation
    }
    entries.set(request, {
      method: request.method(),
      url: request.url(),
      resource_type: request.resourceType(),
      status: null,
      content_type: null,
      from_main_frame: fromMain,
    });
  };

  const onResponse = (response) => {
    const entry = entries.get(response.request());
    if (entry) {
      entry.status = response.status();
      const headers = response.headers();
      entry.content_type = headers['content-type'] ?? null;
    }
  };

  page.on('request', onRequest);
  page.on('response', onResponse);

  const waitUntil = options.wait_until ?? 'networkidle';
  const navTimeout = options.timeout_ms ?? 30000;
  let navError = null;
  try {
    await page.goto(req.url, { waitUntil, timeout: navTimeout });
  } catch (err) {
    navError = err instanceof Error ? err.message : String(err);
  }

  // Allow late SPA XHR/fetch calls to settle after first paint.
  const settle = options.settle_ms ?? 1500;
  if (settle > 0) {
    await page.waitForTimeout(settle);
  }
  try {
    await page.waitForLoadState('networkidle', { timeout: options.idle_timeout_ms ?? 5000 });
  } catch {
    // tolerate idle timeout — long-polling SPAs may never go fully idle
  }

  page.off('request', onRequest);
  page.off('response', onResponse);

  let title = '';
  let finalUrl = req.url;
  try {
    title = await page.title();
  } catch {
    // page may have navigated away
  }
  try {
    finalUrl = page.url();
  } catch {
    // ignore
  }

  return {
    final_url: finalUrl,
    title,
    requests: Array.from(entries.values()),
    nav_error: navError,
  };
}

async function cmdClose() {
  if (context) {
    await context.close().catch(() => {});
    context = null;
    page = null;
  }
  if (browser) {
    await browser.close().catch(() => {});
    browser = null;
  }
  return { closed: true };
}

process.on('SIGTERM', async () => {
  await cmdClose();
  process.exit(0);
});
