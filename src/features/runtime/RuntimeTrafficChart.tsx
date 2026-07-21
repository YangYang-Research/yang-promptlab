import { useEffect, useId, useMemo, useRef, useState } from "react";

import {
  getRuntimeTrafficStats,
  type RuntimeTrafficSnapshot,
} from "@/shared/ipc/runtime";

export type TrafficRangeId =
  | "1m"
  | "15m"
  | "1h"
  | "4h"
  | "1d"
  | "2d"
  | "3d"
  | "5d"
  | "1w"
  | "1M"
  | "3M";

type TrafficRange = {
  id: TrafficRangeId;
  label: string;
  windowMs: number;
  /** Preferred bucket width; backend may coarsen if over max bucket count. */
  bucketMs: number;
  axisStepMs: number;
  pollMs: number;
};

const DAY_MS = 24 * 60 * 60_000;

export const TRAFFIC_RANGES: readonly TrafficRange[] = [
  { id: "1m", label: "1m", windowMs: 60_000, bucketMs: 1_000, axisStepMs: 10_000, pollMs: 1_000 },
  { id: "15m", label: "15m", windowMs: 15 * 60_000, bucketMs: 5_000, axisStepMs: 60_000, pollMs: 2_000 },
  { id: "1h", label: "1h", windowMs: 60 * 60_000, bucketMs: 15_000, axisStepMs: 10 * 60_000, pollMs: 2_500 },
  { id: "4h", label: "4h", windowMs: 4 * 60 * 60_000, bucketMs: 60_000, axisStepMs: 30 * 60_000, pollMs: 5_000 },
  { id: "1d", label: "1D", windowMs: DAY_MS, bucketMs: 5 * 60_000, axisStepMs: 2 * 60 * 60_000, pollMs: 5_000 },
  { id: "2d", label: "2D", windowMs: 2 * DAY_MS, bucketMs: 10 * 60_000, axisStepMs: 6 * 60 * 60_000, pollMs: 8_000 },
  { id: "3d", label: "3D", windowMs: 3 * DAY_MS, bucketMs: 15 * 60_000, axisStepMs: 12 * 60 * 60_000, pollMs: 10_000 },
  { id: "5d", label: "5D", windowMs: 5 * DAY_MS, bucketMs: 20 * 60_000, axisStepMs: 12 * 60 * 60_000, pollMs: 10_000 },
  { id: "1w", label: "1W", windowMs: 7 * DAY_MS, bucketMs: 30 * 60_000, axisStepMs: DAY_MS, pollMs: 10_000 },
  { id: "1M", label: "1M", windowMs: 30 * DAY_MS, bucketMs: 2 * 60 * 60_000, axisStepMs: 5 * DAY_MS, pollMs: 15_000 },
  { id: "3M", label: "3M", windowMs: 90 * DAY_MS, bucketMs: 6 * 60 * 60_000, axisStepMs: 14 * DAY_MS, pollMs: 30_000 },
] as const;

const PRIMARY_RANGE_IDS: readonly TrafficRangeId[] = ["1m", "15m", "1h", "4h"];

export const PRIMARY_TRAFFIC_RANGES = TRAFFIC_RANGES.filter((range) =>
  PRIMARY_RANGE_IDS.includes(range.id),
);

export const MORE_TRAFFIC_RANGES = TRAFFIC_RANGES.filter(
  (range) => !PRIMARY_RANGE_IDS.includes(range.id),
);

type RuntimeTrafficChartProps = {
  enabled: boolean;
  defaultRangeId?: TrafficRangeId;
};

type ChartBand = "upper" | "lower";

