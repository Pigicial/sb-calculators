use crate::images;
use crate::shards::bazaar_data::BazaarData;
use crate::shards::fusion::{generate_all_possible_combinations, generate_all_possible_combinations_per_shard, FusionResults, ShardFusionCombinations};
use crate::shards::shard_data::{ShardData, Shards};
use crate::shards::shards_page::AmountType::{ConsumedInFusion, MadeInFusion};
use crate::shards::shards_page::BuyType::{BuyOrder, InstaBuy};
use crate::shards::shards_page::ProfitType::{InstaSell, SellOffer};
use crate::shards::shards_page::ShardCalculatorType::{AllFusionOutputs, FusionOutputs, FusionProfits};
use crate::shards::{bazaar_api, fusion, shard_data};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::epaint::{FontId, TextureHandle};
use egui::text::LayoutJob;
use egui::{Button, Color32, Context, Grid, Layout, RichText, ScrollArea, Slider, TextFormat, Ui};
use egui_extras::{Column, TableBuilder};
use num_format::Locale::en;
use num_format::ToFormattedString;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use web_time::{Duration, Instant};

pub struct ShardsPage {
    shards: Shards,
    calculator_type: ShardCalculatorType,
    images: Rc<HashMap<String, TextureHandle>>,

    search_query: String,
    left_shard_name: Option<String>,
    right_shard_name: Option<String>,

    bazaar_data_sender: Sender<Option<BazaarData>>,
    bazaar_data_receiver: Receiver<Option<BazaarData>>,
    bazaar_data: Option<BazaarData>,
    looking_up_bazaar_data: bool,
    bazaar_tax_percent: f64,
    bazaar_request_triggered: bool,
    last_bazaar_request_ms: Option<Instant>,
    
