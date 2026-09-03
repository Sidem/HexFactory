import { branchEmblemSvg, paintEmblem } from "../rendering/emblems";
import {
  researchBranchColor,
  researchIconSvg,
} from "../rendering/researchIcons";
import type {
  FactorySnapshot,
  SkillDefinition,
  SkillEffect,
  Technologies,
} from "../core/types";
import { part, syncChildren } from "./dom";
import "./skills.css";

/**
 * What each kind of upgrade is called, in the unit the native answer already counts it in.
 *
 * Keyed by effect rather than by branch so a new upgrade fails to compile until it is written out
 * here, instead of quietly borrowing whichever wording the last one used.
 */
const BRANCHES: Record<
  SkillEffect["kind"],
  {
    label: string;
    /** The gain, both ways: no upgrade is worth reading "+1 cargo slots". */
    gain: [one: string, many: string];
    /** The capacity of yours it widens, and the unit that capacity is measured in. */
    capacity: string;
    unit: [one: string, many: string];
  }
> = {
  carry_slots: {
    label: "01 / Carrying",
    gain: ["cargo slot", "cargo slots"],
    capacity: "pack",
    unit: ["slot", "slots"],
  },
  build_range: {
    label: "02 / Construction reach",
    gain: ["hex of reach", "hexes of reach"],
    capacity: "reach",
    unit: ["hex", "hexes"],
  },
  survey_range: {
    label: "03 / Surveying range",
    gain: ["ring of surveyed ground", "rings of surveyed ground"],
    capacity: "survey",
    unit: ["ring", "rings"],
  },
  swimming: {
    label: "04 / Mobility",
    gain: ["deep-water route", "deep-water routes"],
    capacity: "deep-water access",
    unit: ["route", "routes"],
  },
  move_speed: {
    label: "04 / Mobility",
    gain: ["pace level", "pace levels"],
    capacity: "pace",
    unit: ["level", "levels"],
  },
};

const count = (amount: number, [one, many]: [string, string]) =>
  `${amount} ${amount === 1 ? one : many}`;

type SkillGroup = {
  key: SkillEffect["kind"];
  levels: SkillDefinition[];
};

/** One card per skill, with sequential catalogue entries presented as levels of that skill. */
export function groupSkills(skills: SkillDefinition[]): SkillGroup[] {
  const groups = new Map<SkillEffect["kind"], SkillDefinition[]>();
  for (const skill of skills) {
    const levels = groups.get(skill.effect.kind) ?? [];
    levels.push(skill);
    groups.set(skill.effect.kind, levels);
  }
  return [...groups].map(([key, levels]) => ({ key, levels }));
}

/** Text and affordances from the native purchase answer; never decides affordability. */
export function skillView(skill: SkillDefinition, snapshot: FactorySnapshot) {
  const availability = snapshot.skills.availability.find(
    (row) => row.skill_id === skill.id,
  );
  const complete = availability?.complete ?? false;
  const missing = availability?.missing_prerequisites ?? [];
  const shortfall = availability?.points_shortfall ?? skill.cost;
  return {
    complete,
    canPurchase:
      !!availability && !complete && missing.length === 0 && shortfall === 0,
    status: complete
      ? snapshot.skills.purchased.includes(skill.id)
        ? "Learned"
        : "Already unlocked"
      : missing.length
        ? "Prerequisite needed"
        : shortfall
          ? `Earn ${shortfall} more Skill Point${shortfall === 1 ? "" : "s"}`
          : "Ready to learn",
  };
}

