import type {
  Definitions,
  FactorySnapshot,
  Technologies,
  TechnologyDefinition,
} from "../core/types";
import { technologyAvailability } from "../core/availability";
import {
  researchBranchColor,
  researchIconSvg,
} from "../rendering/researchIcons";
import { part, syncChildren } from "./dom";
import { technologyContext } from "./research";
import {
  layoutResearch,
  researchAncestors,
  researchBenefits,
  researchMatches,
  researchNeighbor,
  RESEARCH_NODE_WIDTH,
  RESEARCH_NODE_HEIGHT,
} from "./researchGraph";
import "./researchTree.css";

/** A stable, presentation-only map. Purchase eligibility always comes from native snapshots. */
export class ResearchTree {
  private readonly layout;
  private readonly technologies;
  private readonly nodes = new Map<number, HTMLButtonElement>();
  private readonly viewport: HTMLElement;
  private readonly surface: HTMLElement;
  private readonly canvas: HTMLElement;
  private readonly details: HTMLElement;
  private readonly tooltip: HTMLElement;
  private readonly search: HTMLInputElement;
  private readonly branch: HTMLSelectElement;
  private readonly purchase: HTMLButtonElement;
  private snapshot: FactorySnapshot | null = null;
  private selected = 0;
  private hovered: number | null = null;
  private reachableOnly = false;
  private list = false;
  private zoom = 1;
  private opened = false;
  private pending: number | null = null;
  private matches = new Set<number>();
  private drag: {
    pointer: number;
    x: number;
    y: number;
    left: number;
    top: number;
  } | null = null;

