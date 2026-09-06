import { describe, expect, it } from "vitest";
import { NativePreview } from "../src/app/nativePreview";

describe("native inspector previews", () => {
  it("discards a stale selection and reads only the newest queued target", async () => {
    const preview = new NativePreview<number>();
    let resolve!: (n: number) => void;
    const painted: number[] = [];
    const read: number[] = [];
    const paint = (n: number) => {
      painted.push(n);
    };
    const fail = () => {
      throw new Error("unexpected failure");
    };
    preview.update(
      1,
      () =>
        new Promise((r) => {
          resolve = r;
        }),
      paint,
      fail,
    );
    preview.update(
      2,
      async () => {
        read.push(2);
        return 2;
      },
      paint,
      fail,
    );
    preview.update(
      3,
      async () => {
        read.push(3);
        return 3;
      },
      paint,
      fail,
    );
    resolve(1);
    await new Promise((r) => setTimeout(r, 0));
    expect(read).toEqual([3]);
    expect(painted).toEqual([3]);
  });

  it("clearing a selection prevents an in-flight response from resurrecting its controls", async () => {
    const preview = new NativePreview<number>();
    let resolve!: (n: number) => void;
    const painted: number[] = [];
    preview.update(
      1,
      () =>
        new Promise((r) => {
          resolve = r;
        }),
      (n) => {
        painted.push(n);
      },
      () => {},
    );
    preview.clear();
    resolve(1);
    await new Promise((r) => setTimeout(r, 0));
    expect(painted).toEqual([]);
  });
});
