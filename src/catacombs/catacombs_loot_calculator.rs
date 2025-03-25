use crate::catacombs::catacombs_loot::{ChestType, LootChest, LootEntry};
use std::cell::RefCell;
use std::rc::Rc;
use num_format::Locale::{en, es, qu};
use rand::distr::weighted::Error;
use rand::prelude::Distribution;
use rand_distr::weighted::WeightedIndex;

pub fn calculate_quality(chest: &LootChest, treasure_talisman_multiplier: f64, boss_luck_increase: u8, s_plus: bool) -> i16 {
    let base_quality = chest.base_quality as f64;

    let s_plus_multiplier = if s_plus { 1.05 } else { 1.0 };
    let floor_quality: f64 = (base_quality * s_plus_multiplier).floor();
    let modified_quality: f64 = ((floor_quality * treasure_talisman_multiplier) + (boss_luck_increase as f64)).round();
    let final_rounded_quality: i16 = ((modified_quality * treasure_talisman_multiplier) + (boss_luck_increase as f64)).round() as i16;
    final_rounded_quality
}

#[derive(Clone)]
pub struct LootChanceEntry {
    pub entry: Rc<LootEntry>,
    pub used_weight: f64,
    pub chance: f64,
    disabled: bool,
}

impl LootChanceEntry {
    fn new(entry: Rc<LootEntry>) -> Self {
        LootChanceEntry {
            entry: Rc::clone(&entry),
            used_weight: entry.get_weight() as f64,
            chance: 0.0,
            disabled: false,
        }
    }

    fn increase_chance(&mut self, increase: f64) {
        self.chance += increase;
    }
}

#[derive(Default, Hash)]
pub struct RngMeterData {
    pub selected_item: Option<SelectedRngMeterItem>,
    pub selected_xp: i32,
}

#[derive(Hash, PartialEq)]
pub struct SelectedRngMeterItem {
    pub identifier: String,
    pub required_xp: i32,
    pub highest_tier_chest_entry: Rc<LootEntry>,
    pub highest_tier_chest_type: ChestType,
    pub lowest_tier_chest_entry: Rc<LootEntry>,
    pub lowest_tier_chest_type: ChestType,
}

struct EntryData {
    weighted_entries: Vec<Rc<RefCell<LootChanceEntry>>>,
    weighted_essence_entry: Rc<RefCell<LootChanceEntry>>,
    leftover_essence_entry: Rc<RefCell<LootChanceEntry>>,
    lowest_non_essence_quality: i16,
}

#[derive(Default)]
struct RecursiveData {
    iterations: i32,
    highest_depth: i32,
}

pub struct AveragesCalculationResult {
    pub chances: Vec<LootChanceEntry>,
    pub total_weight: f64,
}

