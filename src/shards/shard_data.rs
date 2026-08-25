use egui::Color32;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, OneOrMany};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use crate::shards::bazaar_data::QuickStatus;

// key: shard name
pub type Shards = HashMap<String, ShardData>;

#[serde_as]
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ShardData {
    pub shard_name: String,
    pub attribute_name: String,
    pub internal_id: Option<String>,
    pub description: String,
    pub id: u8,
    //#[serde_as(as = "OneOrMany<_>")]
    //pub scaling: Vec<f32>,
    pub rarity: Rarity,
    pub category: Category,
    pub skill: Skill,
    pub families: Option<Vec<String>>,
    pub blacklisted_from_regular_fusions: Option<bool>,
    pub blacklisted_from_chameleon_fusions: Option<bool>,
    pub sources: Sources,
    
    #[serde(skip)]
    pub cached_bazaar_data: Option<QuickStatus>
}

impl Eq for ShardData {}

impl Hash for ShardData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.rarity.hash(state);
    }
}

impl ShardData {
    pub fn meets_conditions(&self, requirements: &ShardConditions) -> bool {
        let only_one_required = requirements.one_requirement.unwrap_or(false);
        for requirement in &requirements.requirements {
            // println!("test");
            let meets_requirement = self.meets_requirement(requirement);
            if only_one_required && meets_requirement {
                return true;
            } else if !meets_requirement && !only_one_required {
                return false;
            }
        }

        !only_one_required
    }
    
    pub fn meets_requirement(&self, requirement: &ShardCondition) -> bool {
        if let Some(whitelisted_shards) = &requirement.shard {
            whitelisted_shards == &self.shard_name
        } else if let Some(required_category) = &requirement.category {
            return required_category == &self.category;
        } else if let Some(required_skill) = &requirement.skill {
            return required_skill == &self.skill;
        } else if let Some(required_family) = &requirement.family {
            return match &self.families {
                None => false,
                Some(families) => {
                    families.contains(required_family)
                }
            }
        } else if let Some(required_rarity) = &requirement.rarity {
            let rarity_or_higher = &requirement.rarity_or_higher.unwrap_or(false);
            return match rarity_or_higher {
                false => required_rarity == &self.rarity,
                true => &self.rarity >= required_rarity,
            }
        } else {
            return false;
        }
    }

    pub fn is_special_fusion_or_special_source_only(&self) -> bool {
        if let Some(blacklisted_from_regular_fusions) = self.blacklisted_from_regular_fusions {
            return blacklisted_from_regular_fusions;
        }
        
        let sources = &self.sources;
        let has_special_fusion_source = sources.special_fusion.is_some();
        has_special_fusion_source
    }
    
    pub fn get_amount_consumed_in_fusion(&self) -> u8 {
        if self.shard_name.eq("Chameleon") {
            return 1;
        }
        
        if let Some(families) = &self.families {
            if families.contains(&"Amphibian".to_string()) || families.contains(&"Reptile".to_string()) || families.contains(&"Elemental".to_string()) {
                return 2;        
            }
        }
        
        5
    }
    
    pub fn get_default_amount_made_in_fusion(&self) -> u8 {
        if self.is_special_fusion_or_special_source_only() {
            if let Some(special_fusion) = &self.sources.special_fusion {
                special_fusion.amount_override.unwrap_or(2)
            } else {
                2
            }
        } else {
            1
        }
    }

    pub fn format(&self) -> String {
        format!("{} ({:?} {:?}-{})", self.shard_name, self.category, self.rarity, self.id)
    }
    
    pub fn get_bazaar_id(&self) -> String {
        if let Some(internal_id) = &self.internal_id {
            format!("SHARD_{}", internal_id.to_uppercase())
        } else {
            format!("SHARD_{}", self.shard_name.to_uppercase().replace(" ", "_"))
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq)]
pub enum Category {
    Water,
    Combat,
    Forest,
}

#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq)]
pub enum Skill {
    Combat,
    Fishing,
    Farming,
    Foraging,
    Mining,
    Taming,
    Enchanting,
    Hunting,
    Global,
    Alchemy
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd, Debug, Hash)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub fn get_color(&self) -> Color32 {
        match self {
            Rarity::Common => Color32::WHITE,
            Rarity::Uncommon => Color32::from_rgb(85, 255, 85),
            Rarity::Rare => Color32::from_rgb(85, 85, 255),
            Rarity::Epic => Color32::from_rgb(170, 0, 170),
            Rarity::Legendary => Color32::from_rgb(255, 170, 0),
        }
    }
    
    pub fn get_next_rarity(&self) -> Option<&Self> {
        match self {
            Rarity::Common => Some(&Rarity::Uncommon),
            Rarity::Uncommon => Some(&Rarity::Rare),
            Rarity::Rare => Some(&Rarity::Epic),
            Rarity::Epic => Some(&Rarity::Legendary),
            Rarity::Legendary => None
        }
    }
    
    pub fn get_char(&self) -> char {
        match self {
            Rarity::Common => 'C',
            Rarity::Uncommon => 'U',
            Rarity::Rare => 'R',
            Rarity::Epic => 'E',
            Rarity::Legendary => 'L',
        }
    }
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct Sources {
    #[serde_as(as = "Option<OneOrMany<_>>")]
    pub traps: Option<Vec<TrapSource>>,
    pub special_fusion: Option<SpecialFusionSource>,
    pub blacklisted_from_regular_fusion: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct MobSource {
    names: Option<Vec<String>>,
    black_hole: Option<bool>,
    salts: Option<bool>,
    location: Option<String>,
    lasso: Option<bool>,
    fishing_net: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct FishingLootSource {
    pub location: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct CraftingSource {
    pub collection: String,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct ShopPurchaseSource {
    pub npc_name: String,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct DungeonLootSource {
    pub floor: u8,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct SpecialFusionSource {
    #[serde_as(as = "OneOrMany<_>")]
    pub first: Vec<ShardConditions>,
    #[serde_as(as = "OneOrMany<_>")]
    pub second: Vec<ShardConditions>,
    pub amount_override: Option<u8>
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct ShardConditions {
    pub one_requirement: Option<bool>,
    pub requirements: Vec<ShardCondition>
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct ShardCondition { 
    pub shard: Option<String>,
    pub category: Option<Category>,
    pub skill: Option<Skill>,
    pub family: Option<String>,
    pub rarity: Option<Rarity>,
    pub rarity_or_higher: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct TrapSource {
    pub location: String,
    pub placement_type: Option<String>,
    pub other_conditions: Option<String>,
}

impl TrapSource {
    pub fn to_string(&self) -> String {
        let mut text = self.location.to_string();
        if let Some(placement_type) = &self.placement_type {
            text += &*format!(" ({placement_type})");
        }
        if let Some(other_conditions) = &self.other_conditions {
            text += &*format!(" ({other_conditions})");
        }
        text
    }
}

pub fn read_all_shards() -> Shards {
    let shard_data_path = include_str!("../../assets/attribute_shards/shard_data.json");
    let shards_list: Vec<ShardData> = serde_json::from_str(shard_data_path).expect("Failed to parse shard data");
    
    let mut shards = HashMap::new();
    for shard in shards_list {
        shards.insert(shard.shard_name.clone(), shard);
    }
    
    shards
}