import { hostCall, readMessage, respond, respondError } from './protocol.js';

export class PluginContext {
  constructor({ pluginId, pluginDir, params = {} }) {
    this.pluginId = pluginId;
    this.pluginDir = pluginDir;
    this.params = params;
  }

  log(message, level = 'info') {
    hostCall('log', { level, message });
  }

  emitFinding(finding) {
    hostCall('emit_finding', finding);
  }
}

export class Plugin {
  static handlers = new Map();

  static register(method, handler) {
    this.handlers.set(method, handler);
  }

  static async run() {
    const base = new PluginContext({
      pluginId: process.env.AISEC_PLUGIN_ID ?? 'unknown',
      pluginDir: process.env.AISEC_PLUGIN_DIR ?? '.',
    });

    while (true) {
      const message = await readMessage();
      if (!message) break;
      if (message.type === 'shutdown') break;

      const { id: requestId, method, params = {} } = message;
      if (!requestId || !method) continue;

      const handler = this.handlers.get(method);
      if (!handler) {
        respondError(requestId, `unknown method: ${method}`);
        continue;
      }

      const ctx = new PluginContext({
        pluginId: base.pluginId,
        pluginDir: base.pluginDir,
        params,
      });

      try {
        const result = await handler(ctx);
        respond(requestId, result);
      } catch (err) {
        respondError(requestId, err instanceof Error ? err.message : String(err));
      }
      break;
    }
  }
}
