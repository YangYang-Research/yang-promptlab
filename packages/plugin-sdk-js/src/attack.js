import { Plugin } from './base.js';

export class AttackPlugin extends Plugin {
  static setupHandlers() {
    this.register('execute_attack', (ctx) => this.executeAttack(ctx));
  }

  static executeAttack(_ctx) {
    throw new Error('executeAttack() not implemented');
  }
}
