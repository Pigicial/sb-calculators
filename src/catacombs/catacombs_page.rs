use crate::catacombs::catacombs_loot::LootChest;
use crate::catacombs::catacombs_loot_calculator::{cache_chances_per_rng_meter_value, calculate_amount_of_times_rolled_for_entry, calculate_average_chances, calculate_quality, generate_random_table, AveragesCalculationResult, ChanceAndWeight, RandomlySelectedLootEntry, RngMeterCalculation, RngMeterData};
use crate::catacombs::catacombs_page::CalculatorType::{
    AveragesLootTable, RandomLootTable, RngMeterDeselection,
};
use crate::catacombs::{catacombs_loot, options};
use crate::images;
use eframe::epaint::{Color32, TextureHandle};
use egui::{Context, Grid, Label, RichText, ScrollArea, SidePanel, TextStyle, TextWrapMode, Ui};
use egui_extras::{Column, TableBuilder};
use egui_plot::LineStyle::Solid;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use include_dir::{include_dir, Dir};
use num_format::Locale::en;
use num_format::ToFormattedString;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

static ASSETS_DIR: Dir<'static> = include_dir!("assets");

pub struct CatacombsLootApp {
    pub floor: Option<String>,
    pub chest: Option<Rc<LootChest>>,

    pub treasure_accessory_multiplier: f64,
    pub boss_luck_increase: u8,
    pub s_plus: bool,
    pub forced_s_plus_const: bool,
    pub rng_meter_data: RngMeterData,

    pub calculator_type: CalculatorType,
    hashed_chances: HashMap<u64, AveragesCalculationResult>,
    random_table: Option<Vec<RandomlySelectedLootEntry>>,
    random_table_source_options_hash: Option<u64>,

    rng_meter_calculations: HashMap<u64, Vec<(f64, RngMeterCalculation)>>, // hash -> map of rng deactivate % -> calc
    rng_meter_calculation_cached_chances: HashMap<u64, Vec<(Rc<LootChest>, HashMap<i32, ChanceAndWeight>)>>,
    rng_meter_calculation_hash: Option<u64>,
    pub rng_meter_calculation_runs: i32,
    pub rng_meter_calculation_iterations: i32,
    pub rng_meter_calculation_use_kismet_feathers: bool,