function formatAxisLabel(atMs: number, spanMs: number): string {
  const date = new Date(atMs);
  if (spanMs <= 15 * 60_000) {
    return date.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }
  if (spanMs <= 24 * 60 * 60_000) {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  if (spanMs <= 7 * 24 * 60 * 60_000) {
    return date.toLocaleString([], {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

function buildBandPolyline(
  values: number[],
  width: number,
  height: number,
  maxValue: number,
  band: ChartBand,
): string {
  if (values.length === 0) return "";
  const maxY = Math.max(maxValue, 1);
  const mid = height / 2;
  const bandHeight = mid;
  return values
    .map((value, index) => {
      const x =
        values.length === 1 ? width / 2 : (index / (values.length - 1)) * width;
      const ratio = Math.min(value / maxY, 1);
      const y =
        band === "upper"
          ? mid - ratio * bandHeight
          : mid + ratio * bandHeight;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}

function buildTimeMarkers(
  firstAtMs: number,
  lastAtMs: number,
  stepMs: number,
  spanMs: number,
): Array<{ atMs: number; label: string; offset: number }> {
  const span = Math.max(lastAtMs - firstAtMs, 1);
  let step = Math.max(stepMs, 1);
  while (span / step > 10) {
    step *= 2;
  }

  const markers: Array<{ atMs: number; label: string; offset: number }> = [];
  for (let atMs = firstAtMs; atMs < lastAtMs; atMs += step) {
    markers.push({
      atMs,
      label: formatAxisLabel(atMs, spanMs),
      offset: (atMs - firstAtMs) / span,
    });
  }
  markers.push({
    atMs: lastAtMs,
    label: formatAxisLabel(lastAtMs, spanMs),
    offset: 1,
  });
  return markers;
}

function resolveRange(id: TrafficRangeId): TrafficRange {
  return TRAFFIC_RANGES.find((range) => range.id === id) ?? TRAFFIC_RANGES[0];
}

export function RuntimeTrafficChart({
  enabled,
  defaultRangeId = "1m",
}: RuntimeTrafficChartProps) {
  const gradientId = useId().replace(/:/g, "");
  const [rangeId, setRangeId] = useState<TrafficRangeId>(defaultRangeId);
  const [moreOpen, setMoreOpen] = useState(false);
  const moreRef = useRef<HTMLDivElement>(null);
  const range = resolveRange(rangeId);
  const [snapshot, setSnapshot] = useState<RuntimeTrafficSnapshot | null>(null);
  const selectedMoreRange = MORE_TRAFFIC_RANGES.find((option) => option.id === rangeId);
  const moreActive = Boolean(selectedMoreRange);

  useEffect(() => {
    if (!moreOpen) return;
    function onPointerDown(event: MouseEvent) {
      if (!moreRef.current?.contains(event.target as Node)) {
        setMoreOpen(false);
      }
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setMoreOpen(false);
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [moreOpen]);

  useEffect(() => {
    if (!enabled) {
      setSnapshot(null);
      return;
    }

    let cancelled = false;

    async function load() {
      try {
        const next = await getRuntimeTrafficStats(range.windowMs, range.bucketMs);
        if (!cancelled) setSnapshot(next);
      } catch {
        if (!cancelled) setSnapshot(null);
      }
    }

    void load();
    const timer = window.setInterval(() => void load(), range.pollMs);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [enabled, range.windowMs, range.bucketMs, range.pollMs]);

  const chart = useMemo(() => {
    const width = 720;
    const height = 160;
    const mid = height / 2;
    const now = Date.now();
    const emptyBucketCount = Math.min(
      241,
      Math.floor(range.windowMs / range.bucketMs) + 1,
    );
    const buckets =
      snapshot?.buckets && snapshot.buckets.length > 0
        ? snapshot.buckets
        : Array.from({ length: emptyBucketCount }, (_, index) => ({
            atMs:
              now -
              range.windowMs +
              (index * range.windowMs) / Math.max(emptyBucketCount - 1, 1),
            sent: 0,
            received: 0,
          }));
    const sent = buckets.map((bucket) => bucket.sent);
    const received = buckets.map((bucket) => bucket.received);
    const hasWindowTraffic =
      sent.some((value) => value > 0) || received.some((value) => value > 0);
    const peak = Math.max(1, ...sent, ...received);
    const maxValue = peak;
    const firstAtMs = buckets[0].atMs;
    const lastAtMs = buckets[buckets.length - 1].atMs;
    const axisEndAtMs = Math.max(
      lastAtMs,
      firstAtMs + (snapshot?.windowMs ?? range.windowMs),
    );
    const spanMs = axisEndAtMs - firstAtMs;
    return {
      width,
      height,
      mid,
      hasWindowTraffic,
      sentLine: hasWindowTraffic
        ? buildBandPolyline(sent, width, height, maxValue, "upper")
        : "",
      receivedLine: hasWindowTraffic
        ? buildBandPolyline(received, width, height, maxValue, "lower")
        : "",
      timeMarkers: buildTimeMarkers(firstAtMs, axisEndAtMs, range.axisStepMs, spanMs),
    };
  }, [snapshot, range.windowMs, range.bucketMs, range.axisStepMs]);

  return (
    <div className="runtime-traffic">
      <div className="runtime-traffic__header">
        <div className="runtime-traffic__ranges" role="group" aria-label="Traffic time range">
          {PRIMARY_TRAFFIC_RANGES.map((option) => (
            <button
              key={option.id}
              type="button"
              className={`runtime-traffic__range${
                option.id === rangeId ? " runtime-traffic__range--active" : ""
              }`}
              aria-pressed={option.id === rangeId}
              onClick={() => {
                setRangeId(option.id);
                setMoreOpen(false);
              }}
            >
              {option.label}
            </button>
          ))}
          <div className="runtime-traffic__more" ref={moreRef}>
            <button
              type="button"
              className={`runtime-traffic__range runtime-traffic__range--more${
                moreActive ? " runtime-traffic__range--active" : ""
              }`}
              aria-haspopup="menu"
              aria-expanded={moreOpen}
              onClick={() => setMoreOpen((open) => !open)}
            >
              {selectedMoreRange?.label ?? "More"}
              <span className="runtime-traffic__more-caret" aria-hidden>
                ▾
              </span>
            </button>
            {moreOpen ? (
              <div className="runtime-traffic__more-menu" role="menu">
                {MORE_TRAFFIC_RANGES.map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    role="menuitemradio"
                    aria-checked={option.id === rangeId}
                    className={`runtime-traffic__more-item${
                      option.id === rangeId ? " runtime-traffic__more-item--active" : ""
                    }`}
                    onClick={() => {
                      setRangeId(option.id);
                      setMoreOpen(false);
                    }}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        </div>
        <div className="runtime-traffic__legend">
          <span className="runtime-traffic__legend-item runtime-traffic__legend-item--sent">
            Sent {snapshot?.totalSent ?? 0}
          </span>
          <span className="runtime-traffic__legend-item runtime-traffic__legend-item--received">
            Received {snapshot?.totalReceived ?? 0}
          </span>
        </div>
      </div>

      <svg
        className="runtime-traffic__chart"
        viewBox={`0 0 ${chart.width} ${chart.height}`}
        preserveAspectRatio="none"
        role="img"
        aria-label="AI Runtime packages sent and received over time"
      >
        <defs>
          <linearGradient id={`${gradientId}-sent`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.28" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
          </linearGradient>
          <linearGradient
            id={`${gradientId}-received`}
            x1="0"
            y1="0"
            x2="0"
            y2="1"
          >
            <stop offset="0%" stopColor="var(--success)" stopOpacity="0" />
            <stop offset="100%" stopColor="var(--success)" stopOpacity="0.24" />
          </linearGradient>
        </defs>
        <line
          x1="0"
          x2={chart.width}
          y1={chart.height * 0.25}
          y2={chart.height * 0.25}
          className="runtime-traffic__grid"
        />
        <line
          x1="0"
          x2={chart.width}
          y1={chart.height * 0.75}
          y2={chart.height * 0.75}
          className="runtime-traffic__grid"
        />
        {chart.timeMarkers.slice(1, -1).map((marker) => (
          <line
            key={marker.atMs}
            x1={chart.width * marker.offset}
            x2={chart.width * marker.offset}
            y1={0}
            y2={chart.height}
            className="runtime-traffic__grid runtime-traffic__grid--tick"
          />
        ))}
        {chart.sentLine || chart.receivedLine ? (
          <>
            {chart.sentLine ? (
              <polygon
                points={`0,${chart.mid} ${chart.sentLine} ${chart.width},${chart.mid}`}
                fill={`url(#${gradientId}-sent)`}
              />
            ) : null}
            {chart.receivedLine ? (
              <polygon
                points={`0,${chart.mid} ${chart.receivedLine} ${chart.width},${chart.mid}`}
                fill={`url(#${gradientId}-received)`}
              />
            ) : null}
          </>
        ) : null}
        {/* Zero baseline — always teal, drawn above fills so it never looks grey. */}
        <line
          x1="0"
          x2={chart.width}
          y1={chart.mid}
          y2={chart.mid}
          className="runtime-traffic__baseline"
        />
        {chart.sentLine ? (
          <polyline
            points={chart.sentLine}
            className="runtime-traffic__line runtime-traffic__line--sent"
          />
        ) : null}
        {chart.receivedLine ? (
          <polyline
            points={chart.receivedLine}
            className="runtime-traffic__line runtime-traffic__line--received"
          />
        ) : null}
      </svg>
      <div className="runtime-traffic__axis">
        {chart.timeMarkers.map((marker) => (
          <span
            key={marker.atMs}
            className="runtime-traffic__axis-tick"
            style={{ left: `${marker.offset * 100}%` }}
          >
            {marker.label}
          </span>
        ))}
      </div>
    </div>
  );
}
