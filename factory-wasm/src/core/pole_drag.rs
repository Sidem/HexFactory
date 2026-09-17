use super::*;

impl Core {
    /// Existing poles can anchor a drag without spending another pole's materials.
    pub(crate) fn pole_at(&self, cell: (i32, i32)) -> Option<u32> {
        let entity = &self.entities[self.entity_at(cell.0, cell.1)?];
        let definition = self.building_definition(entity.placed.definition_id)?;
        (definition.kind == BuildingKind::Pole)
            .then_some(definition.pole_reach.unwrap_or(DEFAULT_POLE_REACH as u32))
    }

    /// Walk a bounded straight route, jumping to the farthest legal site within wire reach.
    /// Obstacles shorten a span; they never create an unconnected island beyond it. The last
    /// legal site towards the pointer is included even when the final span is shorter.
    pub(crate) fn pole_drag_route(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        orientation: u8,
    ) -> Vec<(i32, i32)> {
        let definition = self.building_definition(definition_id).unwrap();
        let reach = definition.pole_reach.unwrap_or(DEFAULT_POLE_REACH as u32);
        let line = line_between(from, to, OrientationAxis::Edge);
        let legal: Vec<bool> = line
            .iter()
            .map(|&cell| {
                self.pole_at(cell).is_some()
                    || self
                        .placement_legality(cell.0, cell.1, definition_id, orientation, None, false)
                        .is_ok()
            })
            .collect();
        let Some(mut index) = legal.iter().position(|&value| value) else {
            return vec![from];
        };
        let mut route = vec![line[index]];
        loop {
            let current = line[index];
            let current_reach = self.pole_at(current).unwrap_or(reach);
            let next = (index + 1..line.len()).rev().find(|&next| {
                legal[next]
                    && axial_distance(current, line[next]) as u32
                        <= current_reach.max(self.pole_at(line[next]).unwrap_or(reach))
            });
            let Some(next) = next else { break };
            index = next;
            route.push(line[index]);
        }
        route
    }
}
