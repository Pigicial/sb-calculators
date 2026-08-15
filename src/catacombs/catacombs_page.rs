use crate::catacombs::catacombs_loot::{LootChest, TestingQualityIncrease};
use crate::catacombs::catacombs_loot_calculator::{cache_chances_per_rng_meter_value, calculate_average_chances, calculate_quality, AveragesCalculationResult, ChanceAndWeight, RandomlySelectedLootEntry, RngMeterCalculation, RngMeterData};
use crate::catacombs::catacombs_page::CalculatorType::{AveragesLootTable, RandomLootTable, RngMeterDeselection, SpecificEntryRollCombinations, WikiChestTable, WikiItemFloorTable};
use crate::catacombs::{catacombs_loot, catacombs_loot_calculator, options};
use crate::images;
use eframe::epaint::{Color32, TextureHandle};
use egui::text::LayoutJob;
use egui::{Align, Context, Grid, Label, RichText, ScrollArea, SidePanel, TextStyle, TextWrapMode, Ui};
use egui_extras::{Column, TableBuilder};
use egui_plot::LineStyle::Solid;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use include_dir::{include_dir, Dir};
use num_format::Locale::{cu, en, it};
use num_format::ToFormattedString;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::fmt::format;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use crate::catacombs::options::{floor_to_text, floor_to_wiki_text};

static ASSETS_DIR: Dir<'static> = include_dir!("assets");

pub struct CatacombsLootPage {
    pub floor: Option<String>,
    pub chest: Option<Rc<LootChest>>,

    pub treasure_accessory_multiplier: f64,
    pub boss_luck_increase: u8,
    pub catacombs_box_attribute_increase: u8,
    pub testing_quality_bonus: TestingQualityIncrease,
    pub s_plus: bool,
    pub forced_s_plus_const: bool,
    pub rng_meter_data: RngMeterData,

    pub wiki_selected_item_identifier: Option<String>,
    pub wiki_selected_item_search_query: String,
    pub calculator_type: CalculatorType,
    hashed_chances: HashMap<u64, AveragesCalculationResult>,
    pub comparison_hash: Option<u64>,

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
    SpecificEntryRollCombinations,
    RandomLootTable,
    RngMeterDeselection,
    WikiChestTable,
    WikiItemFloorTable,
}

impl CalculatorType {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn should_display_rng_meter_section(&self) -> bool {
        self == &AveragesLootTable || self == &SpecificEntryRollCombinations || self == &RandomLootTable
    }

    #[cfg(target_arch = "wasm32")]
    pub fn should_display_rng_meter_section(&self) -> bool {
        self == &AveragesLootTable || self == &SpecificEntryRollCombinations
    }
}

