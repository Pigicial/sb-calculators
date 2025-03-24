use crate::catacombs_loot::{ChestType, LootChest, LootEntry};
use crate::{catacombs_loot, images};
use egui::{Checkbox, Context, Grid, Label, RichText, ScrollArea, TextStyle, TextWrapMode, Ui};
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use eframe::epaint::{Color32, TextureHandle};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use include_dir::{include_dir, Dir};
use num_format::{Locale, ToFormattedString};
use crate::catacombs_loot_calculator::{calculate_chances, calculate_quality, CalculationResult, RngMeterData, SelectedRngMeterItem};

static ASSETS_DIR: Dir<'static> = include_dir!("assets");

pub struct CatacombsLootApp {
    floor: Option<String>,
    chest: Option<Rc<LootChest>>,

    treasure_accessory_multiplier: f64,
    boss_luck_increase: u8,
    s_plus: bool,
    forced_s_plus_const: bool,

    rng_meter_data: RngMeterData,

    hashed_chances: HashMap<u64, CalculationResult>,
    loot: BTreeMap<String, Vec<Rc<LootChest>>>,

    images: Rc<HashMap<String, TextureHandle>>,
}

impl eframe::App for CatacombsLootApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if false {
                ScrollArea::vertical().show(ui, |ui| {
                    StripBuilder::new(ui)
                        .size(Size::initial(100.0)) // top cell
                        .size(Size::remainder()) // for the dungeon_loot
                        .size(Size::exact(16.0)) // for the dungeon_loot
                        .vertical(|mut strip| {
                            // Add the top 'cell'
                            strip.cell(|ui| {
                                ui.label("Fixed");
                            });
                            // We add a nested strip in the bottom cell:
                            strip.strip(|builder| {
                                builder.sizes(Size::remainder(), 2).horizontal(|mut strip| {
                                    strip.cell(|ui| {
                                        ui.label("Top Left");
                                    });
                                    strip.cell(|ui| {
                                        ui.label("Top Right");
                                    });
                                });
                            });
                            strip.cell(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label("Middle");
                                });
                            });
                        });
                });

                return;
            }

            ScrollArea::horizontal().id_salt("cata_loot_config").show(ui, |ui| {
                self.add_regular_settings_section(ui);
            });
            ui.separator();

            if self.floor.is_some() && self.chest.is_some() {
                // Horizontal scrolling is done here, vertical scrolling is done on the table scrolling end
                // (this took painfully long to figure out)
                ScrollArea::horizontal().id_salt("cata_loot").show(ui, |ui| {
                    let hash = self.generate_hash();
                    let chances = self.get_chances();

                    if chances.is_none() {
                        let chest = self.chest.as_ref().unwrap();
                        let starting_quality = calculate_quality(
                            chest,
                            self.treasure_accessory_multiplier,
                            self.boss_luck_increase,
                            self.s_plus || self.require_s_plus(),
                        );

                        let new_chances = calculate_chances(chest, starting_quality, &self.rng_meter_data);
                        self.hashed_chances.insert(hash, new_chances);
                    }

                    self.add_loot_section(ui);
                });
            }
        });
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

impl CatacombsLootApp {
    pub fn new(images: Rc<HashMap<String, TextureHandle>>) -> Self {
        Self {
            floor: None,
            chest: None,

            treasure_accessory_multiplier: 1.0,
            boss_luck_increase: 0,
            s_plus: false,
            forced_s_plus_const: true,

            rng_meter_data: Default::default(),

            hashed_chances: HashMap::new(),

            loot: catacombs_loot::read_all_chests(&ASSETS_DIR)
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(Rc::new).collect()))
                .collect(),