pub fn calculate_average_chances(chest: &LootChest, mut starting_quality: i16, rng_meter_data: &RngMeterData) -> AveragesCalculationResult {
    let mut weighted_entries: Vec<Rc<RefCell<LootChanceEntry>>> = Vec::new();

    let mut weighted_essence_entry: Option<Rc<RefCell<LootChanceEntry>>> = None;

    let mut leftover_essence_entry: Option<Rc<RefCell<LootChanceEntry>>> = None;
    let mut guaranteed_essence_entries: Vec<LootChanceEntry> = Vec::new();

    let mut recursion_data: RecursiveData = Default::default();
    let mut lowest_non_essence_quality: Option<i16> = None;

    for entry in &chest.loot {
        let mut chance_entry = LootChanceEntry::new(Rc::clone(entry));

        match entry.as_ref() {
            LootEntry::Essence { weight, quality, .. } => {
                if weight > &0 && quality > &0 {
                    let pointer = Rc::new(RefCell::new(chance_entry));
                    weighted_entries.push(Rc::clone(&pointer));
                    weighted_essence_entry = Some(Rc::clone(&pointer));
                } else if weight == &0 && quality == &1 {
                    leftover_essence_entry = Some(Rc::new(RefCell::new(chance_entry)));
                } else {
                    assert_eq!(weight, &0, "Weight should be 0");
                    assert_eq!(quality, &0, "Quality should be 0");
                    chance_entry.chance = 1.0;
                    guaranteed_essence_entries.push(chance_entry);
                }
            }
            _ => {
                let quality = entry.get_quality();
                match lowest_non_essence_quality {
                    None => { lowest_non_essence_quality = Some(quality); }
                    Some(lowest) => {
                        if quality < lowest {
                            lowest_non_essence_quality = Some(quality);
                        }
                    }
                }

                if let Some(selected_item_data) = &rng_meter_data.selected_item {
                    // comparing by strings lets it easily check for the same item type (even if the entries are technically different, due to the fact
                    // that the rng meter options are taken from the highest tier chest of the floor, so if you're in a say, obsidian chest with bedrock
                    // loot selected, then that bedrock chest's option needs to be valid for the obsidian chest equivalent entry)
                    if selected_item_data.identifier.eq(&entry.to_string()) {
                        let multiplier = 1.0 + (2.0 * rng_meter_data.selected_xp as f32 / selected_item_data.required_xp as f32).min(2.0) as f64;
                        chance_entry.used_weight *= multiplier;

                        // only guarantee the drop in the lowest tier chest
                        if multiplier >= 3.0 && &selected_item_data.lowest_tier_chest_entry == entry {
                            chance_entry.chance = 1.0;
                            chance_entry.disabled = true;
                            starting_quality -= chance_entry.entry.get_quality();
                        }
                    }
                }

                weighted_entries.push(Rc::new(RefCell::new(chance_entry)));
            }
        };
    }

    let entry_data = Rc::new(RefCell::new(EntryData {
        weighted_entries,
        weighted_essence_entry: weighted_essence_entry.unwrap(),
        leftover_essence_entry: leftover_essence_entry.unwrap(),
        lowest_non_essence_quality: lowest_non_essence_quality.unwrap(),
    }));

    process_random_entries(Rc::clone(&entry_data), &mut recursion_data, 1.0, starting_quality, 0);

    match Rc::try_unwrap(entry_data).map(|refcell| refcell.into_inner()) {
        Ok(data) => {
            let mut results = data.weighted_entries
                .into_iter()
                .filter_map(|rc| Rc::try_unwrap(rc).ok().map(|refcell| refcell.into_inner()))
                .collect::<Vec<LootChanceEntry>>();

            if let Some(leftover) = Rc::try_unwrap(data.weighted_essence_entry).ok().map(|rc| rc.into_inner()) {
                results.push(leftover);
            }

            if let Some(leftover) = Rc::try_unwrap(data.leftover_essence_entry).ok().map(|rc| rc.into_inner()) {
                results.push(leftover);
            }

            for guaranteed_entry in guaranteed_essence_entries {
                results.push(guaranteed_entry);
            }

            sort_entries(&mut results, rng_meter_data.selected_item.as_ref());

            let total_weight = results.iter().map(|e| e.used_weight).sum();
            AveragesCalculationResult { chances: results, total_weight }
        }
        Err(..) => {
            // panic!("Something went wrong");
            AveragesCalculationResult { chances: Vec::new(), total_weight: 0.0 }
        }
    }
}

fn sort_entries(entries: &mut [LootChanceEntry], rng_meter_item: Option<&SelectedRngMeterItem>) {
    let rng_meter_string = rng_meter_item.map_or(String::new(), |e| e.identifier.clone());

    entries.sort_by(|a, b| {
        b.entry.to_string().eq(&rng_meter_string).cmp(&a.entry.to_string().eq(&rng_meter_string))
            .then((a.chance == 0.0).cmp(&(b.chance == 0.0)))
            .then(a.entry.is_essence_and_can_roll_multiple_times().cmp(&b.entry.is_essence_and_can_roll_multiple_times()))
            .then(b.entry.get_quality().cmp(&a.entry.get_quality()))
            .then((b.used_weight.ceil() as i64).cmp(&(a.used_weight.ceil() as i64)))
            .then(a.entry.to_string().cmp(&b.entry.to_string()))
    });
}

