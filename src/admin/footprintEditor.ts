import type { AxialCoordinate } from "@hexlife/embed/hex";

export interface FootprintEditorOptions {
  container?: HTMLElement;
  initialFootprint: AxialCoordinate[];
  gridRadius?: number;
  readOnly?: boolean;
  onChange?: (footprint: AxialCoordinate[]) => void;
}

export class FootprintEditor {
  private container?: HTMLElement;
  private footprint: Set<string>;
  private gridRadius: number;
  private readOnly: boolean;
  private onChange?: (footprint: AxialCoordinate[]) => void;
  private svg?: SVGSVGElement;

  constructor(options: FootprintEditorOptions) {
    this.container = options.container;
    this.gridRadius = options.gridRadius ?? 2;
    this.readOnly = options.readOnly ?? false;
    this.onChange = options.onChange;
    this.footprint = new Set(
      options.initialFootprint.map((c) => `${c.q},${c.r}`),
    );
    // Invariant: (0,0) is always required
    this.footprint.add("0,0");

    if (typeof document !== "undefined" && this.container) {
      this.render();
    }
  }

  public setFootprint(footprint: AxialCoordinate[]): void {
    this.footprint = new Set(footprint.map((c) => `${c.q},${c.r}`));
    this.footprint.add("0,0");
    this.updateGrid();
    if (this.onChange) this.onChange(this.getFootprint());
  }

  public toggle(q: number, r: number): void {
    this.toggleCoord(q, r);
  }

  public getFootprint(): AxialCoordinate[] {
    const list: AxialCoordinate[] = [];
    for (const key of this.footprint) {
      const [q, r] = key.split(",").map(Number);
      if (q !== undefined && r !== undefined) {
        list.push({ q, r });
      }
    }
    // Sort deterministically: (0,0) first, then by q, then r
    return list.sort((a, b) => {
      if (a.q === 0 && a.r === 0) return -1;
      if (b.q === 0 && b.r === 0) return 1;
      return a.q !== b.q ? a.q - b.q : a.r - b.r;
    });
  }