            images,
        }
    }

    fn add_regular_settings_section(&mut self, ui: &mut Ui) {
        ui.heading("Options");

        Grid::new("config_grid")
            .num_columns(2)
            .spacing([15.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                self.add_treasure_talisman_options(ui);
                ui.end_row();
                self.add_boss_luck_options(ui);
                ui.end_row();
                self.add_s_plus_options(ui);
                ui.end_row();
                self.add_floor_options(ui);
                ui.end_row();
                self.add_chest_options(ui);
                ui.end_row();

                self.add_rng_meter_section(ui);
            });
    }

    fn add_treasure_talisman_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "treasure_talisman.png");
            ui.label("Treasure Accessory: ");
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.treasure_accessory_multiplier, 1.0, "None");
            ui.selectable_value(
                &mut self.treasure_accessory_multiplier,
                1.01,
                "Talisman (1%)",
            );
            ui.selectable_value(
                &mut self.treasure_accessory_multiplier,
                1.02,
                "Ring (2%)",
            );
            ui.selectable_value(
                &mut self.treasure_accessory_multiplier,
                1.03,
                "Artifact (3%)",
            );
        });
    }

    fn add_boss_luck_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "boss_luck.png");
            ui.label("Boss Luck: ");
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.boss_luck_increase, 0, "None");
            ui.selectable_value(&mut self.boss_luck_increase, 1, "I (+1)");
            ui.selectable_value(&mut self.boss_luck_increase, 3, "II (+3)");
            ui.selectable_value(&mut self.boss_luck_increase, 5, "III (+5)");
            ui.selectable_value(&mut self.boss_luck_increase, 10, "IV (+10)");
        });
    }

    fn add_s_plus_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "s_plus.png");
            ui.label("S+: ");
        });

        let require_s_plus = self.require_s_plus();
        if require_s_plus {
            let checkbox = Checkbox::new(&mut self.forced_s_plus_const, "Chest type requires S+");
            ui.add_enabled(false, checkbox);
        } else {
            let checkbox = Checkbox::new(&mut self.s_plus, "Click to toggle");
            ui.add(checkbox);
        }
    }

    fn add_floor_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "catacombs.png");
            ui.label("Floor: ");
        });
        egui::ComboBox::from_label("Select a floor")
            .selected_text(catacombs_loot::floor_to_text(
                self.floor.as_deref().unwrap_or("None").to_string(),
            ))
            .show_ui(ui, |ui| {
                for floor in self.loot.keys() {
                    let floor_label = egui::SelectableLabel::new(self.floor == Some(floor.clone()), catacombs_loot::floor_to_text(floor.clone()));
                    if ui.add(floor_label).clicked() {
                        self.floor = Some(floor.clone());

                        if let Some(current_chest) = self.chest.as_ref() {
                            if let Some(new_floor_chests) = self.loot.get(floor) {
                                self.chest = match_chest_type_or_none(current_chest, new_floor_chests);
                            }
                        }

                        // try and find the entry of the same type from the new chest
                        if self.rng_meter_data.selected_item.is_none() {
                            continue;
                        }

                        let selected_item_data = self.rng_meter_data.selected_item.as_mut().unwrap();
                        let selected_xp = self.rng_meter_data.selected_xp;

                        let highest_tier_chest = self.loot.get(floor).and_then(|v| v.last()).unwrap();
                        let highest_tier_chest_total_weight: i32 = highest_tier_chest.loot.iter().map(|e| e.get_weight() as i32).sum();

                        let mut reset_selected = true;
                        for replacement_entry in highest_tier_chest.loot.iter() {
                            if replacement_entry.to_string() == selected_item_data.identifier {
                                let item_weight = replacement_entry.get_weight();
                                let required_xp: i32 = (300.0 * (highest_tier_chest_total_weight as f32 / item_weight as f32)).round() as i32;

                                let lowest_match = find_matching_item_from_lowest_chest(replacement_entry, self.loot.get(floor).unwrap())
                                    .unwrap();

                                selected_item_data.lowest_tier_chest_type = lowest_match.0.chest_type.clone();
                                selected_item_data.lowest_tier_chest_entry = Rc::clone(lowest_match.1);

                                selected_item_data.required_xp = required_xp;
                                selected_item_data.highest_tier_chest_entry = Rc::clone(replacement_entry);
                                self.rng_meter_data.selected_xp = selected_xp.min(required_xp);

                                reset_selected = false;
                            }
                        }

                        if reset_selected {
                            self.rng_meter_data.selected_xp = 0;
                            self.rng_meter_data.selected_item = None;
                        }
                    }
                }
            });
    }

    fn add_chest_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "catacombs.png");
            ui.label("Chest2: ");
        });

        egui::ComboBox::from_label("Select a chest")
            .height(400.0)
            .selected_text(
                self.chest
                    .as_ref()
                    .map(|c| &c.chest_type)
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_else(|| "None".to_string()),
            )
            .show_ui(ui, move |ui| {
                let default = String::new();
                let floor = self.floor.as_ref().unwrap_or(&default);

                let default = Vec::new();
                for chest in self.loot.get(floor).unwrap_or(&default).iter() {
                    // ui.selectable_value(&mut self.chest, Some(chest.clone()), format!("{:?}", chest.chest_type));
                    let label = egui::SelectableLabel::new(self.chest == Some(chest.clone()), format!("{:?}", chest.chest_type));
                    if ui.add(label).clicked() {
                        self.chest = Some(chest.clone());
                    }
                }
            });
    }

    fn add_rng_meter_section(&mut self, ui: &mut Ui) {
        if self.floor.is_none() {
            return;
        }
        let floor = self.floor.as_ref().unwrap();
        let highest_tier_chest = self.loot.get(floor).unwrap().last().unwrap();
        let total_weight: i32 = highest_tier_chest.loot.iter().map(|e| e.get_weight() as i32).sum();

        ui.heading("RNG Meter");
        ui.end_row();

        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "painting.png");
            ui.label("Item: ");
        });

        let selected_item_string = self.rng_meter_data.selected_item
            .as_ref()
            .map(|entry| {
                let required_xp: i32 = (300.0 * (total_weight as f32 / entry.highest_tier_chest_entry.get_weight() as f32)).round() as i32;
                let mut text = RichText::new(format!("{} ({} XP)", entry.highest_tier_chest_entry, required_xp.to_formatted_string(&Locale::en)));

                if self.chest.is_some() && !self.chest.as_ref().unwrap().has_rng_entry(entry) {
                    text = text.strikethrough();
                }

                text
            })
            .unwrap_or(RichText::new(String::from("None")));

        egui::ComboBox::from_label("Select an item")
            .selected_text(selected_item_string)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.rng_meter_data.selected_item, None, "None");

                let mut sorted_loot = highest_tier_chest.loot.iter().map(|e| {
                    if let Some(chest) = &self.chest {
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

                    let selected = self.rng_meter_data.selected_item.as_ref().map_or("", |e| &e.identifier) == entry.to_string();
                    let mut text = RichText::new(format!("{} ({} XP)", entry, required_xp.to_formatted_string(&Locale::en)));

                    if !in_loot {
                        text = text.strikethrough(); // easier way to distinguish entries that don't apply
                    }

                    let label = egui::SelectableLabel::new(selected, text);
                    if ui.add(label).clicked() {
                        let rng_meter_data = &mut self.rng_meter_data;

                        let lowest_match = find_matching_item_from_lowest_chest(entry, self.loot.get(floor).unwrap())
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

        if let Some(selected_item_data) = self.rng_meter_data.selected_item.as_ref() {
            let selected_item = &selected_item_data.lowest_tier_chest_entry;
            ui.horizontal(|ui| {
                images::add_first_valid_image(&self.images, ui, selected_item.get_possible_file_names());
                ui.label("XP: ");
            });

            let required_xp = selected_item_data.required_xp;
            let percent = 100.0 * self.rng_meter_data.selected_xp as f32 / required_xp as f32;

            ui.add(egui::Slider::new(&mut self.rng_meter_data.selected_xp, 0..=required_xp)
                .suffix(format!(" XP ({:.2}%)", percent)));

            let mut add_switch_to_lowest_chest_button = false;
            let mut text_to_add: Option<String> = None;
            if let Some(chest) = &self.chest {
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
                        let lowest_tier_chest = self.loot.get(floor)
                            .unwrap()
                            .iter()
                            .find(|c| c.chest_type == selected_item_data.lowest_tier_chest_type)
                            .unwrap();

                        self.chest = Some(Rc::clone(lowest_tier_chest));
                    }
                }
            }
        }

        ui.end_row();
    }

    fn add_loot_section(&mut self, ui: &mut Ui) {
        let chances = self.get_chances();
        if chances.is_none() {
            return;
        }
        let chances = chances.unwrap();

        let text_height = TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let chest = self.chest.as_ref().unwrap();
        let starting_quality = calculate_quality(
            chest,
            self.treasure_accessory_multiplier,
            self.boss_luck_increase,
            self.s_plus || self.require_s_plus(),
        );

        let available_height = ui.available_size_before_wrap().y;
        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .drag_to_scroll(false)
            .min_scrolled_height(available_height);
        //.drag_to_scroll(true)
            //.min_scrolled_height(available_height)
            //.max_scroll_height(f32::INFINITY);

        table.header(20.0, |mut header| {
            header.col(|ui| { ui.strong("Entry"); });
            header.col(|ui| { ui.strong("Coins Cost"); });
            header.col(|ui| { ui.strong(format!("Quality ({})", starting_quality)); });
            header.col(|ui| {
                ui.strong(format!("Weight ({})", format!("{:.1$}", chances.total_weight, 2).trim_end_matches('0').trim_end_matches('.')));
            });
            header.col(|ui| { ui.strong("First Roll Chance"); });
            header.col(|ui| { ui.strong("Average Chance"); });
        }).body(|mut body| {
            let rng_meter_entry = if let Some(rng_entry) = &self.rng_meter_data.selected_item {
                if self.rng_meter_data.selected_xp >= rng_entry.required_xp {
                    // entry is only guaranteed in the lowest tier chest, although boosted in all chest tiers
                    chances.chances.iter().find(|e| e.entry == rng_entry.lowest_tier_chest_entry)
                } else {
                    None
                }
            } else {
                None
            };

            for entry in chances.chances.iter() {
                let weight = entry.used_weight;
                let chance = entry.chance;
                let entry = &entry.entry;

                if chance == 0.0 {
                    continue;
                }

                body.row(text_height, |mut row| {
                    row.col(|ui| {
                        images::add_first_valid_image(&self.images, ui, entry.get_possible_file_names());

                        let text = entry.to_string();
                        let page_url = entry.get_wiki_page_name();
                        ui.hyperlink_to(text, page_url);
                    });
                    row.col(|ui| {
                        ui.label(RichText::new((chest.base_cost + entry.get_added_chest_price()).to_formatted_string(&Locale::en)).color(Color32::from_rgb(255, 170, 0)));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(format!("{}", entry.get_quality())).color(Color32::from_rgb(85, 255, 255)));
                    });
                    row.col(|ui| {
                        let text = RichText::new(format!("{:.3}", weight).trim_end_matches('0').trim_end_matches('.'));
                        ui.label(text.color(Color32::from_rgb(85, 255, 255))).on_hover_text(format!("More Decimals: {}", weight));
                    });

                    row.col(|ui| {
                        let first_roll_chance: f64 = if let Some(rng_entry) = rng_meter_entry {
                            if rng_entry.entry == *entry { 1.0 } else { 0.0 }
                        } else {
                            weight / chances.total_weight
                        };
                        fill_in_chance_column(ui, first_roll_chance);
                    });

                    row.col(|ui| {
                        fill_in_chance_column(ui, chance);
                    });
                });
            }
        });
    }

    fn require_s_plus(&self) -> bool {
        if self.chest.is_none() {
            return false;
        }

        let chest = self.chest.as_ref().unwrap();
        chest.chest_type == ChestType::Bedrock && (chest.floor == 5 || chest.floor == 6 || (chest.floor == 7 && !chest.master_mode))
    }

    fn generate_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.s_plus || self.require_s_plus()).hash(&mut hasher);
        self.treasure_accessory_multiplier.to_string().hash(&mut hasher);
        self.boss_luck_increase.hash(&mut hasher);
        self.floor.hash(&mut hasher);
        self.chest.hash(&mut hasher);
        self.rng_meter_data.selected_xp.hash(&mut hasher);
        self.rng_meter_data.selected_item.hash(&mut hasher);
        hasher.finish()
    }

    fn get_chances(&self) -> Option<&CalculationResult> {
        let hash = self.generate_hash();
        self.hashed_chances.get(&hash)
    }
}

fn fill_in_chance_column(ui: &mut Ui, chance: f64) {
    let width = ui.fonts(|f| f.glyph_width(&TextStyle::Body.resolve(ui.style()), ' '));
    ui.spacing_mut().item_spacing.x = width;

    ui.label(RichText::new(format!("{:.4}%", chance * 100.0)).color(Color32::from_rgb(85, 255, 85)));
    ui.label(" (");
    ui.label(RichText::new("1").color(Color32::from_rgb(85, 255, 85)));
    ui.label(" in ");
    ui.label(RichText::new(format!("{:.3}", 1.0 / chance)).color(Color32::from_rgb(255, 255, 85)));
    ui.label(" runs)");
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