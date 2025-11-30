use eyre::{Result, WrapErr};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug, Deserialize)]
struct Todo {
    #[serde(rename = "userId")]
    user_id: u32,
    id: u32,
    title: String,
    completed: bool,
}

async fn fetch_todos(client: &Client) -> Result<Vec<Todo>> {
    let url = "https://jsonplaceholder.typicode.com/todos";
    let response = client
        .get(url)
        .send()
        .await
        .wrap_err(format!("failed to send request to {:?}", url))?;

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

    let todos: Vec<Todo> = serde_json::from_str(&body_text)
        .wrap_err_with(|| format!("failed to parse JSON (body was {} bytes)", body_text.len()))?;

    Ok(todos)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(20)
        .build()
        .wrap_err("Failed to build reqwest client")?;

    match fetch_todos(&client).await {
        Ok(todos) => {
            info!("Fetched {} todos successfully", todos.len());
            for todo in todos.iter().take(10) {
                info!(todo.user_id, todo.id, todo.completed, title = %todo.title, "todo");
            }
        }

        Err(e) => {
            error!(error = %e, "Failed to fetch todos — this is expected sometimes");
            if e.to_string().contains("timeout") {
                warn!("Request timed out — consider increasing timeout or checking network");
            } else if e.chain().any(|cause| cause.to_string().contains("429")) {
                warn!("Rate limited by API — implement retry with backoff");
            } else {
                // Re-raise for monitoring / process restart if needed
                return Err(e);
            }

            info!("Continuing with empty todo list due to recoverable error");
        }
    }

    Ok(())
}
