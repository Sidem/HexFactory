//! Player-clock ownership for ground edits that resolve to cut or fill.

use super::*;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct GroundPreview {
    pub cells: Vec<GroundPreviewCell>,
    pub changes: usize,
    pub cost: Vec<Ingredient>,
    pub refund: Vec<Ingredient>,
    /// Steps of material this edit digs out, and steps it puts back.
    pub cut: u32,
    pub fill: u32,
    /// Player-clock steps this edit spends before it changes the world. Zero for surface-only work.
    pub work_steps: u32,
    /// The spoil ledger after the edit, so the tray can show what levelling leaves behind.
    pub spoil: u64,
    /// How many selected cells hold a resource field the surface would cover.
    pub covers: usize,
    /// How many finished edges would be too steep to walk.
    pub retaining: usize,
    /// How many selected cells cannot take this edit and are skipped.
    pub blocked: usize,
    pub error: Option<String>,
}

impl GroundEdit {
    /// Hash a pending command in declaration order. A ground edit that is still being worked is
    /// simulation state: changing its footprint, verb or depth changes the world it will produce.
    pub(crate) fn hash_into(&self, hash: &mut u32) {
        hash_i32(hash, self.q);
        hash_i32(hash, self.r);
        hash_i32(hash, self.to_q);
        hash_i32(hash, self.to_r);
        match self.datum {
            Some((q, r)) => {
                hash_u32(hash, 1);
                hash_i32(hash, q);
                hash_i32(hash, r);
            }
            None => hash_u32(hash, 0),
        }
        hash_u32(hash, u32::from(self.corner));
        hash_u32(hash, u32::from(self.to_corner));
        hash_u32(
            hash,
            match self.shape {
                GroundShape::Cell => 0,
                GroundShape::Path => 1,
                GroundShape::Rect => 2,
                GroundShape::Frame => 3,
                GroundShape::Disc => 4,
                GroundShape::Ring => 5,
            },
        );
        hash_u32(hash, u32::from(self.definition_id));
        hash_u32(
            hash,
            match self.action {
                GroundAction::Pave => 0,
                GroundAction::Clear => 1,
                GroundAction::Raise => 2,
                GroundAction::Lower => 3,
                GroundAction::Smooth => 4,
                GroundAction::Level => 5,
            },
        );
        hash_u32(hash, u32::from(self.steps));
        hash_u32(
            hash,
            match self.reference {
                GroundReference::First => 0,
                GroundReference::Lowest => 1,
                GroundReference::Highest => 2,
            },
        );
        hash_u32(hash, u32::from(self.cover));
    }
}

impl Core {
    /// Start one ground transaction. Earth only moves after the player's clock has paid for the
    /// resolved volume; a surface-only edit still commits immediately because it moves no earth.
    pub(crate) fn begin_groundwork(&mut self, edit: GroundEdit) -> Result<(), String> {
        if self.player.action_cooldown > 0 {
            return Err(
                "Finish the current field work before starting another ground patch".into(),
            );
        }
        let transaction = self.ground_transaction(&edit);
        if let Some(error) = transaction.preview.error {
            return Err(error);
        }
        if transaction.preview.changes == 0 {
            return Err("This ground already matches the selection; nothing spent".into());
        }
        if transaction.preview.work_steps == 0 {
            return self.edit_ground(&edit);
        }
        let work = transaction.preview.work_steps;
        self.player.action_cooldown = work;
        self.last_action_cooldown_total = work;
        self.pending_ground = Some(edit);
        self.events.push(format!(
            "Groundwork started · {:.1} s",
            f64::from(work) / f64::from(PLAYER_TICKS_PER_SECOND)
        ));
        Ok(())
    }

    /// Land a ground action whose work has elapsed. Re-resolving here makes changes during the
    /// work obey the ordinary transaction guards; a failed finish moves no earth or inventory.
    pub(crate) fn finish_groundwork(&mut self) {
        let Some(edit) = self.pending_ground.take() else {
            return;
        };
        if let Err(error) = self.edit_ground(&edit) {
            self.events.push(format!("Groundwork stopped: {error}"));
        }
    }
}
