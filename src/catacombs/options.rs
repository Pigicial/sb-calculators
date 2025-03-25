use std::rc::Rc;
use egui::{Checkbox, Label, RichText, TextWrapMode, Ui};
use num_format::{Locale, ToFormattedString};
use crate::catacombs::catacombs_loot::{LootChest, LootEntry};
use crate::catacombs::catacombs_loot_calculator::SelectedRngMeterItem;
use crate::catacombs::catacombs_page::CalculatorType::AveragesLootTable;
use crate::catacombs::catacombs_page::CatacombsLootApp;
use crate::images;

pub fn add_treasure_talisman_options(calc: &mut CatacombsLootApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        images::add_image(&calc.images, ui, "treasure_talisman.png");
        ui.label("Treasure Accessory: ");
    });
    ui.horizontal(|ui| {
        ui.selectable_value(&mut calc.treasure_accessory_multiplier, 1.0, "None");
        ui.selectable_value(
            &mut calc.treasure_accessory_multiplier,
            1.01,
            "Talisman (1%)",
        );
        ui.selectable_value(
            &mut calc.treasure_accessory_multiplier,
            1.02,
            "Ring (2%)",
        );
        ui.selectable_value(
            &mut calc.treasure_accessory_multiplier,
            1.03,
            "Artifact (3%)",
        );
    });
}

pub fn add_boss_luck_options(calc: &mut CatacombsLootApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        images::add_image(&calc.images, ui, "boss_luck.png");
        ui.label("Boss Luck: ");
    });
    ui.horizontal(|ui| {
        ui.selectable_value(&mut calc.boss_luck_increase, 0, "None");
        ui.selectable_value(&mut calc.boss_luck_increase, 1, "I (+1)");
        ui.selectable_value(&mut calc.boss_luck_increase, 3, "II (+3)");
        ui.selectable_value(&mut calc.boss_luck_increase, 5, "III (+5)");
        ui.selectable_value(&mut calc.boss_luck_increase, 10, "IV (+10)");
    });
}

pub fn add_s_plus_options(calc: &mut CatacombsLootApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        images::add_image(&calc.images, ui, "s_plus.png");
        ui.label("S+: ");
    });

    let require_s_plus = calc.require_s_plus();
    if require_s_plus {
        let checkbox = Checkbox::new(&mut calc.forced_s_plus_const, "Chest type requires S+");
        ui.add_enabled(false, checkbox);
    } else {
        let checkbox = Checkbox::new(&mut calc.s_plus, "Click to toggle");
        ui.add(checkbox);
    }
}

pub fn add_floor_options(calc: &mut CatacombsLootApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        images::add_image(&calc.images, ui, "catacombs.png");
        ui.label("Floor: ");
    });
    egui::ComboBox::from_label("Select a floor")
        .selected_text(floor_to_text(
            calc.floor.as_deref().unwrap_or("None").to_string(),
        ))
        .show_ui(ui, |ui| {
            for floor in calc.loot.keys() {
                let floor_label = egui::SelectableLabel::new(calc.floor == Some(floor.clone()), floor_to_text(floor.clone()));
                if ui.add(floor_label).clicked() {
                    calc.floor = Some(floor.clone());

                    if let Some(current_chest) = calc.chest.as_ref() {
                        if let Some(new_floor_chests) = calc.loot.get(floor) {
                            calc.chest = match_chest_type_or_none(current_chest, new_floor_chests);
                        }
                    }

                    // try and find the entry of the same type from the new chest
                    if calc.rng_meter_data.selected_item.is_none() {
                        continue;
                    }

                    let selected_item_data = calc.rng_meter_data.selected_item.as_mut().unwrap();
                    let selected_xp = calc.rng_meter_data.selected_xp;

                    let highest_tier_chest = calc.loot.get(floor).and_then(|v| v.last()).unwrap();
                    let highest_tier_chest_total_weight: i32 = highest_tier_chest.loot.iter().map(|e| e.get_weight() as i32).sum();

                    let mut reset_selected = true;
                    for replacement_entry in highest_tier_chest.loot.iter() {
                        if replacement_entry.to_string() == selected_item_data.identifier {
                            let item_weight = replacement_entry.get_weight();
                            let required_xp: i32 = (300.0 * (highest_tier_chest_total_weight as f32 / item_weight as f32)).round() as i32;

                            let lowest_match = find_matching_item_from_lowest_chest(replacement_entry, calc.loot.get(floor).unwrap())
                                .unwrap();

                            selected_item_data.lowest_tier_chest_type = lowest_match.0.chest_type.clone();
                            selected_item_data.lowest_tier_chest_entry = Rc::clone(lowest_match.1);

                            selected_item_data.required_xp = required_xp;
                            selected_item_data.highest_tier_chest_entry = Rc::clone(replacement_entry);
                            calc.rng_meter_data.selected_xp = selected_xp.min(required_xp);

                            reset_selected = false;
                        }
                    }

                    if reset_selected {
                        calc.rng_meter_data.selected_xp = 0;
                        calc.rng_meter_data.selected_item = None;
                    }
                }
            }
        });
}

