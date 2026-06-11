#!/usr/bin/env node
/**
 * AISec Playwright runner — JSON-lines protocol over stdin/stdout.
 *
 * Commands: launch, close, record_login, replay_session, extract_tokens,
 *           get_cookies, set_cookies
 */
import { chromium } from 'playwright';
import readline from 'node:readline';
import fs from 'node:fs/promises';

/** @type {import('playwright').Browser | null} */
let browser = null;
/** @type {import('playwright').BrowserContext | null} */
let context = null;
/** @type {import('playwright').Page | null} */
let page = null;

/** @type {Array<{kind: string, source: string, value: string, url?: string}>} */
const capturedTokens = [];

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

rl.on('line', async (line) => {
  let request;
  try {
    request = JSON.parse(line);
    const response = await handleCommand(request);
    emit(response);
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
  const { id, cmd } = req;

  switch (cmd) {
    case 'launch':
      return ok(id, await cmdLaunch(req));
    case 'close':
      return ok(id, await cmdClose());
    case 'record_login':
      return ok(id, await cmdRecordLogin(req));
    case 'replay_session':
      return ok(id, await cmdReplaySession(req));
    case 'extract_tokens':
      return ok(id, await cmdExtractTokens(req));
    case 'get_cookies':
      return ok(id, await cmdGetCookies(req));
    case 'set_cookies':
      return ok(id, await cmdSetCookies(req));
    default:
      throw new Error(`unknown command: ${cmd}`);
  }
}

function ok(id, result) {
  return { id, ok: true, result };
}

async function cmdLaunch(req) {
  await ensureBrowser(req.options ?? {});
  return { launched: true };
}

async function cmdClose() {
  if (context) {
    await context.close();
    context = null;
    page = null;
  }
  if (browser) {
    await browser.close();
    browser = null;
  }
  capturedTokens.length = 0;
  return { closed: true };
}

async function ensureBrowser(options) {
  if (!browser) {
    browser = await chromium.launch({
      headless: options.headless ?? true,
      slowMo: options.slow_mo ?? 0,
    });
  }
  if (!context) {
    const ctxOptions = {};
    if (options.storage_state_path) {
      ctxOptions.storageState = options.storage_state_path;
    }
    context = await browser.newContext(ctxOptions);
    page = await context.newPage();
    wireTokenCapture(page);
  }
  return { browser: true, context: true };
}

function wireTokenCapture(activePage) {
  capturedTokens.length = 0;

  activePage.on('response', async (response) => {
    try {
      const headers = response.headers();
      const auth = headers['authorization'] || headers['Authorization'];
      if (auth) {
        capturedTokens.push({
          kind: auth.toLowerCase().startsWith('bearer') ? 'bearer' : 'authorization',
          source: 'response_header',
          value: auth,
          url: response.url(),
        });
      }

      const ct = headers['content-type'] || '';
      if (ct.includes('json')) {
        const text = await response.text();
        extractTokensFromJson(text, response.url());
      }
    } catch {
      // ignore body read failures on parallel navigation
    }
  });
}

function extractTokensFromJson(text, url) {
  try {
    const data = JSON.parse(text);
    const pairs = [
      ['access_token', 'oauth_access'],
      ['refresh_token', 'oauth_refresh'],
      ['id_token', 'oidc_id'],
      ['token', 'generic'],
    ];
    for (const [field, kind] of pairs) {
      if (typeof data[field] === 'string') {
        capturedTokens.push({ kind, source: 'response_body', value: data[field], url });
      }
    }
  } catch {
    // not json
  }
}

async function cmdRecordLogin(req) {
  const { url, method, config, options } = req;
  await ensureBrowser(options ?? {});

  if (!page) throw new Error('page not initialized');

  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: options?.timeout_ms ?? 30000 });

  const steps = [];

  if (method === 'username_password') {
    const { username, password, username_selector, password_selector, submit_selector } = config;
    if (!username_selector || !password_selector || !submit_selector) {
      throw new Error('username_password requires selectors');
    }
    await page.fill(username_selector, username ?? '');
    steps.push({ action: 'fill', selector: username_selector, field: 'username' });
    await page.fill(password_selector, password ?? '');
    steps.push({ action: 'fill', selector: password_selector, field: 'password' });
    await Promise.all([
      page.waitForNavigation({ waitUntil: 'networkidle', timeout: options?.timeout_ms ?? 30000 }).catch(() => null),
      page.click(submit_selector),
    ]);
    steps.push({ action: 'click', selector: submit_selector, field: 'submit' });
  } else if (['oauth', 'oidc', 'saml'].includes(method)) {
    const waitUntil = config.success_url_pattern;
    const timeout = options?.interactive_timeout_ms ?? 120000;
    if (waitUntil) {
      const regex = new RegExp(waitUntil);
      await page.waitForURL(regex, { timeout });
      steps.push({ action: 'wait_for_url', pattern: waitUntil });
    } else if (options?.headed) {
      await page.waitForTimeout(timeout);
      steps.push({ action: 'interactive_wait', timeout_ms: timeout });
    }
  } else {
    steps.push({ action: 'navigate', url });
  }

  const storageState = await context.storageState();
  const cookies = await context.cookies();
  const localTokens = await scrapeStorageTokens(page);

  return {
    steps,
    storage_state: storageState,
    cookies,
    tokens: dedupeTokens([...capturedTokens, ...localTokens]),
    final_url: page.url(),
  };
}

