use crate::slayer::slayer_loot::{DropType, LootEntry, LootTable};
use std::rc::Rc;

#[derive(Clone)]
pub struct LootChanceEntry {
    pub entry: Rc<LootEntry>,
    pub used_weight: f64,
    pub magic_find_multiplier: f64,
    pub chance: f64,
}

impl LootChanceEntry {
    fn new(entry: Rc<LootEntry>) -> Self {
        LootChanceEntry {
            entry: Rc::clone(&entry),
            used_weight: entry.get_weight(),
            magic_find_multiplier: 1.0,
            chance: 0.0
        }
    }
}

#[derive(Default)]
pub struct RngMeterData {
    pub selected_item: Option<SelectedRngMeterItem>,
    pub selected_xp: i32,
}

#[derive(PartialEq, Hash)]
pub struct SelectedRngMeterItem {
    pub identifier: String,
    pub required_xp: i32,

    pub highest_tier_chest_entry: Rc<LootEntry>,
    pub highest_boss_level: u8,
    pub lowest_tier_chest_entry: Rc<LootEntry>,
    pub lowest_boss_level: u8,
}

pub fn calculate_chances(chest: &LootTable, magic_find: f32, slayer_level: u8, rng_meter_data: &RngMeterData) -> Vec<LootChanceEntry> {
    let mut rng_meter_guaranteed_item_type: Option<DropType> = None;

    let mut entries = chest.loot
        .iter()
        .filter(|entry| { slayer_level >= entry.get_slayer_level_requirement() })
        .map(|entry| {
            let mut entry = LootChanceEntry::new(Rc::clone(entry));

            if entry.entry.get_drop_type() == &DropType::Token {
                entry.chance = 1.0;
                return entry;
            }

            if let Some(selected_item_data) = &rng_meter_data.selected_item {
                if selected_item_data.identifier == entry.entry.to_string() {
                    let multiplier = 1.0 + (2.0 * rng_meter_data.selected_xp as f32 / selected_item_data.required_xp as f32).min(2.0) as f64;
                    entry.used_weight *= multiplier;

                    if multiplier >= 3.0 {
                        entry.chance = 1.0;
                        rng_meter_guaranteed_item_type = Some(entry.entry.get_drop_type().clone());
                    }
                }
            }

            entry
        })
        .collect::<Vec<LootChanceEntry>>();

    if rng_meter_guaranteed_item_type != Some(DropType::Main) {
        process_main_entries(&mut entries, magic_find);
    }

    if rng_meter_guaranteed_item_type != Some(DropType::Extra) {
        process_extra_entries(&mut entries, magic_find);
    }

    entries
}

fn process_main_entries(entries: &mut Vec<LootChanceEntry>, magic_find: f32) {
    let mut total_weight = get_total_token_and_main_weights(entries);

    if magic_find > 0.0 {
        apply_magic_find(entries, DropType::Main, total_weight, magic_find);
        total_weight = get_total_token_and_main_weights(entries);
    }

    apply_chances(entries, total_weight, DropType::Main);
}

fn process_extra_entries(entries: &mut Vec<LootChanceEntry>, magic_find: f32) {
    let mut total_weight = entries.iter().map(|e| e.used_weight * e.magic_find_multiplier).sum::<f64>();

    if magic_find > 0.0 {
        apply_magic_find(entries, DropType::Extra, total_weight, magic_find);
        total_weight = entries.iter().map(|e| e.used_weight * e.magic_find_multiplier).sum::<f64>();
    }

    apply_chances(entries, total_weight, DropType::Extra);
}

fn get_total_token_and_main_weights(entries: &mut [LootChanceEntry]) -> f64 {
    entries.iter()
        .filter(|e| e.entry.get_drop_type() != &DropType::Extra)
        .map(|e| e.used_weight * e.magic_find_multiplier)
        .sum::<f64>()
}

fn apply_magic_find(entries: &mut [LootChanceEntry], drop_type: DropType, total_weight: f64, magic_find: f32) {
    for entry in entries.iter_mut() {
        if entry.entry.get_drop_type() == &drop_type && (entry.used_weight / total_weight) < 0.05 {
            entry.magic_find_multiplier = 1.0 + (magic_find / 100.0) as f64;
        }
    }
}

fn apply_chances(entries: &mut Vec<LootChanceEntry>, total_weight: f64, drop_type: DropType) {
    for entry in entries {
        if entry.entry.get_drop_type() == &drop_type {
            entry.chance = (entry.used_weight * entry.magic_find_multiplier) / total_weight;
        }
    }
}
