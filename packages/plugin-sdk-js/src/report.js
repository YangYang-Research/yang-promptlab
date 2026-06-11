import { Plugin } from './base.js';

export class ReportPlugin extends Plugin {
  static setupHandlers() {
    this.register('render_report', (ctx) => this.renderReport(ctx));
  }

  static renderReport(_ctx) {
    throw new Error('renderReport() not implemented');
  }
}
