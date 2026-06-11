import { Plugin } from './base.js';

export class DiscoveryPlugin extends Plugin {
  static setupHandlers() {
    this.register('discover', (ctx) => this.discover(ctx));
  }

  static discover(_ctx) {
    throw new Error('discover() not implemented');
  }
}