fn process_random_entries(entry_data: Rc<RefCell<EntryData>>,
                          recursion_data: &mut RecursiveData,
                          overall_chance: f64,
                          remaining_quality: i16,
                          depth: i32) {
    if overall_chance <= 1e-10 {
        return;
    }

    // disabled entries are still included in usable_entries, since that list only filters by quality, it doesn't consider if they've been used before
    let mut total_weight = 0.0; // usable_entries.total_weight - disabled_entry_total_weight as i32;
    for entry in entry_data.borrow().weighted_entries.iter() {
        let entry = entry.borrow();
        if !entry.disabled && entry.entry.get_quality() <= remaining_quality {
            total_weight += entry.used_weight;
        }
    }

    for entry in entry_data.borrow().weighted_entries.iter() {
        if entry.borrow().disabled || entry.borrow().entry.get_quality() > remaining_quality {
            continue;
        }

        let entry_weight = entry.borrow().used_weight;
        let entry_quality = entry.borrow().entry.get_quality();

        let weight_roll_chance: f64 = entry_weight / total_weight;

        let new_remaining_quality = if entry_quality <= remaining_quality { remaining_quality - entry_quality } else { 0 };
        // println!("entry quality: {}, remaining quality: {}, chance: {} ({}/{})", entry_quality, remaining_quality, weight_roll_chance, entry_weight, total_weight);

        recursion_data.iterations += 1;
        if depth > recursion_data.highest_depth {
            recursion_data.highest_depth = depth;
        }

        if new_remaining_quality > 0 && new_remaining_quality < entry_data.borrow().lowest_non_essence_quality {
            let is_essence_entry = entry.borrow().entry.is_essence_and_can_roll_multiple_times();
            if !is_essence_entry { // essence entry is handled below
                let chance_increase = weight_roll_chance * overall_chance;
                entry.borrow_mut().increase_chance(chance_increase);
            }

            // if the entry rolled here is also the essence entry, then there's no need to pre-subtract quality, as if it
            // were to roll after (since it's not rolling after, it's rolling now)
            let leftover_quality_for_essence = if is_essence_entry { remaining_quality } else { new_remaining_quality };
            let quality_multiplier = (leftover_quality_for_essence as f64 / 10.0).floor();
            let chance_increase = weight_roll_chance * overall_chance * quality_multiplier;
            entry_data.borrow().weighted_essence_entry.borrow_mut().increase_chance(chance_increase);
            // println!("Quality multiplier from {}: {} (new: {})", leftover_quality_for_essence, quality_multiplier, new_remaining_quality);

            // handle final "leftover essence" entry with 1 quality and 0 weight
            let quality_multiplier = ((remaining_quality % 10) as f64).min(10.0);
            let chance_increase = weight_roll_chance * overall_chance * quality_multiplier;
            entry_data.borrow().leftover_essence_entry.borrow_mut().increase_chance(chance_increase);

            continue;
        } else {
            // println!("Iteration B {}, new remaining quality {}", recursion_data.iterations, new_remaining_quality);
            let chance_increase = weight_roll_chance * overall_chance;
            entry.borrow_mut().increase_chance(chance_increase);

            if new_remaining_quality == 0 {
                continue;
            }

            let roll_once = !entry.borrow().entry.is_essence_and_can_roll_multiple_times();
            if roll_once {
                entry.borrow_mut().disabled = true;
                process_random_entries(Rc::clone(&entry_data), recursion_data, chance_increase, new_remaining_quality, depth + 1);
                entry.borrow_mut().disabled = false;
            } else {
                process_random_entries(Rc::clone(&entry_data), recursion_data, chance_increase, new_remaining_quality, depth + 1);
            }
        }
    }
}

