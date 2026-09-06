/** One in-flight read, with stale selections discarded and the newest request retained. */
export class NativePreview<T> {
  private key: unknown;
  private revision: unknown;
  private generation = 0;
  private pending = false;
  private latest: {
    key: unknown;
    read: () => Promise<T>;
    paint: (value: T) => void;
    fail: (error: unknown) => void;
  } | null = null;

  clear(): void {
    this.key = undefined;
    this.revision = undefined;
    this.latest = null;
    this.generation++;
  }

  update(
    key: unknown,
    read: () => Promise<T>,
    paint: (value: T) => void,
    fail: (error: unknown) => void,
    revision: unknown = key,
  ): void {
    if (this.key === key && this.revision === revision) return;
    if (this.key !== key) this.generation++;
    this.key = key;
    this.revision = revision;
    this.latest = { key, read, paint, fail };
    this.run();
  }

  private run(): void {
    if (this.pending || !this.latest) return;
    const next = this.latest;
    const generation = this.generation;
    this.latest = null;
    this.pending = true;
    void next
      .read()
      .then(
        (value) => {
          if (this.key === next.key && this.generation === generation)
            next.paint(value);
        },
        (error) => {
          if (this.key === next.key && this.generation === generation)
            next.fail(error);
        },
      )
      .finally(() => {
        this.pending = false;
        this.run();
      });
  }
}
