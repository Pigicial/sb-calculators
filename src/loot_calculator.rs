use crate::loot::{LootChest, LootEntry};
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
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
struct ChanceData {
    chance: f64,
    disabled: bool,
}

impl ChanceData {
    fn new() -> Self {
        ChanceData { chance: 0.0, disabled: false }
    }
    fn increase_chance(&mut self, increase: f64) {
        self.chance += increase;
    }
}

#[derive(Default)]
pub struct RngMeterData {
    pub rng_meter_item: Option<Rc<LootEntry>>,
    pub rng_meter_required_xp: Option<f32>,
    pub rng_meter_selected_xp: Option<f32>,
}

struct EntryData {
    weighted_entries: HashMap<Rc<LootEntry>, Rc<RefCell<ChanceData>>>,
    weighted_essence_entry: (Rc<LootEntry>, Rc<RefCell<ChanceData>>),
    leftover_essence_entry: (Rc<LootEntry>, ChanceData),
    guaranteed_essence_entries: Vec<(Rc<LootEntry>, ChanceData)>,
    lowest_non_essence_quality: i16,
}

impl EntryData {
    fn get_weighted_data(&mut self, entry: &Rc<LootEntry>) -> RefMut<ChanceData> {
        self.weighted_entries.get(entry).unwrap().borrow_mut()
    }
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

struct CalculationResult {
    chances: Vec<(Rc<LootEntry>, f64)>,
    total_weight: f64,
}


pub fn calculate_chances(chest: &LootChest, starting_quality: i16) -> CalculationResult {
    let mut weighted_entries: HashMap<Rc<LootEntry>, Rc<RefCell<ChanceData>>> = HashMap::new();

    let weighted_essence_entry_chance_data: Rc<RefCell<ChanceData>> = Rc::new(RefCell::new(ChanceData::new()));
    let mut weighted_essence_entry: Option<(Rc<LootEntry>, Rc<RefCell<ChanceData>>)> = None;

    let mut leftover_essence_entry: Option<(Rc<LootEntry>, ChanceData)> = None;
    let mut guaranteed_essence_entries: Vec<(Rc<LootEntry>, ChanceData)> = Vec::new();

    let mut recursion_data: RecursiveData = Default::default();
    let mut lowest_non_essence_quality: Option<i16> = None;

    for entry in &chest.loot {
        match entry.as_ref() {
            LootEntry::Essence { weight, quality, .. } => {
                if weight > &0 && quality > &0 {
                    weighted_entries.insert(Rc::clone(entry), Rc::clone(&weighted_essence_entry_chance_data));
                    weighted_essence_entry = Some((Rc::clone(entry), Rc::clone(&weighted_essence_entry_chance_data)));
                } else if weight == &0 && quality == &1 {
                    leftover_essence_entry = Some((Rc::clone(entry), ChanceData::new()));
                } else {
                    assert_eq!(weight, &0, "Weight should be 0");
                    assert_eq!(quality, &0, "Quality should be 0");
                    guaranteed_essence_entries.push((Rc::clone(entry), ChanceData { chance: 1.0, disabled: false }));
                }
            }
            _ => {
                weighted_entries.insert(Rc::clone(entry), Rc::new(RefCell::new(ChanceData::new())));
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

    let total_weight = weighted_entries.iter().map(|e| e.0.get_weight()).sum();
    println!("{}", weighted_entries.len());

    let mut entry_data = EntryData {
        weighted_entries,
        weighted_essence_entry: weighted_essence_entry.unwrap(),
        leftover_essence_entry: leftover_essence_entry.unwrap(),
        guaranteed_essence_entries,
        lowest_non_essence_quality: lowest_non_essence_quality.unwrap(),
    };

    let starting_time = std::time::Instant::now();
    process_random_entries(chest, &mut entry_data, &mut recursion_data, 1.0, starting_quality, 0);

    println!(
        "Time Taken = {:.6} seconds, iterations = {}, iterations past d10: {}, highest = {}, add = {:.9} seconds, remove = {:.9} seconds, checks = {:.9} seconds, lowest: {}",
        starting_time.elapsed().as_secs_f64(),
        recursion_data.iterations,
        recursion_data.iterations_past_d10,
        recursion_data.highest_depth,
        recursion_data.total_add_ns as f64 / 1_000_000_000.0,
        recursion_data.total_remove_ns as f64 / 1_000_000_000.0,
        recursion_data.total_checks_ns as f64 / 1_000_000_000.0,
        entry_data.lowest_non_essence_quality
    );


    let mut results = entry_data.weighted_entries
        .iter()
        .map(|(entry, data)| {
            (Rc::clone(entry), data.borrow_mut().chance)
        }).collect::<HashMap<Rc<LootEntry>, f64>>();

    results.insert(entry_data.leftover_essence_entry.0, entry_data.leftover_essence_entry.1.chance);

    println!("Guaranteed entry size: {}", entry_data.guaranteed_essence_entries.len());
    for (entry, data) in entry_data.guaranteed_essence_entries {
        results.insert(entry, data.chance);
    }

    let mut sorted_results: Vec<(_, _)> = results.into_iter().collect();
    sorted_results.sort_by(|a, b| {
        a.0.is_essence_and_can_roll_multiple_times().cmp(&b.0.is_essence_and_can_roll_multiple_times())
            .then(b.0.get_quality().cmp(&a.0.get_quality()))
            .then(a.0.get_weight().cmp(&b.0.get_weight()))
            .then(a.0.to_string().cmp(&b.0.to_string()))
    });

    CalculationResult { chances: sorted_results, total_weight }
}

fn process_random_entries(chest: &LootChest,
                          entry_data: &mut EntryData,
                          recursion_data: &mut RecursiveData,
                          overall_chance: f64,
                          remaining_quality: i16,
                          depth: i32) {
    
    if overall_chance <= 1e-10 {
        return;
    }

    // entries pre-filtered by remaining quality
    let usable_entries = &chest.quality_to_weighted_entries[remaining_quality as usize];

    // disabled entries are still included in usable_entries, since that list only filters by quality, it doesn't consider if they've been used before
    let mut total_weight = 0; // usable_entries.total_weight - disabled_entry_total_weight as i32;
    for entry in &usable_entries.entries {
        if !entry_data.get_weighted_data(entry).disabled {
            total_weight += entry.get_weight();
        }
    }
    
    // let total_weight = usable_entries.total_weight;
    let usable_entries = &usable_entries.entries;

    for entry in usable_entries {
        if entry_data.get_weighted_data(entry).disabled {
            continue;
        }
        
        let entry_weight = entry.get_weight();
        let entry_quality = entry.get_quality();

        let weight_roll_chance: f64 = (entry_weight as f64) / (total_weight as f64);

        let new_remaining_quality = if entry_quality <= remaining_quality { remaining_quality - entry_quality } else { 0 };
        // println!("entry quality: {}, remaining quality: {}, chance: {} ({}/{})", entry_quality, remaining_quality, weight_roll_chance, entry_weight, total_weight);

        recursion_data.iterations += 1;
        if depth > recursion_data.highest_depth {
            recursion_data.highest_depth = depth;
        }

        if new_remaining_quality > 0 && new_remaining_quality < entry_data.lowest_non_essence_quality {
            let is_essence_entry = entry.is_essence_and_can_roll_multiple_times();
            if !is_essence_entry { // essence entry is handled below
                let chance_increase = weight_roll_chance * overall_chance;
                entry_data.get_weighted_data(entry).increase_chance(chance_increase);
            }

            // if the entry rolled here is also the essence entry, then there's no need to pre-subtract quality, as if it
            // were to roll after (since it's not rolling after, it's rolling now)
            let leftover_quality_for_essence = if is_essence_entry { remaining_quality } else { new_remaining_quality };
            let quality_multiplier = (leftover_quality_for_essence as f64 / 10.0).floor();
            let chance_increase = weight_roll_chance * overall_chance * quality_multiplier;
            entry_data.weighted_essence_entry.1.borrow_mut().increase_chance(chance_increase);
            // println!("Quality multiplier from {}: {} (new: {})", leftover_quality_for_essence, quality_multiplier, new_remaining_quality);

            // handle final "leftover essence" entry with 1 quality and 0 weight
            let quality_multiplier = ((remaining_quality % 10) as f64).min(10.0);
            let chance_increase = weight_roll_chance * overall_chance * quality_multiplier;
            entry_data.leftover_essence_entry.1.increase_chance(chance_increase);

            continue;
        } else {
            // println!("Iteration B {}, new remaining quality {}", recursion_data.iterations, new_remaining_quality);
            let chance_increase = weight_roll_chance * overall_chance;
            entry_data.get_weighted_data(entry).increase_chance(chance_increase);

            if entry.is_debug_item() {
                recursion_data.iterations_past_d10 += 1;
                //println!("Essence ({:?}), has fish, rq = {}, nrq = {}", entry, remaining_quality, new_remaining_quality);
            }

            if new_remaining_quality == 0 {
                // println!("Iteration C {}, new remaining quality {}, chance: {}", recursion_data.iterations, new_remaining_quality, chance_increase);
                continue;
            }

            // TODO: fix disabled_entry_total_weight "double removing" - aka the list of valid entries won't contain an entry, so there's no chance its weight
            // TOOD: can be included in the total calculation anyways, but then it's still subtracted by the disabled_entry_total_weight value
            
            let roll_once = !entry.is_essence_and_can_roll_multiple_times();
            if roll_once {
                entry_data.get_weighted_data(entry).disabled = true;
                process_random_entries(chest, entry_data, recursion_data, chance_increase, new_remaining_quality, depth + 1);
                entry_data.get_weighted_data(entry).disabled = false;
            } else {
                process_random_entries(chest, entry_data, recursion_data, chance_increase, new_remaining_quality, depth + 1);
            }
        }
    }
}