use std::collections::HashSet;

use meilisearch_sdk::{client::Client, documents::DocumentDeletionQuery};

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
