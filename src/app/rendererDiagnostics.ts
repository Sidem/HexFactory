import type { RendererDiagnostics } from "../rendering/FactoryRenderer";

/** Opt-in development UI: browser callback pacing is separate from CPU render duration. */
export function installRendererDiagnostics(
  readRenderer: () => RendererDiagnostics,
): void {
  const intervals: number[] = [];
  let previous: number | undefined;
  const sampleFrame = (now: number): void => {
    if (document.hidden) {
      previous = undefined;
      intervals.length = 0;
    } else {
      if (previous !== undefined) {
        intervals.push(now - previous);
        if (intervals.length > 240) intervals.shift();
      }
      previous = now;
    }
    requestAnimationFrame(sampleFrame);
  };
  document.addEventListener("visibilitychange", () => {
    previous = undefined;
    intervals.length = 0;
  });
  requestAnimationFrame(sampleFrame);

  const capture = document.createElement("button");
  const output = document.createElement("output");
  capture.type = "button";
  capture.textContent = "Capture renderer diagnostics";
  capture.style.cssText =
    "position:fixed;z-index:10000;right:12px;top:72px;padding:8px";
  output.id = "renderer-diagnostics";
  output.style.cssText =
    "position:fixed;z-index:10000;right:12px;top:116px;max-width:480px;padding:8px;background:#071110;color:#dcefe9;overflow-wrap:anywhere";
  capture.addEventListener("pointerdown", (event) => event.stopPropagation());
  capture.addEventListener("click", (event) => {
    event.stopPropagation();
    const sorted = intervals.toSorted((a, b) => a - b);
    const total = intervals.reduce((sum, interval) => sum + interval, 0);
    output.textContent = JSON.stringify({
      ...readRenderer(),
      browserFramePacing: {
        samples: intervals.length,
        windowMs: total,
        meanIntervalMs: intervals.length ? total / intervals.length : null,
        p95IntervalMs: sorted[Math.ceil(sorted.length * 0.95) - 1] ?? null,
        maxIntervalMs: sorted.at(-1) ?? null,
        callbackRateHz: total > 0 ? (intervals.length * 1000) / total : null,
        intervalsOver25Ms: intervals.filter((interval) => interval > 25).length,
      },
    });
  });
  document.body.append(capture, output);
}
