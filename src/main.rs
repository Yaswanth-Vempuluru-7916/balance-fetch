use alloy::network::Ethereum;
use alloy_primitives::{Address, utils::format_ether};
use alloy_provider::{Provider, RootProvider};
use eyre::{Context, Ok, Result, bail};
use serde::Deserialize;
use std::{fs, path::Path};
#[derive(Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub address: String,
}

impl Config {
    pub fn load_from_toml<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .wrap_err_with(|| format!("Failed to read config file at {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .wrap_err_with(|| format!("Failed to parse config file at {}", path.display()))?;

        config
            .validate()
            .wrap_err_with(|| format!("Invalid config file at {}", path.display()))?;

        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.rpc_url.is_empty() {
            bail!("rpc_url cannot be empty");
        }

        if self.address.is_empty() {
            bail!("address cannot be empty");
        }

        Ok(())
    }
}

async fn get_balance(rpc_url: &str, address_str: &str) -> Result<String> {
    let provider = RootProvider::<Ethereum>::new_http(rpc_url.parse()?);
    let address: Address = address_str.parse()?;
    let balance = provider.get_balance(address).await?;
    let balance_eth = format_ether(balance);
    Ok(balance_eth)
}
#[tokio::main]
async fn main() -> Result<()> {
    // Read input from toml file
    let config = Config::load_from_toml("config.toml").wrap_err("Failed to load config file")?;
    println!("Configuration loaded successfully:");

    //Fetch the balance
    let balance = get_balance(&config.rpc_url, &config.address).await?;
    println!("\nBalance: {} ETH", balance);

    Ok(())
}
