import type { Technologies } from "./types";

/** Authoring validation mirrors native bounds; native alone awards points and purchases ranks. */
export function validateSkills(data: Partial<Technologies>): void {
  if (
    !Array.isArray(data.skills) ||
    !Array.isArray(data.skill_milestones) ||
    data.skills.length > 64 ||
    data.skill_milestones.length > 64
  )
    throw new TypeError("Skills require bounded skill and milestone arrays");
  const ids = new Set<number>();
  const keys = new Set<string>();
  const legacy = new Set<number>();
  const positive = (n: number, max: number) =>
    Number.isInteger(n) && n > 0 && n <= max;
  let carry = 0;
  let reach = 0;
  let survey = 0;
  let swimming = 0;
  let pace = 0;
  for (const skill of data.skills) {
    if (
      !skill ||
      !positive(skill.id, 65535) ||
      ids.has(skill.id) ||
      !skill.key?.trim() ||
      keys.has(skill.key) ||
      !skill.name?.trim() ||
      !skill.description?.trim() ||
      !["carrying", "construction", "surveying", "mobility"].includes(
        skill.branch,
      ) ||
      !positive(skill.cost, 100) ||
      !Array.isArray(skill.prerequisites) ||
      new Set(skill.prerequisites).size !== skill.prerequisites.length ||
      !skill.effect ||
      ![
        "carry_slots",
        "build_range",
        "survey_range",
        "swimming",
        "move_speed",
      ].includes(skill.effect.kind) ||
      !positive(skill.effect.amount, 32) ||
      (skill.effect.kind === "swimming" && skill.effect.amount !== 1) ||
      (skill.legacy_technology_id !== undefined &&
        (!positive(skill.legacy_technology_id, 65535) ||
          legacy.has(skill.legacy_technology_id) ||
          data.technologies?.some((t) => t.id === skill.legacy_technology_id)))
    )
      throw new TypeError("Invalid skill definition");
    ids.add(skill.id);
    keys.add(skill.key);
    if (skill.legacy_technology_id !== undefined)
      legacy.add(skill.legacy_technology_id);
    if (skill.effect.kind === "carry_slots") carry += skill.effect.amount;
    else if (skill.effect.kind === "build_range") reach += skill.effect.amount;
    else if (skill.effect.kind === "survey_range")
      survey += skill.effect.amount;
    else if (skill.effect.kind === "move_speed") pace += skill.effect.amount;
    else swimming += skill.effect.amount;
  }
  // The survey bound is the tight one: a ring is `3n(n+1)+1` chunks, so a ladder of them costs the
  // generator far more world than the other effects ever ask for. Pace is bounded by the arrival
  // radius: a step that outgrew it would let a walk skip its own waypoint. Native holds all four.
  if (carry > 120 || reach > 32 || survey > 2 || swimming > 1 || pace > 50)
    throw new TypeError("Skill effects exceed player bounds");
  for (const skill of data.skills)
    if (skill.prerequisites.some((id) => !ids.has(id)))
      throw new TypeError("Missing skill prerequisite");
  const reached = new Set<number>();
  while (reached.size < ids.size) {
    const before = reached.size;
    for (const skill of data.skills)
      if (skill.prerequisites.every((id) => reached.has(id)))
        reached.add(skill.id);
    if (before === reached.size)
      throw new TypeError("Skill prerequisite cycle");
  }
  ids.clear();
  keys.clear();
  const events = new Set<string>();
  for (const milestone of data.skill_milestones) {
    const eventKey =
      milestone.event?.kind === "contract_stage"
        ? `contract_stage:${milestone.event.key}`
        : milestone.event?.kind;
    if (
      !milestone ||
      !positive(milestone.id, 65535) ||
      ids.has(milestone.id) ||
      !milestone.key?.trim() ||
      keys.has(milestone.key) ||
      !milestone.name?.trim() ||
      !milestone.description?.trim() ||
      !positive(milestone.points, 100) ||
      !milestone.event ||
      !["workshop_craft", "powered_craft", "contract_stage"].includes(
        milestone.event.kind,
      ) ||
      events.has(eventKey) ||
      (milestone.event.kind === "contract_stage" && !milestone.event.key.trim())
    )
      throw new TypeError("Invalid skill milestone");
    ids.add(milestone.id);
    keys.add(milestone.key);
    events.add(eventKey);
  }
  const budget = data.skill_milestones.reduce((sum, m) => sum + m.points, 0);
  if (budget < data.skills.reduce((sum, s) => sum + s.cost, 0))
    throw new TypeError("Skill milestones cannot fund all ranks");
}
