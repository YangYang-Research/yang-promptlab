#!/usr/bin/env node
/**
 * PromptLab Playwright runner — JSON-lines protocol over stdin/stdout.
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
    case 'begin_interactive_login':
      return ok(id, await cmdBeginInteractiveLogin(req));
    case 'finish_interactive_login':
      return ok(id, await cmdFinishInteractiveLogin(req));
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
    case 'execute_http_request':
      return ok(id, await cmdExecuteHttpRequest(req));
    case 'send_chat_prompt':
      return ok(id, await cmdSendChatPrompt(req));
    default:
      throw new Error(`unknown command: ${cmd}`);
  }
}

function ok(id, result) {
  return { id, ok: true, result };
}

/** @type {boolean} */
let interactiveRecording = false;

async function cmdBeginInteractiveLogin(req) {
  const { url, options } = req;
  await ensureBrowser({
    ...(options ?? {}),
    headless: false,
    headed: true,
  });

  if (!page) throw new Error('page not initialized');

  capturedTokens.length = 0;
  await page.goto(url, {
    waitUntil: 'domcontentloaded',
    timeout: options?.timeout_ms ?? 30000,
  });
  interactiveRecording = true;

  return { recording: true, url: page.url() };
}

async function cmdFinishInteractiveLogin() {
  if (!interactiveRecording) {
    throw new Error('no interactive recording in progress');
  }
  if (!page || !context) throw new Error('browser not initialized');

  const storageState = await context.storageState();
  const cookies = await context.cookies();
  const localTokens = await scrapeStorageTokens(page);
  const result = {
    steps: [{ action: 'interactive_login' }],
    storage_state: storageState,
    cookies,
    tokens: dedupeTokens([...capturedTokens, ...localTokens]),
    final_url: page.url(),
  };

  interactiveRecording = false;
  return result;
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
  interactiveRecording = false;
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

async function cmdSendChatPrompt(req) {
  const {
    url,
    prompt,
    input_selector: inputSelector,
    submit_selector: submitSelector,
    response_selector: responseSelector,
    storage_state_path: storageStatePath,
    file_input_selector: fileInputSelector,
    file_path: filePath,
    keep_page: keepPage,
    wait_stable_ms: waitStableMs,
    options,
  } = req;

  const timeout = options?.timeout_ms ?? 30000;
  if (!keepPage && context) {
    await context.close();
    context = null;
    page = null;
  }

  const launchOpts = { ...(options ?? {}), headless: false, headed: true };
  if (storageStatePath) {
    launchOpts.storage_state_path = storageStatePath;
  }

  await ensureBrowser(launchOpts);
  if (!page) throw new Error('page not initialized');

  const alreadyOnUrl = keepPage && page.url() && page.url() !== 'about:blank';
  if (!alreadyOnUrl) {
    await page.goto(url, {
      waitUntil: 'domcontentloaded',
      timeout,
    });
  }

  if (fileInputSelector && filePath) {
    await page.setInputFiles(fileInputSelector, filePath);
  }

  await page.fill(inputSelector, prompt ?? '');
  await Promise.all([
    page.waitForSelector(responseSelector, { timeout }),
    page.click(submitSelector),
  ]);

  const locator = page.locator(responseSelector).last();
  const stableWait = Number(waitStableMs) || 0;
  if (stableWait > 0) {
    let previous = '';
    const deadline = Date.now() + stableWait;
    while (Date.now() < deadline) {
      const current = (await locator.innerText()) ?? '';
      if (current && current === previous) {
        break;
      }
      previous = current;
      await page.waitForTimeout(Math.min(400, Math.max(50, stableWait / 5)));
    }
  }

  const responseText = await locator.innerText();
  return { response_text: responseText };
}

async function cmdExecuteHttpRequest(req) {
  const { url, method, headers, body, storage_state_path, options } = req;

  if (context) {
    await context.close();
    context = null;
    page = null;
  }

  const launchOpts = { ...(options ?? {}), headless: true };
  if (storage_state_path) {
    launchOpts.storage_state_path = storage_state_path;
  }

  await ensureBrowser(launchOpts);
  if (!context) throw new Error('context not initialized');

  const started = Date.now();
  const response = await context.request.fetch(url, {
    method: method ?? 'GET',
    headers: headers ?? {},
    data: body ?? undefined,
  });
  const responseHeaders = {};
  for (const [key, value] of Object.entries(response.headers())) {
    responseHeaders[key] = value;
  }

  return {
    status: response.status(),
    headers: responseHeaders,
    body: await response.text(),
    duration_ms: Date.now() - started,
  };
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
