//! Recipe identity, joint-output conservation and explicit production-route policy.
use super::*;

impl RecipeDefinition {
    pub(super) fn outputs(&self) -> impl Iterator<Item = &Ingredient> {
        std::iter::once(&self.output).chain(self.co_products.iter())
    }

    pub(super) fn yield_of(&self, item: ItemId) -> u32 {
        self.outputs()
            .find(|output| output.item_id == item)
            .map_or(0, |output| output.quantity)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn share_of(&self, item: ItemId) -> u32 {
        self.outputs()
            .position(|output| output.item_id == item)
            .map_or(0, |index| {
                self.cost_allocation.get(index).copied().unwrap_or(100)
            })
    }

    fn batch_size(&self) -> u32 {
        self.outputs().map(|output| output.quantity).sum()
    }
}

impl DefinitionsInput {
    pub(super) fn production_routes(&self, item: ItemId) -> Vec<&RecipeDefinition> {
        let mut routes: Vec<_> = self
            .recipes
            .iter()
            .filter(|recipe| recipe.yield_of(item) > 0)
            .collect();
        let order = self
            .items
            .iter()
            .find(|value| value.id == item)
            .and_then(|value| value.production_routes.as_ref());
        routes.sort_by_key(|recipe| {
            (
                order
                    .and_then(|ids| ids.iter().position(|id| *id == recipe.id))
                    .unwrap_or(0),
                recipe.id,
            )
        });
        routes
    }
}

impl Core {
    pub(super) fn can_extract(&self, definition_id: DefinitionId, item_id: ItemId) -> bool {
        let Some(building) = self.building_definition(definition_id) else {
            return false;
        };
        building.kind == BuildingKind::Extractor
            && building.output_item_id.is_none_or(|id| id == item_id)
            && self
                .item_definition(item_id)
                .and_then(|item| item.extraction_building_id)
                .is_none_or(|id| id == definition_id)
    }

    pub(super) fn extractable_deposit(&self, definition_id: DefinitionId, key: (i32, i32)) -> bool {
        self.deposit_quantity(key) > 0
            && self
                .field_at(key.0, key.1)
                .is_some_and(|field| self.can_extract(definition_id, field.item_id))
    }

    pub(super) fn room_for_recipe(&self, index: usize, recipe: &RecipeDefinition) -> bool {
        self.room_for_stock(index, StockKind::Output, 0) >= recipe.batch_size()
    }

