import type { Sample } from "../lib/useServerHealth";

const HEIGHT = 24;
const BAR_SHARE = 0.55;
const MIN_SCALE_MS = 100;

export function LatencyBars({
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

  return (
    <svg
      className="block h-6 w-full"
      viewBox={`0 0 100 ${HEIGHT}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={samples.length ? describe(samples[samples.length - 1]) : undefined}
    >
      {Array.from({ length: slots }, (_, slot) => (
        <rect
          key={`track-${slot}`}
          x={slot * width}
          y={HEIGHT - 1}
          width={width}
          height={1}
          className="fill-border"
          shapeRendering="crispEdges"
        />
      ))}
      {samples.map((sample, index) => {
        const failed = sample.ms === null;
        const ratio = failed ? 1 : Math.max(0.08, (sample.ms as number) / scale);
        const barHeight = ratio * (HEIGHT - 2);
        const latest = index === samples.length - 1;
        return (
          <g key={sample.at}>
            <rect
              x={(offset + index) * width}
              y={0}
              width={width}
              height={HEIGHT}
              fill="transparent"
            >
              <title>{describe(sample)}</title>
            </rect>
            <rect
              x={(offset + index + (1 - BAR_SHARE) / 2) * width}
              y={HEIGHT - 1 - barHeight}
              width={width * BAR_SHARE}
              height={barHeight}
              className={failed ? "fill-danger" : latest ? "fill-fg" : "fill-fg-muted/45"}
              shapeRendering="crispEdges"
              pointerEvents="none"
            />
          </g>
        );
      })}
    </svg>
  );
}
