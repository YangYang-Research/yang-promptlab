import readline from 'node:readline';

export function writeMessage(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

export async function readMessage() {
  const rl = readline.createInterface({ input: process.stdin });
  for await (const line of rl) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    rl.close();
    return JSON.parse(trimmed);
  }
  return null;
}

export function respond(requestId, result) {
  writeMessage({ id: requestId, result });
}

export function respondError(requestId, message) {
  writeMessage({ id: requestId, error: { message } });
}

export function hostCall(method, params = {}) {
  writeMessage({ type: 'host', method, params });
}
