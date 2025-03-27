use crate::slayer::slayer_loot_calculator::SelectedRngMeterItem;
use convert_case::{Case, Casing};
use include_dir::Dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::rc::Rc;

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Hash)]
pub struct LootTable {
    pub boss_type: String,
    pub boss_tier: u8,
    pub loot: Vec<Rc<LootEntry>>,

    #[serde(skip_serializing, skip_deserializing, default)]
    pub loot_strings: Vec<String>,
}

impl LootTable {
    pub fn has_matching_entry_type(&self, entry: &Rc<LootEntry>) -> bool {
        self.loot_strings.contains(&entry.to_string())
    }

    pub fn has_rng_entry(&self, entry: &SelectedRngMeterItem) -> bool {
        self.loot_strings.contains(&entry.identifier)
    }
}
#[derive(Debug, PartialEq, Clone, Default, Deserialize)]
pub struct FilteredEntryData {
    pub entries: Vec<Rc<LootEntry>>,
    pub total_weight: i32, // pre-calculation is faster than iterating a bunch of times later on
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Hash)]
#[serde(untagged)]
pub enum LootEntry {
    Item {
        item: String,
        item_name: Option<String>,
        level_requirement: u8,
        loot_table: DropType,
        weight: u16,
        quantity_range: String,
    },
    Enchantment {
        enchantment: String,
        enchantment_level: u8,
        level_requirement: u8,
        loot_table: DropType,
        weight: u16,
        quantity_range: String,
    },
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Eq, Hash)]
pub enum DropType {
    Token,
    Main,
    Extra,
}

impl LootEntry {
    pub fn get_weight(&self) -> f64 {
        match self {
            LootEntry::Item { weight, .. } => *weight as f64,
            LootEntry::Enchantment { weight, .. } => *weight as f64,
        }
    }

    pub fn get_drop_type(&self) -> &DropType {
        match self {
            LootEntry::Item { loot_table, .. } => loot_table,
            LootEntry::Enchantment { loot_table, .. } => loot_table,
        }
    }

    pub fn get_slayer_level_requirement(&self) -> u8 {
        match self {
            LootEntry::Item {
                level_requirement, ..
            } => *level_requirement,
            LootEntry::Enchantment {
                level_requirement, ..
            } => *level_requirement,
        }
    }

    pub fn get_wiki_page_name(&self) -> String {
        format!(
            "https://wiki.hypixel.net/{}",
            match self {
                LootEntry::Item { item, .. } => item.clone(),
                LootEntry::Enchantment { enchantment, .. } =>
                    format!("{} Enchantment", enchantment.to_case(Case::Title)),
            }
        )
    }

    pub fn get_possible_file_names(&self) -> Vec<String> {
        match self {
            LootEntry::Item { item, .. } => {
                let id = item.clone().to_lowercase().to_string();
                vec![format!("{}.png", id), format!("{}.gif", id)]
            }
            LootEntry::Enchantment { .. } => vec!["enchanted_book.gif".to_string()],
        }
    }
}

impl Display for LootEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LootEntry::Item {
                item,
                item_name,
                quantity_range,
                ..
            } => {
                let default = item.to_case(Case::Title);
                if quantity_range == "1" {
                    write!(f, "{}", item_name.as_ref().unwrap_or(&default))
                } else {
                    write!(
                        f,
                        "{} ({})",
                        item_name.as_ref().unwrap_or(&default),
                        quantity_range
                    )
                }
            }
            LootEntry::Enchantment {
                enchantment,
                enchantment_level,
                quantity_range,
                ..
            } => {
                if quantity_range == "1" {
                    write!(
                        f,
                        "{} {} Book",
                        enchantment.to_case(Case::Title),
                        roman::to(*enchantment_level as i32).unwrap()
                    )
                } else {
                    write!(
                        f,
                        "{} {} Book ({})",
                        enchantment.to_case(Case::Title),
                        roman::to(*enchantment_level as i32).unwrap(),
                        quantity_range
                    )
                }
            }
        }
    }
}

pub fn read_all_loot(dir: &Dir) -> BTreeMap<String, Vec<LootTable>> {
    let mut loot = BTreeMap::new();

    let json_files = dir.find("slayer_loot/**/*.json").unwrap();
    for entry in json_files {
        let path = entry.path();
        match serde_json::from_slice::<LootTable>(entry.as_file().unwrap().contents()) {
            Ok(mut loot_table) => {
                for entry in loot_table.loot.iter() {
                    loot_table.loot_strings.push(entry.to_string());
                }

                println!("Parsing loot JSON file from path {:?}", entry);
                let boss_type = path
                    .to_str()
                    .unwrap()
                    .to_string()
                    .replace("slayer_loot/", "")
                    .split("/")
                    .next()
                    .unwrap()
                    .to_string();

                let registered_loot = loot.entry(boss_type).or_insert(Vec::new());
                registered_loot.push(loot_table);
                registered_loot.sort_by(|a, b| a.boss_tier.cmp(&b.boss_tier))
            }
            Err(e) => println!("Failed to parse JSON from {}: {}", path.display(), e),
        }
    }

    loot
}
