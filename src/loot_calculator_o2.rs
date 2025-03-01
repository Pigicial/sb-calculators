/*
use crate::loot::{LootChest, LootEntry};
use std::collections::HashMap;
use std::rc::Rc;

pub fn calculate_chances(chest: &LootChest, starting_quality: u16) -> HashMap<Rc<LootEntry>, f64> {
    let mut data = CalculationData::new(starting_quality);
    data.fill_data(chest)
}

pub fn calculate_weight(
    chest: &LootChest,
    treasure_talisman_multiplier: f32,
    boss_luck_increase: u8,
    s_plus: bool,
) -> u16 {
    let base_quality = chest.base_quality;

    let s_plus_multiplier = if s_plus { 1.05 } else { 1.0 };
    let floor_quality: u16 = (base_quality as f32 * s_plus_multiplier).floor() as u16;
    let modified_quality: u16 = ((floor_quality as f32 * treasure_talisman_multiplier)
        + (boss_luck_increase as f32))
        .round() as u16;
    let final_quality: u16 = ((modified_quality as f32 * treasure_talisman_multiplier)
        + (boss_luck_increase as f32))
        .round() as u16;
    final_quality
}

#[derive(Clone)]
struct ChanceData {
    chance: f64,
    disabled: bool,
}

impl ChanceData {
    fn new() -> Self {
        ChanceData {
            chance: 0.0,
            disabled: false,
        }
    }
    fn increase_chance(&mut self, increase: f64) {
        self.chance += increase;
    }
}

struct CalculationData {
    starting_quality: u16,
    iterations: i32,
    highest_depth: i32,
    total_add_ns: u64,
    total_remove_ns: u64,
    total_enumerate_create_ns: u64,
    total_checks_ns: u64,
}

impl CalculationData {
    fn new(starting_quality: u16) -> Self {
        CalculationData {
            starting_quality,
            iterations: 0,
            highest_depth: 0,
            total_add_ns: 0,
            total_remove_ns: 0,
            total_enumerate_create_ns: 0,
            total_checks_ns: 0,
        }
    }

    fn fill_data(&mut self, chest: &LootChest) -> HashMap<Rc<LootEntry>, f64> {
        let mut entries: HashMap<Rc<LootEntry>, ChanceData> = HashMap::new();
        for entry in &chest.loot {
            entries.insert(Rc::clone(entry), ChanceData::new());
        }

        let starting_time = std::time::Instant::now();

        let starting_quality = self.starting_quality;
        self.process_random_entries_recursively(chest, &mut entries, 1.0, starting_quality, 0);
        self.process_guaranteed_entries(&mut entries);

        let duration = starting_time.elapsed();
        let seconds = duration.as_secs_f64();

        println!(
            "Time Taken = {:.4} seconds, iterations = {}, quality = {}, highest = {}, add = {:.9} seconds, remove = {:.9} seconds, checks = {:.9} seconds",
            seconds,
            self.iterations,
            self.starting_quality,
            self.highest_depth,
            self.total_add_ns as f64 / 1_000_000_000.0,
            self.total_remove_ns as f64 / 1_000_000_000.0,
            self.total_checks_ns as f64 / 1_000_000_000.0,
        );

        entries
            .iter()
            .map(|(entry, data)| (Rc::clone(entry), data.chance))
            .collect::<HashMap<Rc<LootEntry>, f64>>()
    }

    fn process_random_entries_recursively(
        &mut self,
        chest: &LootChest,
        entries: &mut HashMap<Rc<LootEntry>, ChanceData>,
        chance_so_far: f64,
        remaining_quality: u16,
        depth: i32,
    ) {
        let mut total_weight = 0;

        // entries pre-filtered by remaining quality
        let start_ns = std::time::Instant::now();

        let usable_entries = &chest.quality_to_weighted_entries[remaining_quality as usize];
        for entry in usable_entries {
            total_weight += entry.get_weight();
        }
        self.total_checks_ns += start_ns.elapsed().as_nanos() as u64;

        for entry in usable_entries {
            let entry_weight = entry.get_weight();
            let entry_quality = entry.get_quality();

            if entry_weight == 0 && total_weight > 0 {
                continue;
            }
            if entry_weight == 0 && entry_quality == 0 {
                continue;
            }

            let mut iteration_roll_chance: f64 = if total_weight == 0 || entry_weight == 0 {
                1.0
            } else {
                (entry_weight / total_weight) as f64
            };

            let mut roll_chance_to_this_iteration = iteration_roll_chance * chance_so_far;
            let mut new_remaining_quality = remaining_quality - entry_quality;

            let mut roll_once = true;

            if entry.is_essence_and_can_roll_multiple_times() {
                roll_once = false;
                if entry_weight == 0 {
                    roll_once = true;
                    if entry_quality > 0 {
                        new_remaining_quality = 0;
                        roll_chance_to_this_iteration =
                            iteration_roll_chance * chance_so_far * remaining_quality as f64;
                    }
                }
            }

            self.iterations += 1;
            if depth > self.highest_depth {
                self.highest_depth = depth;
                println!("new highest depth = {} ({})", depth, self.iterations);
            }

            entries
                .get_mut(entry)
                .unwrap()
                .increase_chance(roll_chance_to_this_iteration);
            if roll_once {
                entries.get_mut(entry).unwrap().disabled = true;
                if entries.len() > 1 {
                    self.process_random_entries_recursively(
                        chest,
                        entries,
                        roll_chance_to_this_iteration,
                        new_remaining_quality,
                        depth + 1,
                    );
                }
                entries.get_mut(entry).unwrap().disabled = false;
            } else {
                self.process_random_entries_recursively(
                    chest,
                    entries,
                    roll_chance_to_this_iteration,
                    new_remaining_quality,
                    depth + 1,
                );
            }
        }
    }

    fn process_guaranteed_entries(&mut self, entries: &mut HashMap<Rc<LootEntry>, ChanceData>) {
        for (entry, chance_data) in entries.iter_mut() {
            if entry.get_quality() == 0 && entry.get_weight() == 0 {
                chance_data.increase_chance(1.0);
            }
        }
    }
}
 */