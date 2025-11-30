use std::{fs, time::Duration};

use eyre::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{error, info, warn};
#[derive(Debug, Deserialize)]
struct ApiResponse {
  status : String,
  message : String,
  result : String
}

#[derive(Debug, Deserialize)]
struct Config {
  rpc_url : String,
  #[serde(rename = "address")]
  wallet_address : String,
  #[serde(rename = "ETHERSCAN_BASE_URL")]
  etherscan_base_url : String,
  #[serde(rename = "ETHERSCAN_API_TOKEN")]
  etherscan_api_key : String,
  #[serde(rename = "CONTRACT_ADDRESS")]
  contract_address : String
}

#[tokio::main]
async fn main()->Result<()>{

  color_eyre::install()?;
  tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

  let config_content = fs::read_to_string("config.toml")?;
  let config : Config = toml::from_str(&config_content)?;

  info!("{:?}",config);

  // Client Builder -> Client
  let client = Client::builder()
    .timeout(Duration::from_secs(5))
    .connect_timeout(Duration::from_secs(5))
    .pool_idle_timeout(Duration::from_secs(90))
    .pool_max_idle_per_host(20)
    .build()
    .wrap_err(format!("Failed to build the client"))?;

  match fetch_erc_20_balance(&client, &config).await {
    Ok(balance_response) => {
      info!("Balance {:?} ", balance_response);
      info!("Balance {:?}", balance_response.result);
    },
    Err(e) => {
       error!(error = %e, "Failed to fetch balance — this is expected sometimes");
            if e.to_string().contains("timeout") {
                warn!("Request timed out — consider increasing timeout or checking network");
            } else if e.chain().any(|cause| cause.to_string().contains("429")) {
                warn!("Rate limited by API — implement retry with backoff");
            } else {
                // Re-raise for monitoring / process restart if needed
                return Err(e);
            }
    }
  }
  Ok(())

}


async fn fetch_erc_20_balance(client : &Client, config : &Config) -> Result<ApiResponse> {
  let url = format!(
    "{base}?apikey={key}&chainid=42161&module=account&action=tokenbalance&contractaddress={contract}&address={addr}&tag=latest",
    base = config.etherscan_base_url,
    key = config.etherscan_api_key,
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

    let resp : ApiResponse = serde_json::from_str(&body_text)
        .wrap_err_with(|| format!("failed to parse JSON (body was {} bytes)", body_text.len()))?;

    Ok(resp)

}
