//! Personal progression: bounded event rewards, separate currency and atomic purchases.
use super::*;

#[derive(Clone, Deserialize)]
pub(super) struct SkillDefinition {
    pub(super) id: u16,
    pub(super) key: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) branch: String,
    pub(super) prerequisites: Vec<u16>,
    pub(super) cost: u32,
    pub(super) effect: SkillEffect,
    pub(super) legacy_technology_id: Option<TechnologyId>,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum SkillEffect {
    CarrySlots { amount: u32 },
    BuildRange { amount: u32 },
}

#[derive(Clone, Deserialize)]
pub(super) struct SkillMilestone {
    pub(super) id: u16,
    pub(super) key: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) points: u32,
    pub(super) event: SkillEvent,
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum SkillEvent {
    WorkshopCraft,
    PoweredCraft,
    ContractStage { key: String },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct SkillsState {
    pub(super) points: u32,
    pub(super) purchased: BTreeSet<u16>,
    /// Grandfathered and creative capabilities never spend or mint earned points.
    pub(super) granted: BTreeSet<u16>,
    pub(super) completed: BTreeSet<u16>,
    /// Creative provenance persists on returning to priced play.
    pub(super) sandbox: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct SkillAvailability {
    pub(super) skill_id: u16,
    pub(super) complete: bool,
    pub(super) missing_prerequisites: Vec<u16>,
    pub(super) points_shortfall: u32,
    pub(super) current_value: u32,
    pub(super) resulting_value: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct SkillsSnapshot {
    #[serde(flatten)]
    pub(super) state: SkillsState,
    pub(super) availability: Vec<SkillAvailability>,
}

impl SkillsState {
    pub(super) fn owns(&self, id: u16) -> bool {
        self.purchased.contains(&id) || self.granted.contains(&id)
    }

    pub(super) fn bonuses(&self, technologies: &TechnologiesInput) -> (u32, u32) {
        technologies
            .skills
            .iter()
            .filter(|skill| self.owns(skill.id))
            .fold((0, 0), |(carry, reach), skill| match skill.effect {
                SkillEffect::CarrySlots { amount } => (carry + amount, reach),
                SkillEffect::BuildRange { amount } => (carry, reach + amount),
            })
    }

    pub(super) fn hash(&self, hash: &mut u32) {
        // An empty group hashes as absent, preserving pre-skills checksum verification.
        if self == &Self::default() {
            return;
        }
        hash_u32(hash, u32::MAX - 25);
        hash_u32(hash, self.points);
        hash_u32(hash, u32::from(self.sandbox));
        for ids in [&self.purchased, &self.granted, &self.completed] {
            hash_u32(hash, ids.len() as u32);
            for id in ids {
                hash_u32(hash, u32::from(*id));
            }
        }
    }
}

impl Core {
    pub(super) fn skill_availability(&self, skill: &SkillDefinition) -> SkillAvailability {
        let complete = self.skills.owns(skill.id);
        let current_value = match skill.effect {
            SkillEffect::CarrySlots { .. } => self.player.carry_slots,
            SkillEffect::BuildRange { .. } => self.player.build_range / HEX_X as u32,
        };
        let resulting_value = if complete {
            current_value
        } else {
            match skill.effect {
                SkillEffect::CarrySlots { amount } => current_value.max(
                    self.earned_carry_slots()
                        .saturating_add(amount)
                        .min(MAX_CARRY_SLOTS),
                ),
                SkillEffect::BuildRange { amount } => current_value + amount,
            }
        };
        SkillAvailability {
            current_value,
            resulting_value,
            skill_id: skill.id,
            complete: self.skills.owns(skill.id),
            missing_prerequisites: skill
                .prerequisites
                .iter()
                .copied()
                .filter(|id| !self.skills.owns(*id))
                .collect(),
            points_shortfall: skill.cost.saturating_sub(self.skills.points),
        }
    }

    pub(super) fn skills_snapshot(&self) -> SkillsSnapshot {
        SkillsSnapshot {
            state: self.skills.clone(),
            availability: self
                .technologies
                .skills
                .iter()
                .map(|skill| self.skill_availability(skill))
                .collect(),
        }
    }

    pub(super) fn purchase_skill(&mut self, skill_id: u16) -> Result<(), String> {
        let skill = self
            .technologies
            .skills
            .iter()
            .find(|skill| skill.id == skill_id)
            .cloned()
            .ok_or("unknown skill")?;
        let availability = self.skill_availability(&skill);
        if availability.complete {
            return Err("skill already learned".into());
        }
        if !availability.missing_prerequisites.is_empty() {
            return Err("learn the prerequisite skills first".into());
        }
        if availability.points_shortfall > 0 {
            return Err(format!(
                "requires {} Skill Points; complete a journey milestone",
                skill.cost
            ));
        }
        self.skills.points -= skill.cost;
        self.skills.purchased.insert(skill.id);
        self.apply_research_effects();
        self.events.push(format!("Learned {}", skill.name));
        Ok(())
    }

    /// Called only when actual native work completes, never from snapshots or per-cell scans.
    pub(super) fn observe_skill_event(&mut self, event: SkillEvent) {
        if self.skills.sandbox || self.creative {
            return;
        }
        for milestone in &self.technologies.skill_milestones {
            if milestone.event == event && self.skills.completed.insert(milestone.id) {
                self.skills.points += milestone.points;
                self.events.push(format!(
                    "{} — +{} Skill Point. Open Skills to choose an upgrade.",
                    milestone.name, milestone.points
                ));
            }
        }
    }

    pub(super) fn grant_creative_skills(&mut self) {
        self.skills.sandbox = true;
        for skill in &self.technologies.skills {
            if !self.skills.purchased.contains(&skill.id) {
                self.skills.granted.insert(skill.id);
            }
        }
        self.apply_research_effects();
    }

    pub(super) fn migrate_player_skills(&mut self) {
        for skill in &self.technologies.skills {
            if skill
                .legacy_technology_id
                .is_some_and(|id| self.researched.remove(&id))
            {
                self.skills.granted.insert(skill.id);
            }
        }
        // Old completed commissions are trustworthy facts, but must never pay again on load.
        // Workshop and powered-craft rewards remain available through new observed work.
        for milestone in &self.technologies.skill_milestones {
            if let SkillEvent::ContractStage { key } = &milestone.event {
                if self
                    .scenario
                    .contract
                    .stages
                    .iter()
                    .take(self.contract_stage)
                    .any(|stage| &stage.key == key)
                {
                    self.skills.completed.insert(milestone.id);
                }
            }
        }
        self.apply_research_effects();
    }
}

pub(super) fn validate_skills(technologies: &TechnologiesInput) -> Result<(), String> {
    let skills = &technologies.skills;
    let milestones = &technologies.skill_milestones;
    if skills.len() > 64 || milestones.len() > 64 {
        return Err("skill catalogue exceeds 64 skills or milestones".into());
    }
    unique_positive_ids(skills.iter().map(|s| s.id), "skill")?;
    unique_positive_ids(milestones.iter().map(|m| m.id), "skill milestone")?;
    let mut keys = BTreeSet::new();
    let mut legacy_ids = BTreeSet::new();
    let mut carry = 0u32;
    let mut reach = 0u32;
    let mut cost = 0u32;
    for skill in skills {
        let amount = match skill.effect {
            SkillEffect::CarrySlots { amount } => {
                carry = carry.saturating_add(amount);
                amount
            }
            SkillEffect::BuildRange { amount } => {
                reach = reach.saturating_add(amount);
                amount
            }
        };
        if skill.key.trim().is_empty()
            || !keys.insert(&skill.key)
            || skill.name.trim().is_empty()
            || skill.description.trim().is_empty()
            || !matches!(skill.branch.as_str(), "carrying" | "construction")
            || skill.cost == 0
            || skill.cost > 100
            || amount == 0
            || amount > 32
            || skill
                .prerequisites
                .iter()
                .any(|id| !skills.iter().any(|s| s.id == *id))
            || skill.prerequisites.iter().collect::<BTreeSet<_>>().len()
                != skill.prerequisites.len()
            || skill.legacy_technology_id.is_some_and(|id| {
                id == 0
                    || !legacy_ids.insert(id)
                    || technologies.technologies.iter().any(|t| t.id == id)
            })
        {
            return Err(format!("invalid skill {}", skill.key));
        }
        cost += skill.cost;
    }
    if carry > MAX_CARRY_SLOTS / 2 || reach > 32 {
        return Err("skill effects exceed player bounds".into());
    }
    let mut reachable = BTreeSet::new();
    loop {
        let count = reachable.len();
        for skill in skills {
            if skill.prerequisites.iter().all(|id| reachable.contains(id)) {
                reachable.insert(skill.id);
            }
        }
        if reachable.len() == count {
            break;
        }
    }
    if reachable.len() != skills.len() {
        return Err("skill prerequisite cycle".into());
    }
    keys.clear();
    let mut budget = 0u32;
    for (i, milestone) in milestones.iter().enumerate() {
        if milestone.key.trim().is_empty()
            || !keys.insert(&milestone.key)
            || milestone.name.trim().is_empty()
            || milestone.description.trim().is_empty()
            || milestone.points == 0
            || milestone.points > 100
            || milestones[..i].iter().any(|m| m.event == milestone.event)
        {
            return Err(format!("invalid skill milestone {}", milestone.key));
        }
        budget += milestone.points;
    }
    if budget < cost {
        return Err("skill milestone budget cannot fund all ranks".into());
    }
    Ok(())
}

pub(super) fn validate_skill_state(
    technologies: &TechnologiesInput,
    state: &SkillsState,
) -> Result<(), String> {
    let budget: u32 = technologies.skill_milestones.iter().map(|m| m.points).sum();
    let spent: u32 = technologies
        .skills
        .iter()
        .filter(|s| state.purchased.contains(&s.id))
        .map(|s| s.cost)
        .sum();
    let earned: u32 = technologies
        .skill_milestones
        .iter()
        .filter(|m| state.completed.contains(&m.id))
        .map(|m| m.points)
        .sum();
    if state.points > budget
        || spent.saturating_add(state.points) > earned
        || state
            .purchased
            .intersection(&state.granted)
            .next()
            .is_some()
        || state
            .purchased
            .union(&state.granted)
            .any(|id| !technologies.skills.iter().any(|s| s.id == *id))
        || state
            .completed
            .iter()
            .any(|id| !technologies.skill_milestones.iter().any(|m| m.id == *id))
        || technologies
            .skills
            .iter()
            .filter(|s| state.purchased.contains(&s.id))
            .any(|s| s.prerequisites.iter().any(|id| !state.owns(*id)))
    {
        return Err("save contains invalid skill state".into());
    }
    Ok(())
}
