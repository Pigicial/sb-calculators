use crate::shards::bazaar_data::{BazaarData, BazaarResponse};
use crossbeam_channel::Sender;
use reqwest::{Client, Error};

#[cfg(target_arch = "wasm32")]
pub fn set_shard_prices(sender: Sender<Option<BazaarData>>) {
    println!("Getting prices");
    wasm_bindgen_futures::spawn_local(async move {
        let client = Client::new();
        if let Ok(response) = client.get("https://api.hypixel.net/skyblock/bazaar").send().await {
            let json_response = response.json::<BazaarResponse>().await;
            let bazaar_json = process_bazaar_json(json_response);
            let _ = sender.send(bazaar_json);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_shard_prices(sender: Sender<Option<BazaarData>>) {
    println!("Getting prices");

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let client = Client::new();
            if let Ok(response) = client.get("https://api.hypixel.net/skyblock/bazaar").send().await {
                let json_response = response.json::<BazaarResponse>().await;
                let bazaar_json = process_bazaar_json(json_response);
                let _ = sender.send(bazaar_json);
            }
        });
    });
}

pub fn process_bazaar_json(json_response: Result<BazaarResponse, Error>) -> Option<BazaarData> {
    match json_response {
        Ok(bazaar_data) => {
            Some(bazaar_data
                .products
                .iter()
                .filter(|(k, _)| k.starts_with("SHARD_"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BazaarData>())
        }
        Err(_) => None
    }
}