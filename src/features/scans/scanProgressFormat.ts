/** Format scan progress percent for display (two decimal places). */
export function formatScanProgressPercent(value: number): string {
  return `${(Math.round(value * 100) / 100).toFixed(2)}%`;
}
