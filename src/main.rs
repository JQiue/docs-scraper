mod config;
mod crawl;
mod extract;
mod model;
mod sink;

use crate::{config::Config, sink::preflight_check};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let config = Config::from_file("./config.json")?;

  if let Err(message) = preflight_check(&config.meilisearch).await {
    eprintln!("{message}");
    std::process::exit(1);
  }

  let records = crawl::crawl(&config).await?;
  sink::write_to_meilisearch(
    &config.meilisearch,
    &records,
    !config.only_urls.is_empty(),
    &config.only_urls,
  )
  .await?;
  Ok(())
}