    pub(super) fn reachable_recipe(&self, item: ItemId, depth: u32) -> Option<&RecipeDefinition> {
        if depth > MAX_RECIPE_DEPTH {
            return None;
        }
        self.definitions
            .production_routes(item)
            .into_iter()
            .find(|recipe| {
                self.recipe_unlocked(recipe)
                    && recipe
                        .inputs
                        .iter()
                        .all(|input| self.item_reachable(input.item_id, depth + 1))
            })
    }
}

pub(super) fn validate_routes(definitions: &DefinitionsInput) -> Result<(), String> {
    for recipe in &definitions.recipes {
        let outputs: Vec<_> = recipe.outputs().collect();
        if outputs.len() > 8
            || outputs
                .iter()
                .map(|output| output.item_id)
                .collect::<BTreeSet<_>>()
                .len()
                != outputs.len()
            || recipe
                .inputs
                .iter()
                .map(|input| input.item_id)
                .collect::<BTreeSet<_>>()
                .len()
                != recipe.inputs.len()
        {
            return Err(format!(
                "recipe {} has duplicate or excessive ingredients",
                recipe.id
            ));
        }
        if (outputs.len() > 1 || !recipe.cost_allocation.is_empty())
            && (recipe.cost_allocation.len() != outputs.len()
                || recipe.cost_allocation.contains(&0)
                || recipe
                    .cost_allocation
                    .iter()
                    .map(|share| u64::from(*share))
                    .sum::<u64>()
                    != 100)
        {
            return Err(format!(
                "recipe {} requires positive cost shares summing to 100",
                recipe.id
            ));
        }
        let batch: u64 = outputs
            .iter()
            .map(|output| u64::from(output.quantity))
            .sum();
        if batch > u64::from(u32::MAX)
            || definitions.buildings.iter().any(|building| {
                building.supports_recipe(recipe)
                    && u64::from(building.capacity.unwrap_or(u32::MAX)) < batch
            })
        {
            return Err(format!(
                "recipe {} output batch exceeds machine capacity",
                recipe.id
            ));
        }
    }
    for item in &definitions.items {
        let producers: BTreeSet<_> = definitions
            .production_routes(item.id)
            .iter()
            .map(|recipe| recipe.id)
            .collect();
        if producers.len() > 1 || item.production_routes.is_some() {
            let ids = item.production_routes.as_deref().unwrap_or(&[]);
            if ids.len() != producers.len()
                || ids.iter().copied().collect::<BTreeSet<_>>() != producers
            {
                return Err(format!(
                    "item {} requires an explicit production route order",
                    item.id
                ));
            }
        }
        if let Some(id) = item.extraction_building_id {
            if !definitions.buildings.iter().any(|building| {
                building.id == id
                    && building.kind == BuildingKind::Extractor
                    && building.output_item_id == Some(item.id)
            }) {
                return Err(format!(
                    "item {} has an invalid extraction building",
                    item.id
                ));
            }
        }
    }
    fn visit(
        item: ItemId,
        definitions: &DefinitionsInput,
        visiting: &mut BTreeSet<ItemId>,
        checked: &mut BTreeSet<ItemId>,
    ) -> Result<(), String> {
        if visiting.contains(&item) {
            return Err(format!("recipe cycle through item {item}"));
        }
        if checked.contains(&item) {
            return Ok(());
        }
        visiting.insert(item);
        for recipe in definitions.production_routes(item) {
            for input in &recipe.inputs {
                visit(input.item_id, definitions, visiting, checked)?;
            }
        }
        visiting.remove(&item);
        checked.insert(item);
        Ok(())
    }
    let mut checked = BTreeSet::new();
    for item in &definitions.items {
        visit(item.id, definitions, &mut BTreeSet::new(), &mut checked)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joint_output_contract_rejects_ambiguous_costs_and_cycles() {
        let mut definitions: DefinitionsInput =
            serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
        definitions.recipes[0].co_products = vec![Ingredient {
            item_id: 3,
            quantity: 2,
        }];
        assert!(validate_routes(&definitions)
            .unwrap_err()
            .contains("cost shares"));
        definitions.recipes[0].cost_allocation = vec![70, 30];
        assert_eq!(definitions.recipes[0].share_of(3), 30);
        definitions.recipes[0].co_products[0] = definitions.recipes[0].inputs[0];
        assert!(validate_routes(&definitions).is_err());
    }

    #[test]
    fn production_route_order_is_explicit_and_reachability_can_use_an_unlocked_alternative() {
        let mut definitions: DefinitionsInput =
            serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
        let technologies =
            serde_json::from_str(include_str!("../../src/data/technologies.json")).unwrap();
        let scenarios: ScenariosInput =
            serde_json::from_str(include_str!("../../src/data/scenarios.json")).unwrap();
        let original = definitions
            .recipes
            .iter()
            .find(|recipe| recipe.id == 2)
            .unwrap()
            .clone();
        let mut preferred = original.clone();
        preferred.id = 1001;
        preferred.key = "petroleum-plate-test".into();
        preferred.category = "refining".into();
        definitions.recipes.push(preferred);
        assert!(validate_routes(&definitions)
            .unwrap_err()
            .contains("route order"));
        definitions
            .items
            .iter_mut()
            .find(|item| item.id == original.output.item_id)
            .unwrap()
            .production_routes = Some(vec![1001, original.id]);
        validate_routes(&definitions).unwrap();
        definitions.recipes.reverse();
        let mut core = Core::new(
            &definitions,
            &technologies,
            &scenarios.scenarios[0],
            None,
            None,
        )
        .unwrap();
        core.researched.clear();
        core.researched.insert(5);
        assert_eq!(
            core.reachable_recipe(original.output.item_id, 0)
                .unwrap()
                .id,
            original.id,
            "smelter can make the fallback"
        );
        core.researched.insert(21);
        assert_eq!(
            core.reachable_recipe(original.output.item_id, 0)
                .unwrap()
                .id,
            1001
        );
        let refinery = definitions
            .buildings
            .iter_mut()
            .find(|building| building.id == 30)
            .unwrap();
        refinery.capacity = Some(3);
        assert!(validate_routes(&definitions)
            .unwrap_err()
            .contains("batch exceeds"));
    }
}