    combinations_shard_name: Option<String>,
    buy_type: BuyType,
    profit_type: ProfitType,
    sort_type: AllFusionsSortType,
    pure_reptile_attribute_level: u8,
    profit_data_hash: Option<u64>,
    all_combinations_by_shards: Option<ShardFusionCombinations>,
    all_combinations: Option<Vec<FusionResults>>,
    combination_profit_data: Option<Vec<(String, FusionResults)>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ShardCalculatorType {
    FusionProfits,
    AllFusionOutputs,
    FusionOutputs,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BuyType {
    BuyOrder,
    InstaBuy,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ProfitType {
    SellOffer,
    InstaSell,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum AllFusionsSortType {
    ShardIdAndRarity,
    ShardName,
    Profit,
}

impl AllFusionsSortType {
    fn get_label_name(&self) -> &str {
        match self {
            AllFusionsSortType::ShardIdAndRarity => "Rarity/ID",
            AllFusionsSortType::ShardName => "Shard Name",
            AllFusionsSortType::Profit => "Profit"
        }
    }

    fn values() -> Vec<Self> {
        vec![AllFusionsSortType::ShardIdAndRarity, AllFusionsSortType::ShardName, AllFusionsSortType::Profit]
    }
}

impl eframe::App for ShardsPage {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let system_time = Instant::now();
        
        if let Ok(data) = self.bazaar_data_receiver.try_recv() {
            self.bazaar_data = data;
            self.last_bazaar_request_ms = Some(system_time);
            self.looking_up_bazaar_data = false;
            self.bazaar_request_triggered = false;
            self.cache_bazaar_prices();
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.calculator_type, FusionProfits, "Fusions Profits");
                ui.selectable_value(&mut self.calculator_type, AllFusionOutputs, "All Fusion Outputs");
                ui.selectable_value(&mut self.calculator_type, FusionOutputs, "Fusion Outputs");
            });
            ui.separator();

            ScrollArea::horizontal().show(ui, |ui| {
                ui.heading("Options");
                Grid::new("shards_config_grid")
                    .num_columns(2)
                    .spacing([15.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        if self.calculator_type == FusionOutputs {
                            ui.label("Search Shards:");
                            ui.text_edit_singleline(&mut self.search_query);
                            ui.end_row();

                            ui.label("First Shard:");
                            add_shard_option(ui, "first_shard", &mut self.left_shard_name, &self.shards, &self.search_query, &self.images);
                            ui.end_row();
                            ui.label("Second Shard:");
                            add_shard_option(ui, "second_shard", &mut self.right_shard_name, &self.shards, &self.search_query, &self.images);
                            ui.end_row();
                        } else if self.calculator_type == FusionProfits {
                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "redstone_repeater.png");
                                ui.label("Sort By:");
                            });

                            ui.horizontal(|ui| {
                                for sort_type in AllFusionsSortType::values() {
                                    ui.selectable_value(&mut self.sort_type, sort_type, sort_type.get_label_name());
                                }
                            });
                            ui.end_row();

                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "golden_horse_armor.png");
                                ui.label("Buying Method:");
                            });
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut self.buy_type, InstaBuy, "Insta-Buy");
                                ui.selectable_value(&mut self.buy_type, BuyOrder, "Buy Order");
                            });
                            ui.end_row();

                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "hopper.png");
                                ui.label("Selling Method:");
                            });
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut self.profit_type, InstaSell, "Insta-Sell");
                                ui.selectable_value(&mut self.profit_type, SellOffer, "Sell Offer");
                            });
                            ui.end_row();

                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "book.png");
                                ui.label("Bazaar Tax Rate:");
                            });
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut self.bazaar_tax_percent, 0.01, "1%");
                                ui.selectable_value(&mut self.bazaar_tax_percent, 0.01125, "1.125%");
                                ui.selectable_value(&mut self.bazaar_tax_percent, 0.0125, "1.25%");
                            });
                            ui.end_row();

                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "attribute_pure_reptile.png");
                                ui.label("Pure Reptile Level:");
                            });
                            ui.add(Slider::new(&mut self.pure_reptile_attribute_level, 0..=10));
                            ui.end_row();

                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "oak_sign.png");
                                ui.label("Output Filter Search:");
                            });
                            ui.text_edit_singleline(&mut self.search_query);
                            ui.end_row();

                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "eye_of_ender.png");
                                ui.label("Output Filter:");
                            });
                            add_shard_option(ui, "output_filter", &mut self.combinations_shard_name, &self.shards, &self.search_query, &self.images);
                            ui.end_row();
                            
                            ui.label(""); // to force button into row two
                            if self.bazaar_request_triggered && self.looking_up_bazaar_data {
                                ui.add_enabled(false, Button::new("Refreshing data..."));
                            } else if self.last_bazaar_request_ms.is_some_and(|last_update| system_time.duration_since(last_update).as_millis() < 60000) {
                                let difference_in_ms = (system_time - self.last_bazaar_request_ms.unwrap()).as_millis();
                                let time_until_button_enabled = 60 - (difference_in_ms) / 1000;
                                ctx.request_repaint_after(Duration::from_millis(1000 - (difference_in_ms % 1000) as u64));
                                ui.add_enabled(false, Button::new(format!("Can refresh Bazaar data in {time_until_button_enabled}s")));
                            } else if ui.button("Refresh Bazaar").clicked() {
                                self.bazaar_request_triggered = true;
                            }
                        } else if self.calculator_type == AllFusionOutputs {
                            ui.horizontal(|ui| {
                                images::add_image(&self.images, ui, "attribute_pure_reptile.png");
                                ui.label("Pure Reptile Level:");
                            });
                            ui.add(Slider::new(&mut self.pure_reptile_attribute_level, 0..=10));
                            ui.end_row();
                        }
                    });
            });

            ui.separator();
            match self.calculator_type {
                FusionOutputs => self.add_fusion_data(ui),
                AllFusionOutputs => self.add_all_fusion_combinations(ui),
                FusionProfits => self.add_most_profitable_shard_combinations(ui),
            }
        });
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

impl ShardsPage {
    pub fn new(images: Rc<HashMap<String, TextureHandle>>) -> Self {
        let shards = shard_data::read_all_shards();
        let (bz_tx, bz_rx) = unbounded();

        Self {
            shards,
            images,
            search_query: String::new(),
            left_shard_name: None,
            right_shard_name: None,
            combinations_shard_name: None,
            calculator_type: FusionProfits,
            profit_type: InstaSell,
            looking_up_bazaar_data: false,
            bazaar_data_sender: bz_tx,
            bazaar_data_receiver: bz_rx,
            bazaar_data: None,
            bazaar_tax_percent: 0.01,
            bazaar_request_triggered: false,
            last_bazaar_request_ms: None,
            pure_reptile_attribute_level: 10,
            profit_data_hash: None,
            all_combinations_by_shards: None,
            all_combinations: None,
            combination_profit_data: None,
            buy_type: InstaBuy,
            sort_type: AllFusionsSortType::Profit,
        }
    }

