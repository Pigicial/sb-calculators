use crate::loot::{ChestType, LootChest, LootEntry};
use crate::loot;
use egui::{vec2, widget_text, Checkbox, Context, Grid, Image, ScrollArea, TextStyle, ThemePreference, Ui, Widget};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use eframe::epaint::{Color32, TextureHandle};
use egui::scroll_area::ScrollBarVisibility;
use egui_extras::{Column, TableBuilder};
use egui_extras::image::load_image_bytes;
use include_dir::{include_dir, Dir};
use num_format::{Locale, ToFormattedString};
use crate::loot_calculator::{calculate_chances, calculate_weight, CalculationResult, LootChanceEntry, RngMeterData};

static ASSETS_DIR: Dir = include_dir!("assets");

pub struct LootApp {
    floor: Option<String>,
    chest: Option<Rc<LootChest>>,

    treasure_accessory_multiplier: f32,
    boss_luck_increase: u8,
    s_plus: bool,
    forced_s_plus_const: bool,

    rng_meter_data: RngMeterData,

    hashed_chances: HashMap<u64, CalculationResult>,

    images: HashMap<String, TextureHandle>,
    loot: HashMap<String, Vec<Rc<LootChest>>>,
}

impl eframe::App for LootApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                } else {
                    egui::gui_zoom::zoom_menu_buttons(ui);
                }

                ui.ctx().set_theme(ThemePreference::Dark);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            self.add_regular_settings_section(ui);
            ui.separator();

            if self.floor.is_some() && self.chest.is_some() {
                let hash = self.generate_hash();
                let chances = self.get_chances();

                if chances.is_some() {
                    self.add_loot_section(ui);
                } else {
                    let chest = self.chest.as_ref().unwrap();
                    let starting_quality = calculate_weight(
                        chest,
                        self.treasure_accessory_multiplier,
                        self.boss_luck_increase,
                        self.s_plus || self.require_s_plus(),
                    );

                    let new_chances = calculate_chances(chest, starting_quality, &self.rng_meter_data);
                    self.hashed_chances.insert(hash, new_chances);

                    self.add_loot_section(ui);
                }
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                // powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
            ui.max_rect().with_max_x(200.0)
        });
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