  private render(): void {
    if (!this.container) return;
    this.container.innerHTML = "";

    const wrapper = document.createElement("div");
    wrapper.className = "footprint-editor-wrap";

    // Presets bar
    if (!this.readOnly) {
      const presets = document.createElement("div");
      presets.className = "footprint-presets";

      const btn1 = document.createElement("button");
      btn1.type = "button";
      btn1.className = "preset-btn";
      btn1.textContent = "1 Cell (0,0)";
      btn1.onclick = () => this.setFootprint([{ q: 0, r: 0 }]);

      const btn2E = document.createElement("button");
      btn2E.type = "button";
      btn2E.className = "preset-btn";
      btn2E.textContent = "2 Cells East (+1, 0)";
      btn2E.onclick = () =>
        this.setFootprint([
          { q: 0, r: 0 },
          { q: 1, r: 0 },
        ]);

      const btn2SE = document.createElement("button");
      btn2SE.type = "button";
      btn2SE.className = "preset-btn";
      btn2SE.textContent = "2 Cells SE (0, +1)";
      btn2SE.onclick = () =>
        this.setFootprint([
          { q: 0, r: 0 },
          { q: 0, r: 1 },
        ]);

      const btnTriad = document.createElement("button");
      btnTriad.type = "button";
      btnTriad.className = "preset-btn";
      btnTriad.textContent = "Triad (3 Cells)";
      btnTriad.onclick = () =>
        this.setFootprint([
          { q: 0, r: 0 },
          { q: 1, r: 0 },
          { q: 0, r: 1 },
        ]);

      presets.appendChild(btn1);
      presets.appendChild(btn2E);
      presets.appendChild(btn2SE);
      presets.appendChild(btnTriad);
      wrapper.appendChild(presets);
    }

    // SVG Hex Grid
    const hexRadius = 24;
    const width = 280;
    const height = 240;
    const centerX = width / 2;
    const centerY = height / 2;

    this.svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    this.svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
    this.svg.setAttribute("class", "footprint-svg");

    const allCoords: AxialCoordinate[] = [];
    const rad = this.gridRadius;
    for (let q = -rad; q <= rad; q++) {
      const r1 = Math.max(-rad, -q - rad);
      const r2 = Math.min(rad, -q + rad);
      for (let r = r1; r <= r2; r++) {
        allCoords.push({ q, r });
      }
    }

    for (const { q, r } of allCoords) {
      // Pointy-top hex cartesian coordinates
      const cx = centerX + hexRadius * Math.sqrt(3) * (q + r / 2);
      const cy = centerY + hexRadius * 1.5 * r;

      const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
      group.setAttribute("class", "hex-cell-group");
      group.dataset.coord = `${q},${r}`;

      const polygon = document.createElementNS(
        "http://www.w3.org/2000/svg",
        "polygon",
      );
      const points = this.calculateHexPoints(cx, cy, hexRadius - 1.5);
      polygon.setAttribute("points", points);
      polygon.setAttribute("class", "hex-cell-poly");

      const text = document.createElementNS(
        "http://www.w3.org/2000/svg",
        "text",
      );
      text.setAttribute("x", String(cx));
      text.setAttribute("y", String(cy + 4));
      text.setAttribute("text-anchor", "middle");
      text.setAttribute("class", "hex-cell-text");
      text.textContent = q === 0 && r === 0 ? "⚓ 0,0" : `${q},${r}`;

      group.appendChild(polygon);
      group.appendChild(text);

      if (!this.readOnly) {
        group.style.cursor = "pointer";
        group.addEventListener("click", () => this.toggleCoord(q, r));
      }

      this.svg.appendChild(group);
    }

    wrapper.appendChild(this.svg);

    const helper = document.createElement("div");
    helper.className = "footprint-helper-text";
    helper.textContent = this.readOnly
      ? `Footprint: ${this.footprint.size} cell(s)`
      : `Click hexes to toggle footprint. Origin (0,0) is permanent anchor. Total: ${this.footprint.size} cell(s)`;
    wrapper.appendChild(helper);

    this.container.appendChild(wrapper);
    this.updateGrid();
  }

  private toggleCoord(q: number, r: number): void {
    if (q === 0 && r === 0) return; // Cannot remove anchor
    const key = `${q},${r}`;
    if (this.footprint.has(key)) {
      this.footprint.delete(key);
    } else {
      this.footprint.add(key);
    }
    this.updateGrid();
    if (this.onChange) this.onChange(this.getFootprint());
  }

  private updateGrid(): void {
    if (!this.svg || !this.container) return;
    const groups = this.svg.querySelectorAll<SVGGElement>(".hex-cell-group");
    groups.forEach((group) => {
      const coord = group.dataset.coord;
      const isSelected = coord ? this.footprint.has(coord) : false;
      const isAnchor = coord === "0,0";

      if (isSelected) {
        group.classList.add("hex-selected");
        if (isAnchor) group.classList.add("hex-anchor");
      } else {
        group.classList.remove("hex-selected", "hex-anchor");
      }
    });

    const helper = this.container.querySelector(".footprint-helper-text");
    if (helper) {
      helper.textContent = this.readOnly
        ? `Footprint: ${this.footprint.size} cell(s)`
        : `Click hexes to toggle footprint. Origin (0,0) is permanent anchor. Total: ${this.footprint.size} cell(s)`;
    }
  }

  private calculateHexPoints(cx: number, cy: number, radius: number): string {
    const points: string[] = [];
    for (let i = 0; i < 6; i++) {
      const angleDeg = 60 * i - 30;
      const angleRad = (Math.PI / 180) * angleDeg;
      const x = cx + radius * Math.cos(angleRad);
      const y = cy + radius * Math.sin(angleRad);
      points.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    }
    return points.join(" ");
  }
}
