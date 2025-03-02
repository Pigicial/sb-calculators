use crate::loot::{LootChest, LootEntry};
use std::cell::RefCell;
use std::rc::Rc;

pub fn calculate_weight(chest: &LootChest, treasure_talisman_multiplier: f32, boss_luck_increase: u8, s_plus: bool) -> i16 {
    let base_quality = chest.base_quality;

    let s_plus_multiplier = if s_plus { 1.05 } else { 1.0 };
    let floor_quality: i16 = (base_quality as f32 * s_plus_multiplier).floor() as i16;
    let modified_quality: i16 = ((floor_quality as f32 * treasure_talisman_multiplier) + (boss_luck_increase as f32)).round() as i16;
    let final_quality: i16 = ((modified_quality as f32 * treasure_talisman_multiplier) + (boss_luck_increase as f32)).round() as i16;
    final_quality
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

#[derive(Default)]
pub struct RngMeterData {
    pub selected_item: Option<Rc<LootEntry>>,
    pub selected_xp: i32,
    pub required_xp: Option<i32>,
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
    iterations_past_d10: i32,
    highest_depth: i32,
    total_add_ns: u64,
    total_remove_ns: u64,
    total_checks_ns: u64,
}

pub struct CalculationResult {
    pub chances: Vec<LootChanceEntry>,
    pub total_weight: f64,
}

pub fn calculate_chances(chest: &LootChest, starting_quality: i16, rng_meter_data: &RngMeterData) -> CalculationResult {
    let mut weighted_entries: Vec<Rc<RefCell<LootChanceEntry>>> = Vec::new();

    let mut weighted_essence_entry: Option<Rc<RefCell<LootChanceEntry>>> = None;

    let mut leftover_essence_entry: Option<Rc<RefCell<LootChanceEntry>>> = None;
    let mut guaranteed_essence_entries: Vec<LootChanceEntry> = Vec::new();

    let mut recursion_data: RecursiveData = Default::default();
    let mut lowest_non_essence_quality: Option<i16> = None;

    for entry in &chest.loot {
        let mut chance_entry = LootChanceEntry::new(Rc::clone(&entry));

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
                if let Some(item) = &rng_meter_data.selected_item {
                    if item.eq(entry) {
                        let multiplier = 1.0 + (2.0 * rng_meter_data.selected_xp as f32 / rng_meter_data.required_xp.unwrap() as f32).min(2.0) as f64;
                        chance_entry.used_weight *= multiplier;
                    }
                }

                weighted_entries.push(Rc::new(RefCell::new(chance_entry)));
                let quality = entry.get_quality();
                match lowest_non_essence_quality {
                    None => { lowest_non_essence_quality = Some(quality); }
                    Some(lowest) => {
                        if quality < lowest {
                            lowest_non_essence_quality = Some(quality);
                        }
                    }
                }
            }
        };
    }

    println!("{}", weighted_entries.len());

    let entry1 = weighted_essence_entry.unwrap();
    let entry_data = Rc::new(RefCell::new(EntryData {
        weighted_entries,
        weighted_essence_entry: entry1,
        leftover_essence_entry: leftover_essence_entry.unwrap(),
        lowest_non_essence_quality: lowest_non_essence_quality.unwrap(),
    }));

    // let starting_time = std::time::Instant::now();
    process_random_entries(chest, Rc::clone(&entry_data), &mut recursion_data, 1.0, starting_quality, 0);

    /*
    println!(
        "Time Taken = {:.6} seconds, iterations = {}, iterations past d10: {}, highest = {}, add = {:.9} seconds, remove = {:.9} seconds, checks = {:.9} seconds, lowest: {}",
        starting_time.elapsed().as_secs_f64(),
        recursion_data.iterations,
        recursion_data.iterations_past_d10,
        recursion_data.highest_depth,
        recursion_data.total_add_ns as f64 / 1_000_000_000.0,
        recursion_data.total_remove_ns as f64 / 1_000_000_000.0,
        recursion_data.total_checks_ns as f64 / 1_000_000_000.0,
        entry_data.borrow().lowest_non_essence_quality
    );
     */

    match Rc::try_unwrap(entry_data).map(|refcell| refcell.into_inner()) {
        Ok(data) => {
            let mut results = data.weighted_entries
                .into_iter()
                .filter_map(|rc| Rc::try_unwrap(rc).ok().map(|refcell| refcell.into_inner()))
                .collect::<Vec<LootChanceEntry>>();

            if let Some(leftover) = Rc::try_unwrap(data.leftover_essence_entry).ok().map(|rc| rc.into_inner()) {
                results.push(leftover);
            }

            for guaranteed_entry in guaranteed_essence_entries {
                results.push(guaranteed_entry);
            }

            sort_entries(&mut results, rng_meter_data.selected_item.as_ref());

            let total_weight = results.iter().map(|e| e.used_weight).sum();
            CalculationResult { chances: results, total_weight }
        }
        Err(..) => {
            // panic!("Something went wrong");
            CalculationResult { chances: Vec::new(), total_weight: 0.0 }
        }
    }
}

pub fn sort_entries(entries: &mut[LootChanceEntry], rng_meter_item: Option<&Rc<LootEntry>>) {
    entries.sort_by(|a, b| {
        Some(&b.entry).eq(&rng_meter_item).cmp(&Some(&a.entry).eq(&rng_meter_item))
            .then(a.entry.is_essence_and_can_roll_multiple_times().cmp(&b.entry.is_essence_and_can_roll_multiple_times()))
            .then(b.entry.get_quality().cmp(&a.entry.get_quality()))
            .then((a.used_weight.ceil() as i64).cmp(&(b.used_weight.ceil() as i64)))
            .then(a.entry.to_string().cmp(&b.entry.to_string()))
    });
}

fn process_random_entries(chest: &LootChest,
                          entry_data: Rc<RefCell<EntryData>>,
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

            if entry.borrow().entry.is_debug_item() {
                recursion_data.iterations_past_d10 += 1;
                //println!("Essence ({:?}), has fish, rq = {}, nrq = {}", entry, remaining_quality, new_remaining_quality);
            }

            if new_remaining_quality == 0 {
                // println!("Iteration C {}, new remaining quality {}, chance: {}", recursion_data.iterations, new_remaining_quality, chance_increase);
                continue;
            }

            // TODO: fix disabled_entry_total_weight "double removing" - aka the list of valid entries won't contain an entry, so there's no chance its weight
            // TOOD: can be included in the total calculation anyways, but then it's still subtracted by the disabled_entry_total_weight value

            let roll_once = !entry.borrow().entry.is_essence_and_can_roll_multiple_times();
            if roll_once {
                entry.borrow_mut().disabled = true;
                process_random_entries(chest, Rc::clone(&entry_data), recursion_data, chance_increase, new_remaining_quality, depth + 1);
                entry.borrow_mut().disabled = false;
            } else {
                process_random_entries(chest, Rc::clone(&entry_data), recursion_data, chance_increase, new_remaining_quality, depth + 1);
            }
        }
    }
}