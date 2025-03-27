/*
    // todo IDEA:
    // - save every chance combination to obtain an entry in the form of its weight / totalWeight and iterationRollChance
    // with those, the weight can be increased, and then the iterationRollChance can be increased by newWeight - weight (difference)
    // then those chances can be recalculated and added together
    // so basically instead of a weight table at every point, just go off these chances and do averages

    // (10 / 50) * 1
    // (10 / 75) * 0.4
    // (10 / 184) * 0.25
    // (10 / 64) * 0.3384

    // (10/50) + ((10/75)*0.4) + ((10/184)*0.25) + ((10/64)*0.384)               = 0.326920289855
    // 50% increase: (15/55) + ((15/80)*0.4) + ((15/189)*0.25) + ((15/69)*0.384) = 0.451046803438
    // 10% increase: (20/60) + ((20/85)*0.4) + ((20/194)*0.25) + ((20/74)*0.384) = 0.557007960052

    // 50+75+184+64 = 373  /  4  =  93.25

    // TODO: just realized a thing
    // this might not work because the iteration chance multipliers are based on the previous weight/totalWeight values, but if the whole point is changing all of those
    // then the iteration chance multipliers need to be changed too

use std::cell::RefCell;
use std::rc::Rc;
use crate::catacombs::catacombs_loot::LootChest;

#[derive(Default)]
struct WeightCalculationData {
    base_entry_weight: i16,
    entry_calculations: Vec<WeightCalculation>,
}

impl WeightCalculationData {
    fn calculate_chance(&self, rng_meter_weight_multiplier: f64) -> f64 {
        let new_weight = self.base_entry_weight as f64 * rng_meter_weight_multiplier;
        let difference = new_weight - self.base_entry_weight as f64;

        let mut added_chances = 0.0;
        for calculation in self.entry_calculations.iter() {
            added_chances += calculation.calculate_chance(self.base_entry_weight, difference);
        }
        added_chances
    }
}

struct WeightCalculation {
    total_weight: i32,
    iteration_chance_multiplier: f64,
    // TODO: just realized a thing
    // this might not work because the iteration chance multipliers are based on the previous weight/totalWeight values, but if the whole point is changing all of those
    // then the iteration chance multipliers need to be changed too
}

impl WeightCalculation {
    fn calculate_chance(&self, base_entry_weight: i16, extra_weight: f64) -> f64 {
        if extra_weight == 0.0 {
            (base_entry_weight as f64 / self.total_weight as f64) * self.iteration_chance_multiplier
        } else {
            let new_weight = base_entry_weight as f64 + extra_weight;
            let new_total_weight = self.total_weight as f64 + extra_weight;
            (new_weight / new_total_weight) * self.iteration_chance_multiplier
        }
    }
}

/// Generates the data needed to manually calculate the chance for a given entry and any weight value it has.
fn generate_weight_calculations(chest: &LootChest, entry: String, starting_quality: i16) -> WeightCalculationData {
    let data = Default::default();

    let entry_quality = chest.get_matching_entry_quality(&entry).unwrap();



    data
}

fn process_weight_calculations(entry_data: Rc<RefCell<EntryData>>,
                               identifier: &String,
                               overall_chance: f64,
                               remaining_quality: i16) {
    if overall_chance <= 1e-10 {
        return;
    }

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

        if new_remaining_quality > 0 && new_remaining_quality < entry_data.borrow().lowest_non_essence_quality {
            continue;
        }
        // println!("Iteration B {}, new remaining quality {}", recursion_data.iterations, new_remaining_quality);
        let chance_increase = weight_roll_chance * overall_chance;
        entry.borrow_mut().increase_chance(chance_increase);

        if new_remaining_quality == 0 {
            continue;
        }

        let roll_once = !entry.borrow().entry.is_essence_and_can_roll_multiple_times();
        if roll_once {
            entry.borrow_mut().disabled = true;
            process_weight_calculations(Rc::clone(&entry_data), identifier, chance_increase, new_remaining_quality);
            entry.borrow_mut().disabled = false;
        } else {
            process_weight_calculations(Rc::clone(&entry_data), identifier, chance_increase, new_remaining_quality);
        }
    }
}
*/