pub fn add_chest_options(calc: &mut CatacombsLootApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        images::add_image(&calc.images, ui, "catacombs.png");
        ui.label("Chest: ");
    });

    egui::ComboBox::from_label("Select a chest")
        .height(400.0)
        .selected_text(
            calc.chest
                .as_ref()
                .map(|c| &c.chest_type)
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "None".to_string()),
        )
        .show_ui(ui, move |ui| {
            let default = String::new();
            let floor = calc.floor.as_ref().unwrap_or(&default);

            let default = Vec::new();
            for chest in calc.loot.get(floor).unwrap_or(&default).iter() {
                // ui.selectable_value(&mut calc.chest, Some(chest.clone()), format!("{:?}", chest.chest_type));
                let label = egui::SelectableLabel::new(calc.chest == Some(chest.clone()), format!("{:?}", chest.chest_type));
                if ui.add(label).clicked() {
                    calc.chest = Some(chest.clone());
                }
            }
        });
}

pub fn add_rng_meter_options(calc: &mut CatacombsLootApp, ui: &mut Ui) {
    if calc.floor.is_none() {
        return;
    }
    let floor = calc.floor.as_ref().unwrap();
    let highest_tier_chest = calc.loot.get(floor).unwrap().last().unwrap();
    let total_weight: i32 = highest_tier_chest.loot.iter().map(|e| e.get_weight() as i32).sum();

    ui.heading("RNG Meter");
    ui.end_row();

    ui.horizontal(|ui| {
        images::add_image(&calc.images, ui, "painting.png");
        ui.label("Item: ");
    });

    let selected_item_string = calc.rng_meter_data.selected_item
        .as_ref()
        .map(|entry| {
            let required_xp: i32 = (300.0 * (total_weight as f32 / entry.highest_tier_chest_entry.get_weight() as f32)).round() as i32;
            let mut text = RichText::new(format!("{} ({} XP)", entry.highest_tier_chest_entry, required_xp.to_formatted_string(&Locale::en)));

            if calc.chest.is_some() && !calc.chest.as_ref().unwrap().has_rng_entry(entry) {
                text = text.strikethrough();
            }

            text
        })
        .unwrap_or(RichText::new(String::from("None")));

    egui::ComboBox::from_label("Select an item")
        .selected_text(selected_item_string)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut calc.rng_meter_data.selected_item, None, "None");

            let mut sorted_loot = highest_tier_chest.loot.iter().map(|e| {
                if let Some(chest) = &calc.chest {
                    (e, chest.has_matching_entry_type(e))
                } else {
                    (e, true)
                }
            }).collect::<Vec<(&Rc<LootEntry>, bool)>>();
            sorted_loot.sort_by(|a, b| b.1.cmp(&a.1));

            for entry in sorted_loot {
                let in_loot = entry.1;
                let entry = entry.0;

                if entry.is_essence_and_can_roll_multiple_times() { // essence doesn't show in rng meter
                    continue;
                }
                let item_weight = entry.get_weight();
                let required_xp: i32 = (300.0 * (total_weight as f32 / item_weight as f32)).round() as i32;

                let selected = calc.rng_meter_data.selected_item.as_ref().map_or("", |e| &e.identifier) == entry.to_string();
                let mut text = RichText::new(format!("{} ({} XP)", entry, required_xp.to_formatted_string(&Locale::en)));

                if !in_loot {
                    text = text.strikethrough(); // easier way to distinguish entries that don't apply
                }

                let label = egui::SelectableLabel::new(selected, text);
                if ui.add(label).clicked() {
                    let rng_meter_data = &mut calc.rng_meter_data;

                    let lowest_match = find_matching_item_from_lowest_chest(entry, calc.loot.get(floor).unwrap())
                        .unwrap();

                    let new_selected_item_data = SelectedRngMeterItem {
                        identifier: entry.to_string(),
                        highest_tier_chest_entry: Rc::clone(entry),
                        highest_tier_chest_type: highest_tier_chest.chest_type.clone(),
                        lowest_tier_chest_entry: Rc::clone(lowest_match.1),
                        lowest_tier_chest_type: lowest_match.0.chest_type.clone(),
                        required_xp,
                    };
                    rng_meter_data.selected_item = Some(new_selected_item_data);
                    rng_meter_data.selected_xp = rng_meter_data.selected_xp.min(required_xp);
                }
            }
        });

    ui.end_row();

    if let Some(selected_item_data) = calc.rng_meter_data.selected_item.as_ref() {
        let selected_item = &selected_item_data.lowest_tier_chest_entry;
        ui.horizontal(|ui| {
            images::add_first_valid_image(&calc.images, ui, selected_item.get_possible_file_names());
            ui.label("XP: ");
        });

        let required_xp = selected_item_data.required_xp;
        let percent = 100.0 * calc.rng_meter_data.selected_xp as f32 / required_xp as f32;

        let slider = egui::Slider::new(&mut calc.rng_meter_data.selected_xp, 0..=required_xp)
            .suffix(format!(" XP ({:.2}%)", percent))
            .custom_parser(|text| parse_rng_meter_xp_input(text, required_xp));
        ui.add(slider);

        if calc.calculator_type == AveragesLootTable {
            let mut add_switch_to_lowest_chest_button = false;
            let mut text_to_add: Option<String> = None;
            if let Some(chest) = &calc.chest {
                if !chest.has_rng_entry(selected_item_data) {
                    if selected_item_data.lowest_tier_chest_type == selected_item_data.highest_tier_chest_type {
                        text_to_add = Some(format!("This entry only appears in the {:?} chest.", selected_item_data.lowest_tier_chest_type));
                    } else {
                        text_to_add = Some(format!("This entry only appears in {:?} to {:?} chests.",
                                                   selected_item_data.lowest_tier_chest_type,
                                                   selected_item_data.highest_tier_chest_type));
                    }
                    add_switch_to_lowest_chest_button = true;
                } else if selected_item_data.lowest_tier_chest_type != chest.chest_type {
                    if percent >= 100.0 {
                        text_to_add = Some(format!("This entry is guaranteed to appear in the {:?} chest.", selected_item_data.lowest_tier_chest_type));
                    } else {
                        text_to_add = Some(format!("At 100%, this entry is guaranteed to appear in the {:?} chest.", selected_item_data.lowest_tier_chest_type));
                    }
                    add_switch_to_lowest_chest_button = true;
                } else if percent >= 100.0 {
                    text_to_add = Some("This entry is guaranteed to appear in this chest.".to_string());
                } else {
                    text_to_add = Some("At 100%, this entry is guaranteed to appear in this chest.".to_string());
                }
            }

            if let Some(text_to_add) = text_to_add {
                ui.end_row();
                ui.label("");
                ui.add(Label::new(text_to_add).wrap_mode(TextWrapMode::Wrap));

                if add_switch_to_lowest_chest_button {
                    let button_text = format!("Switch to {:?}", selected_item_data.lowest_tier_chest_type);
                    if ui.button(button_text).clicked() {
                        let lowest_tier_chest = calc.loot.get(floor)
                            .unwrap()
                            .iter()
                            .find(|c| c.chest_type == selected_item_data.lowest_tier_chest_type)
                            .unwrap();

                        calc.chest = Some(Rc::clone(lowest_tier_chest));
                    }
                }
            }   
        }
    }

    ui.end_row();
}