impl LootApp {
    pub fn new(context: &Context) -> Self {
        let mut app = Self {
            floor: None,
            chest: None,

            treasure_accessory_multiplier: 1.0,
            boss_luck_increase: 0,
            s_plus: false,
            forced_s_plus_const: true,

            rng_meter_data: Default::default(),

            hashed_chances: HashMap::new(),

            images: HashMap::new(),
            loot: loot::read_all_chests(&ASSETS_DIR)
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(Rc::new).collect()))
                .collect(),
        };
        app.load_images(context);
        app
    }

    fn add_regular_settings_section(&mut self, ui: &mut Ui) {
        ui.heading("Options");

        Grid::new("config_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
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
            self.add_image(ui, "treasure_talisman.png");
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
            self.add_image(ui, "boss_luck.png");
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
            self.add_image(ui, "s_plus.png");
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
            self.add_image(ui, "catacombs.png");
            ui.label("Floor: ");
        });
        egui::ComboBox::from_label("Select a floor")
            .selected_text(loot::floor_to_text(
                self.floor.as_deref().unwrap_or("None").to_string(),
            ))
            .show_ui(ui, |ui| {
                for floor in self.loot.keys() {
                    let floor_label = egui::SelectableLabel::new(self.floor == Some(floor.clone()), loot::floor_to_text(floor.clone()));
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
                        let selected_item = Rc::clone(self.rng_meter_data.selected_item.as_ref().unwrap());
                        let selected_xp = self.rng_meter_data.selected_xp;
                        let highest_tier_chest = self.loot.get(floor).and_then(|v| v.last()).unwrap();
                        let total_weight: i32 = highest_tier_chest.loot.iter().map(|e| e.get_weight() as i32).sum();

                        self.rng_meter_data.selected_item = None;
                        self.rng_meter_data.required_xp = None;
                        self.rng_meter_data.selected_xp = 0;

                        for replacement_entry in highest_tier_chest.loot.iter() {
                            if replacement_entry.to_string() == selected_item.to_string() {
                                let item_weight = replacement_entry.get_weight();
                                let required_xp: i32 = (300.0 * (total_weight as f32 / item_weight as f32)).round() as i32;

                                self.rng_meter_data.selected_item = Some(Rc::clone(replacement_entry));
                                self.rng_meter_data.required_xp = Some(required_xp);
                                self.rng_meter_data.selected_xp = selected_xp.min(required_xp);
                            }
                        }
                    }
                }
            });
    }

    fn add_chest_options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            self.add_image(ui, "catacombs.png");
            ui.label("Chest: ");
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
        let highest_tier_chest = if let Some(floor) = &self.floor {
            self.loot.get(floor).unwrap().last()
        } else {
            return;
        };

        let highest_tier_chest = highest_tier_chest.unwrap();
        let total_weight: i32 = highest_tier_chest.loot.iter().map(|e| e.get_weight() as i32).sum();

        ui.heading("RNG Meter");
        ui.end_row();

        ui.horizontal(|ui| {
            self.add_image(ui, "painting.png");
            ui.label("Item: ");
        });
        
        egui::ComboBox::from_label("Select an item")
            .selected_text(loot::floor_to_text(
                self.rng_meter_data.selected_item.as_deref().map(|entry| {
                    let required_xp: i32 = (300.0 * (total_weight as f32 / entry.get_weight() as f32)).round() as i32;
                    format!("{} ({} XP)", entry, required_xp.to_formatted_string(&Locale::en))
                }).unwrap_or(String::from("None")).to_string(),
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.rng_meter_data.selected_item, None, "None");

                for entry in highest_tier_chest.loot.iter() {
                    if entry.is_essence_and_can_roll_multiple_times() { // essence doesn't show in rng meter
                        continue;
                    }
                    let item_weight = entry.get_weight();
                    let required_xp: i32 = (300.0 * (total_weight as f32 / item_weight as f32)).round() as i32;

                    let text = format!("{} ({} XP)", entry, required_xp.to_formatted_string(&Locale::en));
                    let label = egui::SelectableLabel::new(self.rng_meter_data.selected_item == Some(Rc::clone(entry)), text);
                    if ui.add(label).clicked() {
                        let rng_meter_data = &mut self.rng_meter_data;
                        rng_meter_data.selected_item = Some(Rc::clone(entry));
                        rng_meter_data.required_xp = Some(required_xp);
                        rng_meter_data.selected_xp = rng_meter_data.selected_xp.min(required_xp);
                    }
                }
            });

        ui.end_row();
        let selected_item = &self.rng_meter_data.selected_item;
        if selected_item.is_some() {
            ui.horizontal(|ui| {
                self.add_first_valid_image(ui, selected_item.as_ref().unwrap().get_possible_file_names());
                ui.label("XP: ");
            });

            let required_xp = self.rng_meter_data.required_xp.unwrap();
            let xp_and_percent = format!(" XP ({:.2}%)", 100.0 * self.rng_meter_data.selected_xp as f32 / required_xp as f32);
            ui.add(egui::Slider::new(&mut self.rng_meter_data.selected_xp, 0..=required_xp).suffix(xp_and_percent));
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
        let starting_quality = calculate_weight(
            chest,
            self.treasure_accessory_multiplier,
            self.boss_luck_increase,
            self.s_plus || self.require_s_plus(),
        );

        ScrollArea::vertical()
            .auto_shrink(false)
            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
            .stick_to_right(true)
            .show(ui, |ui| {
                let available_height = ui.available_height();
                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .min_scrolled_height(0.0)
                    .max_scroll_height(available_height);

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
                        if self.rng_meter_data.selected_xp >= self.rng_meter_data.required_xp.unwrap() {
                            chances.chances.iter().find(|e| e.entry.to_string() == rng_entry.to_string())
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
                                self.add_first_valid_image(ui, entry.get_possible_file_names());
                                ui.label(entry.to_string());
                            });
                            row.col(|ui| {
                                ui.label(widget_text::RichText::new((chest.base_cost + entry.get_added_chest_price()).to_formatted_string(&Locale::en)).color(Color32::from_rgb(255, 170, 0)));
                            });
                            row.col(|ui| {
                                ui.label(widget_text::RichText::new(format!("{}", entry.get_quality())).color(Color32::from_rgb(85, 255, 255)));
                            });
                            row.col(|ui| {
                                let text = widget_text::RichText::new(format!("{:.3}", weight).trim_end_matches('0').trim_end_matches('.'));
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
            });
        ui.separator();
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

    fn load_images(&mut self, ctx: &Context) {
        for file in ASSETS_DIR.find("**/*.png").unwrap().chain(ASSETS_DIR.find("**/*.gif").unwrap()) {
            let file_name = file.path().file_name().and_then(|n| n.to_str()).unwrap();
            println!("Loading {}", file_name);
            let bytes = file.as_file().unwrap().contents();

            if let Ok(dynamic_image) = load_image_bytes(bytes) {
                let texture = ctx.load_texture(file_name, dynamic_image, egui::TextureOptions::default());
                self.images.insert(file_name.to_string(), texture);
            }
        }
    }

    fn add_image(&self, ui: &mut Ui, file_name: &str) {
        let texture = self.images.get(file_name);
        if let Some(texture) = texture {
            Image::new(texture).fit_to_exact_size(vec2(18.0, 18.0)).ui(ui);
        }
    }

    fn add_first_valid_image(&self, ui: &mut Ui, possible_file_names: Vec<String>) {
        for file_name in possible_file_names {
            let png_texture = self.images.get(&file_name);
            if let Some(texture) = png_texture { //test
                Image::new(texture).fit_to_exact_size(vec2(18.0, 18.0)).ui(ui);
                break;
            }
        }
    }
}

fn fill_in_chance_column(ui: &mut Ui, chance: f64) {
    let width = ui.fonts(|f| f.glyph_width(&TextStyle::Body.resolve(ui.style()), ' '));
    ui.spacing_mut().item_spacing.x = width;

    ui.label(widget_text::RichText::new(format!("{:.4}%", chance * 100.0)).color(Color32::from_rgb(85, 255, 85)));
    ui.label(" (");
    ui.label(widget_text::RichText::new("1").color(Color32::from_rgb(85, 255, 85)));
    ui.label(" in ");
    ui.label(widget_text::RichText::new(format!("{:.3}", 1.0 / chance)).color(Color32::from_rgb(255, 255, 85)));
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

fn match_item_or_none(entry: &Option<Rc<LootEntry>>, others: &Vec<LootChanceEntry>) -> Option<Rc<LootEntry>> {
    if entry.is_some() {
        let entry = entry.as_ref()?;
        for other_entry in others {
            if entry.to_string() == other_entry.entry.to_string() {
                return Some(Rc::clone(&other_entry.entry));
            }
        }
    }

    None
}

fn powered_by_egui_and_eframe(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