    pub loot: BTreeMap<String, Vec<Rc<LootChest>>>,
    pub images: Rc<HashMap<String, TextureHandle>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CalculatorType {
    AveragesLootTable,
    RandomLootTable,
    RngMeterDeselection,
}

impl CalculatorType {
    pub(crate) fn should_display_rng_meter_section(&self) -> bool {
        self == &AveragesLootTable || self == &RandomLootTable
    }
}

impl eframe::App for CatacombsLootApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        SidePanel::left("cata_loot_config")
            .resizable(true)
            .default_width(500.0)
            .max_width(500.0)
            .show(ctx, |ui| {
                ScrollArea::horizontal().id_salt("cata_loot_config").show(ui, |ui| {
                    ui.heading("Options");
                    Grid::new("config_grid")
                        .num_columns(2)
                        .spacing([15.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            options::add_treasure_talisman_options(self, ui);
                            ui.end_row();
                            options::add_boss_luck_options(self, ui);
                            ui.end_row();
                            options::add_s_plus_options(self, ui);
                            ui.end_row();
                            options::add_floor_options(self, ui);
                            ui.end_row();
                            options::add_chest_options(self, ui);
                            ui.end_row();
                            options::add_rng_meter_options(self, ui);

                            if self.calculator_type == RngMeterDeselection {
                                options::add_rng_meter_simulation_options(self, ui);
                                ui.end_row();
                            }
                        });
                });
            });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.calculator_type, AveragesLootTable, "Loot Tables");
                ui.selectable_value(&mut self.calculator_type, RandomLootTable, "Casino");
                ui.selectable_value(&mut self.calculator_type, RngMeterDeselection, "RNG Meter Deselection Calculator");
            });
            ui.separator();

            if self.floor.is_none() || self.chest.is_none() {
                ui.label("Select a floor and chest to see its loot.");
                return;
            }

            match self.calculator_type {
                AveragesLootTable => {
                    let hash = self.generate_loot_table_hash();

                    let chances = self.get_loot_table_chances();
                    if chances.is_none() {
                        let chest = self.chest.as_ref().unwrap();
                        let starting_quality = calculate_quality(
                            chest,
                            self.treasure_accessory_multiplier,
                            self.boss_luck_increase,
                            self.s_plus || chest.require_s_plus(),
                        );

                        let new_chances = calculate_average_chances(chest, starting_quality, &self.rng_meter_data);
                        self.hashed_chances.insert(hash, new_chances);
                    }

                    // Horizontal scrolling is done here, vertical scrolling is done on the table scrolling end
                    // (this took painfully long to figure out)
                    ScrollArea::horizontal().id_salt("cata_loot").show(ui, |ui| {
                        self.add_loot_section(ui);
                    });
                }
                RandomLootTable => {
                    let hash = self.generate_loot_table_hash();
                    let current_hash = self.random_table_source_options_hash.unwrap_or(0);

                    let mut button_clicked = false;
                    ui.horizontal(|ui| {
                        if ui.button("Click to gamble!").clicked() {
                            button_clicked = true;
                        }
                        if hash != current_hash {
                            ui.add(Label::new("The settings used to generate this table don't match the current settings.").wrap_mode(TextWrapMode::Wrap));
                        }
                    });

                    if self.random_table.is_none() || button_clicked {
                        let chest = self.chest.as_ref().unwrap();
                        let starting_quality = calculate_quality(
                            chest,
                            self.treasure_accessory_multiplier,
                            self.boss_luck_increase,
                            self.s_plus || chest.require_s_plus(),
                        );

                        self.random_table = Some(generate_random_table(chest, starting_quality, &self.rng_meter_data));
                        self.random_table_source_options_hash = Some(hash);
                    }

                    // Horizontal scrolling is done here, vertical scrolling is done on the table scrolling end
                    // (this took painfully long to figure out)
                    ScrollArea::horizontal().id_salt("cata_random_loot").show(ui, |ui| {
                        self.add_random_loot_section(ui);
                    });
                }
                RngMeterDeselection => {
                    let selected_item_data = &self.rng_meter_data.selected_item;
                    if selected_item_data.is_none() {
                        return;
                    }
                    let selected_item_data = selected_item_data.as_ref().unwrap();
                    let selected_item = &selected_item_data.identifier;

                    let hash = self.generate_rng_meter_calculation_overall_hash();
                    let current_hash = self.rng_meter_calculation_hash.unwrap_or(0);

                    let mut button_clicked = false;
                    ui.horizontal(|ui| {
                        if ui.button("Click to generate!").clicked() {
                            button_clicked = true;
                        }
                        if hash != current_hash {
                            ui.add(Label::new("The settings used to generate this data don't match the current settings.").wrap_mode(TextWrapMode::Wrap));
                        }
                    });

                    let chests = find_chests_with_entry(selected_item, self.loot.get(self.floor.as_ref().unwrap()).unwrap());
                    let meter_xp = self.rng_meter_data.selected_xp;
                    let meter_data = self.rng_meter_data.selected_item.as_ref().unwrap();

                    let per_run_score_increase = match 300 {
                        s if s >= 300 => s,
                        s if s >= 270 => (s as f64 * 0.7) as i32,
                        _ => 0,
                    };

                    if button_clicked {
                        let chest_data_hash = self.generate_rng_meter_calculation_chests_and_item_hash();
                        if let Entry::Vacant(e) = self.rng_meter_calculation_cached_chances.entry(chest_data_hash) {
                            let mut chest_data: Vec<(Rc<LootChest>, HashMap<i32, ChanceAndWeight>)> = Vec::with_capacity(chests.len());

                            for chest in chests {
                                let chest_quality = calculate_quality(
                                    chest,
                                    self.treasure_accessory_multiplier,
                                    self.boss_luck_increase,
                                    self.s_plus || chest.require_s_plus(),
                                );

                                let rng_meter_cached_chances = cache_chances_per_rng_meter_value(chest, chest_quality, meter_xp, per_run_score_increase, meter_data);
                                chest_data.push((Rc::clone(chest), rng_meter_cached_chances));
                            }
                            e.insert(chest_data);
                        }
                        let chest_data = self.rng_meter_calculation_cached_chances.get(&chest_data_hash).unwrap();

                        let mut rng_meter_calculation = Vec::new();
                        for meter_deselection_threshold in 0..=100 {
                            println!("Calculating threshold {}", meter_deselection_threshold);
                            let meter_deselection_threshold = meter_deselection_threshold as f32 / 100.0;
                            let mut combined_calculations: RngMeterCalculation = Default::default();

                            for _ in 0..self.rng_meter_calculation_iterations {
                                let result = calculate_amount_of_times_rolled_for_entry(
                                    chest_data,
                                    self,
                                    self.rng_meter_calculation_runs,
                                    300,
                                    meter_deselection_threshold
                                );
                                match result {
                                    Ok(calculation) => {
                                        combined_calculations += calculation;
                                    }
                                    Err(message) => {
                                        ui.label(message);
                                        return;
                                    }
                                }
                            }

                            combined_calculations /= self.rng_meter_calculation_iterations;
                            rng_meter_calculation.push((meter_deselection_threshold as f64, combined_calculations));
                        }
                        self.rng_meter_calculations.insert(hash, rng_meter_calculation);
                        self.rng_meter_calculation_hash = Some(hash);
                    };

                    if let Some(hash) = self.rng_meter_calculation_hash {
                        let data = self.rng_meter_calculations.get(&hash).unwrap();
                        let total_roll_plot_points: PlotPoints<'_> = data
                            .iter()
                            .map(|(rng_meter_trigger_threshold, result)| [*rng_meter_trigger_threshold, result.total_rolls])
                            .collect();
                        let random_unboosted_roll_plot_points: PlotPoints<'_> = data
                            .iter()
                            .map(|(rng_meter_trigger_threshold, result)| [*rng_meter_trigger_threshold, result.total_rolls_from_random_rolls_unboosted])
                            .collect();
                        let random_boosted_roll_plot_points: PlotPoints<'_> = data
                            .iter()
                            .map(|(rng_meter_trigger_threshold, result)| [*rng_meter_trigger_threshold, result.total_rolls_from_random_rolls_boosted])
                            .collect();
                        let guaranteed_roll_plot_points: PlotPoints<'_> = data
                            .iter()
                            .map(|(rng_meter_trigger_threshold, result)| [*rng_meter_trigger_threshold, result.total_rolls_from_maxed_rng_meter])
                            .collect();
                        
                        let highest_possible_score_reachable = meter_xp + (per_run_score_increase * self.rng_meter_calculation_runs);

                        Plot::new("lines_demo")
                            .legend(Legend::default())
                            .allow_zoom([false, true])
                            .allow_scroll([false, true])
                            .auto_bounds([true, false])
                            .show(ui, |ui| {
                                ui.line(Line::new(total_roll_plot_points)
                                    .color(Color32::from_rgb(100, 200, 100))
                                    .name("Total Rolls")
                                    .style(Solid));
                                ui.line(Line::new(random_unboosted_roll_plot_points)
                                    .color(Color32::from_rgb(200, 200, 100))
                                    .name("From Random Rolls (No RNG Meter or would've rolled even without boosted rates)")
                                    .style(Solid));
                                ui.line(Line::new(random_boosted_roll_plot_points)
                                    .color(Color32::from_rgb(200, 100, 100))
                                    .name("From Random Rolls (Boosted from the RNG Meter)")
                                    .style(Solid));
                                ui.line(Line::new(guaranteed_roll_plot_points)
                                    .color(Color32::from_rgb(100, 100, 200))
                                    .name("From Guaranteed Rolls")
                                    .style(Solid));
                                
                                /*
                                if highest_possible_score_reachable < meter_data.required_xp {
                                    let percentage_of_max = highest_possible_score_reachable as f64 / meter_data.required_xp as f64;
                                    let points: PlotPoints<'_> = (0..=200)
                                        .map(|i| {
                                            [percentage_of_max, i as f64 * 0.01] 
                                        })
                                        .collect();
                                    ui.line(Line::new(points)
                                        .color(Color32::WHITE)
                                        .name("Maximum Score Reachable")
                                        .style(LineStyle::dotted_loose())
                                        .style(Solid));
                                }
                                 */
                            });

                        Plot::new("chances")
                            .legend(Legend::default())
                            .allow_zoom([false, true])
                            .allow_scroll([false, true])
                            .auto_bounds([true, false])
                            .show(ui, |ui| {

                            });
                    }
                }
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
            calculator_type: AveragesLootTable,
            random_table: None,
            random_table_source_options_hash: None,
            rng_meter_calculations: HashMap::new(),
            rng_meter_calculation_cached_chances: HashMap::new(),
            rng_meter_calculation_hash: None,
            rng_meter_calculation_runs: 200,
            rng_meter_calculation_iterations: 200,
            rng_meter_calculation_use_kismet_feathers: false,

            loot: catacombs_loot::read_all_chests(&ASSETS_DIR)
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(Rc::new).collect()))
                .collect(),
            images,
        }
    }

    fn add_loot_section(&mut self, ui: &mut Ui) {
        let chances = self.get_loot_table_chances();
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
            self.s_plus || chest.require_s_plus(),
        );

        let available_height = ui.available_height();
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
            .drag_to_scroll(true)
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Entry");
                });
                header.col(|ui| {
                    ui.strong("Coins Cost");
                });
                header.col(|ui| {
                    ui.strong(format!("Quality ({})", starting_quality));
                });
                header.col(|ui| {
                    ui.strong(format!(
                        "Weight ({})",
                        format!("{:.1$}", chances.total_weight, 2)
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                    ));
                });
                header.col(|ui| {
                    ui.strong("First Roll Chance");
                });
                header.col(|ui| {
                    ui.strong("Average Chance");
                });
            })
            .body(|mut body| {
                let rng_meter_entry = if let Some(rng_entry) = &self.rng_meter_data.selected_item {
                    if self.rng_meter_data.selected_xp >= rng_entry.required_xp {
                        // entry is only guaranteed in the lowest tier chest, although boosted in all chest tiers
                        chances
                            .entries
                            .iter()
                            .find(|e| e.entry == rng_entry.lowest_tier_chest_entry)
                    } else {
                        None
                    }
                } else {
                    None
                };

                for entry in chances.entries.iter() {
                    let weight = entry.used_weight;
                    let chance = entry.chance;
                    let entry = &entry.entry;

                    if chance == 0.0 {
                        continue;
                    }

                    body.row(text_height, |mut row| {
                        row.col(|ui| {
                            images::add_first_valid_image(
                                &self.images,
                                ui,
                                entry.get_possible_file_names(),
                            );

                            let text = entry.to_string();
                            let page_url = entry.get_wiki_page_name();
                            ui.hyperlink_to(text, page_url);
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(
                                    (chest.base_cost + entry.get_added_chest_price())
                                        .to_formatted_string(&en),
                                )
                                .color(Color32::from_rgb(255, 170, 0)),
                            );
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(format!("{}", entry.get_quality()))
                                    .color(Color32::from_rgb(85, 255, 255)),
                            );
                        });
                        row.col(|ui| {
                            let text = RichText::new(
                                format!("{:.3}", weight)
                                    .trim_end_matches('0')
                                    .trim_end_matches('.'),
                            );
                            ui.label(text.color(Color32::from_rgb(85, 255, 255)))
                                .on_hover_text(format!("More Decimals: {}", weight));
                        });

                        row.col(|ui| {
                            let first_roll_chance: f64 = if let Some(rng_entry) = rng_meter_entry {
                                if rng_entry.entry == *entry {
                                    1.0
                                } else {
                                    0.0
                                }
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

    fn add_random_loot_section(&mut self, ui: &mut Ui) {
        if self.random_table.is_none() {
            return;
        }
        let loot = self.random_table.as_ref().unwrap();

        let text_height = TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let chest = self.chest.as_ref().unwrap();
        let starting_quality = calculate_quality(
            chest,
            self.treasure_accessory_multiplier,
            self.boss_luck_increase,
            self.s_plus || chest.require_s_plus(),
        );

        let available_height = ui.available_height();
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
            .drag_to_scroll(true)
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Entry");
                });
                header.col(|ui| {
                    ui.strong("Added Cost");
                });
                header.col(|ui| {
                    ui.strong(format!("Quality ({})", starting_quality));
                });
                header.col(|ui| {
                    ui.strong("Weight (Total)");
                });
                header.col(|ui| {
                    ui.strong("Slot Roll Chance");
                });
                header.col(|ui| {
                    ui.strong("Combined Chances");
                });
            })
            .body(|mut body| {
                for entry in loot.iter() {
                    let weight = entry.used_weight;
                    let total_weight = entry.total_weight;
                    let roll_chance = entry.roll_chance;
                    let overall_chance = entry.overall_chance;
                    let before_quality = entry.before_quality;
                    let after_quality = before_quality - entry.entry.get_quality();
                    let entry = &entry.entry;

                    body.row(text_height, |mut row| {
                        row.col(|ui| {
                            images::add_first_valid_image(
                                &self.images,
                                ui,
                                entry.get_possible_file_names(),
                            );

                            let text = entry.to_string();
                            let page_url = entry.get_wiki_page_name();
                            ui.hyperlink_to(text, page_url);
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(
                                    entry.get_added_chest_price().to_formatted_string(&en),
                                )
                                .color(Color32::from_rgb(255, 170, 0)),
                            );
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(format!("{}", entry.get_quality()))
                                    .color(Color32::from_rgb(85, 255, 255)),
                            );
                            ui.label(format!(
                                "({} -> {})",
                                before_quality,
                                format!("{}", after_quality as f32)
                                    .trim_end_matches('0')
                                    .trim_end_matches('.')
                            ));
                        });
                        row.col(|ui| {
                            let text = RichText::new(
                                format!("{:.3}", weight)
                                    .trim_end_matches('0')
                                    .trim_end_matches('.'),
                            );
                            ui.label(text.color(Color32::from_rgb(85, 255, 255)))
                                .on_hover_text(format!("More Decimals: {}", weight));

                            ui.label(format!(
                                " ({})",
                                format!("{:.3}", total_weight)
                                    .trim_end_matches('0')
                                    .trim_end_matches('.')
                            ));
                        });

                        row.col(|ui| {
                            fill_in_chance_column(ui, roll_chance);
                        });

                        row.col(|ui| {
                            fill_in_chance_column(ui, overall_chance);
                        });
                    });
                }
            });
    }

    pub fn require_s_plus(&self) -> bool {
        if let Some(chest) = self.chest.as_ref() {
            chest.require_s_plus()
        } else {
            false
        }
    }

    fn generate_loot_table_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.s_plus || self.require_s_plus()).hash(&mut hasher);
        self.treasure_accessory_multiplier
            .to_string()
            .hash(&mut hasher);
        self.boss_luck_increase.hash(&mut hasher);
        self.floor.hash(&mut hasher);
        self.chest.hash(&mut hasher);
        self.rng_meter_data.selected_xp.hash(&mut hasher);
        self.rng_meter_data.selected_item.hash(&mut hasher);
        hasher.finish()
    }

    fn generate_rng_meter_calculation_chests_and_item_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.s_plus || self.require_s_plus()).hash(&mut hasher);
        self.treasure_accessory_multiplier
            .to_string()
            .hash(&mut hasher);
        self.boss_luck_increase.hash(&mut hasher);
        self.floor.hash(&mut hasher);
        self.rng_meter_data.selected_item.hash(&mut hasher);
        hasher.finish()
    }

    fn generate_rng_meter_calculation_overall_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.s_plus || self.require_s_plus()).hash(&mut hasher);
        self.treasure_accessory_multiplier
            .to_string()
            .hash(&mut hasher);
        self.boss_luck_increase.hash(&mut hasher);
        self.floor.hash(&mut hasher);
        self.chest.hash(&mut hasher);
        self.rng_meter_data.selected_item.hash(&mut hasher);
        self.rng_meter_data.selected_xp.hash(&mut hasher);
        self.rng_meter_calculation_runs.hash(&mut hasher);
        self.rng_meter_calculation_iterations.hash(&mut hasher);
        self.rng_meter_calculation_use_kismet_feathers
            .hash(&mut hasher);
        hasher.finish()
    }

    fn get_loot_table_chances(&self) -> Option<&AveragesCalculationResult> {
        let hash = self.generate_loot_table_hash();
        self.hashed_chances.get(&hash)
    }
}

