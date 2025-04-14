use crate::catacombs::catacombs_loot::{ChestType, LootChest, LootEntry};
use crate::catacombs::catacombs_loot_calculator::SuccessfulRollReason::{
    RandomRollBoosted, RandomRollNotBoosted,
};
use crate::catacombs::catacombs_page::CatacombsLootApp;
use rand::distr::weighted::Error;
use rand::prelude::Distribution;
use rand::rngs::ThreadRng;
use rand::Rng;
use rand_distr::weighted::WeightedIndex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::ops::{AddAssign, DivAssign};
use std::rc::Rc;
use csv::Writer;

pub fn calculate_quality(
    chest: &LootChest,
    treasure_talisman_multiplier: f64,
    boss_luck_increase: u8,
    s_plus: bool,
) -> i16 {
    let base_quality = chest.base_quality as f64;

    let s_plus_multiplier = if s_plus { 1.05 } else { 1.0 };
    let floor_quality: f64 = (base_quality * s_plus_multiplier).floor();
    let modified_quality: f64 =
        ((floor_quality * treasure_talisman_multiplier) + (boss_luck_increase as f64)).round();
    let final_rounded_quality: i16 = ((modified_quality * treasure_talisman_multiplier)
        + (boss_luck_increase as f64))
        .round() as i16;
    final_rounded_quality
}

#[derive(Clone, Debug)]
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

#[derive(Hash, PartialEq, Clone)]
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
    pub entries: Vec<LootChanceEntry>,
    pub total_weight: f64,
}

