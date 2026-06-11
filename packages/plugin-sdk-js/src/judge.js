import { Plugin } from './base.js';

export class JudgePlugin extends Plugin {
  static setupHandlers() {
    this.register('evaluate', (ctx) => this.evaluate(ctx));
  }

  static evaluate(_ctx) {
    throw new Error('evaluate() not implemented');
  }
}
