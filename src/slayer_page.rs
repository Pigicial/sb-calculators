use crate::slayer_loot::{DropType, LootEntry, LootTable};
use crate::slayer_loot_calculator::{calculate_chances, LootChanceEntry, RngMeterData, SelectedRngMeterItem};
use crate::{app, images, slayer_loot};
use eframe::epaint::{Color32, TextureHandle};
use egui::{Context, Grid, Label, RichText, ScrollArea, TextStyle, TextWrapMode, Ui};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use num_format::{Locale, ToFormattedString};
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

pub struct SlayerLootApp {
    boss_type: Option<String>,
    loot_table: Option<Rc<LootTable>>,

    slayer_level: u8,
    magic_find: f32,

    rng_meter_data: RngMeterData,

    hashed_chances: HashMap<u64, Vec<LootChanceEntry>>,
    loot: BTreeMap<String, Vec<Rc<LootTable>>>,

    images: Rc<HashMap<String, TextureHandle>>,
}

impl eframe::App for SlayerLootApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::CentralPanel::default().show(ctx, |ui| {
            StripBuilder::new(ui)
                .size(Size::relative(0.1)) // for the options
                .size(Size::relative(0.9)) // for the dungeon_loot
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        ScrollArea::horizontal().show(ui, |ui| {
                            self.add_regular_settings_section(ui);
                        });
                        ui.separator();
                    });

                    if self.boss_type.is_some() && self.loot_table.is_some() {
                        strip.cell(|ui| {
                            ScrollArea::both().show(ui, |ui| {
                                let hash = self.generate_hash();
                                let chances = self.get_chances();

                                if chances.is_none() {
                                    let chest = self.loot_table.as_ref().unwrap();
                                    let new_chances = calculate_chances(chest, self.magic_find, self.slayer_level, &self.rng_meter_data);
                                    self.hashed_chances.insert(hash, new_chances);
                                }

                                self.add_loot_section(ui);
                            });
                        });
                    }
                });

            ui.max_rect().with_max_x(200.0)
        });
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