/** Legible upgrade branches and the full, finite journey that pays for them. */
export class SkillsView {
  private snapshot: FactorySnapshot | null = null;
  constructor(
    private readonly root: HTMLDialogElement,
    private readonly technologies: Technologies,
    purchase: (id: number) => void,
  ) {
    part(root, "#skills-show-milestones").addEventListener("click", () => {
      const heading = part(root, "#skills-journey-title");
      heading.scrollIntoView({ block: "start", behavior: "instant" });
      heading.focus({ preventScroll: true });
    });
    root.addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>(
        "[data-skill-purchase]",
      );
      if (!button || !this.snapshot) return;
      const skill = technologies.skills.find(
        (row) => row.id === Number(button.dataset.skillPurchase),
      );
      if (skill && skillView(skill, this.snapshot).canPurchase)
        purchase(skill.id);
    });
  }

  update(snapshot: FactorySnapshot): void {
    if (
      this.snapshot?.skills === snapshot.skills &&
      this.snapshot?.player.carry_slots === snapshot.player.carry_slots &&
      this.snapshot?.player.build_range === snapshot.player.build_range
    )
      return;
    this.snapshot = snapshot;
    const state = snapshot.skills;
    const groups = groupSkills(this.technologies.skills);
    part(this.root, "#skills-points").textContent = String(state.points);
    part(this.root, "#skills-points-unit").textContent =
      state.points === 1 ? "Skill Point" : "Skill Points";
    part(this.root, "#skills-summary").textContent = state.sandbox
      ? "Creative save: skills are unlocked and milestone rewards stay off."
      : `${state.points} Skill Point${state.points === 1 ? "" : "s"} available. ${state.purchased.length + state.granted.length} levels unlocked across ${groups.length} skills.`;
    const rows = syncChildren(
      part(this.root, "#skill-branches"),
      groups.map((group) => group.key),
      (key) => {
        const card = document.createElement("article");
        card.className = "skill-card";
        card.innerHTML = `<div class="skill-card-top"><i class="skill-branch-emblem"></i><span class="skill-branch"></span><span class="skill-state"></span></div>
        <h3><span class="skill-icon"></span><span class="skill-name"></span></h3><div class="skill-benefit"></div><p class="skill-description"></p>
        <p class="skill-capacity"></p><p class="skill-requirements"></p><button type="button" class="skill-purchase"></button>`;
        const button = part<HTMLButtonElement>(card, "button");
        button.setAttribute("aria-describedby", `skill-requirements-${key}`);
        part(card, ".skill-requirements").id = `skill-requirements-${key}`;
        return card;
      },
    );
    rows.forEach((card, index) => {
      const group = groups[index]!;
      const completedLevels = group.levels.filter(
        (skill) =>
          snapshot.skills.availability.find((row) => row.skill_id === skill.id)
            ?.complete,
      ).length;
      const skill = group.levels[completedLevels] ?? group.levels.at(-1)!;
      const view = skillView(skill, snapshot);
      const copy = BRANCHES[skill.effect.kind];
      card.dataset.branch = skill.branch;
      card.classList.toggle("learned", completedLevels === group.levels.length);
      paintEmblem(part<HTMLElement>(card, ".skill-branch-emblem"), {
        key: skill.branch,
        markup: branchEmblemSvg(skill.branch),
        accent: researchBranchColor(skill.branch),
      });
      part(card, ".skill-branch").textContent = copy.label;
      part(card, ".skill-state").textContent = view.status;
      part(card, ".skill-name").textContent = group.levels[0]!.name;
      if (!part(card, ".skill-icon").hasChildNodes())
        part(card, ".skill-icon").innerHTML = researchIconSvg(
          group.levels[0]!.key,
        );
      part(card, ".skill-benefit").textContent =
        group.levels.length > 1
          ? `Level ${completedLevels} / ${group.levels.length}`
          : `+${count(skill.effect.amount, copy.gain)}`;
      part(card, ".skill-description").textContent = skill.description;
      const native = snapshot.skills.availability.find(
        (r) => r.skill_id === skill.id,
      );
      const current = native?.current_value ?? 0;
      part(card, ".skill-capacity").textContent = view.complete
        ? `Your ${copy.capacity}: ${count(current, copy.unit)}`
        : `${current} → ${count(native?.resulting_value ?? current, copy.unit)} after learning`;
      const missing =
        snapshot.skills.availability.find((r) => r.skill_id === skill.id)
          ?.missing_prerequisites ?? [];
      part(card, ".skill-requirements").textContent = view.complete
        ? state.granted.includes(skill.id)
          ? state.sandbox
            ? "Granted in Creative. Your earned points are unchanged."
            : "Preserved from an earlier save. No points charged."
          : "Permanent upgrade. Your choice is saved."
        : missing.length
          ? `First learn: ${missing.map((id) => this.technologies.skills.find((s) => s.id === id)?.name ?? id).join(", ")}`
          : `${view.canPurchase ? "Learn these in any order." : view.status + "."} Journey milestones fund the whole set.`;
      const button = part<HTMLButtonElement>(card, "button");
      button.dataset.skillPurchase = String(skill.id);
      button.textContent = view.complete
        ? view.status
        : `Learn · ${skill.cost} Skill Point${skill.cost === 1 ? "" : "s"}`;
      button.setAttribute(
        "aria-label",
        `${view.complete ? view.status : "Learn"} ${skill.name}${view.complete ? "" : ` for ${skill.cost} Skill Point${skill.cost === 1 ? "" : "s"}`}`,
      );
      // Keep keyboard focus on the control after purchase. Native still validates every command.
      button.setAttribute("aria-disabled", String(!view.canPurchase));
    });
    const milestones = syncChildren(
      part(this.root, "#skill-milestones"),
      this.technologies.skill_milestones.map((m) => String(m.id)),
      () => {
        const row = document.createElement("li");
        row.innerHTML =
          '<span class="milestone-mark" aria-hidden="true"></span><div><h4></h4><p></p></div><span class="milestone-reward"></span>';
        return row;
      },
    );
    milestones.forEach((row, index) => {
      const milestone = this.technologies.skill_milestones[index]!;
      const complete = state.completed.includes(milestone.id);
      row.classList.toggle("complete", complete);
      part(row, ".milestone-mark").textContent = complete ? "✓" : "○";
      part(row, "h4").textContent = milestone.name;
      part(row, "p").textContent = milestone.description;
      part(row, ".milestone-reward").textContent = complete
        ? "Completed"
        : state.sandbox
          ? "Sandbox"
          : `+${milestone.points} Skill Point${milestone.points === 1 ? "" : "s"}`;
    });
  }
}
