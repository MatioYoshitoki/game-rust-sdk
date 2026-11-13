use super::*;
impl Config {
    pub(crate) fn item_limit(&self, id: &str) -> Option<u32> {
        if id == self.item_id {
            Some(self.stack_limit)
        } else {
            self.pve.as_ref()?.items.get(id).map(|i| i.stack_limit)
        }
    }
    /// Verify aggregate loss by reconstructing feasible selected stack counts, not a single-stack shortcut.
    pub(crate) fn valid_death_loss(&self, quantities: &[(&str, u32, u32)]) -> bool {
        let mut possible = std::collections::BTreeSet::from([0usize]);
        let mut slots = 0usize;
        for &(id, quantity, dropped) in quantities {
            let Some(limit) = self.item_limit(id) else {
                return false;
            };
            let full = quantity / limit;
            let tail = quantity % limit;
            slots += quantity.div_ceil(limit) as usize;
            if slots > self.inventory_capacity as usize || dropped > quantity {
                return false;
            }
            let full_loss = self.params.death.amount(limit);
            let mut choices = std::collections::BTreeSet::new();
            for partial in 0..=usize::from(tail > 0) {
                let partial_loss = if partial > 0 {
                    self.params.death.amount(tail)
                } else {
                    0
                };
                let Some(rest) = dropped.checked_sub(partial_loss) else {
                    continue;
                };
                if full_loss == 0 {
                    if rest == 0 {
                        for count in 0..=full {
                            choices.insert(count as usize + partial);
                        }
                    }
                } else if rest % full_loss == 0 && rest / full_loss <= full {
                    choices.insert((rest / full_loss) as usize + partial);
                }
            }
            possible = possible
                .iter()
                .flat_map(|a| choices.iter().map(move |b| a + b))
                .filter(|n| *n <= self.inventory_capacity as usize)
                .collect();
            if possible.is_empty() {
                return false;
            }
        }
        possible.contains(&self.params.death.count(slots))
    }
}
