use crate::loot_calculator;
use convert_case::{Case, Casing};
use include_dir::Dir;
use roman;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::rc::Rc;

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Hash)]
pub struct LootChest {
    pub floor: u8,
    pub master_mode: bool,
    pub chest_type: ChestType,
    pub base_quality: u16,
    pub base_cost: u32,
    pub loot: Vec<Rc<LootEntry>>,

    #[serde(skip_serializing, skip_deserializing, default)]
    pub quality_to_weighted_entries: Vec<FilteredEntryData>,
}

#[derive(Debug, PartialEq, Clone, Default, Hash)]
pub struct FilteredEntryData {
    pub entries: Vec<Rc<LootEntry>>,
    pub total_weight: i32, // pre-calculation is faster than iterating a bunch of times later on
}

impl LootChest {
    fn fill_in_quality(&mut self) {
        let max_quality = loot_calculator::calculate_weight(self, 1.03, 10, true);

        let mut array: Vec<FilteredEntryData> = vec![Default::default(); (max_quality + 1) as usize];
        for quality_threshold in 0..=max_quality {
            let possible_entries = &mut array[quality_threshold as usize];

            for loot_entry in &self.loot {
                if quality_threshold >= loot_entry.get_quality() {
                    match loot_entry.as_ref() {
                        LootEntry::Essence { weight, quality, .. } => {
                            // only use the "regular" essence roll
                            if weight > &0 && quality > &0 {
                                possible_entries.entries.push(Rc::clone(loot_entry));
                            }
                        }
                        _ => {
                            possible_entries.entries.push(Rc::clone(loot_entry));
                        }
                    }
                }
            }

            for entry in possible_entries.entries.iter() {
                possible_entries.total_weight += entry.get_weight() as i32;
            }
        }
        self.quality_to_weighted_entries = array;
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Hash)]
pub enum ChestType {
    Wood,
    Gold,
    Diamond,
    Emerald,
    Obsidian,
    Bedrock,
}

impl ChestType {
    pub fn get_order(&self) -> u8 {
        match self {
            ChestType::Wood => 1,
            ChestType::Gold => 2,
            ChestType::Diamond => 3,
            ChestType::Emerald => 4,
            ChestType::Obsidian => 5,
            ChestType::Bedrock => 6,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Eq, Hash)]
#[serde(untagged)]
pub enum LootEntry {
    Item {
        item: String,
        item_name: Option<String>,
        weight: u16,
        quality: i16,
        extra_chest_cost: u32,
    },
    Pet {
        pet: String,
        tier: String,
        weight: u16,
        quality: i16,
        extra_chest_cost: u32,
    },
    Enchantment {
        enchantment: String,
        enchantment_level: u8,
        weight: u16,
        quality: i16,
        extra_chest_cost: u32,
    },
    Essence {
        essence_type: String,
        essence_amount: u8,
        weight: u16,
        quality: i16,
        extra_chest_cost: u32,
    },
}

impl LootEntry {
    pub fn get_weight(&self) -> u16 {
        match self {
            LootEntry::Item { weight, .. } => *weight,
            LootEntry::Pet { weight, .. } => *weight,
            LootEntry::Enchantment { weight, .. } => *weight,
            LootEntry::Essence { weight, .. } => *weight,
        }
    }

    pub fn get_quality(&self) -> i16 {
        match self {
            LootEntry::Item { quality, .. } => *quality,
            LootEntry::Pet { quality, .. } => *quality,
            LootEntry::Enchantment { quality, .. } => *quality,
            LootEntry::Essence { quality, .. } => *quality,
        }
    }

    pub fn get_added_chest_price(&self) -> u32 {
        match self {
            LootEntry::Item { extra_chest_cost, .. } => *extra_chest_cost,
            LootEntry::Pet { extra_chest_cost, .. } => *extra_chest_cost,
            LootEntry::Enchantment { extra_chest_cost, .. } => *extra_chest_cost,
            LootEntry::Essence { extra_chest_cost, .. } => *extra_chest_cost,
        }
    }

    pub fn get_wiki_page_name(&self) -> String {
        format!("https://wiki.hypixel.net/{}", match self {
            LootEntry::Item { item, ..} => item.clone(),
            LootEntry::Pet { pet, .. } => pet.to_case(Case::Title),
            LootEntry::Enchantment { enchantment, .. } => format!("{} Enchantment", enchantment.to_case(Case::Title)),
            LootEntry::Essence { essence_type, .. } => format!("{} Essence", essence_type.to_case(Case::Title))
        })
    }

    pub fn is_essence_and_can_roll_multiple_times(&self) -> bool {
        matches!(self, LootEntry::Essence { .. })
    }

    pub fn get_possible_file_names(&self) -> Vec<String> {
        match self {
            LootEntry::Item { item, .. } => {
                let id = item.clone().to_lowercase().to_string();
                vec!(format!("{}.png", id), format!("{}.gif", id))
            },
            LootEntry::Pet { pet, .. } => vec!(format!("pet_{}.png", pet.to_lowercase())),
            LootEntry::Enchantment { .. } => vec!("enchanted_book.gif".to_string()),
            LootEntry::Essence { essence_type, .. } => vec!(format!("{}_essence.png", essence_type.to_lowercase())),
        }
    }
}

impl Display for LootEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LootEntry::Item { item, item_name, ..} => {
                let default = item.to_case(Case::Title);
                write!(f, "{}", item_name.as_ref().unwrap_or(&default))
            },
            LootEntry::Pet { pet, .. } => write!(f, "{}", pet.to_case(Case::Title)),
            LootEntry::Enchantment { enchantment, enchantment_level, .. } => write!(
                f,
                "{} {} Book",
                enchantment.to_case(Case::Title),
                roman::to(*enchantment_level as i32).unwrap()
            ),
            LootEntry::Essence { essence_type, essence_amount, .. } => write!(
                f,
                "{} Essence ({})",
                essence_type.to_case(Case::Title),
                essence_amount
            )
        }
    }
}


pub fn floor_to_text(floor: String) -> String {
    match floor.chars().next().unwrap() {
        'f' => {
            format!("Floor {}", floor.chars().last().unwrap())
        }
        'b' => {
            format!("Potato Mode Floor {}", floor.chars().last().unwrap())
        }
        'm' => {
            format!("Master Mode Floor {}", floor.chars().last().unwrap())
        }
        _ => floor.to_string(),
    }
}

pub fn read_all_chests(dir: &Dir) -> BTreeMap<String, Vec<LootChest>> {
    let mut chests = BTreeMap::new();

    let json_files = dir.find("loot/**/*.json").unwrap();
    for entry in json_files {
        let path = entry.path();
        match serde_json::from_slice::<LootChest>(entry.as_file().unwrap().contents()) {
            Ok(mut chest) => {
                println!("Parsing loot JSON file from path {:?}", entry);
                let floor = path
                    .to_str()
                    .unwrap()
                    .to_string()
                    .replace("loot/", "")
                    .split("/")
                    .next()
                    .unwrap()
                    .to_string();

                chest.fill_in_quality();

                let registered_chests = chests.entry(floor).or_insert(Vec::new());
                registered_chests.push(chest);
                registered_chests.sort_by(|a, b| a.chest_type.get_order().cmp(&b.chest_type.get_order()))
            }
            Err(e) => println!("Failed to parse JSON from {}: {}", path.display(), e),
        }
    }

    chests
}
