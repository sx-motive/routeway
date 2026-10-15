import type { Sample } from "../lib/useServerHealth";

const HEIGHT = 24;
const BASELINE = HEIGHT - 2;
const MIN_SCALE_MS = 100;

type Point = { x: number; y: number };

function smoothPath(points: Point[]) {
  return points.reduce((path, point, index) => {
    if (index === 0) return `M ${point.x} ${point.y}`;
    const previous = points[index - 1];
    const middle = (previous.x + point.x) / 2;
    // Horizontal tangents keep the curve smooth without overshooting measured values.
    return `${path} C ${middle} ${previous.y}, ${middle} ${point.y}, ${point.x} ${point.y}`;
  }, "");
}

export function LatencyChart({
  samples,
  slots,
  describe,
}: {
  samples: Sample[];
  slots: number;
  describe: (sample: Sample) => string;
}) {
  const scale = Math.max(MIN_SCALE_MS, ...samples.map((s) => s.ms ?? 0));
  const offset = slots - samples.length;
  const width = 100 / slots;
  const segments: Point[][] = [];
  let segment: Point[] = [];

  samples.forEach((sample, index) => {
    if (sample.ms === null) {
      // A failed probe is a gap, never a zero-latency point or an interpolated success.
      if (segment.length) segments.push(segment);
      segment = [];
    } else {
      segment.push({
        x: (offset + index + 0.5) * width,
        y: BASELINE - Math.max(0.08, sample.ms / scale) * (BASELINE - 2),
      });
    }
  });
  if (segment.length) segments.push(segment);

  return (
    <svg
      className="block h-6 w-full"
      viewBox={`0 0 100 ${HEIGHT}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={samples.length ? describe(samples[samples.length - 1]) : undefined}
    >
      <path d={`M 0 ${BASELINE} H 100`} className="stroke-border" strokeWidth={1} vectorEffect="non-scaling-stroke" />
      {segments.map((points, index) => {
        const path = smoothPath(points);
        const first = points[0];
        const last = points[points.length - 1];
        return (
          <g key={index} pointerEvents="none">
            {points.length > 1 ? (
              <>
                <path d={`${path} L ${last.x} ${BASELINE} L ${first.x} ${BASELINE} Z`} className="fill-fg/5" />
                <path d={path} fill="none" className="stroke-fg-muted" strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
              </>
            ) : null}
            {points.length === 1 || (index === segments.length - 1 && samples.at(-1)?.ms !== null) ? (
              <path d={`M ${last.x} ${last.y} h 0`} className="stroke-fg" strokeWidth={3.5} strokeLinecap="round" vectorEffect="non-scaling-stroke" />
            ) : null}
          </g>
        );
      })}
      {samples.map((sample, index) => {
        const x = (offset + index + 0.5) * width;
        return (
          <g key={sample.at}>
            {sample.ms === null ? (
              <path d={`M ${x} ${BASELINE} h 0`} className="stroke-danger" strokeWidth={3.5} strokeLinecap="round" vectorEffect="non-scaling-stroke" pointerEvents="none" />
            ) : null}
            <rect
              x={(offset + index) * width}
              y={0}
              width={width}
              height={HEIGHT}
              fill="transparent"
            >
              <title>{describe(sample)}</title>
            </rect>
          </g>
        );
      })}
    </svg>
  );
}
