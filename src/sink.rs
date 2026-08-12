use std::collections::HashSet;

use meilisearch_sdk::{
  client::Client,
  documents::DocumentDeletionQuery,
  errors::{Error, ErrorCode},
};

use crate::{config::MeilisearchConfig, model::SearchRecord};

pub async fn write_to_meilisearch(
  config: &MeilisearchConfig,
  records: &[SearchRecord],
  incremental: bool,
  only_urls: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
  // 未配置索引时允许只执行抓取和本地处理，不建立 Meilisearch 连接。
  if config.host.trim().is_empty() || config.index_uid.trim().is_empty() {
    return Ok(());
  }

  let client = Client::new(&config.host, Some(&config.api_key))?;
  let index = client.index(&config.index_uid);
  index
    .set_searchable_attributes(&config.index_setting.searchable_attributes)
    .await?;
  index
    .set_displayed_attributes(&config.index_setting.displayed_attributes)
    .await?;
  index
    .set_filterable_attributes(&config.index_setting.filterable_attributes)
    .await?;
  index
    .set_sortable_attributes(&config.index_setting.sortable_attributes)
    .await?;
  index
    .set_ranking_rules(&config.index_setting.ranking_rules)
    .await?;

  // 全量模式清空整个索引；增量模式只删除受影响页面，避免破坏其他页面的数据。
  if !incremental {
    index.delete_all_documents().await?;
    println!("MeiliSearch: cleared existing documents");
  } else {
    let urls: HashSet<&str> = only_urls.iter().map(String::as_str).collect();
    for url in urls {
      let escaped_url = url.replace('\\', "\\\\").replace('\'', "\\'");
      let filter = format!("url_without_anchor = '{escaped_url}'");
      let mut query = DocumentDeletionQuery::new(&index);
      query.with_filter(&filter);
      let task = index.delete_documents_with(&query).await?;
      task.wait_for_completion(&client, None, None).await?;
      println!("MeiliSearch: cleared existing documents for {url}");
    }
  }

  // 分批导入降低单次请求体大小，也避免一次任务占用过多服务端内存。
  for (batch_number, batch) in records.chunks(config.patch_size).enumerate() {
    index.add_documents(batch, Some("objectID")).await?;
    println!(
      "MeiliSearch: uploaded batch {} ({} records)",
      batch_number + 1,
      batch.len()
    );
  }
  Ok(())
}

/// Check the health of the MeiliSearch service and API key permissions before crawling.
/// When returning Err(message), message is a user-friendly error statement.
pub async fn preflight_check(config: &MeilisearchConfig) -> Result<(), String> {
  // Skip the check when the index is not configured (consistent with the write logic, allowing only fetching without writing).
  if config.host.trim().is_empty() || config.index_uid.trim().is_empty() {
    return Ok(());
  }

  let client = Client::new(&config.host, Some(&config.api_key)).map_err(|e| {
    format!(
      "MeiliSearch: MeiliSearch client initialization failed (host: {}) : {e}",
      config.host
    )
  })?;

  // Health Check
  if let Err(e) = client.health().await {
    return Err(format!(
      "MeiliSearch: Unable to connect to MeiliSearch ({}): {e}. Please check the meilisearch.host configuration, service status and network connectivity.",
      config.host
    ));
  }

  println!("MeiliSearch: MeiliSearch host is available.");

  // API key permission check: Read the target index and verify whether the key is valid and has index access permission.
  match client.get_index(&config.index_uid).await {
    Ok(_) => {
      println!("MeiliSearch: The API key is valid and the index \"{}\" is accessible.", config.index_uid);
      Ok(())
    }
    Err(Error::Meilisearch(e)) if e.error_code == ErrorCode::InvalidApiKey => {
      Err("MeiliSearch: API key is invalid (invalid_api_key). Please check whether the MEILISEARCH_API_KEY environment variable is set correctly.".into())
    }
    Err(Error::Meilisearch(e)) if e.error_code == ErrorCode::MissingAuthorizationHeader => {
      Err("No valid Authorization header (missing_authorization_header) was provided. Please set the MEILISEARCH_API_KEY environment variable.".into())
    }
    Err(Error::Meilisearch(e)) if e.error_code == ErrorCode::IndexNotFound => {
      // Meilisearch also returns index_not_found for indexes without permission. It is impossible to distinguish them and prompt them together.
      println!("MeiliSearch: The index \"{}\" does not exist yet (it will be automatically created during the first run). If the index already exists, please confirm that the API key has the permissions for this index: indexes.get, settings.update, documents.add, documents.delete, tasks.get", config.index_uid);
      Ok(())
    }
    Err(e) => Err(format!("MeiliSearch check failed: {e}")),
  }
}
