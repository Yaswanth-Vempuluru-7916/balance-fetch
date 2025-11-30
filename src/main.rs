use std::{fs, time::Duration};

use eyre::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{info, warn};

fn units_to_human(units: u128, decimals: u8) -> f64 {
    units as f64 / (10_f64).powi(decimals as i32)
}

fn string_to_u128<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<u128>().map_err(serde::de::Error::custom)
}
#[derive(Debug, Deserialize)]
struct ApiResponse {
    status: String,
    message: String,
    #[serde(deserialize_with = "string_to_u128")]
    result: u128,
}

#[derive(Debug, Deserialize)]
struct Config {
    rpc_url: String,
    wallet_address: String,
    etherscan_base_url: String,
    etherscan_api_key: String,
    contract_address: String,
    chain_id: u64,
    chain_name: String,
    token_symbol: String,
    token_decimals: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config_content = fs::read_to_string("config.toml")?;
    let config: Config = toml::from_str(&config_content)?;

    // Client Builder -> Client
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(20)
        .build()
        .wrap_err(format!("Failed to build the client"))?;

    let wei = fetch_erc_20_balance(&client, &config).await?;
    let usdt_eth = units_to_human(wei, config.token_decimals); // USDT, USDC, BUSD, etc.
    info!(
        chain = config.chain_name,
        wallet = config.wallet_address,
        token = config.token_symbol,
        balance_wei = wei,
        balance_human = %format!("{:.precision$}", usdt_eth, precision = config.token_decimals as usize),
        "ERC20 balance"
    );
    Ok(())
}

async fn fetch_erc_20_balance(client: &Client, config: &Config) -> Result<u128> {
    let url = format!(
        "{base}?apikey={key}&chainid={chain_id}&module=account&action=tokenbalance&contractaddress={contract}&address={addr}&tag=latest",
        base = config.etherscan_base_url,
        key = config.etherscan_api_key,
        chain_id = config.chain_id,
        contract = config.contract_address,
        addr = config.wallet_address,
    );

    let response = client
        .get(&url)
        .send()
        .await
        .wrap_err(format!("failed to send request to {:?}", &url))?;

    let status = response.status();
    let body_text = match response.text().await {
        Ok(text) => text,
        Err(err) => {
            return Err(err)
                .wrap_err("failed to read response body as text (needed for error debugging)");
        }
    };

    if !status.is_success() {
        match status {
            StatusCode::TOO_MANY_REQUESTS => {
                warn!("Received 429 — we are being rate limited");
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                warn!("503 from upstream — service temporarily down");
            }
            _ => {
                warn!(%status, body = %body_text, "non-2xx response from API");
            }
        }
        return Err(eyre::eyre!("HTTP {status} — {body_text}"));
    }

    let resp: ApiResponse =
        serde_json::from_str(&body_text).wrap_err("failed to parse Etherscan response")?;

    if resp.status != "1" {
        return Err(eyre::eyre!(
            "Etherscan error: {} - {}",
            resp.status,
            resp.message
        ));
    }
    Ok(resp.result)
}
