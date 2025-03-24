#![warn(clippy::all, rust_2018_idioms)]

mod catacombs_page;
mod catacombs_loot;
mod catacombs_loot_calculator;
mod app;
mod slayer_page;
mod slayer_loot_calculator;
mod slayer_loot;
mod images;

pub use app::CalculatorApp;