impl SlayerLootApp {
    pub fn new(images: Rc<HashMap<String, TextureHandle>>) -> Self {
        Self {
            boss_type: None,
            loot_table: None,

            slayer_level: 9,
            magic_find: 0.0,
            rng_meter_data: Default::default(),

            hashed_chances: HashMap::new(),

            loot: slayer_loot::read_all_loot(&app::ASSETS_DIR)
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(Rc::new).collect()))
                .collect(),
            images
        }
    }

    fn add_regular_settings_section(&mut self, ui: &mut Ui) {
        ui.heading("Options");

        Grid::new("config_grid")
            .num_columns(2)
            .spacing([15.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                self.add_boss_type_options(ui);
                ui.end_row();
                self.add_boss_tier_options(ui);
                ui.end_row();
                self.add_slayer_level_option(ui);
                ui.end_row();
                self.add_magic_find_options(ui);
                ui.end_row();

                self.add_rng_meter_section(ui);
            });
    }

    fn add_magic_find_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "magic_find.png");
            ui.label("Magic Find: ");
        });

        ui.add(egui::DragValue::new(&mut self.magic_find)
            .speed(0.5)
            .range(0.0..=900.0));
    }

    fn add_boss_type_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "slayer.png");
            ui.label("Boss: ");
        });
        let default = String::from("None");
        egui::ComboBox::from_label("Select the boss")
            .selected_text(self.boss_type.as_ref().unwrap_or(&default))
            .show_ui(ui, |ui| {
                for boss_type in self.loot.keys() {
                    let boss_label = egui::SelectableLabel::new(self.boss_type == Some(boss_type.clone()), boss_type.clone());
                    if ui.add(boss_label).clicked() {
                        self.boss_type = Some(boss_type.clone());

                        if let Some(current_chest) = self.loot_table.as_ref() {
                            if let Some(new_floor_chests) = self.loot.get(boss_type) {
                                self.loot_table = match_loot_type_or_none(current_chest, new_floor_chests);
                            }
                        }

                        // try and find the entry of the same type from the new chest
                        if self.rng_meter_data.selected_item.is_none() {
                            continue;
                        }

                        let selected_item_data = self.rng_meter_data.selected_item.as_mut().unwrap();
                        let selected_xp = self.rng_meter_data.selected_xp;

                        let highest_tier_chest = self.loot.get(boss_type).and_then(|v| v.last()).unwrap();
                        let highest_tier_chest_total_weight: i32 = highest_tier_chest.loot.iter().map(|e| e.get_weight() as i32).sum();

                        let mut reset_selected = true;
                        for replacement_entry in highest_tier_chest.loot.iter() {
                            if replacement_entry.to_string() == selected_item_data.identifier {
                                let item_weight = replacement_entry.get_weight();
                                let required_xp: i32 = (300.0 * (highest_tier_chest_total_weight as f32 / item_weight as f32)).round() as i32;

                                let lowest_match = find_matching_item_from_lowest_chest(replacement_entry, self.loot.get(boss_type).unwrap())
                                    .unwrap();

                                selected_item_data.lowest_boss_level = lowest_match.0.boss_tier.clone();
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

    fn add_boss_tier_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "slayer_tiers.png");
            ui.label("Tier: ");
        });

        let selected_text = self.loot_table
            .as_ref()
            .map(|c| format!("Tier {:?}", &c.boss_tier))
            .unwrap_or_else(|| "None".to_string());

        egui::ComboBox::from_label("Select the boss tier")
            .height(400.0)
            .selected_text(selected_text, )
            .show_ui(ui, move |ui| {
                let default = String::new();
                let floor = self.boss_type.as_ref().unwrap_or(&default);

                let default = Vec::new();
                for loot_table in self.loot.get(floor).unwrap_or(&default).iter() {
                    // ui.selectable_value(&mut self.loot_table, Some(chest.clone()), format!("{:?}", chest.boss_tier));
                    let label = egui::SelectableLabel::new(self.loot_table == Some(loot_table.clone()), format!("Tier {:?}", loot_table.boss_tier));
                    if ui.add(label).clicked() {
                        self.loot_table = Some(loot_table.clone());
                    }
                }
            });
    }

    fn add_slayer_level_option(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            images::add_image(&self.images, ui, "slayer_level.png");
            ui.label("Slayer Level: ");
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.slayer_level, 0..=9).step_by(1.0));
        });
    }

    fn add_rng_meter_section(&mut self, ui: &mut Ui) {
        if self.boss_type.is_none() {
            return;
        }
        let floor = self.boss_type.as_ref().unwrap();
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
                // todo fix required xp logic
                let required_xp: i32 = (300.0 * (total_weight as f32 / entry.highest_tier_chest_entry.get_weight() as f32)).round() as i32;
                let mut text = RichText::new(format!("{} ({} XP)", entry.highest_tier_chest_entry, required_xp.to_formatted_string(&Locale::en)));

                if self.loot_table.is_some() && !self.loot_table.as_ref().unwrap().has_rng_entry(entry) {
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
                    if let Some(chest) = &self.loot_table {
                        (e, chest.has_matching_entry_type(e))
                    } else {
                        (e, true)
                    }
                }).collect::<Vec<(&Rc<LootEntry>, bool)>>();
                sorted_loot.sort_by(|a, b| b.1.cmp(&a.1));

                for entry in sorted_loot {
                    let in_loot = entry.1;
                    let entry = entry.0;

                    if entry.get_drop_type() == &DropType::Token { // essence doesn't show in rng meter
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
                            highest_boss_level: highest_tier_chest.boss_tier,
                            lowest_tier_chest_entry: Rc::clone(lowest_match.1),
                            lowest_boss_level: lowest_match.0.boss_tier,
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
            if let Some(chest) = &self.loot_table {
                if !chest.has_rng_entry(selected_item_data) {
                    if selected_item_data.lowest_boss_level == selected_item_data.highest_boss_level {
                        text_to_add = Some(format!("This entry only drops from tier {:?} loot.", selected_item_data.lowest_boss_level));
                    } else {
                        text_to_add = Some(format!("This entry only drops from the tier {:?} to {:?} loot.",
                                                   selected_item_data.lowest_boss_level,
                                                   selected_item_data.highest_boss_level));
                    }
                    add_switch_to_lowest_chest_button = true;
                } else if percent >= 100.0 {
                    text_to_add = Some("This entry is guaranteed to appear from this loot.".to_string());
                } else {
                    text_to_add = Some("At 100%, this entry is guaranteed to appear from this loot.".to_string());
                }
            }

            if let Some(text_to_add) = text_to_add {
                ui.end_row();
                ui.label("");
                ui.add(Label::new(text_to_add).wrap_mode(TextWrapMode::Wrap));

                if add_switch_to_lowest_chest_button {
                    let button_text = format!("Switch to {:?}", selected_item_data.lowest_boss_level);
                    if ui.button(button_text).clicked() {
                        let lowest_tier_chest = self.loot.get(floor)
                            .unwrap()
                            .iter()
                            .find(|c| c.boss_tier == selected_item_data.lowest_boss_level)
                            .unwrap();

                        self.loot_table = Some(Rc::clone(lowest_tier_chest));
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

        let available_height = ui.available_height();
        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto().clip(false))
            .column(Column::auto())
            .drag_to_scroll(true)
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        table.header(20.0, |mut header| {
            header.col(|ui| { ui.strong("Entry"); });
            header.col(|ui| { ui.strong("Loot Table"); });
            header.col(|ui| { ui.strong("Requirement"); });
            header.col(|ui| { ui.strong("Weight"); });
            header.col(|ui| { ui.strong("Average Chance"); });
        }).body(|mut body| {
            for entry in chances.iter() {
                let weight = entry.used_weight * entry.magic_find_multiplier;
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
                        ui.label(format!("{:?}", entry.get_drop_type()));
                    });
                    row.col(|ui| {
                        if entry.get_slayer_level_requirement() == 0 {
                            ui.label("None");
                        } else {
                            ui.label(format!("{} Slayer {}", self.boss_type.as_ref().unwrap(), roman::to(entry.get_slayer_level_requirement() as i32).unwrap()));
                        }
                    });
                    row.col(|ui| {
                        let text = RichText::new(format!("{:.3}", weight).trim_end_matches('0').trim_end_matches('.'));
                        ui.label(text.color(Color32::from_rgb(85, 255, 255))).on_hover_text(format!("More Decimals: {}", weight));
                    });

                    row.col(|ui| {
                        fill_in_chance_column(ui, chance);
                    });
                });
            }

            for entry in &self.loot_table.as_ref().unwrap().loot {
                if self.slayer_level < entry.get_slayer_level_requirement() {

                }
            }
        });
    }

    fn generate_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.boss_type.hash(&mut hasher);
        self.loot_table.hash(&mut hasher);
        self.slayer_level.hash(&mut hasher);
        self.magic_find.to_string().hash(&mut hasher);
        self.rng_meter_data.selected_xp.hash(&mut hasher);
        self.rng_meter_data.selected_item.hash(&mut hasher);
        hasher.finish()
    }

    fn get_chances(&self) -> Option<&Vec<LootChanceEntry>> {
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

fn match_loot_type_or_none(chest: &Rc<LootTable>, others: &Vec<Rc<LootTable>>) -> Option<Rc<LootTable>> {
    for other_chest in others {
        if chest.boss_tier == other_chest.boss_tier {
            return Some(Rc::clone(other_chest));
        }
    }

    None
}

fn find_matching_item_from_lowest_chest<'a>(selected_item: &'a Rc<LootEntry>, loot_tables: &'a [Rc<LootTable>]) -> Option<(&'a Rc<LootTable>, &'a Rc<LootEntry>)> {
    let identifier = selected_item.to_string();
    // auto-sorted from lowest to highest
    for loot in loot_tables.iter() {
        if let Some(matching_entry) = loot.loot.iter().find(|e| e.to_string() == identifier) {
            return Some((loot, matching_entry));
        }
    }

    None
}