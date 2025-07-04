use crate::catacombs::catacombs_page::CatacombsLootPage;
use crate::slayer::slayer_page::SlayerLootPage;
use crate::shards::shards_page::ShardsPage;
use eframe::epaint::{Color32, FontId, TextureHandle};
use egui::{vec2, Context, Direction, Image, Label, ThemePreference, Ui, Widget};
use egui_extras::image::load_image_bytes;
use include_dir::{include_dir, Dir};
use std::collections::HashMap;
use std::rc::Rc;
use eframe::epaint::text::{TextFormat, TextWrapMode};
use egui::text::LayoutJob;
use num_format::Locale::is;

pub(crate) static ASSETS_DIR: Dir<'static> = include_dir!("assets");

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Page {
    Catacombs,
    Slayer,
    Shards,
}

impl Page {
    fn from_str_case_insensitive(page: &str) -> Option<Self> {
        let page = page.to_lowercase();
        if page.to_lowercase() == "catacombs" {
            Some(Page::Catacombs)
        } else if page.to_lowercase() == "slayer" {
            Some(Page::Slayer)
        } else if page.to_lowercase() == "shards" {
            Some(Page::Shards)
        } else {
            None
        }
    }
}

impl std::fmt::Display for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut name = format!("{self:?}");
        name.make_ascii_lowercase();
        f.write_str(&name)
    }
}

pub struct CalculatorApp {
    selected_page: Page,
    catacombs_page: CatacombsLootPage,
    slayer_page: SlayerLootPage,
    shards_page: ShardsPage,

    images: Rc<HashMap<String, TextureHandle>>,
}

impl eframe::App for CalculatorApp {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        let is_web = cfg!(target_arch = "wasm32");
        let screen_size = ctx.screen_rect().size();
        let is_mobile = screen_size.x < 550.0 && is_web;
        
        // todo: mobile only since cata loot is disabled on mobile
        if !is_mobile {
            #[cfg(target_arch = "wasm32")]
            if let Some(page) = frame.info().web_info.location.hash.strip_prefix('#').and_then(Page::from_str_case_insensitive) {
                self.selected_page = page;
            }
        }


        ctx.set_theme(ThemePreference::Dark);

        egui::TopBottomPanel::top("top_panel")
            .frame(egui::Frame::new().inner_margin(4))
            .show(ctx, |ui| {
                // The top panel is often a good place for a menu bar:

                if is_mobile {
                    egui::menu::bar(ui, |ui| {
                        add_code_pig_text(ui);
                    });
                    egui::menu::bar(ui, |ui| {
                        ui.with_layout(egui::Layout::centered_and_justified(Direction::TopDown), |ui| {
                            ui.add(Label::new("(Note: A Catacombs loot calculator is also available on desktop!)").wrap_mode(TextWrapMode::Wrap));
                        });
                    });
                } else {
                    // none of the conditions in here support mobile
                    egui::menu::bar(ui, |ui| {
                        if !is_web {
                            ui.menu_button("File", |ui| {
                                if ui.button("Quit").clicked() {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                            ui.add_space(16.0);
                        }

                        // todo: this is temporary in the non-mobile-only section as cata loot is broken on mobile
                        let mut selected_page = self.selected_page;
                        for (name, page, _app) in self.apps_iter_mut() {
                            if ui.selectable_label(selected_page == page, name).clicked() {
                                selected_page = page;
                                if frame.is_web() {
                                    ui.ctx().open_url(egui::OpenUrl::same_tab(format!("#{page}")));
                                }
                            }
                        }
                        self.selected_page = selected_page;

                        if !is_web {
                            ui.add_space(30.0);
                            egui::gui_zoom::zoom_menu_buttons(ui);
                        }

                        if !is_mobile {
                            add_code_pig_text(ui);
                        }
                    });
                }
            });

        self.show_selected_page(ctx, frame);
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

impl CalculatorApp {
    pub fn new(context: &Context) -> Self {
        let images = Rc::new(load_images(context));
        Self {
            selected_page: Page::Shards,
            catacombs_page: CatacombsLootPage::new(Rc::clone(&images)),
            slayer_page: SlayerLootPage::new(Rc::clone(&images)),
            shards_page: ShardsPage::new(Rc::clone(&images)),
            images,
        }
    }

    fn show_selected_page(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        let selected_page = self.selected_page;
        for (_name, page, app) in self.apps_iter_mut() {
            if page == selected_page || ctx.memory(|mem| mem.everything_is_visible()) {
                app.update(ctx, frame);
            }
        }
    }

    pub fn apps_iter_mut(&mut self) -> impl Iterator<Item=(&'static str, Page, &mut dyn eframe::App)> {
        let vec = vec![
            (
                "★ Shards",
                Page::Shards,
                &mut self.shards_page as &mut dyn eframe::App,
            ),
            (
                "☠ Catacombs",
                Page::Catacombs,
                &mut self.catacombs_page as &mut dyn eframe::App,
            ),
            //("⚔ Slayer", Page::Slayer, &mut self.slayer_page as &mut dyn eframe::App),
            // todo: enable slayer page
            
        ];

        vec.into_iter()
    }
}

fn load_images(ctx: &Context) -> HashMap<String, TextureHandle> {
    let mut images = HashMap::new();
    for file in ASSETS_DIR.find("**/*.png").unwrap().chain(ASSETS_DIR.find("**/*.gif").unwrap()) {
        let file_name = file.path().file_name().and_then(|n| n.to_str()).unwrap();
        println!("Loading {}", file_name);
        let bytes = file.as_file().unwrap().contents();

        if let Ok(dynamic_image) = load_image_bytes(bytes) {
            let texture = ctx.load_texture(file_name, dynamic_image, egui::TextureOptions::default());
            images.insert(file_name.to_string(), texture);
        }
    }

    images
}

fn add_code_pig_text(ui: &mut Ui) {
    let mut text = LayoutJob::default();
    text.append("Use code \"", 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::GRAY,
        ..Default::default()
    });
    text.append("Pig", 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::from_rgb(255, 85, 255),
        ..Default::default()
    });
    text.append("\" on the ", 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::GRAY,
        ..Default::default()
    });
    text.append("Hypixel Store", 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::from_rgb(85, 255, 255),
        ..Default::default()
    });
    text.append(" for ", 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::GRAY,
        ..Default::default()
    });
    text.append("5% off", 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::from_rgb(85, 255, 85),
        ..Default::default()
    });
    text.append(" your order!", 0.0, TextFormat {
        font_id: FontId::proportional(14.0),
        color: Color32::GRAY,
        ..Default::default()
    });

    ui.with_layout(egui::Layout::centered_and_justified(Direction::RightToLeft), |ui| {
        ui.add(Label::new(text).wrap_mode(TextWrapMode::Wrap));
    });
}
