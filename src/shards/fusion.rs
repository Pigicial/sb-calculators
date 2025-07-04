use crate::shards::shard_data::{ShardData, Shards};
use crate::shards::shards_page::{BuyType, ProfitType};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

pub type ShardFusionCombinations = HashMap<String, HashSet<FusionResults>>;

#[derive(Clone)]
pub struct FusionResults {
    pub first_input_shard_name: String,
    pub second_input_shard_name: String,
    pub first_base_fusion: Option<String>,
    pub second_base_fusion: Option<String>,
    pub special_fusions: Vec<String>,
    pub listed_fusions: Vec<String>,
    pub was_chameleon: bool,
    pub is_reptile_fusion: bool,
}

impl PartialEq for FusionResults {
    fn eq(&self, other: &Self) -> bool {
        (self.first_input_shard_name == other.first_input_shard_name && self.second_input_shard_name == other.second_input_shard_name)
            || (self.first_input_shard_name == other.second_input_shard_name && self.second_input_shard_name == other.first_input_shard_name)
    }
}

impl Eq for FusionResults {}

impl Hash for FusionResults {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut vec = vec![self.first_input_shard_name.clone(), self.second_input_shard_name.clone()];
        vec.sort();
        vec.hash(state);

        let mut listed_fusions: Vec<String> = self.listed_fusions.clone();
        listed_fusions.sort();
        listed_fusions.hash(state);
    }
}

impl FusionResults {
    pub fn get_total_cost(&self, buy_type: BuyType, shards: &Shards) -> Result<u64, String> {
        let first_shard = shards.get(&self.first_input_shard_name).unwrap();
        let second_shard = shards.get(&self.second_input_shard_name).unwrap();
        let first_shard_data = first_shard.cached_bazaar_data.as_ref();
        let second_shard_data = second_shard.cached_bazaar_data.as_ref();

        if first_shard_data.is_none() || second_shard_data.is_none() {
            Err(format!("Failed to get price ({}, {})", first_shard_data.is_some(), second_shard_data.is_some()))
        } else {
            let mut first_shards_price = first_shard_data.unwrap().get_buy_price(buy_type);
            first_shards_price *= first_shard.get_amount_consumed_in_fusion() as f64;

            let mut second_shards_price = second_shard_data.unwrap().get_buy_price(buy_type);
            second_shards_price *= second_shard.get_amount_consumed_in_fusion() as f64;

            let total_price = (first_shards_price + second_shards_price) as u64;
            Ok(total_price)
        }
    }

    pub fn get_result_profit(&self, resulting_shard_name: &String, buy_type: BuyType, profit_type: ProfitType, pure_reptile_attribute_level: u8, bazaar_tax_rate: f64, shards: &Shards) -> f64 {
        let shard = shards.get(resulting_shard_name).unwrap();
        let bazaar_quick_status = shard.cached_bazaar_data.as_ref().unwrap();
        
        let mut amount_created = if self.was_chameleon { 1 } else { shard.get_default_amount_made_in_fusion() } as f64;
        if self.is_reptile_fusion {
            amount_created *= 1.0 + (pure_reptile_attribute_level as f64 * 0.02);
        }

        let cost_of_fusion = self.get_total_cost(buy_type, shards).unwrap() as f64;
        let sell_price = bazaar_quick_status.get_sell_price(profit_type) * amount_created * (1.0 - bazaar_tax_rate);

        sell_price - cost_of_fusion
    }
}