impl eframe::App for CatacombsLootPage {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        SidePanel::left("cata_loot_config")
            .resizable(false)
            .min_width(468.0)
            .default_width(468.0)
            .show(ctx, |ui| {
                ScrollArea::horizontal().id_salt("cata_loot_config").show(ui, |ui| {
                    ui.heading("Options");
                    Grid::new("config_grid")
                        .num_columns(2)
                        .spacing([15.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            if self.calculator_type == WikiItemFloorTable {
                                options::add_wiki_item_option(self, ui);
                                return;
                            }

                            options::add_floor_options(self, ui);
                            ui.end_row();
                            options::add_chest_options(self, ui);

                            if self.calculator_type != WikiChestTable {
                                ui.end_row();
                                options::add_treasure_talisman_options(self, ui);
                                ui.end_row();
                                options::add_boss_luck_options(self, ui);
                                ui.end_row();
                                options::add_catacombs_box_attribute_options(self, ui);
                                ui.end_row();
                                options::add_s_plus_options(self, ui);
                                ui.end_row();
                                options::add_testing_quality_option(self, ui);
                                ui.end_row();
                                options::add_rng_meter_options(self, ui);

                                if self.calculator_type == RngMeterDeselection {
                                    options::add_rng_meter_simulation_options(self, ui);
                                    ui.end_row();
                                }

                                // 7#[cfg(not(target_arch = "wasm32"))]
                                if self.calculator_type == AveragesLootTable && self.get_loot_table_chances().is_some() {
                                    options::add_comparison_options(self, ui);
                                    ui.end_row();
                                }
                            }
                        });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if cfg!(not(target_arch = "wasm32")) {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.calculator_type, AveragesLootTable, "Loot Tables");
                    // todo: broken it seems, so hiding
                    //ui.selectable_value(&mut self.calculator_type, SpecificEntryRollCombinations, "Roll Combinations");
                    ui.selectable_value(&mut self.calculator_type, RandomLootTable, "Casino");
                    ui.selectable_value(&mut self.calculator_type, RngMeterDeselection, "RNG Meter Deselection Calculator");
                    ui.selectable_value(&mut self.calculator_type, CalculatorType::WikiChestTable, "Wiki Chest Table Syntax");
                    ui.selectable_value(&mut self.calculator_type, CalculatorType::WikiItemFloorTable, "Wiki Item Table Syntax");
                });
                ui.separator();
            }

            match self.calculator_type {
                AveragesLootTable => {
                    if self.floor.is_none() || self.chest.is_none() {
                        ui.label("Select a floor and chest to see its loot.");
                        return;
                    }
                    let hash = self.generate_loot_table_hash();

                    let chances = self.get_loot_table_chances();
                    if chances.is_none() {
                        let chest = self.chest.as_ref().unwrap();
                        let starting_quality = calculate_quality(
                            chest,
                            self.treasure_accessory_multiplier,
                            self.boss_luck_increase,
                            self.catacombs_box_attribute_increase,
                            &self.testing_quality_bonus,
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
                SpecificEntryRollCombinations => {
                    if self.floor.is_none() || self.chest.is_none() {
                        ui.label("Select a floor and chest to see its loot.");
                        return;
                    }
                    let hash = self.generate_loot_table_hash();

                    let chances = self.get_loot_table_chances();
                    if chances.is_none() {
                        let chest = self.chest.as_ref().unwrap();
                        let starting_quality = calculate_quality(
                            chest,
                            self.treasure_accessory_multiplier,
                            self.boss_luck_increase,
                            self.catacombs_box_attribute_increase,
                            &self.testing_quality_bonus,
                            self.s_plus || chest.require_s_plus(),
                        );

                        let new_chances = calculate_average_chances(chest, starting_quality, &self.rng_meter_data);
                        self.hashed_chances.insert(hash, new_chances);
                    }

                    // Horizontal scrolling is done here, vertical scrolling is done on the table scrolling end
                    // (this took painfully long to figure out)
                    ScrollArea::horizontal().id_salt("roll_combinations").show(ui, |ui| {
                        self.add_loot_combinations_section(ui);
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                RandomLootTable => {
                    if self.floor.is_none() || self.chest.is_none() {
                        ui.label("Select a floor and chest to see its loot.");
                        return;
                    }
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
                            self.catacombs_box_attribute_increase,
                            &self.testing_quality_bonus,
                            self.s_plus || chest.require_s_plus(),
                        );

                        self.random_table = Some(catacombs_loot_calculator::generate_random_table(chest, starting_quality, &self.rng_meter_data));
                        self.random_table_source_options_hash = Some(hash);
                    }

                    // Horizontal scrolling is done here, vertical scrolling is done on the table scrolling end
                    // (this took painfully long to figure out)
                    ScrollArea::horizontal().id_salt("cata_random_loot").show(ui, |ui| {
                        self.add_random_loot_section(ui);
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                RngMeterDeselection => {
                    if self.floor.is_none() || self.chest.is_none() {
                        ui.label("Select a floor and chest to see its loot.");
                        return;
                    }
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
                                    self.catacombs_box_attribute_increase,
                                    &self.testing_quality_bonus,
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
                                let result = catacombs_loot_calculator::calculate_amount_of_times_rolled_for_entry(
                                    chest_data,
                                    self,
                                    self.rng_meter_calculation_runs,
                                    300,
                                    meter_deselection_threshold,
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


                        let average_random_roll_chances_plot_points: PlotPoints<'_> = data
                            .iter()
                            .map(|(rng_meter_trigger_threshold, result)| [*rng_meter_trigger_threshold, result.average_regular_entry_roll_chance])
                            .collect();
                        let average_random_roll_weights_plot_points: PlotPoints<'_> = data
                            .iter()
                            .map(|(rng_meter_trigger_threshold, result)| [*rng_meter_trigger_threshold, result.average_regular_entry_roll_weight])
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

                                ui.line(Line::new(average_random_roll_chances_plot_points)
                                    .color(Color32::from_rgb(200, 100, 100))
                                    .name("Averages Chances from Random Rolls")
                                    .style(Solid));
                                ui.line(Line::new(average_random_roll_weights_plot_points)
                                    .color(Color32::from_rgb(100, 100, 200))
                                    .name("Average Weights from Random Rolls")
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
                    }
                }
                CalculatorType::WikiChestTable => {
                    if self.floor.is_none() || self.chest.is_none() {
                        ui.label("Select a floor and chest to see its loot.");
                        return;
                    }
                    let base_table_hash = self.generate_base_wiki_table_hash();
                    let max_table_hash = self.generate_max_wiki_table_hash();
                    let chest = self.chest.as_ref().unwrap();

                    let base_table_quality = calculate_quality(chest,
                                                               1.0,
                                                               0,
                                                               0,
                                                               &TestingQualityIncrease::default(),
                                                               chest.require_s_plus(),
                    );
                    let max_table_quality = calculate_quality(chest,
                                                              1.03,
                                                              10,
                                                              13,
                                                              &TestingQualityIncrease::default(),
                                                              true,
                    );

                    let base_chances = self.get_loot_table_chances_by_hash(base_table_hash);
                    if base_chances.is_none() {
                        let new_chances = calculate_average_chances(chest, base_table_quality, &RngMeterData::default());
                        self.hashed_chances.insert(base_table_hash, new_chances);
                    }

                    let max_chances = self.get_loot_table_chances_by_hash(max_table_hash);
                    if max_chances.is_none() {
                        let new_chances = calculate_average_chances(chest, max_table_quality, &RngMeterData::default());
                        self.hashed_chances.insert(max_table_hash, new_chances);
                    }


                    // Horizontal scrolling is done here, vertical scrolling is done on the table scrolling end
                    // (this took painfully long to figure out)
                    ScrollArea::horizontal().id_salt("wiki_cata_loot").show(ui, |ui| {
                        self.add_wiki_loot_section(ui, ctx, base_table_hash, max_table_hash, base_table_quality, max_table_quality);
                    });
                }
                WikiItemFloorTable => {
                    if self.wiki_selected_item_identifier.is_none() {
                        ui.label("Select an item obtainable from any of its floors to see its loot.");
                        return;
                    }

                    // Horizontal scrolling is done here, vertical scrolling is done on the table scrolling end
                    // (this took painfully long to figure out)
                    ScrollArea::horizontal().id_salt("wiki_item_loot").show(ui, |ui| {
                        self.add_wiki_single_item_loot_section(ui, ctx);
                    });
                }
                _ => {}
            }
        });
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

impl CatacombsLootPage {
    pub fn new(images: Rc<HashMap<String, TextureHandle>>) -> Self {
        Self {
            floor: None,
            chest: None,

            treasure_accessory_multiplier: 1.0,
            boss_luck_increase: 0,
            catacombs_box_attribute_increase: 0,
            testing_quality_bonus: TestingQualityIncrease::default(),
            s_plus: false,
            forced_s_plus_const: true,
            rng_meter_data: Default::default(),

            wiki_selected_item_identifier: None,
            wiki_selected_item_search_query: String::new(),
            hashed_chances: HashMap::new(),
            calculator_type: AveragesLootTable,
            random_table: None,
            random_table_source_options_hash: None,
            comparison_hash: None,

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
        //println!("{:?}", chances.entries.first().unwrap().roll_combinations.clone());

        let text_height = TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let chest = self.chest.as_ref().unwrap();
        let starting_quality = calculate_quality(
            chest,
            self.treasure_accessory_multiplier,
            self.boss_luck_increase,
            self.catacombs_box_attribute_increase,
            &self.testing_quality_bonus,
            self.s_plus || chest.require_s_plus(),
        );

        let available_height = ui.available_height();
        let mut table = TableBuilder::new(ui)
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

        let hash = self.generate_loot_table_hash();
        let include_comparison_data = self.comparison_hash.is_some_and(|h| h != hash);
        if include_comparison_data {
            table = table.column(Column::auto())
                .column(Column::auto())
                .column(Column::auto());
        }

        table.header(20.0, |mut header| {
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
            if include_comparison_data {
                header.col(|_| {}); // spacer
                header.col(|ui| {
                    ui.strong("Difference");
                });
                header.col(|ui| {
                    ui.strong("Previous Average");
                });
            }
        }).body(|mut body| {
            let rng_meter_entry = if let Some(rng_entry) = &self.rng_meter_data.selected_item {
                if self.rng_meter_data.selected_xp >= rng_entry.required_xp {
                    // entry is only guaranteed in the lowest tier chest, although boosted in all chest tiers
                    chances
                        .entries
                        .iter()
                        .find(|e| e.borrow().entry == rng_entry.lowest_tier_chest_entry)
                } else {
                    None
                }
            } else {
                None
            };

            let comparison_data = if include_comparison_data {
                self.hashed_chances.get(&self.comparison_hash.unwrap())
            } else {
                None
            };

            for entry in chances.entries.iter() {
                let entry = entry.borrow();
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
                        ui.label(RichText::new((chest.base_cost + entry.get_added_chest_price()).to_formatted_string(&en))
                            .color(Color32::from_rgb(255, 170, 0)));
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
                            if rng_entry.borrow().entry == *entry {
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

                    if let Some(comparison_data) = comparison_data {
                        let previous_chance = comparison_data
                            .entries
                            .iter()
                            .find(|e| e.borrow().entry.to_string() == entry.to_string());

                        row.col(|_| {}); // spacer

                        if let Some(previous_chance) = previous_chance {
                            let previous_chance = previous_chance.borrow().chance;
                            row.col(|ui| {
                                fill_in_chance_differences_column(ui, chance, previous_chance);
                            });
                            row.col(|ui| {
                                fill_in_chance_column(ui, previous_chance);
                            });
                        } else {
                            row.col(|ui| {
                                ui.label(RichText::new("-").color(Color32::GRAY));
                            });
                            row.col(|ui| {
                                ui.label(RichText::new("-").color(Color32::GRAY));
                            });
                        }
                    }
                });
            }
        });
    }

    fn add_wiki_loot_section(&mut self, ui: &mut Ui, ctx: &Context, base_table_hash: u64, max_table_hash: u64, base_quality: i16, max_quality: i16) {
        let base_chances = self.get_loot_table_chances_by_hash(base_table_hash);
        let max_chances = self.get_loot_table_chances_by_hash(max_table_hash);
        if base_chances.is_none() || max_chances.is_none() {
            return;
        }

        let base_chances = base_chances.unwrap();
        let max_chances = max_chances.unwrap();
        let chest = self.chest.as_ref().unwrap();

        let mut lines: Vec<String> = Vec::new();

        lines.push("{|class=\"wikitable\"".to_string());
        lines.push(format!("! colspan=\"7\" | [[File:{} Chest Render.png|x40px]] {} [[Dungeon Reward Chest|Chest Loot]] - {}", chest.chest_type, chest.chest_type, floor_to_wiki_text(self.floor.as_ref().unwrap())));
        lines.push("|-".to_string());
        lines.push("! Entry".to_string());
        lines.push(format!("! Added Cost <br> (Base {{{{C|{}}}}})", chest.base_cost.to_formatted_string(&en)));
        lines.push("! [[Dungeon_Reward_Chest#Loot_Rolling_Process|Quality]]".to_string());
        lines.push("! [[Dungeon_Reward_Chest#Loot_Rolling_Process|Weight]]".to_string());
        lines.push("! First Roll Chance".to_string());
        if chest.require_s_plus() {
            lines.push(format!("! Average Chance <br> ({{{{Dungeon Ranking|S+}}}} No Bonuses, {{{{Aqua|{base_quality}}}}} Quality)"));
        } else {
            lines.push(format!("! Average Chance <br> (No Bonuses, {{{{Aqua|{base_quality}}}}} Quality)"));
        }
        lines.push(format!("! Average Chance <br> ({{{{Dungeon Ranking|S+}}}} [[Dungeon_Reward_Chest#Quality_Upgrades|Max Bonuses]], {{{{Aqua|{max_quality}}}}} Quality)"));
        lines.push("|-".to_string());

        for base_chance_entry in base_chances.entries.iter() {
            let base_chance_entry = base_chance_entry.borrow();
            let loot_entry = &base_chance_entry.entry;
            let max_chance_entry = max_chances.entries.iter().find(|e| e.borrow().entry == *loot_entry).unwrap().borrow();

            let added_chest_price = loot_entry.get_added_chest_price();
            let min_entry_price = added_chest_price + chest.base_cost;
            let quality = loot_entry.get_quality();
            let weight = base_chance_entry.used_weight;

            let first_roll_chance = weight / base_chances.total_weight;
            let no_modifiers_average_chance = base_chance_entry.chance;
            let max_modifiers_average_chance = max_chance_entry.chance;

            lines.push("{{Catacombs Chest Loot Table Entry".to_string());
            lines.push(format!("|floor = {}", chest.floor));
            if chest.master_mode {
                lines.push("|master_mode = yes".to_string());
            }
            lines.push(format!("|chest = {}", chest.chest_type));
            lines.push(format!("|entry = {}", loot_entry.get_wiki_templateless_template_reference()));
            lines.push(format!("|added_cost = {}", added_chest_price.to_formatted_string(&en)));
            lines.push(format!("|total_cost = {}", min_entry_price.to_formatted_string(&en)));
            lines.push(format!("|quality = {quality}"));
            lines.push(format!("|weight = {}", format!("{:.3}", weight).trim_end_matches('0').trim_end_matches('.')));
            lines.push(format!("|first_roll_chance = {}", get_wiki_chance_text(first_roll_chance)));
            lines.push(format!("|average_chance_no_bonuses = {}", get_wiki_chance_text(no_modifiers_average_chance)));
            lines.push(format!("|average_chance_max_bonuses = {}", get_wiki_chance_text(max_modifiers_average_chance)));
            lines.push("}}".to_string());
        }
        lines.push("|}".to_string());

        let line_height = TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        if ui.button("Copy to Clipboard").clicked() {
            ctx.copy_text(lines.join("\n"));
        }

        ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let mut job = LayoutJob::single_section(
                    lines.join("\n"),
                    egui::TextFormat {
                        extra_letter_spacing: 0.0,
                        line_height: Some(line_height),
                        ..Default::default()
                    },
                );
                job.wrap = egui::text::TextWrapping {
                    max_rows: 10000,
                    break_anywhere: true,
                    overflow_character: None,
                    ..Default::default()
                };

                // NOTE: `Label` overrides some of the wrapping settings,
                // e.g. wrap width, halign, and justify.
                ui.with_layout(
                    egui::Layout::top_down(Align::LEFT).with_cross_justify(true),
                    |ui| {
                        ui.label(job);
                    },
                );
            });
    }

    fn add_wiki_single_item_loot_section(&mut self, ui: &mut Ui, ctx: &Context) {
        if self.wiki_selected_item_identifier.is_none() {
            return;
        }

        let entry_identifier = self.wiki_selected_item_identifier.as_ref().unwrap();

        // let mut replaced = false;
        let mut lines = vec![
            "{|class=\"wikitable ct\"".to_string(),
            //"! colspan=\"7\" | ITEM_HERE Catacombs Drop Rates".to_string(),
            //"|-".to_string(),
            "! [[Catacombs#Floors|Floor]]".to_string(),
            "! [[Dungeon_Reward_Chest#Chest_Types|Chest]]".to_string(),
            "! Cost <br> (Chest + Added)".to_string(),
            //"! First Roll Chance <br> (Shown in [[RNG Meter]])".to_string(),
            "! Average Chance <br> (No Bonuses)".to_string(),
            "! Average Chance <br> ({{Dungeon Ranking|S+}} [[Dungeon_Reward_Chest#Quality_Upgrades|Max Bonuses]])".to_string(),
            "! [[Dungeon_Reward_Chest#Loot_Rolling_Process|Quality]]".to_string(),
            "! [[Dungeon_Reward_Chest#Loot_Rolling_Process|Weight]]".to_string()
        ];

        for (floor, chests) in self.loot.iter() {
            for chest in chests {
                if !chest.has_matching_entry_identifier(entry_identifier) {
                    continue;
                }

                let base_table_hash = generate_base_wiki_table_hash(floor, chest);
                let max_table_hash = generate_max_wiki_table_hash(floor, chest);
                let base_table_quality = calculate_quality(chest,
                                                           1.0,
                                                           0,
                                                           0,
                                                           &TestingQualityIncrease::default(),
                                                           chest.require_s_plus(),
                );
                let max_table_quality = calculate_quality(chest,
                                                          1.03,
                                                          10,
                                                          13,
                                                          &TestingQualityIncrease::default(),
                                                          true,
                );

                let base_chances = self.get_loot_table_chances_by_hash(base_table_hash);
                if base_chances.is_none() {
                    let new_chances = calculate_average_chances(chest, base_table_quality, &RngMeterData::default());
                    self.hashed_chances.insert(base_table_hash, new_chances);
                }

                let max_chances = self.get_loot_table_chances_by_hash(max_table_hash);
                if max_chances.is_none() {
                    let new_chances = calculate_average_chances(chest, max_table_quality, &RngMeterData::default());
                    self.hashed_chances.insert(max_table_hash, new_chances);
                }

                let base_chances = self.get_loot_table_chances_by_hash(base_table_hash);
                let max_chances = self.get_loot_table_chances_by_hash(max_table_hash);
                if base_chances.is_none() || max_chances.is_none() {
                    continue;
                }

                let base_chances = base_chances.unwrap();
                let base_chance_entry = base_chances.entries.iter()
                    .find(|e| e.borrow().entry.to_string() == *entry_identifier).unwrap().borrow();

                let max_chances = max_chances.unwrap();
                let max_chance_entry = max_chances.entries.iter()
                    .find(|e| e.borrow().entry.to_string() == *entry_identifier).unwrap().borrow();

                let loot_entry = &base_chance_entry.entry;
                /*
                if !replaced {
                    replaced = true;
                    let entry_to_replace = lines.get(1);
                    let replacement = entry_to_replace.unwrap().replace("ITEM_HERE", &loot_entry.get_wiki_template_reference());
                    lines[1] = replacement;
                }
                 */

                let chest_price = chest.base_cost + loot_entry.get_added_chest_price();
                let quality = loot_entry.get_quality();
                let weight = base_chance_entry.used_weight;

                //let first_roll_chance = weight / base_chances.total_weight;
                let no_modifiers_average_chance = base_chance_entry.chance;
                let max_modifiers_average_chance = max_chance_entry.chance;

                if no_modifiers_average_chance == 0.0 {
                    continue;
                }

                lines.push("|-".to_string());
                lines.push(format!("| style=\"text-align: left\" | {}", floor_to_wiki_text(floor)));
                lines.push(format!("| [[File:{} Chest Render.png|x25px]] {}", chest.chest_type, chest.chest_type));
                lines.push(format!("| {{{{Coins|{}}}}}", chest_price.to_formatted_string(&en)));

                //lines.push(get_wiki_chance_text(first_roll_chance));
                lines.push(format!("| {}", get_wiki_chance_text(no_modifiers_average_chance)));
                lines.push(format!("| {}", get_wiki_chance_text(max_modifiers_average_chance)));

                lines.push(format!("| {{{{Aqua|{quality}}}}}"));
                lines.push(format!("| {{{{Aqua|{}}}}}", format!("{:.3}", weight).trim_end_matches('0').trim_end_matches('.')));
            }
        }


        lines.push("|}".to_string());

        let line_height = TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        if ui.button("Copy to Clipboard").clicked() {
            ctx.copy_text(lines.join("\n"));
        }

        ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let mut job = LayoutJob::single_section(
                    lines.join("\n"),
                    egui::TextFormat {
                        extra_letter_spacing: 0.0,
                        line_height: Some(line_height),
                        ..Default::default()
                    },
                );
                job.wrap = egui::text::TextWrapping {
                    max_rows: 10000,
                    break_anywhere: true,
                    overflow_character: None,
                    ..Default::default()
                };

                // NOTE: `Label` overrides some of the wrapping settings,
                // e.g. wrap width, halign, and justify.
                ui.with_layout(
                    egui::Layout::top_down(Align::LEFT).with_cross_justify(true),
                    |ui| {
                        ui.label(job);
                    },
                );
            });
    }

    fn add_loot_combinations_section(&mut self, ui: &mut Ui) {
        let chances = self.get_loot_table_chances();
        if chances.is_none() {
            return;
        }
        let chances = chances.unwrap();

        // todo: change this from rng meter item to otherwise selected item
        let selected_item = self.rng_meter_data.selected_item.as_ref();
        if selected_item.is_none() {
            return;
        }
        let selected_item = selected_item.unwrap();

        let selected_item_combinations = chances.entries
            .iter()
            .find(|e| e.borrow().entry.to_string() == selected_item.identifier);
        if selected_item_combinations.is_none() {
            return;
        }
        let selected_item_combinations = &selected_item_combinations.unwrap().borrow().roll_combinations;

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
            .drag_to_scroll(true)
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        let chest = self.chest.as_ref().unwrap();
        let starting_quality = calculate_quality(
            chest,
            self.treasure_accessory_multiplier,
            self.boss_luck_increase,
            self.catacombs_box_attribute_increase,
            &self.testing_quality_bonus,
            self.s_plus || chest.require_s_plus(),
        );

        table.header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Slot");
            });
            header.col(|ui| {
                ui.strong("Chance");
            });
            header.col(|ui| {
                ui.strong("Entries");
            });
        }).body(|mut body| {
            for combination in selected_item_combinations.iter() {
                body.row(text_height, |mut row| {
                    row.col(|ui| {
                        let slot = combination.entries.len();
                        ui.label(RichText::new(format!("{}", slot)).color(Color32::from_rgb(85, 255, 85)));
                    });

                    row.col(|ui| {
                        fill_in_chance_column(ui, combination.total_chance);
                    });

                    row.col(|ui| {
                        let mut iter = combination.entries.iter().peekable();
                        while let Some(entry) = iter.next() {
                            let entry = &entry.borrow().entry;
                            images::add_first_valid_image(
                                &self.images,
                                ui,
                                entry.get_possible_file_names(),
                            );

                            let text = entry.to_string();
                            let page_url = entry.get_wiki_page_name();
                            ui.hyperlink_to(text, page_url);

                            if iter.peek().is_some() {
                                ui.label(RichText::new("->").color(Color32::GRAY));
                            }
                        }
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
            self.catacombs_box_attribute_increase,
            &self.testing_quality_bonus,
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

        table.header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Entry");
            });
            header.col(|ui| {
                ui.strong("Added Cost");
            });
            header.col(|ui| {
                ui.strong(format!("Quality ({starting_quality})"));
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

    pub fn generate_loot_table_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.s_plus || self.require_s_plus()).hash(&mut hasher);
        self.treasure_accessory_multiplier
            .to_string()
            .hash(&mut hasher);
        self.boss_luck_increase.hash(&mut hasher);
        self.catacombs_box_attribute_increase.hash(&mut hasher);
        self.testing_quality_bonus.hash(&mut hasher);
        self.floor.hash(&mut hasher);
        self.chest.hash(&mut hasher);
        self.rng_meter_data.selected_xp.hash(&mut hasher);
        self.rng_meter_data.selected_item.hash(&mut hasher);
        hasher.finish()
    }

    pub fn generate_base_wiki_table_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.floor.hash(&mut hasher);
        self.chest.hash(&mut hasher);
        0.hash(&mut hasher);
        hasher.finish()
    }
    pub fn generate_max_wiki_table_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.floor.hash(&mut hasher);
        self.chest.hash(&mut hasher);
        100.hash(&mut hasher);
        hasher.finish()
    }

    fn generate_rng_meter_calculation_chests_and_item_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.s_plus || self.require_s_plus()).hash(&mut hasher);
        self.treasure_accessory_multiplier
            .to_string()
            .hash(&mut hasher);
        self.boss_luck_increase.hash(&mut hasher);
        self.catacombs_box_attribute_increase.hash(&mut hasher);
        self.testing_quality_bonus.hash(&mut hasher);
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
        self.catacombs_box_attribute_increase.hash(&mut hasher);
        self.testing_quality_bonus.hash(&mut hasher);
        self.floor.hash(&mut hasher);
        self.chest.hash(&mut hasher);
        self.rng_meter_data.selected_item.hash(&mut hasher);
        self.rng_meter_data.selected_xp.hash(&mut hasher);
        self.rng_meter_calculation_runs.hash(&mut hasher);
        self.rng_meter_calculation_iterations.hash(&mut hasher);
        self.rng_meter_calculation_use_kismet_feathers.hash(&mut hasher);
        hasher.finish()
    }

    pub(crate) fn get_loot_table_chances(&self) -> Option<&AveragesCalculationResult> {
        let hash = self.generate_loot_table_hash();
        self.hashed_chances.get(&hash)
    }

    pub fn get_loot_table_chances_by_hash(&self, hash: u64) -> Option<&AveragesCalculationResult> {
        self.hashed_chances.get(&hash)
    }
}

fn generate_base_wiki_table_hash(floor: &String, chest: &Rc<LootChest>) -> u64 {
    let mut hasher = DefaultHasher::new();
    floor.hash(&mut hasher);
    chest.hash(&mut hasher);
    0.hash(&mut hasher);
    hasher.finish()
}

fn generate_max_wiki_table_hash(floor: &String, chest: &Rc<LootChest>) -> u64 {
    let mut hasher = DefaultHasher::new();
    floor.hash(&mut hasher);
    chest.hash(&mut hasher);
    100.hash(&mut hasher);
    hasher.finish()
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
        ui.label(" (guaranteed)");
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

fn get_wiki_chance_text(chance: f64) -> String {
    let chance_text = format!("{}%", format!("{:.4}", chance * 100.0).trim_end_matches('0').trim_end_matches('.'));

    let green_text = format!("{{{{Green|{chance_text}}}}}");
    if chance == 1.0 {
        green_text + " (guaranteed)"
    } else if chance == 0.0 {
        green_text + " (never)"
    } else {
        format!("{{{{Chance|{chance_text}|1|{}|runs}}}}", format!("{:.1}", 1.0 / chance).trim_end_matches('0').trim_end_matches('.'))
    }
}

fn fill_in_chance_differences_column(ui: &mut Ui, current_chance: f64, previous_chance: f64) {
    let width = ui.fonts(|f| f.glyph_width(&TextStyle::Body.resolve(ui.style()), ' '));
    ui.spacing_mut().item_spacing.x = width;

    let multiplier = (current_chance / previous_chance) - 1.0;
    let run_difference = (1.0 / current_chance) - (1.0 / previous_chance);

    let formatted_current_chance = format!("{:.4}", current_chance * 100.0);
    let formatted_current_chance = format!("{}%", formatted_current_chance.trim_end_matches('0').trim_end_matches('.'));

    let formatted_previous_chance = format!("{:.4}", previous_chance * 100.0);
    let formatted_previous_chance = format!("{}%", formatted_previous_chance.trim_end_matches('0').trim_end_matches('.'));

    if formatted_current_chance == formatted_previous_chance {
        ui.label("Identical");
    } else {
        let multiplier_text = format!("{:.4}", multiplier * 100.0);
        let multiplier_text = format!("{}%", multiplier_text.trim_end_matches('0').trim_end_matches('.'));

        let run_difference_text = format!("{:.3}", run_difference);
        let run_difference_text = format!("{}", run_difference_text.trim_end_matches('0').trim_end_matches('.'));

        if current_chance > previous_chance {
            ui.label(RichText::new(format!("+{}", multiplier_text)).color(Color32::from_rgb(255, 85, 255)));
            ui.label(" (");
            ui.label(RichText::new(run_difference_text).color(Color32::from_rgb(85, 255, 85)));
            ui.label(" runs)");
        } else {
            ui.label(RichText::new(multiplier_text).color(Color32::from_rgb(170, 0, 170)));
            ui.label(" (");
            ui.label(RichText::new(format!("+{}", run_difference_text)).color(Color32::from_rgb(255, 85, 85)));
            ui.label(" runs)");
        }
    }
}
