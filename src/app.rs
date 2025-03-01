use crate::loot::{ChestType, LootChest, LootEntry};
use crate::{loot, loot_calculator};
use egui::{vec2, widget_text, Checkbox, Context, Grid, Image, ScrollArea, TextStyle, Ui, Widget};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use eframe::epaint::{Color32, TextureHandle};
use egui::scroll_area::ScrollBarVisibility;
use egui_extras::{Column, TableBuilder};
use egui_extras::image::load_image_bytes;
use include_dir::{include_dir, Dir};
use crate::loot_calculator::{calculate_chances, calculate_weight, RngMeterData};

static ASSETS_DIR: Dir = include_dir!("assets");

pub struct LootApp {
    floor: Option<String>,
    chest: Option<Rc<LootChest>>,

    treasure_accessory_multiplier: f32,
    boss_luck_increase: u8,
    s_plus: bool,
    forced_s_plus_const: bool,

    rng_meter_data: RngMeterData,

    hashed_chances: HashMap<u64, Vec<(Rc<LootEntry>, f64)>>,

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
                }

                egui::widgets::global_theme_preference_buttons(ui);
                ui.add_space(16.0);
                egui::gui_zoom::zoom_menu_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            self.add_settings_section(ui);
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

                    let new_chances = calculate_chances(chest, starting_quality);
                    self.hashed_chances.insert(hash, new_chances);

                    self.add_loot_section(ui);
                }
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                powered_by_egui_and_eframe(ui);
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

    fn add_settings_section(&mut self, ui: &mut Ui) {
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
                    let label = egui::SelectableLabel::new(self.floor == Some(floor.clone()), loot::floor_to_text(floor.clone()));
                    if ui.add(label).clicked() {
                        self.floor = Some(floor.clone());

                        if self.chest.is_none() {
                            continue;
                        }
                        let current_chest = self.chest.as_ref().unwrap();

                        if let Some(new_floor_chests) = self.loot.get(floor) {
                            self.chest = match_chest_type_or_none(current_chest, new_floor_chests);
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
                    ui.selectable_value(&mut self.chest, Some(chest.clone()), format!("{:?}", chest.chest_type));
                }
            });
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
        let starting_quality = loot_calculator::calculate_weight(
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
                    .min_scrolled_height(0.0)
                    .max_scroll_height(available_height);

                table.header(20.0, |mut header| {
                    header.col(|ui| { ui.strong("Entry"); });
                    header.col(|ui| { ui.strong(format!("Quality ({})", starting_quality)); });
                    header.col(|ui| { ui.strong("Weight"); });
                    header.col(|ui| { ui.strong("First Roll Chance").on_hover_text("As shown in the RNG Meter"); });
                    header.col(|ui| { ui.strong("Chance"); });
                }).body(|mut body| {
                    for (entry, chance) in chances {
                        body.row(text_height, |mut row| {
                            row.col(|ui| {
                                self.add_first_valid_image(ui, entry.get_possible_file_names());
                                ui.label(entry.to_string());
                            });
                            row.col(|ui| {
                                ui.label(widget_text::RichText::new(format!("{}", entry.get_quality())).color(Color32::from_rgb(85, 255, 255)));
                            });
                            row.col(|ui| {
                                ui.label(widget_text::RichText::new(format!("{}", entry.get_weight())).color(Color32::from_rgb(85, 255, 255)));
                            });

                            row.col(|ui| {
                                let width = ui.fonts(|f| f.glyph_width(&TextStyle::Body.resolve(ui.style()), ' '));
                                ui.spacing_mut().item_spacing.x = width;

                                ui.label(widget_text::RichText::new(format!("{:.4}%", chance * 100.0)).color(Color32::from_rgb(85, 255, 85)));
                                ui.label(" (");
                                ui.label(widget_text::RichText::new("1").color(Color32::from_rgb(85, 255, 85)));
                                ui.label(" in ");
                                ui.label(widget_text::RichText::new(format!("{:.3}", 1.0 / chance)).color(Color32::from_rgb(255, 255, 85)));
                                ui.label(" runs)");
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
        hasher.finish()
    }

    fn get_chances(&self) -> Option<&Vec<(Rc<LootEntry>, f64)>> {
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
            if let Some(texture) = png_texture {
                Image::new(texture).fit_to_exact_size(vec2(18.0, 18.0)).ui(ui);
                break;
            }
        }
    }
}

fn match_chest_type_or_none(chest: &Rc<LootChest>, others: &Vec<Rc<LootChest>>) -> Option<Rc<LootChest>> {
    for other_chest in others {
        if chest.chest_type == other_chest.chest_type {
            return Some(Rc::clone(other_chest));
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