    fn add_fusion_data(&self, ui: &mut Ui) {
        if let (Some(left_name), Some(right_name)) = (self.left_shard_name.as_ref(), self.right_shard_name.as_ref()) {
            let left_shard = self.shards.get(left_name).unwrap();
            let right_shard = self.shards.get(right_name).unwrap();

            let fusion_results = fusion::generate_outputs(left_shard, right_shard, &self.shards);
            if !fusion_results.listed_fusions.is_empty() {
                for fusion_shard_name in fusion_results.listed_fusions.iter() {
                    let shard = self.shards.get(fusion_shard_name).unwrap();
                    self.add_numbered_shard_text(ui, shard, MadeInFusion, &fusion_results);

                    ui.end_row();
                }
            } else {
                ui.label("No results :(");
            }
        }
    }

    fn add_most_profitable_shard_combinations(&mut self, ui: &mut Ui) {
        if !self.looking_up_bazaar_data && (self.bazaar_data.is_none() || self.bazaar_request_triggered) {
            self.looking_up_bazaar_data = true;
            bazaar_api::set_shard_prices(self.bazaar_data_sender.clone());
        }

        if self.bazaar_data.is_none() {
            ui.label("Fetching Bazaar Data...");
            return;
        }

        if self.all_combinations_by_shards.is_none() {
            self.all_combinations_by_shards = Some(generate_all_possible_combinations_per_shard(&self.shards));
        }

        if self.combination_profit_data.is_none() {
            let mut profit_sorted_combinations = Vec::new();
            let all_combinations = self.all_combinations_by_shards.as_ref().unwrap();
            for (shard, combinations) in all_combinations.iter() {
                for fusion_results in combinations.iter() {
                    profit_sorted_combinations.push((shard.clone(), fusion_results.clone()));
                }
            }

            self.combination_profit_data = Some(profit_sorted_combinations);
        }
        self.sort_all_shards_if_necessary(self.sort_type);

        let bazaar_data = self.bazaar_data.as_ref().unwrap();

        let profit_sorted_combinations = if let Some(shard_output_filter) = self.combinations_shard_name.as_ref() {
            self.combination_profit_data
                .as_ref()
                .unwrap()
                .iter()
                .filter(|(_, results)| results.has_both_input_costs_above_zero(self.buy_type, &self.shards))
                .filter(|(a, _)| a == shard_output_filter)
                .collect::<Vec<_>>()
        } else {
            self.combination_profit_data
                .as_ref()
                .unwrap()
                .iter()
                .filter(|(_, results)| results.has_both_input_costs_above_zero(self.buy_type, &self.shards))
                .collect::<Vec<_>>()
        };

        ui.end_row();
        let available_height = ui.available_height();
        TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(Layout::left_to_right(egui::Align::Center))
            .columns(Column::auto(), 9)
            .column(Column::remainder())
            .drag_to_scroll(true)
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Shard");
                });
                header.col(|ui| {
                    ui.strong("Left Input");
                });
                header.col(|ui| {
                    ui.strong("Left Cost");
                });
                header.col(|ui| {
                    ui.strong("Right Input");
                });
                header.col(|ui| {
                    ui.strong("Right Cost");
                });
                header.col(|ui| {
                    ui.strong("Total Cost");
                });
                header.col(|ui| {
                    ui.strong("Revenue");
                });
                header.col(|ui| {
                    ui.strong("Profit");
                });
                header.col(|ui| {
                    ui.strong("% Gain");
                });
                header.col(|ui| {
                    ui.strong("7 Day Insta-Buys");
                });
            })
            .body(|body| {
                body.rows(16.0, profit_sorted_combinations.len(), |mut row| {
                    let index = row.index();
                    let (shard_name, combination) = profit_sorted_combinations.get(index).unwrap();
                    let output_shard = self.shards.get(shard_name).unwrap();
                    let first_input_shard = self.shards.get(&combination.first_input_shard_name).unwrap();
                    let second_input_shard = self.shards.get(&combination.second_input_shard_name).unwrap();

                    row.col(|ui| {
                        self.add_numbered_shard_text(ui, output_shard, MadeInFusion, combination);
                    });
                    row.col(|ui| {
                        self.add_numbered_shard_text(ui, first_input_shard, ConsumedInFusion, combination);
                    });
                    row.col(|ui| {
                        self.add_shard_cost(ui, first_input_shard, bazaar_data);
                    });
                    row.col(|ui| {
                        self.add_numbered_shard_text(ui, second_input_shard, ConsumedInFusion, combination);
                    });
                    row.col(|ui| {
                        self.add_shard_cost(ui, second_input_shard, bazaar_data);
                    });

                    match combination.get_total_cost(self.buy_type, &self.shards) {
                        Err(error) =>
                            row.col(|ui| {
                                ui.label(error);
                            }),
                        Ok(cost_of_fusion) => {
                            let profit = combination.get_result_profit(shard_name, self.buy_type, self.profit_type, self.pure_reptile_attribute_level, self.bazaar_tax_percent, &self.shards) as i64;
                            let revenue = profit + cost_of_fusion as i64;
                            row.col(|ui| {
                                ui.label(RichText::new(cost_of_fusion.to_formatted_string(&en)).color(Color32::from_rgb(255, 170, 0)));
                            });
                            row.col(|ui| {
                                ui.label(RichText::new(revenue.to_formatted_string(&en)).color(Color32::from_rgb(255, 170, 0)));
                            });
                            row.col(|ui| {
                                ui.label(RichText::new(profit.to_formatted_string(&en)).color(get_profit_color(profit)));
                            });
                            row.col(|ui| {
                                let percent_gain = (((revenue as f64 / cost_of_fusion as f64) * 100.0) - 100.0) as i64;
                                let possible_plus_sign = if profit >= 0 { "+" } else { ""};
                                ui.label(RichText::new(format!("{}{}%", possible_plus_sign, percent_gain.to_formatted_string(&en))).color(get_profit_color(profit)));
                            });
                            row.col(|ui| {
                                let purchased_in_last_week = bazaar_data.get(&output_shard.get_bazaar_id()).unwrap().quick_status.buy_moving_week;
                                ui.label(RichText::new(purchased_in_last_week.to_formatted_string(&en)).color(Color32::from_rgb(255, 255, 85)));
                            })
                        }
                    };
                });
            });
    }

    fn add_all_fusion_combinations(&mut self, ui: &mut Ui) {
        if self.all_combinations.is_none() {
            self.all_combinations = Some(generate_all_possible_combinations(&self.shards));
        }
        let all_combinations = self.all_combinations.as_ref().unwrap();

        ui.end_row();
        let available_height = ui.available_height();
        TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(Layout::left_to_right(egui::Align::Center))
            .columns(Column::auto(), 2)
            .column(Column::initial(210.0))
            .column(Column::auto())
            .columns(Column::initial(190.0), 2)
            .column(Column::remainder())
            .drag_to_scroll(true)
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Left Input");
                });
                header.col(|ui| {
                    ui.strong("+");
                });
                header.col(|ui| {
                    ui.strong("Right Input");
                });
                header.col(|ui| {
                    ui.strong("=");
                });
                header.col(|ui| {
                    ui.strong("Output #1");
                });
                header.col(|ui| {
                    ui.strong("Output #2");
                });
                header.col(|ui| {
                    ui.strong("Output #3");
                });
            })
            .body(|body| {
                body.rows(16.0, all_combinations.len(), |mut row| {
                    let index = row.index();
                    let combination = all_combinations.get(index).unwrap();

                    let first_input_shard = self.shards.get(&combination.first_input_shard_name).unwrap();
                    let second_input_shard = self.shards.get(&combination.second_input_shard_name).unwrap();

                    row.col(|ui| {
                        self.add_numbered_shard_text(ui, first_input_shard, ConsumedInFusion, combination);
                    });
                    row.col(|ui| {
                        ui.label("+");
                    });
                    row.col(|ui| {
                        self.add_numbered_shard_text(ui, second_input_shard, ConsumedInFusion, combination);
                    });
                    row.col(|ui| {
                        ui.label("=");
                    });

                    for i in 0..3 {
                        row.col(|ui| {
                            if let Some(output) = combination.listed_fusions.get(i) {
                                self.add_numbered_shard_text(ui, self.shards.get(output).unwrap(), MadeInFusion, combination);
                            }
                        });
                    }
                });
            });
    }

    pub fn cache_bazaar_prices(&mut self) {
        if let Some(bazaar_data) = self.bazaar_data.as_ref() {
            for shard_data in self.shards.values_mut() {
                let quick_status = &bazaar_data.get(&shard_data.get_bazaar_id()).unwrap().quick_status;
                shard_data.cached_bazaar_data = Some(quick_status.clone());
            }
        }
    }

    fn add_numbered_shard_text(
        &self,
        ui: &mut Ui,
        shard: &ShardData,
        amount_type: AmountType,
        combination: &FusionResults,
    ) {
        let amount = match amount_type {
            ConsumedInFusion => shard.get_amount_consumed_in_fusion(),
            MadeInFusion => shard.get_default_amount_made_in_fusion(),
        };
        ui.horizontal(|ui| {
            if amount_type == MadeInFusion && combination.is_reptile_fusion && self.pure_reptile_attribute_level > 0 {
                let higher_amount = amount * 2;
                ui.label(format!("{amount}-{higher_amount}x"));
            } else {
                ui.label(format!("{amount}x"));
            }
            add_shard_image(ui, shard, &self.images);
            ui.label(RichText::new(shard.shard_name.clone()).color(shard.rarity.get_color()));
            ui.label(format!("{}{}", shard.rarity.get_char(), shard.id))
        });
    }

    fn add_shard_cost(&self, ui: &mut Ui, shard: &ShardData, bazaar_data: &BazaarData) {
        let amount = shard.get_amount_consumed_in_fusion();
        if let Some(price_data) = bazaar_data.get(&shard.get_bazaar_id()) {
            let amount_string = ((price_data.quick_status.get_buy_price(self.buy_type) * amount as f64) as i64).to_formatted_string(&en);
            ui.label(RichText::new(amount_string).color(Color32::from_rgb(255, 170, 0)));
        }
    }

    fn sort_all_shards_if_necessary(&mut self, sort_type: AllFusionsSortType) {
        let new_hash = self.generate_sorting_recalculation_hash();
        if self.profit_data_hash.is_some_and(|h| h != new_hash) {
            return;
        }

        if let Some(profit_sorted_combinations) = self.combination_profit_data.as_mut() {
            profit_sorted_combinations.sort_by(|(a_shard_name, a_data), (b_shard_name, b_data)| {
                match sort_type {
                    AllFusionsSortType::ShardName => a_shard_name.cmp(b_shard_name)
                        .then(a_data.first_input_shard_name.cmp(&b_data.first_input_shard_name))
                        .then(a_data.second_input_shard_name.cmp(&b_data.second_input_shard_name)),
                    AllFusionsSortType::ShardIdAndRarity => {
                        let a_shard = &self.shards.get(a_shard_name).unwrap();
                        let a_shard_first_input = &self.shards.get(&a_data.first_input_shard_name).unwrap();
                        let a_shard_second_input = &self.shards.get(&a_data.second_input_shard_name).unwrap();
                        let b_shard = &self.shards.get(b_shard_name).unwrap();
                        let b_shard_first_input = &self.shards.get(&b_data.first_input_shard_name).unwrap();
                        let b_shard_second_input = &self.shards.get(&b_data.second_input_shard_name).unwrap();

                        a_shard.rarity.cmp(&b_shard.rarity)
                            .then(a_shard.id.cmp(&b_shard.id))
                            .then(a_shard_first_input.rarity.cmp(&b_shard_first_input.rarity).then(a_shard_first_input.id.cmp(&b_shard_first_input.id)))
                            .then(a_shard_second_input.rarity.cmp(&b_shard_second_input.rarity).then(a_shard_second_input.id.cmp(&b_shard_second_input.id)))
                    }
                    AllFusionsSortType::Profit => {
                        let a_profit = a_data.get_result_profit(a_shard_name, self.buy_type, self.profit_type, self.pure_reptile_attribute_level, self.bazaar_tax_percent, &self.shards);
                        let b_profit = b_data.get_result_profit(b_shard_name, self.buy_type, self.profit_type, self.pure_reptile_attribute_level, self.bazaar_tax_percent, &self.shards);
                        b_profit.total_cmp(&a_profit)
                    }
                }
            });
        }
    }

    fn generate_sorting_recalculation_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.sort_type.hash(&mut hasher);
        self.buy_type.hash(&mut hasher);
        self.profit_type.hash(&mut hasher);
        self.bazaar_tax_percent.to_string().hash(&mut hasher);
        self.pure_reptile_attribute_level.hash(&mut hasher);
        hasher.finish()
    }
}