fn find_chests_with_entry<'a>(
    selected_item: &'a String,
    floor_chests: &'a [Rc<LootChest>],
) -> Vec<&'a Rc<LootChest>> {
    floor_chests
        .iter()
        .filter(|c| c.has_matching_entry_identifier(selected_item))
        .collect::<Vec<&Rc<LootChest>>>()
}

fn fill_in_chance_column(ui: &mut Ui, chance: f64) {
    let width = ui.fonts(|f| f.glyph_width(&TextStyle::Body.resolve(ui.style()), ' '));
    ui.spacing_mut().item_spacing.x = width;

    ui.label(
        RichText::new(format!(
            "{}%",
            format!("{:.4}", chance * 100.0)
                .trim_end_matches('0')
                .trim_end_matches('.')
        ))
        .color(Color32::from_rgb(85, 255, 85)),
    );

    if chance == 1.0 {
        ui.label(" (guaranted)");
    } else if chance == 0.0 {
        ui.label(" (never)");
    } else {
        ui.label(" (");
        ui.label(RichText::new("1").color(Color32::from_rgb(85, 255, 85)));
        ui.label(" in ");
        ui.label(
            RichText::new(
                format!("{:.3}", 1.0 / chance)
                    .trim_end_matches('0')
                    .trim_end_matches('.'),
            )
            .color(Color32::from_rgb(255, 255, 85)),
        );
        ui.label(" runs)");
    }
}