  constructor(
    private readonly dialog: HTMLDialogElement,
    private readonly catalog: Technologies,
    private readonly definitions: Definitions,
    private readonly onResearch: (id: number) => boolean,
  ) {
    this.layout = layoutResearch(catalog);
    this.technologies = new Map(
      catalog.technologies.map((tech) => [tech.id, tech]),
    );
    this.viewport = part(dialog, ".research-viewport");
    this.surface = part(dialog, ".research-surface");
    this.canvas = part(dialog, ".research-canvas");
    this.details = part(dialog, ".research-details");
    this.tooltip = part(dialog, ".research-tooltip");
    this.search = part(dialog, "#research-search");
    this.branch = part(dialog, "#research-branch");
    this.purchase = part(dialog, "#research-purchase");
    const list = part(dialog, "#technology-list");
    syncChildren(
      list,
      this.layout.nodes.map((node) => String(node.id)),
      (key) => {
        const tech = this.technologies.get(Number(key))!;
        const node = this.layout.nodes.find((value) => value.id === tech.id)!;
        const button = document.createElement("button");
        button.type = "button";
        button.className = "research-node";
        button.dataset.technologyId = key;
        button.style.left = `${node.x}px`;
        button.style.top = `${node.y}px`;
        button.style.setProperty(
          "--discipline",
          researchBranchColor(tech.branch),
        );
        button.innerHTML = `<span class="research-icon">${researchIconSvg(tech.key)}</span><strong></strong><small class="research-node-cost"></small><span class="research-node-status"></span><span class="research-state-mark" aria-hidden="true"></span>`;
        part(button, "strong").textContent = tech.name;
        part(button, ".research-node-cost").textContent =
          `${tech.cost} insight`;
        button.addEventListener("click", () => {
          this.select(tech.id);
          this.dialog.classList.add("research-inspecting");
          this.hideTooltip();
          this.center(tech.id);
        });
        button.addEventListener("focus", () => {
          this.select(tech.id);
          this.showTooltip(tech.id);
        });
        button.addEventListener("blur", () => this.hideTooltip());
        button.addEventListener("pointerenter", (event) => {
          if (event.pointerType !== "touch" && !this.drag)
            this.showTooltip(tech.id);
        });
        button.addEventListener("pointerleave", () => this.hideTooltip());
        button.addEventListener("keydown", (event) => {
          if (!event.key.startsWith("Arrow")) return;
          const next = researchNeighbor(
            this.layout.nodes.filter((value) => this.matches.has(value.id)),
            tech.id,
            event.key,
          );
          if (next !== undefined) this.select(next, true);
          event.preventDefault();
        });
        this.nodes.set(tech.id, button);
        return button;
      },
    );
    const svg = dialog.querySelector<SVGSVGElement>(".research-edges")!;
    svg.setAttribute(
      "viewBox",
      `0 0 ${this.layout.width} ${this.layout.height}`,
    );
    svg.innerHTML =
      '<defs><marker id="research-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 8 4 L 0 8" fill="context-stroke"/></marker></defs>';
    for (const edge of this.layout.edges) {
      const path = document.createElementNS(
        "http://www.w3.org/2000/svg",
        "path",
      );
      path.setAttribute("d", edge.path);
      path.setAttribute("marker-end", "url(#research-arrow)");
      path.dataset.from = String(edge.from);
      path.dataset.to = String(edge.to);
      svg.append(path);
    }
    for (const group of [...catalog.branches].sort(
      (a, b) => a.order - b.order,
    )) {
      const option = document.createElement("option");
      option.value = group.key;
      option.textContent = group.name;
      this.branch.append(option);
    }
    this.search.addEventListener("input", () => this.filter(true));
    this.branch.addEventListener("change", () => this.filter(true));
    part(dialog, "#research-scope").addEventListener("click", () => {
      this.reachableOnly = !this.reachableOnly;
      this.filter(true);
    });
    part(dialog, "#research-clear").addEventListener("click", () => {
      this.search.value = "";
      this.branch.value = "";
      this.reachableOnly = false;
      this.filter(true);
      this.search.focus();
    });
    part(dialog, "#research-view").addEventListener("click", () =>
      this.setList(!this.list),
    );
    part(dialog, "#research-zoom-in").addEventListener("click", () =>
      this.setZoom(this.zoom + 0.15),
    );
    part(dialog, "#research-zoom-out").addEventListener("click", () =>
      this.setZoom(this.zoom - 0.15),
    );
    part(dialog, "#research-fit").addEventListener("click", () => {
      this.setZoom(
        Math.min(
          this.viewport.clientWidth / this.layout.width,
          this.viewport.clientHeight / this.layout.height,
        ),
      );
      this.viewport.scrollTo(0, 0);
    });
    part(dialog, "#research-reset").addEventListener("click", () => {
      this.setZoom(1);
      this.center(this.selected);
    });
    this.purchase.addEventListener("click", () => {
      const tech = this.technologies.get(this.selected);
      if (!tech || !this.snapshot || this.pending !== null) return;
      const state = technologyAvailability(tech, this.snapshot);
      if (
        !state.known ||
        state.complete ||
        !state.prerequisitesMet ||
        !state.affordable
      )
        return;
      if (this.onResearch(tech.id)) {
        this.pending = tech.id;
        this.renderDetails();
      } else
        part(dialog, "#research-announcement").textContent =
          "Command queue busy. Please try again.";
    });
    this.details.addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>(
        "[data-jump]",
      );
      if (button) {
        // Follow a dependency even if a search currently excludes it.
        this.search.value = "";
        this.branch.value = "";
        this.reachableOnly = false;
        this.filter();
        this.select(Number(button.dataset.jump), true);
      }
    });
    this.viewport.addEventListener(
      "scroll",
      () => {
        const active = document.activeElement;
        // Arrow navigation pans to the next icon. Keep its keyboard preview attached after that
        // scroll; pointer scrolling still dismisses hover previews so they cannot cover the map.
        if (
          active instanceof HTMLButtonElement &&
          active.matches(".research-node:focus-visible") &&
          !this.dialog.classList.contains("research-inspecting")
        )
          this.showTooltip(Number(active.dataset.technologyId));
        else this.hideTooltip();
      },
      { passive: true },
    );
    this.viewport.addEventListener("pointerdown", (event) => {
      if (
        this.list ||
        event.pointerType === "touch" ||
        event.button !== 0 ||
        (event.target as Element).closest("button")
      )
        return;
      this.drag = {
        pointer: event.pointerId,
        x: event.clientX,
        y: event.clientY,
        left: this.viewport.scrollLeft,
        top: this.viewport.scrollTop,
      };
      this.viewport.setPointerCapture(event.pointerId);
      this.viewport.classList.add("panning");
      this.hideTooltip();
    });
    this.viewport.addEventListener("pointermove", (event) => {
      if (!this.drag || this.drag.pointer !== event.pointerId) return;
      this.viewport.scrollLeft = this.drag.left + this.drag.x - event.clientX;
      this.viewport.scrollTop = this.drag.top + this.drag.y - event.clientY;
    });
    const stop = (): void => {
      this.drag = null;
      this.viewport.classList.remove("panning");
    };
    this.viewport.addEventListener("pointerup", stop);
    this.viewport.addEventListener("pointercancel", stop);
    this.viewport.addEventListener("lostpointercapture", stop);
    this.dialog.addEventListener("close", () => this.hideTooltip());
    part(dialog, "#research-details-close").addEventListener("click", () => {
      this.dialog.classList.remove("research-inspecting");
      this.search.focus({ preventScroll: true });
    });
    this.setZoom(1);
  }

  onOpen(): void {
    const firstOpen = !this.opened;
    this.opened = true;
    requestAnimationFrame(() => {
      if (firstOpen) {
        this.setZoom(
          window.innerWidth <= 720
            ? 0.9
            : Math.min(
                1.15,
                this.viewport.clientWidth / this.layout.width,
                this.viewport.clientHeight / this.layout.height,
              ),
        );
        if (window.innerWidth <= 720) this.center(this.selected);
        else this.viewport.scrollTo(0, 0);
      }
      this.search.focus({ preventScroll: true });
    });
  }

  update(snapshot: FactorySnapshot): void {
    const previous = this.snapshot;
    this.snapshot = snapshot;
    if (previous && previous.researched !== snapshot.researched) {
      const completed = snapshot.researched.filter(
        (id) => !previous.researched.includes(id),
      );
      if (completed.length)
        part(this.dialog, "#research-announcement").textContent =
          `${completed.map((id) => this.technologies.get(id)?.name ?? id).join(", ")} researched.`;
    }
    if (this.pending !== null && previous !== snapshot) {
      if (snapshot.researched.includes(this.pending))
        part(this.dialog, "#research-announcement").textContent =
          `${this.technologies.get(this.pending)!.name} researched.`;
      this.pending = null;
    } else if (
      previous?.research_availability === snapshot.research_availability &&
      previous?.insight === snapshot.insight
    )
      return;
    if (!this.selected)
      this.selected =
        this.catalog.technologies.find((tech) => {
          const state = technologyAvailability(tech, snapshot);
          return !state.complete && state.prerequisitesMet;
        })?.id ??
        this.catalog.technologies[0]?.id ??
        0;
    part(this.dialog, "#research-insight").textContent = String(
      snapshot.insight,
    );
    part(this.dialog, "#research-progress").textContent =
      `${snapshot.researched.length} / ${this.nodes.size} researched`;
    for (const [id, button] of this.nodes) {
      const tech = this.technologies.get(id)!;
      const state = technologyAvailability(tech, snapshot);
      const status = this.status(tech);
      button.dataset.state = !state.known
        ? "unknown"
        : state.complete
          ? "complete"
          : !state.prerequisitesMet
            ? "locked"
            : state.affordable
              ? "ready"
              : "shortfall";
      part(button, ".research-node-status").textContent = status;
      part(button, ".research-state-mark").textContent = state.complete
        ? "✓"
        : state.prerequisitesMet
          ? "+"
          : "";
      button.setAttribute(
        "aria-label",
        `${tech.name}. ${tech.cost} insight. ${status}. Select for details.`,
      );
    }
    this.filter();
    this.renderDetails();
    if (this.hovered !== null) this.showTooltip(this.hovered);
  }

  private status(tech: TechnologyDefinition): string {
    if (!this.snapshot) return "Status unavailable";
    const state = technologyAvailability(tech, this.snapshot);
    return !state.known
      ? "Status unavailable"
      : state.complete
        ? "✓ Researched"
        : !state.prerequisitesMet
          ? "Locked"
          : state.affordable
            ? "Ready to research"
            : `Need ${state.insightShortfall} more insight`;
  }

  private filter(reveal = false): void {
    this.matches = new Set(
      this.catalog.technologies
        .filter((tech) => {
          const state = this.snapshot
            ? technologyAvailability(tech, this.snapshot)
            : null;
          return (
            (!this.branch.value || this.branch.value === tech.branch) &&
            researchMatches(
              tech,
              this.search.value,
              this.catalog,
              this.definitions,
            ) &&
            (!this.reachableOnly ||
              (state?.prerequisitesMet && !state.complete))
          );
        })
        .map((tech) => tech.id),
    );
    for (const [id, button] of this.nodes) {
      button.classList.toggle("filtered", !this.matches.has(id));
      button.tabIndex = this.matches.has(id) ? 0 : -1;
    }
    part(this.dialog, "#research-results").textContent =
      this.matches.size === this.nodes.size
        ? "Explore every breakthrough"
        : `${this.matches.size} of ${this.nodes.size} match`;
    part(this.dialog, "#research-empty").hidden = this.matches.size !== 0;
    part(this.dialog, "#research-clear").hidden =
      !this.search.value && !this.branch.value && !this.reachableOnly;
    part(this.dialog, "#research-scope").setAttribute(
      "aria-pressed",
      String(this.reachableOnly),
    );
    this.highlight();
    if (reveal && this.matches.size) {
      const first = this.matches.has(this.selected)
        ? this.selected
        : this.matches.values().next().value!;
      this.select(first);
      this.center(first);
    }
  }

  private select(id: number, focus = false): void {
    if (!this.technologies.has(id)) return;
    if (this.selected !== id) {
      part(this.details, ".research-detail-content").scrollTop = 0;
      this.details.scrollTop = 0;
    }
    this.selected = id;
    this.highlight();
    this.renderDetails();
    if (focus) {
      this.center(id);
      this.nodes.get(id)?.focus({ preventScroll: true });
    }
  }

  private highlight(): void {
    const ancestors = researchAncestors(
      this.hovered ?? this.selected,
      this.catalog,
    );
    for (const [id, button] of this.nodes) {
      button.setAttribute("aria-pressed", String(id === this.selected));
      button.classList.toggle("on-path", ancestors.has(id));
    }
    for (const edge of this.dialog.querySelectorAll<SVGPathElement>(
      ".research-edges > path",
    )) {
      const from = Number(edge.dataset.from),
        to = Number(edge.dataset.to);
      edge.classList.toggle(
        "on-path",
        ancestors.has(from) && ancestors.has(to),
      );
      edge.classList.toggle(
        "complete",
        this.snapshot?.researched.includes(from) === true,
      );
      edge.classList.toggle(
        "filtered",
        !this.matches.has(from) && !this.matches.has(to),
      );
    }
  }

  private renderDetails(): void {
    const tech = this.technologies.get(this.selected);
    if (!tech || !this.snapshot) return;
    const state = technologyAvailability(tech, this.snapshot);
    const icon = part(this.details, ".research-detail-icon");
    if (icon.dataset.key !== tech.key) {
      icon.innerHTML = researchIconSvg(tech.key);
      icon.dataset.key = tech.key;
    }
    icon.style.setProperty("--discipline", researchBranchColor(tech.branch));
    part(this.details, "h3").textContent = tech.name;
    part(this.details, ".research-context").textContent = technologyContext(
      tech,
      this.catalog,
    );
    part(this.details, ".research-description").textContent = tech.description;
    const benefits = researchBenefits(tech, this.definitions);
    syncChildren(part(this.details, ".research-benefits"), benefits, (text) => {
      const item = document.createElement("li");
      item.textContent = text;
      return item;
    });
    const links = (container: HTMLElement, ids: number[]): void => {
      const buttons = syncChildren(container, ids.map(String), (key) => {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.jump = key;
        return button;
      });
      buttons.forEach((button, index) => {
        const id = ids[index]!;
        button.textContent = `${this.snapshot!.researched.includes(id) ? "✓ " : ""}${this.technologies.get(id)!.name} →`;
      });
    };
    links(part(this.details, ".research-prerequisites"), tech.prerequisites);
    part(this.details, ".research-no-prerequisites").hidden =
      tech.prerequisites.length > 0;
    const next = this.catalog.technologies
      .filter((value) => value.prerequisites.includes(tech.id))
      .map((value) => value.id);
    links(part(this.details, ".research-next"), next);
    part(this.details, ".research-next-section").hidden = !next.length;
    part(this.details, ".research-cost").textContent = `${tech.cost} insight`;
    part(this.details, ".research-wallet").textContent =
      `You have ${this.snapshot.insight}${state.affordable && !state.complete ? ` · ${this.snapshot.insight - tech.cost} after` : ""}`;
    const missing = state.missingPrerequisites.map(
      (id) => this.technologies.get(id)?.name ?? `#${id}`,
    );
    part(this.details, ".research-reason").textContent = state.complete
      ? "This capability is available in your factory."
      : !state.known
        ? "Waiting for native research status."
        : missing.length
          ? `Requires ${missing.join(" and ")}.${state.insightShortfall ? ` Also need ${state.insightShortfall} more insight.` : ""}`
          : !state.affordable
            ? `Earn ${state.insightShortfall} more insight by completing hub jobs.`
            : `Unlock this capability for ${tech.cost} insight.`;
    this.purchase.disabled =
      this.pending !== null ||
      !state.known ||
      state.complete ||
      !state.prerequisitesMet ||
      !state.affordable;
    this.purchase.textContent =
      this.pending === tech.id
        ? "Researching…"
        : state.complete
          ? "✓ Researched"
          : `Research · ${tech.cost} insight`;
  }

  private showTooltip(id: number): void {
    const tech = this.technologies.get(id)!;
    this.hovered = id;
    this.highlight();
    this.nodes.get(id)?.setAttribute("aria-describedby", "research-tooltip");
    part(this.tooltip, "strong").textContent = tech.name;
    part(this.tooltip, ".research-tooltip-description").textContent =
      tech.description;
    part(this.tooltip, ".research-tooltip-benefits").textContent =
      researchBenefits(tech, this.definitions).join(" · ");
    part(this.tooltip, ".research-tooltip-cost").textContent =
      `${tech.cost} insight · ${this.status(tech)}`;
    const prerequisites = tech.prerequisites.map(
      (parent) => this.technologies.get(parent)!.name,
    );
    part(this.tooltip, ".research-tooltip-needs").textContent =
      prerequisites.length
        ? `Requires ${prerequisites.join(" + ")}`
        : "No prerequisites";
    this.tooltip.hidden = false;
    const bounds = this.nodes.get(id)!.getBoundingClientRect();
    const left = Math.max(
      12,
      Math.min(bounds.left, window.innerWidth - this.tooltip.offsetWidth - 12),
    );
    const below = bounds.bottom + 10;
    const top =
      below + this.tooltip.offsetHeight < window.innerHeight - 12
        ? below
        : Math.max(12, bounds.top - this.tooltip.offsetHeight - 10);
    this.tooltip.style.left = `${left}px`;
    this.tooltip.style.top = `${top}px`;
  }

  private hideTooltip(): void {
    if (this.hovered !== null)
      this.nodes.get(this.hovered)?.removeAttribute("aria-describedby");
    this.hovered = null;
    this.tooltip.hidden = true;
    this.highlight();
  }

  private setList(list: boolean): void {
    this.list = list;
    this.dialog.classList.toggle("research-list-view", list);
    part(this.dialog, "#research-view").textContent = list
      ? "Tree view"
      : "List view";
    part(this.dialog, "#research-view").setAttribute(
      "aria-pressed",
      String(list),
    );
    part(this.dialog, ".research-zoom").hidden = list;
    this.hideTooltip();
    this.applyScale();
    this.center(this.selected);
  }

  private setZoom(value: number): void {
    const next = Math.max(0.45, Math.min(1.6, value));
    const x =
      (this.viewport.scrollLeft + this.viewport.clientWidth / 2) / this.zoom;
    const y =
      (this.viewport.scrollTop + this.viewport.clientHeight / 2) / this.zoom;
    this.zoom = next;
    this.applyScale();
    this.viewport.scrollTo(
      x * next - this.viewport.clientWidth / 2,
      y * next - this.viewport.clientHeight / 2,
    );
    part(this.dialog, "#research-reset").textContent =
      `${Math.round(next * 100)}%`;
    part<HTMLButtonElement>(this.dialog, "#research-zoom-out").disabled =
      next <= 0.45;
    part<HTMLButtonElement>(this.dialog, "#research-zoom-in").disabled =
      next >= 1.6;
    this.hideTooltip();
  }

  private applyScale(): void {
    this.surface.style.width = this.list
      ? "100%"
      : `${this.layout.width * this.zoom}px`;
    this.surface.style.height = this.list
      ? "auto"
      : `${this.layout.height * this.zoom}px`;
    this.canvas.style.width = this.list ? "100%" : `${this.layout.width}px`;
    this.canvas.style.height = this.list ? "auto" : `${this.layout.height}px`;
    this.canvas.style.transform = this.list ? "none" : `scale(${this.zoom})`;
  }

  private center(id: number): void {
    if (this.list) {
      this.nodes.get(id)?.scrollIntoView({ block: "nearest" });
      return;
    }
    const node = this.layout.nodes.find((value) => value.id === id);
    if (node)
      this.viewport.scrollTo(
        (node.x + RESEARCH_NODE_WIDTH / 2) * this.zoom -
          this.viewport.clientWidth / 2,
        (node.y + RESEARCH_NODE_HEIGHT / 2) * this.zoom -
          this.viewport.clientHeight / 2,
      );
  }
}
