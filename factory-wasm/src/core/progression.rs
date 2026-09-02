//! progression — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn request_definition(&self, id: RequestId) -> Option<&RequestDefinition> {
        self.definitions
            .requests
            .iter()
            .find(|request| request.id == id)
    }

    /// Put a delivery against the board, and pay for whatever it finishes.
    ///
    /// This is the only path in the game that adds insight. Before it, every hub delivery paid
    /// `insight_value × quantity` whether the hub had a use for the item or not, which meant the
    /// price of a material was a number the player could only learn by giving it away. Now the
    /// price is posted first and paid on completion — once, and only once.
    ///
    /// A filled slot is replaced in place rather than compacted out, so the row the player was
    /// reading does not jump to another slot the moment it completes. The replacement is not filled
    /// from the same delivery: it starts empty, and the next delivery is what moves it. The
    /// completed project does not come back into the draw, and when nothing is left that the player
    /// can reach the slot closes rather than reposting paid work.
    pub(crate) fn credit_requests(&mut self, item_id: ItemId, quantity: u32) {
        let mut remaining = quantity;
        let mut slot = 0;
        while slot < self.requests.len() && remaining > 0 {
            let Some(definition) = self
                .request_definition(self.requests[slot].request_id)
                .cloned()
            else {
                slot += 1;
                continue;
            };
            if definition.item_id != item_id {
                slot += 1;
                continue;
            }
            // A project pays once. Posting is already gated on this, so reaching it here means a
            // save was edited or a slot survived a migration it should not have — and the failure
            // mode is minting insight without bound, which is the one thing finite demand exists to
            // prevent. Cheaper to refuse it at the till than to trust every path in.
            if self.project_complete(definition.id) {
                slot += 1;
                continue;
            }
            let held = self.project_delivered(definition.id);
            let take = definition.quantity.saturating_sub(held).min(remaining);
            remaining -= take;
            let now = held + take;
            self.request_delivered.insert(definition.id, now);
            if now < definition.quantity {
                slot += 1;
                continue;
            }
            self.insight += u64::from(definition.insight);
            *self.request_rounds.entry(definition.id).or_default() += 1;
            *self.request_fills.entry(definition.id).or_default() += 1;
            // The bill is consumed by completion. Keeping the count would leave a retired project
            // reading as permanently full, and the catalogue draws its progress from this map.
            self.request_delivered.remove(&definition.id);
            self.events.push(format!(
                "{} complete — the hub pays {} insight",
                definition.name, definition.insight
            ));
            let posted = self.posted_requests(Some(slot));
            match self.next_request(&posted) {
                Some(id) => {
                    self.requests[slot] = RequestState { request_id: id };
                    slot += 1;
                }
                // Nothing left the player can reach. The slot closes rather than reposting the row
                // that was just paid for, and `refill_requests` opens it again when research does.
                None => {
                    self.requests.remove(slot);
                    if self.requests.is_empty() {
                        self.events.push(
                            "The hub has nothing further to ask for — its demand is satisfied"
                                .into(),
                        );
                    }
                }
            }
        }
        self.refill_requests();
    }

    /// How much has been handed over against one project, posted or not.
    pub(crate) fn project_delivered(&self, id: RequestId) -> u32 {
        self.request_delivered.get(&id).copied().unwrap_or_default()
    }

    /// Whether this project has been completed and paid. A skipped project has not.
    pub(crate) fn project_complete(&self, id: RequestId) -> bool {
        self.request_fills.get(&id).copied().unwrap_or_default() > 0
    }

    /// The request ids currently on the board, optionally ignoring one slot.
    pub(crate) fn posted_requests(&self, ignore: Option<usize>) -> BTreeSet<RequestId> {
        self.requests
            .iter()
            .enumerate()
            .filter(|&(slot, _)| Some(slot) != ignore)
            .map(|(_, state)| state.request_id)
            .collect()
    }

    /// Whether this project can be drawn into a slot: unfinished, and something the player could
    /// actually supply.
    pub(crate) fn request_eligible(&self, request: &RequestDefinition) -> bool {
        !self.project_complete(request.id) && self.item_reachable(request.item_id, 0)
    }

    /// The row that should be posted next: the least-used one the player can actually supply,
    /// unless the board currently holds no row at the deepest reachable depth — then that depth
    /// is reserved, so a three-slot board still leads once processing unlocks rather than cycling
    /// eight raw surveys first.
    ///
    /// A finished project is never a candidate. The catalogue is finite, so the draw order is
    /// walking a budget down rather than cycling forever, and it ends.
    ///
    /// There is no randomness here, and that is deliberate. A board that is a pure function of
    /// state is a board a save restores exactly, a checksum agrees about, and a test can walk —
    /// and one whose progression a player can learn rather than reroll. Reservation still walks
    /// `item_reachable`, so a player who cannot yet build a smelter never faces a board of three
    /// things they cannot make.
    pub(crate) fn next_request(&self, posted: &BTreeSet<RequestId>) -> Option<RequestId> {
        let eligible: Vec<&RequestDefinition> = self
            .definitions
            .requests
            .iter()
            .filter(|request| !posted.contains(&request.id))
            .filter(|request| self.request_eligible(request))
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let max_depth = self
            .definitions
            .requests
            .iter()
            .filter(|request| self.request_eligible(request))
            .map(|request| self.item_depth(request.item_id))
            .max()
            .unwrap_or(0);
        let posted_has_max = self
            .definitions
            .requests
            .iter()
            .filter(|request| posted.contains(&request.id))
            .any(|request| self.item_depth(request.item_id) == max_depth);
        let pool = if posted_has_max {
            eligible
        } else {
            eligible
                .into_iter()
                .filter(|request| self.item_depth(request.item_id) == max_depth)
                .collect()
        };
        pool.into_iter()
            .min_by_key(|request| {
                (
                    self.request_rounds
                        .get(&request.id)
                        .copied()
                        .unwrap_or_default(),
                    request.id,
                )
            })
            .map(|request| request.id)
    }

    /// Recipe-tree depth of an item: zero for something that comes out of the ground or a source
    /// building, one plus the deepest input for a craft. The reserved board slot is this number,
    /// not catalogue order, so a plate leads a second ore assay once a smelter is unlocked.
    pub(crate) fn item_depth(&self, item: ItemId) -> u32 {
        self.item_depth_at(item, 0)
    }

    pub(crate) fn item_depth_at(&self, item: ItemId, guard: u32) -> u32 {
        if guard > MAX_RECIPE_DEPTH {
            return 0;
        }
        match self
            .reachable_recipe(item, guard)
            .or_else(|| self.definitions.production_routes(item).into_iter().next())
        {
            Some(recipe) => {
                let inner = recipe
                    .inputs
                    .iter()
                    .map(|input| self.item_depth_at(input.item_id, guard + 1))
                    .max()
                    .unwrap_or(0);
                inner + 1
            }
            None => 0,
        }
    }

    /// Post requests into every empty slot.
    pub(crate) fn refill_requests(&mut self) {
        let capacity = REQUEST_SLOTS.min(self.definitions.requests.len());
        while self.requests.len() < capacity {
            let posted = self.posted_requests(None);
            let Some(id) = self.next_request(&posted) else {
                return;
            };
            self.requests.push(RequestState { request_id: id });
        }
    }

    /// Whether the player could actually produce this item with what they have researched.
    ///
    /// The board is drawn against this rather than against an unlock column written by hand, so a
    /// request can never ask for something the rules do not yet allow, and a new item is gated
    /// correctly by existing. The walk is the recipe tree: every craft along it needs a machine the
    /// player may build, and every leaf needs a source they may use — water is nobody's field, so
    /// an item a building outputs directly is reachable exactly when that building is.
    pub(crate) fn item_reachable(&self, item: ItemId, depth: u32) -> bool {
        if depth > MAX_RECIPE_DEPTH {
            return false;
        }
        match self.reachable_recipe(item, depth) {
            Some(_) => true,
            None if !self.definitions.production_routes(item).is_empty() => false,
            None => {
                let mut sources = self
                    .definitions
                    .buildings
                    .iter()
                    .filter(|building| building.output_item_id == Some(item))
                    .peekable();
                if sources.peek().is_some() {
                    return sources.any(|building| self.technology_met(building));
                }
                // A field item the hand can take is reachable from a standing start. A field item
                // it cannot — signal crystal — is reachable once an extractor is unlocked, the
                // same way water waits on a pump.
                match self
                    .item_definition(item)
                    .and_then(|definition| definition.hand_gather_steps)
                {
                    Some(_) => true,
                    None => self.definitions.buildings.iter().any(|building| {
                        building.kind == BuildingKind::Extractor
                            && building.buildable
                            && self.technology_met(building)
                    }),
                }
            }
        }
    }

    pub(crate) fn recipe_unlocked(&self, recipe: &RecipeDefinition) -> bool {
        self.definitions.buildings.iter().any(|building| {
            building.buildable
                && building.supports_recipe(recipe)
                && self.technology_met(building)
                // Baseline primitive knowledge should not put gears on a brand-new player's
                // board before they have any station. Purchased industrial knowledge keeps its
                // existing eligibility rule; primitive requests appear once their station exists.
                && (building.recipe_ids.is_none() || self.entities.iter().any(|entity| entity.placed.definition_id == building.id))
        })
    }

    pub(crate) fn technology_met(&self, building: &BuildingDefinition) -> bool {
        match building.unlock_technology_id {
            Some(id) => self.researched.contains(&id),
            None => true,
        }
    }

    /// How much of one item the landing hub still has a use for: what the posted requests are
    /// short, plus what the founding contract has not been given yet.
    ///
    /// The contract half counts every remaining stage rather than only the current one, which is
    /// what keeps the v0.18 surplus rule true — a player who automates a line early is still
    /// credited when the stage that wants it arrives. What the hub does *not* want, it no longer
    /// takes: an item nobody asked for used to vanish into the hub for a coin of insight, and the
    /// player had no way to see that happening.
    pub(crate) fn hub_demand(&self, item: ItemId) -> u64 {
        let posted: u64 = self
            .requests
            .iter()
            .filter_map(|state| {
                self.request_definition(state.request_id)
                    .map(|definition| (definition, state))
            })
            .filter(|(definition, _)| definition.item_id == item)
            .map(|(definition, _)| {
                u64::from(
                    definition
                        .quantity
                        .saturating_sub(self.project_delivered(definition.id)),
                )
            })
            .sum();
        let billed: u64 = self
            .scenario
            .contract
            .stages
            .get(self.contract_stage..)
            .unwrap_or_default()
            .iter()
            .flat_map(|stage| stage.requirements.iter())
            .filter(|need| need.item_id == item)
            .map(|need| u64::from(need.quantity))
            .sum();
        let held = self
            .contract_contributed
            .get(&item)
            .copied()
            .unwrap_or_default();
        posted + billed.saturating_sub(held)
    }

    /// Pass on a posted request, so another takes its slot.
    ///
    /// Without this the board is a trap rather than an offer: three materials the player has not
    /// found yet would hold every slot, and the only source of insight in the game with them.
    /// Passing costs the row one place in the draw order — it comes round again behind everything
    /// not yet seen.
    ///
    /// It no longer forfeits what has been delivered against the row. That forfeit was affordable
    /// when a row could be filled again for the same price; under finite demand it would destroy
    /// goods whose reward can never be re-earned, turning an offer to look at something else into a
    /// trap of its own. Progress lives in `request_delivered` and waits for the project to come
    /// back.
    pub(crate) fn skip_request(&mut self, slot: usize) -> Result<(), String> {
        let state = *self
            .requests
            .get(slot)
            .ok_or("no request is posted in that slot")?;
        let name = self
            .request_definition(state.request_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| format!("request {}", state.request_id));
        // The round is counted first, because what is being passed on is still a candidate for the
        // slot it is leaving — and it must not win it back while anything less used is waiting.
        let rounds = self.request_rounds.entry(state.request_id).or_default();
        *rounds += 1;
        let posted = self.posted_requests(Some(slot));
        let Some(id) = self.next_request(&posted) else {
            *self.request_rounds.entry(state.request_id).or_default() -= 1;
            return Err("the hub has nothing else to ask for".into());
        };
        self.requests[slot] = RequestState { request_id: id };
        self.events.push(format!("Passed on {name}"));
        Ok(())
    }

    /// Put one named project on the board, in place of whichever posted row the player is least
    /// committed to.
    ///
    /// This is what makes a finite catalogue browsable rather than a lottery. The draw order is a
    /// good default and a bad constraint: once each project pays only once, "the row I need is not
    /// posted" is no longer a wait, it is a route the player cannot take. Choosing costs the
    /// displaced row nothing — its progress persists like any other — and the chosen project keeps
    /// whatever it had already been given.
    ///
    /// The displaced slot is the posted row with the least delivered against it, ties broken by
    /// slot, so asking for a project never silently unposts the one being worked on.
    pub(crate) fn post_request(&mut self, request_id: RequestId) -> Result<(), String> {
        let definition = self
            .request_definition(request_id)
            .ok_or_else(|| format!("no project {request_id}"))?
            .clone();
        if self.project_complete(definition.id) {
            return Err(format!("{} is already complete", definition.name));
        }
        if !self.item_reachable(definition.item_id, 0) {
            return Err(format!(
                "{} asks for something you cannot make yet",
                definition.name
            ));
        }
        if let Some(slot) = self
            .requests
            .iter()
            .position(|state| state.request_id == definition.id)
        {
            // Already posted. Saying so is a better answer than moving it to another slot.
            let _ = slot;
            return Err(format!("{} is already on the board", definition.name));
        }
        let target = self
            .requests
            .iter()
            .enumerate()
            .min_by_key(|(slot, state)| (self.project_delivered(state.request_id), *slot))
            .map(|(slot, _)| slot);
        match target {
            Some(slot) => {
                let displaced = self.requests[slot].request_id;
                // The displaced row leaves the board the same way a pass leaves it: one place back
                // in the draw order, its progress intact.
                *self.request_rounds.entry(displaced).or_default() += 1;
                self.requests[slot] = RequestState {
                    request_id: definition.id,
                };
            }
            None => self.requests.push(RequestState {
                request_id: definition.id,
            }),
        }
        self.events
            .push(format!("{} posted to the board", definition.name));
        Ok(())
    }

    /// Close every stage the hub can now afford, in order.
    ///
    /// The loop is not decoration: contributions carry forward, so a stage whose bill a previous
    /// surplus already covers must complete in the same delivery rather than wait for one more
    /// item to arrive and re-ask the question.
    pub(crate) fn advance_contract(&mut self) {
        self.advance_contract_with_rewards(true);
    }

    pub(crate) fn advance_contract_with_rewards(&mut self, award_skill_points: bool) {
        while let Some(stage) = self.scenario.contract.stages.get(self.contract_stage) {
            let met = stage.requirements.iter().all(|need| {
                self.contract_contributed
                    .get(&need.item_id)
                    .copied()
                    .unwrap_or(0)
                    >= u64::from(need.quantity)
            });
            if !met {
                return;
            }
            let consumed = stage.requirements.clone();
            let name = stage.name.clone();
            let key = stage.key.clone();
            for need in &consumed {
                let held = self.contract_contributed.entry(need.item_id).or_default();
                *held = held.saturating_sub(u64::from(need.quantity));
            }
            self.contract_stage += 1;
            self.events
                .push(format!("{name} complete — the landing hub grows"));
            self.grant_contract_stage(&key);
            if award_skill_points {
                self.observe_skill_event(SkillEvent::ContractStage { key });
            }
            if self.contract_stage >= self.scenario.contract.stages.len() {
                self.victory = true;
                self.events
                    .push("Founding contract complete — free play continues".into());
            }
        }
    }
}
