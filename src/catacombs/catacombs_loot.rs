use convert_case::{Case, Casing};
use include_dir::Dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::process::id;
use std::rc::Rc;
use crate::catacombs::catacombs_loot_calculator::SelectedRngMeterItem;

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Hash)]
pub struct LootChest {
    pub floor: u8,
    pub master_mode: bool,
    pub chest_type: ChestType,
    pub base_quality: u16,
    pub base_cost: u32,
    pub loot: Vec<Rc<LootEntry>>,

    #[serde(skip_serializing, skip_deserializing, default)]
    pub loot_strings: Vec<String>,
}

impl LootChest {
    pub fn has_matching_entry(&self, entry: &Rc<LootEntry>) -> bool {
        self.loot_strings.contains(&entry.to_string())
    }
    pub fn has_matching_entry_identifier(&self, entry_identifier: &String) -> bool {
        self.loot_strings.contains(entry_identifier)
    }

    pub fn has_rng_entry(&self, entry: &SelectedRngMeterItem) -> bool {
        self.loot_strings.contains(&entry.identifier)
    }

    pub fn require_s_plus(&self) -> bool {
        self.chest_type == ChestType::Bedrock && (self.floor == 5 || self.floor == 6 || (self.floor == 7 && !self.master_mode))
    }
    
    pub fn get_matching_entry_quality(&self, identifier: &String) -> Option<i16> {
        self.loot.iter().find(|i| &i.to_string() == identifier).map(|i| i.get_quality())
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

pub fn read_all_chests(dir: &Dir) -> BTreeMap<String, Vec<LootChest>> {
    let mut chests = BTreeMap::new();

    let json_files = dir.find("dungeon_loot/**/*.json").unwrap();
    for entry in json_files {
        let path = entry.path();
        match serde_json::from_slice::<LootChest>(entry.as_file().unwrap().contents()) {
            Ok(mut chest) => {
                println!("Parsing loot JSON file from path {:?}", entry);
                let floor = path
                    .to_str()
                    .unwrap()
                    .to_string()
                    .replace("dungeon_loot/", "")
                    .split("/")
                    .next()
                    .unwrap()
                    .to_string();

                for entry in chest.loot.iter() {
                    chest.loot_strings.push(entry.to_string());
                }

                let registered_chests = chests.entry(floor).or_insert(Vec::new());
                registered_chests.push(chest);
                registered_chests.sort_by(|a, b| a.chest_type.get_order().cmp(&b.chest_type.get_order()))
            }
            Err(e) => println!("Failed to parse JSON from {}: {}", path.display(), e),
        }
    }

    chests
}