#[derive(Clone)]
pub struct RandomlySelectedLootEntry {
    pub entry: Rc<LootEntry>,
    pub used_weight: f64,
    pub total_weight: f64,
    pub before_quality: i16, // subtract by entry.getQuality() to get after
    pub roll_chance: f64,
    pub overall_chance: f64,
}

pub fn generate_random_table(chest: &LootChest, mut quality: i16, rng_meter_data: &RngMeterData) -> Vec<RandomlySelectedLootEntry> {
    let mut rolled_entries: Vec<RandomlySelectedLootEntry> = Vec::new();
    let mut guaranteed_essence_entries = Vec::new();
    
    let mut weighted_entries: Vec<(Rc<LootEntry>, f64)> = Vec::new();

    for entry in &chest.loot {
        let mut weight = entry.get_weight() as f64;
        match entry.as_ref() {
            LootEntry::Essence { weight, quality, .. } => {
                if weight == &0 && quality == &0 {
                    guaranteed_essence_entries.push(RandomlySelectedLootEntry {
                        entry: Rc::clone(entry),
                        used_weight: 0.0,
                        total_weight: 0.0,
                        before_quality: 0,
                        roll_chance: 1.0,
                        overall_chance: 1.0,
                    });
                } else {
                    weighted_entries.push((Rc::clone(entry), *weight as f64));
                }
            }
            _ => {
                if let Some(selected_item_data) = &rng_meter_data.selected_item {
                    if selected_item_data.identifier.eq(&entry.to_string()) {
                        let multiplier = 1.0 + (2.0 * rng_meter_data.selected_xp as f32 / selected_item_data.required_xp as f32).min(2.0) as f64;
                        weight *= multiplier;

                        // only guarantee the drop in the lowest tier chest
                        if multiplier >= 3.0 && &selected_item_data.lowest_tier_chest_entry == entry {
                            rolled_entries.push(RandomlySelectedLootEntry {
                                entry: Rc::clone(entry),
                                used_weight: weight,
                                total_weight: weight,
                                before_quality: quality,
                                roll_chance: 1.0,
                                overall_chance: 1.0,
                            });
                            quality -= entry.get_quality();
                            continue;
                        }
                    }
                }
                weighted_entries.push((Rc::clone(entry), weight));
            }
        };
    }

    // run early here since the rng meter can affect it
    weighted_entries.retain(|(e, _)| quality >= e.get_quality());

    let mut rng = rand::rng();
    let mut index = 0;
    let mut overall_chance_so_far = 1.0;
    
    while quality > 0 && !weighted_entries.is_empty() {
        let table_result = WeightedIndex::new(weighted_entries.iter().map(|item| item.1));
        let (random_index, total_weight) = match table_result {
            Ok(table) => (table.sample(&mut rng), table.total_weight()),
            Err(Error::InvalidWeight) => (0, 0.0), // for the leftover 0 weight entry
            _ => (0, 0.0)
        };
        let random_entry = &weighted_entries[random_index];
        let weight = random_entry.1;
        let iteration_chance = if total_weight == 0.0 { 1.0 } else { weight / total_weight };
        overall_chance_so_far *= iteration_chance;
        index += 1;
        
        let data = RandomlySelectedLootEntry {
            entry: Rc::clone(&random_entry.0),
            used_weight: weight,
            total_weight,
            before_quality: quality,
            roll_chance: iteration_chance,
            overall_chance: overall_chance_so_far,
        };
        
        rolled_entries.push(data);
        quality -= random_entry.0.get_quality();
        
        if !random_entry.0.is_essence_and_can_roll_multiple_times() {
            weighted_entries.remove(random_index);
        }
        weighted_entries.retain(|(e, _)| quality >= e.get_quality());
    }

    for essence_entry in guaranteed_essence_entries {
        rolled_entries.push(essence_entry);
    }

    rolled_entries
}