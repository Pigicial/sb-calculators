/*
use crate::loot::{LootChest, LootEntry};
use std::collections::{HashMap};

pub fn calculate_chances(chest: &LootChest, starting_quality: u16) -> HashMap<LootEntry, f64> {
    let data = CalculationData::new(chest, starting_quality);
    fill_data(data)
}

pub fn calculate_weight(chest: &LootChest, treasure_talisman_multiplier: f32, boss_luck_increase: u8, s_plus: bool) -> u16 {
    let base_quality = chest.base_quality;

    let s_plus_multiplier = if s_plus { 1.05 } else { 1.0 };
    let floor_quality: u16 = (base_quality as f32 * s_plus_multiplier).floor() as u16;
    let modified_quality: u16 = ((floor_quality as f32 * treasure_talisman_multiplier) + (boss_luck_increase as f32)).round() as u16;
    let final_quality: u16 = ((modified_quality as f32 * treasure_talisman_multiplier) + (boss_luck_increase as f32)).round() as u16;
    final_quality
}

#[derive(Clone)]
struct ChanceData {
    pub entry: LootEntry,
    chance: f64,
    disabled: bool,
}

impl ChanceData {
    fn new(entry: LootEntry) -> Self {
        ChanceData { entry, chance: 0.0, disabled: false }
    }
    fn increase_chance(&mut self, increase: f64) {
        self.chance += increase;
    }

    fn get_entry(&mut self) -> &mut LootEntry {
        &mut self.entry
    }
}

struct CalculationData {
    chest: LootChest,
    entries: Vec<ChanceData>,
    starting_quality: u16,
    iterations: i32,
    highest_depth: i32,
    total_add_ns: u64,
    total_remove_ns: u64,
    total_enumerate_create_ns: u64,
    total_checks_ns: u64,
}

impl CalculationData {
    fn new(chest: &LootChest, starting_quality: u16) -> Self {
        let entries = chest.loot.iter().map(|entry| {
            ChanceData::new(entry.clone())
        }).collect();

        CalculationData {
            chest: chest.clone(),
            entries,
            starting_quality,
            iterations: 0,
            highest_depth: 0,
            total_add_ns: 0,
            total_remove_ns: 0,
            total_enumerate_create_ns: 0,
            total_checks_ns: 0,
        }
    }

    fn get_entry(&mut self, index: usize) -> &mut ChanceData {
        &mut self.entries[index]
    }
}

fn fill_data(mut data: CalculationData) -> HashMap<LootEntry, f64> {
    data.entries.clear();
    for entry in &data.chest.loot {
        data.entries.push(ChanceData::new(entry.clone()));
    }

    let starting_time = std::time::Instant::now();

    let starting_quality = data.starting_quality;
    process_random_entries_recursively(&mut data, 1.0, starting_quality, 0);
    process_guaranteed_entries(&mut data);

    let duration = starting_time.elapsed();
    let seconds = duration.as_secs_f64();
    println!(
        "Time Taken = {:.4} seconds, iterations = {}, quality = {}, highest = {}, add = {:.9} seconds, remove = {:.9} seconds, checks = {:.9} seconds",
        seconds,
        data.iterations,
        data.starting_quality,
        data.highest_depth,
        data.total_add_ns as f64 / 1_000_000_000.0,
        data.total_remove_ns as f64 / 1_000_000_000.0,
        data.total_checks_ns as f64 / 1_000_000_000.0,
    );

    data.entries.clone().iter().map(|entry| {
        (entry.entry.clone(), entry.chance)
    }).collect::<HashMap<LootEntry, f64>>()
}

fn get_entry(data: &mut CalculationData, index: usize) -> &mut ChanceData {
    &mut data.entries[index]
}


fn process_random_entries_recursively(data: &mut CalculationData, chance_so_far: f64, remaining_quality: u16, depth: i32) {
    let mut total_weight = 0;

    for entry_data in &data.entries {
        let entry = &entry_data.entry;
        if entry.get_quality() <= remaining_quality {
            total_weight += entry.get_weight();
        }
    }

    for i in 0..data.entries.len() {
        if data.entries[i].disabled { continue; }

        let entry = &data.entries[i].entry;
        let entry_weight = entry.get_weight();
        let entry_quality = entry.get_quality();

        if entry_quality > remaining_quality { continue; }
        if entry_weight == 0 && total_weight > 0 { continue; }
        if entry_weight == 0 && entry_quality == 0 { continue; }

        let start_ns = std::time::Instant::now();

        let mut iteration_roll_chance: f64 = if total_weight == 0 {
            1.0
        } else {
            (entry_weight / total_weight) as f64
        };

        if entry_weight == 0 {
            iteration_roll_chance = 1.0;
        }

        let mut roll_chance_to_this_iteration = iteration_roll_chance * chance_so_far;
        let mut new_remaining_quality = remaining_quality - entry_quality;

        let mut roll_once = true;

        if entry.is_essence_and_can_roll_multiple_times() {
            roll_once = false;
            if entry_weight == 0 {
                roll_once = true;
                if entry_quality > 0 {
                    new_remaining_quality = 0;
                    roll_chance_to_this_iteration = iteration_roll_chance * chance_so_far * remaining_quality as f64;
                }
            }
        }

        data.entries[i].increase_chance(roll_chance_to_this_iteration);

        data.total_checks_ns += start_ns.elapsed().as_nanos() as u64;
        data.iterations += 1;
        if depth > data.highest_depth {
            data.highest_depth = depth;
            println!("new highest depth = {} ({})", depth, data.iterations);
        }

        //if roll_chance_to_this_iteration > 0.0 && roll_chance_to_this_iteration < 1e-9 {
        //    return;
        //}

        if roll_once {
            data.entries[i].disabled = true;
            if data.entries.len() > 1 {
                process_random_entries_recursively(data, roll_chance_to_this_iteration, new_remaining_quality, depth + 1);
            }
            data.entries[i].disabled = false;
        } else {
            process_random_entries_recursively(data, roll_chance_to_this_iteration, new_remaining_quality, depth + 1);
        }
    }
}

fn process_guaranteed_entries(data: &mut CalculationData) {
    for entry_data in &mut data.entries {
        let entry = entry_data.get_entry();
        if entry.get_quality() == 0 && entry.get_weight() == 0 {
            entry_data.increase_chance(1.0);
        }
    }
}
 */