fn parse_rng_meter_xp_input(text: &str, required_xp: i32) -> Option<f64> {
    // copied from drag_value::default_parser
    let mut text: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        // Replace special minus character with normal minus (hyphen):
        .map(|c| if c == '−' { '-' } else { c })
        .collect();

    if text.ends_with('%') {
        text = text.replace('%', "");

        return match text.parse::<f64>() {
            Ok(mut percentage) => {
                percentage = percentage.clamp(0.0, 100.0);
                Some((percentage / 100.0) * (required_xp as f64))
            }
            Err(_) => None
        };
    }

    text.parse().ok()
}

fn match_chest_type_or_none(chest: &Rc<LootChest>, others: &Vec<Rc<LootChest>>) -> Option<Rc<LootChest>> {
    for other_chest in others {
        if chest.chest_type == other_chest.chest_type {
            return Some(Rc::clone(other_chest));
        }
    }

    None
}

fn find_matching_item_from_lowest_chest<'a>(selected_item: &'a Rc<LootEntry>, floor_chests: &'a [Rc<LootChest>]) -> Option<(&'a Rc<LootChest>, &'a Rc<LootEntry>)> {
    let identifier = selected_item.to_string();
    // auto-sorted from lowest to highest
    for chest in floor_chests.iter() {
        if let Some(matching_entry) = chest.loot.iter().find(|e| e.to_string() == identifier) {
            return Some((chest, matching_entry));
        }
    }

    None
}

pub fn floor_to_text(floor: String) -> String {
    match floor.chars().next().unwrap() {
        'f' => {
            format!("Floor {}", floor.chars().last().unwrap())
        }
        'm' => {
            format!("Master Mode Floor {}", floor.chars().last().unwrap())
        }
        _ => floor.to_string(),
    }
}