async function scrapeStorageTokens(activePage) {
  return activePage.evaluate(() => {
    const found = [];
    const keys = ['access_token', 'refresh_token', 'id_token', 'token', 'jwt', 'auth_token'];
    for (const storage of [localStorage, sessionStorage]) {
      for (const key of keys) {
        const value = storage.getItem(key);
        if (value) {
          found.push({ kind: key, source: 'browser_storage', value });
        }
      }
    }
    return found;
  });
}

async function cmdReplaySession(req) {
  const { url, storage_state, storage_state_path, options } = req;

  if (context) {
    await context.close();
    context = null;
    page = null;
  }

  const launchOpts = { ...(options ?? {}) };
  if (storage_state_path) {
    launchOpts.storage_state_path = storage_state_path;
  }

  await ensureBrowser(launchOpts);

  if (storage_state && context) {
    if (storage_state.cookies) {
      await context.addCookies(storage_state.cookies);
    }
  }

  if (!page) throw new Error('page not initialized');

  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: options?.timeout_ms ?? 30000 });

  return {
    url: page.url(),
    cookies: await context.cookies(),
    tokens: dedupeTokens([...capturedTokens, ...(await scrapeStorageTokens(page))]),
  };
}

async function cmdExtractTokens(req) {
  await ensureBrowser(req.options ?? {});
  if (!page) throw new Error('page not initialized');

  if (req.url) {
    await page.goto(req.url, { waitUntil: 'domcontentloaded' });
  }

  return {
    tokens: dedupeTokens([...capturedTokens, ...(await scrapeStorageTokens(page))]),
    cookies: await context.cookies(),
  };
}

async function cmdGetCookies(req) {
  await ensureBrowser(req.options ?? {});
  if (!context) throw new Error('context not initialized');
  const cookies = req.url ? await context.cookies(req.url) : await context.cookies();
  return { cookies };
}

async function cmdSetCookies(req) {
  await ensureBrowser(req.options ?? {});
  if (!context) throw new Error('context not initialized');
  if (req.cookies?.length) {
    await context.addCookies(req.cookies);
  }
  return { cookies: await context.cookies() };
}

function dedupeTokens(tokens) {
  const seen = new Set();
  return tokens.filter((t) => {
    const key = `${t.kind}:${t.value}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

process.on('SIGTERM', async () => {
  await cmdClose();
  process.exit(0);
});
