use serde::Deserialize;
use std::collections::HashMap;
use crate::shards::shards_page::{BuyType, ProfitType};

pub type BazaarData = HashMap<String, BazaarProduct>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BazaarResponse {
    pub success: bool,
    pub last_updated: u64,
    pub products: HashMap<String, BazaarProduct>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BazaarProduct {
    pub product_id: String,
    pub sell_summary: Vec<OrderSummary>,
    pub buy_summary: Vec<OrderSummary>,
    pub quick_status: QuickStatus,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OrderSummary {
    amount: f64,
    price_per_unit: f64,
    orders: u64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuickStatus {
    pub product_id: String,
    pub sell_price: f64,
    pub sell_volume: u64,
    pub sell_moving_week: u64,
    pub sell_orders: u64,
    pub buy_price: f64,
    pub buy_volume: u64,
    pub buy_moving_week: u64,
    pub buy_orders: u64,
}

impl QuickStatus {
    pub fn get_buy_price(&self, buy_type: BuyType) -> f64 {
        match buy_type {
            BuyType::BuyOrder => self.sell_price,
            BuyType::InstaBuy => self.buy_price,
        }
    }

    pub fn get_sell_price(&self, profit_type: ProfitType) -> f64 {
        match profit_type {
            ProfitType::InstaSell => self.sell_price,
            ProfitType::SellOffer => self.buy_price,
        }
    }
}