pub fn calculate_average_chances(
    chest: &LootChest,
    mut starting_quality: i16,
    rng_meter_data: &RngMeterData,
) -> AveragesCalculationResult {
    let mut weighted_entries: Vec<Rc<RefCell<LootChanceEntry>>> = Vec::new();

    let mut weighted_essence_entry: Option<Rc<RefCell<LootChanceEntry>>> = None;

    let mut leftover_essence_entry: Option<Rc<RefCell<LootChanceEntry>>> = None;
    let mut guaranteed_essence_entries: Vec<LootChanceEntry> = Vec::new();

    let mut recursion_data: RecursiveData = Default::default();
    let mut lowest_non_essence_quality: Option<i16> = None;

    for entry in &chest.loot {
        let mut chance_entry = LootChanceEntry::new(Rc::clone(entry));

        match entry.as_ref() {
            LootEntry::Essence {
                weight, quality, ..
            } => {
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
                    None => {
                        lowest_non_essence_quality = Some(quality);
                    }
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
                        let multiplier = 1.0
                            + (2.0 * rng_meter_data.selected_xp as f32
                                / selected_item_data.required_xp as f32)
                                .min(2.0) as f64;
                        chance_entry.used_weight *= multiplier;

                        // only guarantee the drop in the lowest tier chest
                        if multiplier >= 3.0 && &selected_item_data.lowest_tier_chest_entry == entry
                        {
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

    process_random_entries(
        Rc::clone(&entry_data),
        &mut recursion_data,
        1.0,
        starting_quality,
        0,
    );

    match Rc::try_unwrap(entry_data).map(|refcell| refcell.into_inner()) {
        Ok(data) => {
            let mut results = data
                .weighted_entries
                .into_iter()
                .filter_map(|rc| Rc::try_unwrap(rc).ok().map(|refcell| refcell.into_inner()))
                .collect::<Vec<LootChanceEntry>>();

            if let Some(leftover) = Rc::try_unwrap(data.weighted_essence_entry)
                .ok()
                .map(|rc| rc.into_inner())
            {
                results.push(leftover);
            }

            if let Some(leftover) = Rc::try_unwrap(data.leftover_essence_entry)
                .ok()
                .map(|rc| rc.into_inner())
            {
                results.push(leftover);
            }

            for guaranteed_entry in guaranteed_essence_entries {
                results.push(guaranteed_entry);
            }

            sort_entries(&mut results, rng_meter_data.selected_item.as_ref());

            let total_weight = results.iter().map(|e| e.used_weight).sum();
            AveragesCalculationResult {
                entries: results,
                total_weight,
            }
        }
        Err(..) => {
            // panic!("Something went wrong");
            AveragesCalculationResult {
                entries: Vec::new(),
                total_weight: 0.0,
            }
        }
    }
}

fn sort_entries(entries: &mut [LootChanceEntry], rng_meter_item: Option<&SelectedRngMeterItem>) {
    let rng_meter_string = rng_meter_item.map_or(String::new(), |e| e.identifier.clone());

    entries.sort_by(|a, b| {
        b.entry
            .to_string()
            .eq(&rng_meter_string)
            .cmp(&a.entry.to_string().eq(&rng_meter_string))
            .then((a.chance == 0.0).cmp(&(b.chance == 0.0)))
            .then(
                a.entry
                    .is_essence_and_can_roll_multiple_times()
                    .cmp(&b.entry.is_essence_and_can_roll_multiple_times()),
            )
            .then(b.entry.get_quality().cmp(&a.entry.get_quality()))
            .then((b.used_weight.ceil() as i64).cmp(&(a.used_weight.ceil() as i64)))
            .then(a.entry.to_string().cmp(&b.entry.to_string()))
    });
}

fn process_random_entries(
    entry_data: Rc<RefCell<EntryData>>,
    recursion_data: &mut RecursiveData,
    overall_chance: f64,
    remaining_quality: i16,
    depth: i32,
) {
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

        let new_remaining_quality = if entry_quality <= remaining_quality {
            remaining_quality - entry_quality
        } else {
            0
        };
        // println!("entry quality: {}, remaining quality: {}, chance: {} ({}/{})", entry_quality, remaining_quality, weight_roll_chance, entry_weight, total_weight);

        recursion_data.iterations += 1;
        if depth > recursion_data.highest_depth {
            recursion_data.highest_depth = depth;
        }

        if new_remaining_quality > 0
            && new_remaining_quality < entry_data.borrow().lowest_non_essence_quality
        {
            let is_essence_entry = entry
                .borrow()
                .entry
                .is_essence_and_can_roll_multiple_times();
            if !is_essence_entry {
                // essence entry is handled below
                let chance_increase = weight_roll_chance * overall_chance;
                entry.borrow_mut().increase_chance(chance_increase);
            }

            // if the entry rolled here is also the essence entry, then there's no need to pre-subtract quality, as if it
            // were to roll after (since it's not rolling after, it's rolling now)
            let leftover_quality_for_essence = if is_essence_entry {
                remaining_quality
            } else {
                new_remaining_quality
            };
            let quality_multiplier = (leftover_quality_for_essence as f64 / 10.0).floor();
            let chance_increase = weight_roll_chance * overall_chance * quality_multiplier;
            entry_data
                .borrow()
                .weighted_essence_entry
                .borrow_mut()
                .increase_chance(chance_increase);
            // println!("Quality multiplier from {}: {} (new: {})", leftover_quality_for_essence, quality_multiplier, new_remaining_quality);

            // handle final "leftover essence" entry with 1 quality and 0 weight
            let quality_multiplier = ((remaining_quality % 10) as f64).min(10.0);
            let chance_increase = weight_roll_chance * overall_chance * quality_multiplier;
            entry_data
                .borrow()
                .leftover_essence_entry
                .borrow_mut()
                .increase_chance(chance_increase);

            continue;
        } else {
            // println!("Iteration B {}, new remaining quality {}", recursion_data.iterations, new_remaining_quality);
            let chance_increase = weight_roll_chance * overall_chance;
            entry.borrow_mut().increase_chance(chance_increase);

            if new_remaining_quality == 0 {
                continue;
            }

            let roll_once = !entry
                .borrow()
                .entry
                .is_essence_and_can_roll_multiple_times();
            if roll_once {
                entry.borrow_mut().disabled = true;
                process_random_entries(
                    Rc::clone(&entry_data),
                    recursion_data,
                    chance_increase,
                    new_remaining_quality,
                    depth + 1,
                );
                entry.borrow_mut().disabled = false;
            } else {
                process_random_entries(
                    Rc::clone(&entry_data),
                    recursion_data,
                    chance_increase,
                    new_remaining_quality,
                    depth + 1,
                );
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

pub fn generate_random_table(
    chest: &LootChest,
    mut quality: i16,
    rng_meter_data: &RngMeterData,
) -> Vec<RandomlySelectedLootEntry> {
    let mut rolled_entries: Vec<RandomlySelectedLootEntry> = Vec::new();
    let mut guaranteed_essence_entries = Vec::new();

    let mut weighted_entries: Vec<(Rc<LootEntry>, f64)> = Vec::new();

    for entry in &chest.loot {
        let mut weight = entry.get_weight() as f64;
        match entry.as_ref() {
            LootEntry::Essence {
                weight, quality, ..
            } => {
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
                        let multiplier = 1.0
                            + (2.0 * rng_meter_data.selected_xp as f32
                                / selected_item_data.required_xp as f32)
                                .min(2.0) as f64;
                        weight *= multiplier;

                        // only guarantee the drop in the lowest tier chest
                        if multiplier >= 3.0 && &selected_item_data.lowest_tier_chest_entry == entry
                        {
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
    let mut overall_chance_so_far = 1.0;

    while quality > 0 && !weighted_entries.is_empty() {
        let table_result = WeightedIndex::new(weighted_entries.iter().map(|item| item.1));
        let (random_index, total_weight) = match table_result {
            Ok(table) => (table.sample(&mut rng), table.total_weight()),
            Err(Error::InvalidWeight) => (0, 0.0), // for the leftover 0 weight entry
            _ => (0, 0.0),
        };
        let random_entry = &weighted_entries[random_index];
        let weight = random_entry.1;
        let iteration_chance = if total_weight == 0.0 {
            1.0
        } else {
            weight / total_weight
        };
        overall_chance_so_far *= iteration_chance;

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

#[derive(Default, Debug)]
pub struct RngMeterCalculation {
    pub total_rolls: f64,
    pub total_rolls_from_maxed_rng_meter: f64,
    pub total_rolls_from_random_rolls_boosted: f64,
    pub total_rolls_from_random_rolls_unboosted: f64,
    pub average_entry_roll_weight: f64,
    pub average_entry_roll_chance: f64,
}

impl AddAssign for RngMeterCalculation {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            total_rolls: self.total_rolls + other.total_rolls,
            total_rolls_from_maxed_rng_meter: self.total_rolls_from_maxed_rng_meter + other.total_rolls_from_maxed_rng_meter,
            total_rolls_from_random_rolls_boosted: self.total_rolls_from_random_rolls_boosted + other.total_rolls_from_random_rolls_boosted,
            total_rolls_from_random_rolls_unboosted: self.total_rolls_from_random_rolls_unboosted + other.total_rolls_from_random_rolls_unboosted,
            average_entry_roll_weight: self.average_entry_roll_weight + other.average_entry_roll_weight,
            average_entry_roll_chance: self.average_entry_roll_chance + other.average_entry_roll_chance,
        };
    }
}

impl DivAssign<i32> for RngMeterCalculation {
    fn div_assign(&mut self, divider: i32) {
        *self = Self {
            total_rolls: self.total_rolls / divider as f64,
            total_rolls_from_maxed_rng_meter: self.total_rolls_from_maxed_rng_meter / divider as f64,
            total_rolls_from_random_rolls_boosted: self.total_rolls_from_random_rolls_boosted / divider as f64,
            total_rolls_from_random_rolls_unboosted: self.total_rolls_from_random_rolls_unboosted / divider as f64,
            average_entry_roll_weight: self.average_entry_roll_weight / divider as f64,
            average_entry_roll_chance: self.average_entry_roll_weight / divider as f64,
        };
    }
}

pub enum SuccessfulRollReason {
    RandomRollNotBoosted(ChanceAndWeight),
    RandomRollBoosted(ChanceAndWeight),
    MaxedRngMeter,
}

/*
pub fn calculate_meter_item_random_roll_chance(
    chest_data: &[(Rc<LootChest>, HashMap<i32, f64>)],
    calc: &CatacombsLootApp,
    runs: i32,
    average_score: i32,
    meter_deselection_threshold: f32,
)-> Result<f64, String> {
    let mut result: RngMeterCalculation = Default::default();
    if calc.rng_meter_data.selected_item.is_none() {
        return Err("No selected item for the RNG meter".to_string());
    }

    let mut rng = rand::rng();
    let mut meter_xp = calc.rng_meter_data.selected_xp;
    let meter_data = calc.rng_meter_data.selected_item.as_ref().unwrap();
    let use_kismets = calc.rng_meter_calculation_use_kismet_feathers;

    let per_run_score_increase = match average_score {
        s if s >= 300 => s,
        s if s >= 270 => (s as f64 * 0.7) as i32,
        _ => 0,
    };

    let meter_xp_amounts = chest_data.first().unwrap().1.keys().clone();
    let mut chance_to_not_roll = 1;
    for meter_xp in meter_xp_amounts {
        let use_meter = (*meter_xp as f32 / meter_data.required_xp as f32) < meter_deselection_threshold;

        for (chest, chances) in chest_data.iter() {
            let chance = if use_meter {
                chances.get(&meter_xp).unwrap();
            } else {
                chances.get(&0).unwrap();
            };
        }

    }

    for _ in 0..runs {
        let mut new_meter_xp = None;
        let use_meter = (meter_xp as f32 / meter_data.required_xp as f32) < meter_deselection_threshold;

        for data in chest_data.iter() {
            let chest = &data.0;
            let chances = &data.1;

            let mut roll = roll_item(chest, chances, meter_xp, meter_data, use_meter, &mut rng);
            if roll.is_none() && use_kismets && chest.chest_type == meter_data.highest_tier_chest_type {
                roll = roll_item(chest, chances, meter_xp, meter_data, use_meter, &mut rng);
            }
            match roll {
                Some(any) => {
                    result.total_rolls += 1.0;
                    match any {
                        RandomRollNotBoosted => {
                            result.total_rolls_from_random_rolls_unboosted += 1.0;
                            if use_meter && new_meter_xp.is_none() {
                                // is none check so the version below (maxed rng meter) can override it
                                new_meter_xp = Some(0);
                            }
                        }
                        RandomRollBoosted => {
                            result.total_rolls_from_random_rolls_boosted += 1.0;
                            if use_meter && new_meter_xp.is_none() {
                                // is none check so the version below (maxed rng meter) can override it
                                new_meter_xp = Some(0);
                            }
                        }
                        SuccessfulRollReason::MaxedRngMeter => {
                            result.total_rolls_from_maxed_rng_meter += 1.0;
                            new_meter_xp = Some(0); // todo: change to Some(meter_xp - meter_data.required_xp)
                        }
                    }
                }
                None => continue,
            }
        }

        if let Some(new_meter_xp) = new_meter_xp {
            meter_xp = new_meter_xp;
        }
        // xp is added after rolling :shrug:
        meter_xp += per_run_score_increase;
    }

    Ok(result)
}
 */

pub fn calculate_amount_of_times_rolled_for_entry(
    chest_data: &[(Rc<LootChest>, HashMap<i32, ChanceAndWeight>)],
    calc: &CatacombsLootApp,
    runs: i32,
    average_score: i32,
    meter_deselection_threshold: f32,
) -> Result<RngMeterCalculation, String> {
    let mut result: RngMeterCalculation = Default::default();
    if calc.rng_meter_data.selected_item.is_none() {
        return Err("No selected item for the RNG meter".to_string());
    }

    let mut added_up_roll_chances = 0.0;
    let mut added_up_roll_weights = 0.0;
    
    let mut rng = rand::rng();
    let mut meter_xp = calc.rng_meter_data.selected_xp;
    let meter_data = calc.rng_meter_data.selected_item.as_ref().unwrap();
    let use_kismets = calc.rng_meter_calculation_use_kismet_feathers;

    let per_run_score_increase = match average_score {
        s if s >= 300 => s,
        s if s >= 270 => (s as f64 * 0.7) as i32,
        _ => 0,
    };

    for _ in 0..runs {
        let mut new_meter_xp = None;
        let use_meter = (meter_xp as f32 / meter_data.required_xp as f32) < meter_deselection_threshold;

        for data in chest_data.iter() {
            let chest = &data.0;
            let chances = &data.1;

            let mut roll = roll_item(chest, chances, meter_xp, meter_data, use_meter, &mut rng);
            if roll.is_none() && use_kismets && chest.chest_type == meter_data.highest_tier_chest_type {
                roll = roll_item(chest, chances, meter_xp, meter_data, use_meter, &mut rng); 
            }
            match roll {
                Some(any) => {
                    result.total_rolls += 1.0;
                    match any {
                        RandomRollNotBoosted(data) => {
                            result.total_rolls_from_random_rolls_unboosted += 1.0;
                            if use_meter && new_meter_xp.is_none() {
                                // is none check so the version below (maxed rng meter) can override it
                                new_meter_xp = Some(0);
                            }

                            added_up_roll_chances += data.chance;
                            added_up_roll_weights += data.weight;
                        }
                        RandomRollBoosted(data) => {
                            result.total_rolls_from_random_rolls_boosted += 1.0;
                            if use_meter && new_meter_xp.is_none() {
                                // is none check so the version below (maxed rng meter) can override it
                                new_meter_xp = Some(0);
                            }

                            added_up_roll_chances += data.chance;
                            added_up_roll_weights += data.weight;
                        }
                        SuccessfulRollReason::MaxedRngMeter => {
                            result.total_rolls_from_maxed_rng_meter += 1.0;
                            new_meter_xp = Some(0); // todo: change to Some(meter_xp - meter_data.required_xp)
                        }
                    }
                }
                None => continue,
            }
        }

        if let Some(new_meter_xp) = new_meter_xp {
            meter_xp = new_meter_xp;
        }
        // xp is added after rolling :shrug:
        meter_xp += per_run_score_increase;
    }

    result.average_entry_roll_weight = added_up_roll_weights / result.total_rolls;
    result.average_entry_roll_chance = added_up_roll_chances / result.total_rolls;

    Ok(result)
}

fn roll_item(
    chest: &Rc<LootChest>,
    chances: &HashMap<i32, ChanceAndWeight>,
    meter_xp: i32,
    meter_data: &SelectedRngMeterItem,
    use_meter: bool,
    rng: &mut ThreadRng,
) -> Option<SuccessfulRollReason> {
    if meter_xp >= meter_data.required_xp && meter_data.lowest_tier_chest_type == chest.chest_type {
        Some(SuccessfulRollReason::MaxedRngMeter)
    } else {
        let random_number: f64 = rng.random();

        if use_meter {
            let boosted_chance_data = chances.get(&meter_xp).unwrap();

            let boosted_chance = boosted_chance_data.chance;
            let unboosted_chance = chances.get(&0).unwrap().chance;
            if random_number <= boosted_chance {
                if random_number <= unboosted_chance {
                    Some(RandomRollNotBoosted(boosted_chance_data.clone()))
                } else {
                    Some(RandomRollBoosted(boosted_chance_data.clone()))
                }
            } else {
                None
            }
        } else {
            let chance_and_weight = chances.get(&0).unwrap();
            if random_number <= chance_and_weight.chance {
                Some(RandomRollNotBoosted(chance_and_weight.clone()))
            } else {
                None
            }
        }
    }
}

#[derive(Clone)]
pub struct ChanceAndWeight {
    chance: f64,
    weight: f64,
}

pub fn cache_chances_per_rng_meter_value(
    chest: &LootChest,
    quality: i16,
    starting_meter_score: i32,
    score_increase: i32,
    meter_data: &SelectedRngMeterItem,
) -> HashMap<i32, ChanceAndWeight> {
    let scores_to_cache = generate_possible_rng_meter_scores(
        starting_meter_score,
        score_increase,
        meter_data.required_xp,
    );
    let mut cached_chances = HashMap::new();

    for meter_score in scores_to_cache {
        println!("Caching score {}", meter_score);
        let chances = calculate_average_chances(
            chest,
            quality,
            &RngMeterData {
                selected_item: Some(meter_data.clone()),
                selected_xp: meter_score,
            },
        );
        println!("Successfully cached the data for score {}", meter_score);

        let entry = chances
            .entries
            .iter()
            .find(|e| e.entry.to_string() == meter_data.identifier)
            .unwrap();
        cached_chances.insert(meter_score, ChanceAndWeight {
            chance: entry.chance,
            weight: entry.used_weight
        });
    }

    /*
    if let Ok(file) = File::create("page_timestamps.csv") {
        let mut writer = Writer::from_writer(file);
    }
     */


    cached_chances
}

fn generate_possible_rng_meter_scores(
    starting_score: i32,
    per_run_increase: i32,
    max_value: i32,
) -> Vec<i32> {
    let mut values = Vec::new();

    // Generate increasing values
    let mut value = starting_score;
    while value <= max_value {
        values.push(value);
        value += per_run_increase;
    }

    // Generate decreasing values
    value = starting_score;
    while value >= 0 {
        values.push(value);
        value -= per_run_increase;
    }

    values.push(0);
    values.sort();
    values.dedup(); // Remove duplicates in case starting_value is min or max
    values
}