fn add_shard_option(ui: &mut Ui, id: &str, selected_shard_name: &mut Option<String>, shards: &Shards, search_query: &str, images: &Rc<HashMap<String, TextureHandle>>) {
    let selected_text = selected_shard_name
        .as_ref()
        .map(|shard_name| {
            let shard = shards.get(shard_name).unwrap();
            create_shard_text_layout_job(shard)
        });

    let mut shards: Vec<(&String, &ShardData)> = shards.iter().collect();
    shards.sort_by(|(_, a), (_, b)| {
        a.rarity.cmp(&b.rarity).then(a.id.cmp(&b.id))
    });

    ui.horizontal(|ui| {
        let mut combo_box = egui::ComboBox::from_id_salt(id).height(400.0);
        if let Some(selected_text) = selected_text {
            combo_box = combo_box.selected_text(selected_text);
        } else {
            combo_box = combo_box.selected_text("None");
        }
        
        combo_box.show_ui(ui, |ui| {
            if ui.selectable_label(selected_shard_name.is_none(), "None").clicked() {
                *selected_shard_name = None;
            }

            for (_, shard) in shards.iter() {
                let shard_label = shard.format();
                if !shard_label.to_lowercase().contains(search_query.to_lowercase().as_str()) {
                    continue;
                }

                ui.horizontal(|ui| {
                    add_shard_image(ui, shard, images);
                    let text = create_shard_text_layout_job(shard);
                    let clicked = selected_shard_name.as_ref().is_some_and(|selected_shard_name| selected_shard_name == &shard.shard_name);
                    if ui.selectable_label(clicked, text).clicked() {
                        *selected_shard_name = Some(shard.shard_name.clone());
                    }
                });
            }
        });

        if selected_shard_name.is_some() && ui.button("Reset").clicked() {
            *selected_shard_name = None;
        }
    });
}