pub fn generate_outputs(first_shard: &ShardData, second_shard: &ShardData, all_shards: &Shards) -> FusionResults {
    let mut is_reptile_fusion = first_shard.families.as_ref().is_some_and(|f| f.contains(&"Reptile".to_string()));
    if !is_reptile_fusion {
        is_reptile_fusion = second_shard.families.as_ref().is_some_and(|f| f.contains(&"Reptile".to_string()));
    }
    
    if first_shard.shard_name.eq("Chameleon") || second_shard.shard_name.eq("Chameleon") {
        let opposing_shard = if first_shard.shard_name.eq("Chameleon") {
            second_shard
        } else {
            first_shard
        };

        let rarity = &opposing_shard.rarity;
        let next_rarity = rarity.get_next_rarity();

        let id = opposing_shard.id;
        let mut missing_ids = 0;

        let mut found_outputs = Vec::with_capacity(3);
        for id_inc in 1..=3 {
            let next_id = id + id_inc;
            let mut next_shard = all_shards.values().find(|s| &s.rarity == rarity && s.id == next_id);
            if let (None, Some(next_rarity)) = (next_shard, next_rarity) {
                missing_ids += 1;
                next_shard = all_shards.values().find(|s| &s.rarity == next_rarity && s.id == missing_ids);
            }

            if let Some(next_shard) = next_shard {
                if next_shard != first_shard && next_shard != second_shard {
                    found_outputs.push(next_shard.shard_name.clone());
                }
            }
        }
        
        return FusionResults {
            first_input_shard_name: first_shard.shard_name.clone(),
            second_input_shard_name: second_shard.shard_name.clone(),
            first_base_fusion: None,
            second_base_fusion: None,
            special_fusions: vec![],
            listed_fusions: found_outputs,
            was_chameleon: true,
            is_reptile_fusion
        };
    }

    let mut first_base_fusion = find_next_same_type_normal_fusion_shard(first_shard, all_shards);
    let mut second_base_fusion = find_next_same_type_normal_fusion_shard(second_shard, all_shards);

    if first_shard.category == second_shard.category {
        // weird rule but whatever
        if first_base_fusion.is_some() && second_shard.rarity >= first_shard.rarity {
            first_base_fusion = None;
        } else if second_base_fusion.is_some() && first_shard.rarity >= second_shard.rarity {
            second_base_fusion = None;
        }
    }

    // prevent results being inputs
    if first_base_fusion.is_some_and(|first| first == first_shard || first == second_shard) {
        first_base_fusion = None;
    }
    if second_base_fusion.is_some_and(|first| first == first_shard || first == second_shard) {
        second_base_fusion = None;
    }

    let mut special_fusions = find_applicable_special_fusions(first_shard, second_shard, all_shards);
    special_fusions.sort_by(|a, b| {
        b.rarity.cmp(&a.rarity).then(a.id.cmp(&b.id))
    });

    let mut listed_fusions = Vec::with_capacity(3);
    if let Some(first_base_fusion) = first_base_fusion {
        listed_fusions.push(first_base_fusion.shard_name.clone());
    }
    if let Some(second_base_fusion) = second_base_fusion {
        listed_fusions.push(second_base_fusion.shard_name.clone());
    }

    for special_fusion_output in &special_fusions {
        if listed_fusions.len() < 3 {
            listed_fusions.push(special_fusion_output.shard_name.clone());
        } else {
            break;
        }
    }

    FusionResults {
        first_input_shard_name: first_shard.shard_name.clone(),
        second_input_shard_name: second_shard.shard_name.clone(),
        first_base_fusion: first_base_fusion.map(|s| s.shard_name.clone()),
        second_base_fusion: second_base_fusion.map(|s| s.shard_name.clone()),
        special_fusions: special_fusions.iter().map(|s| s.shard_name.clone()).collect(),
        listed_fusions,
        was_chameleon: false,
        is_reptile_fusion
    }
}

pub fn find_next_same_type_normal_fusion_shard<'a>(shard: &ShardData, all_shards: &'a Shards) -> Option<&'a ShardData> {
    let mut next_shards: Vec<&ShardData> = all_shards.values()
        .filter(|s| s.rarity == shard.rarity && s.category == shard.category)
        .filter(|s| s.id > shard.id)
        .filter(|s| !s.is_special_fusion_or_special_source_only())
        .collect::<Vec<&ShardData>>();

    next_shards.sort_by(|a, b| a.id.cmp(&b.id));
    if next_shards.is_empty() {
        None
    } else {
        Some(next_shards.first()?)
    }
}

pub fn find_applicable_special_fusions<'a>(left_shard: &ShardData, right_shard: &ShardData, all_shards: &'a Shards) -> Vec<&'a ShardData> {
    let mut fusions = Vec::new();
    for fusable_shard in all_shards.values() {
        if let Some(special_fusions) = fusable_shard.sources.special_fusion.as_ref() {
            let left_first = special_fusions.first.iter().any(|first_reqs| left_shard.meets_conditions(first_reqs));
            let right_first = special_fusions.first.iter().any(|first_reqs| right_shard.meets_conditions(first_reqs));

            let left_second = special_fusions.second.iter().any(|second_reqs| left_shard.meets_conditions(second_reqs));
            let right_second = special_fusions.second.iter().any(|second_reqs| right_shard.meets_conditions(second_reqs));

            if ((left_first && right_second) || (left_second && right_first)) && fusable_shard != left_shard && fusable_shard != right_shard {
                fusions.push(fusable_shard);
                continue;
            }
        }
    }

    fusions.dedup();
    fusions
}

pub fn generate_all_possible_combinations_per_shard(shards: &Shards) -> ShardFusionCombinations {
    let mut combinations = HashMap::with_capacity(shards.len());

    for left_shard in shards.values() {
        for right_shard in shards.values() {
            let results = generate_outputs(left_shard, right_shard, shards);
            for resulting_shard in &results.listed_fusions {
                let saved_combinations = combinations.entry(resulting_shard.clone()).or_insert_with(HashSet::new);
                saved_combinations.insert(results.clone());
            }
        }
    }

    combinations
}

pub fn generate_all_possible_combinations(shards: &Shards) -> Vec<FusionResults> {
    let mut combinations = Vec::new();

    for left_shard in shards.values() {
        for right_shard in shards.values() {
            let results = generate_outputs(left_shard, right_shard, shards);
            if !results.listed_fusions.is_empty() {
                combinations.push(results);
            }
        }
    }

    combinations.sort_by(|a_data, b_data| {
        let a_shard_first_input = &shards.get(&a_data.first_input_shard_name).unwrap();
        let a_shard_second_input = &shards.get(&a_data.second_input_shard_name).unwrap();

        let b_shard_first_input = &shards.get(&b_data.first_input_shard_name).unwrap();
        let b_shard_second_input = &shards.get(&b_data.second_input_shard_name).unwrap();

        let first_cmp = a_shard_first_input.rarity.cmp(&b_shard_first_input.rarity).then(a_shard_first_input.id.cmp(&b_shard_first_input.id));
        let second_cmp = a_shard_second_input.rarity.cmp(&b_shard_second_input.rarity).then(a_shard_second_input.id.cmp(&b_shard_second_input.id));
        first_cmp.then(second_cmp)
    });
    
    combinations
}