#[derive(PartialEq)]
enum AmountType {
    ConsumedInFusion,
    MadeInFusion,
}

fn create_shard_text_layout_job(shard: &ShardData) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(&shard.shard_name.clone(), 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: shard.rarity.get_color(),
        ..Default::default()
    });
    job.append(&format!("{}{}", shard.rarity.get_char(), shard.id), 5.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::GRAY,
        ..Default::default()
    });

    job
}

fn add_shard_text(ui: &mut Ui, shard: &ShardData, images: &Rc<HashMap<String, TextureHandle>>) {
    ui.horizontal(|ui| {
        add_shard_image(ui, shard, images);
        ui.label(RichText::new(shard.shard_name.clone()).color(shard.rarity.get_color()));
    });
}

fn add_shard_image(ui: &mut Ui, shard: &ShardData, images: &Rc<HashMap<String, TextureHandle>>) {
    let id = shard.id_override.clone().unwrap_or_else(|| shard.attribute_name.to_lowercase().replace("'", "").replace(" ", "_"));
    let image_name = format!("attribute_{id}.png");
    images::add_first_valid_image(images, ui, vec![image_name]);
}

fn get_profit_color(profit: i64) -> Color32 {
    if profit > 0 {
        Color32::from_rgb(85, 255, 85)
    } else {
        Color32::from_rgb(170, 0, 0)
    }
}