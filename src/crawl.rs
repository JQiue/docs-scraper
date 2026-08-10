use std::{
  collections::{HashSet, VecDeque},
  time::Duration,
};

use quick_xml::de::from_str;
use reqwest::{Client, header::CONTENT_TYPE};
use scraper::{Html, Selector};
use serde::Deserialize;
use url::Url;

use crate::{config::Config, extract::extract_page, model::SearchRecord};

type FetchTask = Result<Result<Option<(Url, String)>, String>, reqwest::Error>;

/// 抓取入口：收集初始 URL，按并发窗口获取 HTML，并将页面交给结构化抽取器。
///
/// `only_urls` 是严格的增量模式：它只抓取配置中的页面，不从页面链接继续扩展。
/// 普通模式则以起始 URL 和 Sitemap 为种子，只跟随同域名链接。
pub async fn crawl(config: &Config) -> Result<Vec<SearchRecord>, Box<dyn std::error::Error>> {
  let client = Client::builder()
    .timeout(Duration::from_secs(config.crawl.timeout_seconds))
    .user_agent(config.crawl.user_agent.as_deref().unwrap_or("docs-scraper"))
    .build()?;
  // queue 是待抓取队列。增量模式直接使用白名单，普通模式先合并起始 URL 和 Sitemap。
  let mut queue: VecDeque<Url> = if !config.only_urls.is_empty() {
    config
      .only_urls
      .iter()
      .filter_map(|value| Url::parse(value).ok())
      .collect()
  } else {
    let mut urls: VecDeque<Url> = config
      .start_urls
      .iter()
      .filter_map(|value| Url::parse(value).ok())
      .collect();

    for sitemap in &config.sitemap_urls {
      let sitemap_urls = load_sitemap(&client, sitemap).await?;
      println!(
        "Docs-Scraper: loaded {} URLs from {}",
        sitemap_urls.len(),
        sitemap
      );
      urls.extend(sitemap_urls);
    }

    urls
  };
  // queued 防止同一个页面被重复加入队列；visited 则记录已经开始请求的页面。
  let mut queued: HashSet<String> = queue.iter().map(canonical_url).collect();
  // 保存规范化后的白名单，用来拦截重定向到白名单之外的 URL。
  let only_urls: Option<HashSet<String>> = if config.only_urls.is_empty() {
    None
  } else {
    Some(queued.clone())
  };
  let allowed: HashSet<String> = queue
    .iter()
    .filter_map(|u| u.host_str().map(str::to_string))
    .collect();
  let mut visited = HashSet::new();
  let mut records = Vec::new();
  let mut fetched = 0;
  let mut failed = 0;
  let mut skipped = 0;
  let links = Selector::parse("a[href]").unwrap();
  let workers = config.crawl.concurrency.max(1);

  // 每轮先填满并发窗口，再逐个接收完成的任务；任务完成后立即用新发现的链接补位。
  while !queue.is_empty() && visited.len() < config.crawl.max_pages {
    let mut tasks = tokio::task::JoinSet::new();
    spawn_available_tasks(
      &mut tasks,
      &mut queue,
      &mut visited,
      &client,
      config,
      workers,
    );

    // 请求错误、非 HTML 响应和非法增量重定向都不会进入抽取器。
    while let Some(result) = tasks.join_next().await {
      let page = match result? {
        Ok(p) => p,
        Err(e) => {
          failed += 1;
          eprintln!("Docs-Scraper: request failed: {e}");
          continue;
        }
      };
      let page = match page {
        Ok(page) => page,
        Err(error) => {
          failed += 1;
          eprintln!("Docs-Scraper: request failed: {error}");
          continue;
        }
      };
      let Some((final_url, html)) = page else {
        skipped += 1;
        continue;
      };

      if only_urls
        .as_ref()
        .is_some_and(|allowed_urls| !allowed_urls.contains(&canonical_url(&final_url)))
      {
        skipped += 1;
        eprintln!(
          "Docs-Scraper: skipped redirect outside only_urls: {}",
          final_url
        );
        continue;
      }

      fetched += 1;
      let page_records =
        extract_page(&html, final_url.as_str(), config).map_err(std::io::Error::other)?;
      println!(
        "Docs-Scraper: {} ({} records)",
        final_url,
        page_records.len()
      );
      records.extend(page_records);

      // 只有完整抓取才扩展链接；增量模式必须保持严格的 URL 边界。
      if only_urls.is_none() {
        for link in Html::parse_document(&html).select(&links) {
          let Some(href) = link.value().attr("href") else {
            continue;
          };
          let Ok(next) = final_url.join(href) else {
            continue;
          };
          if allowed.contains(next.host_str().unwrap_or("")) && queued.insert(canonical_url(&next))
          {
            queue.push_back(next);
          }
        }
      }

      // 页面完成后用新发现的 URL 补充并发窗口，避免等待整轮结束才继续抓取。
      spawn_available_tasks(
        &mut tasks,
        &mut queue,
        &mut visited,
        &client,
        config,
        workers,
      );
    }
  }
  println!(
    "Docs-Scraper: finished: {fetched} pages fetched, {failed} failed, {skipped} skipped, {} records",
    records.len()
  );
  Ok(records)
}

/// 从队列填充并发窗口。
///
/// URL 在创建任务前加入 `visited`，因此同一页面即使由多个页面链接发现，也只会请求一次。
fn spawn_available_tasks(
  tasks: &mut tokio::task::JoinSet<FetchTask>,
  queue: &mut VecDeque<Url>,
  visited: &mut HashSet<String>,
  client: &Client,
  config: &Config,
  workers: usize,
) {
  while tasks.len() < workers && visited.len() + tasks.len() < config.crawl.max_pages {
    let Some(url) = queue.pop_front() else { break };
    if !visited.insert(canonical_url(&url)) || is_stop(config, &url) {
      continue;
    }
    tasks.spawn(fetch_page(client.clone(), url));
  }
}

async fn fetch_page(client: Client, url: Url) -> FetchTask {
  // 三层结果分别表示：网络请求错误、HTTP/业务错误、以及成功但可能不是 HTML。
  let response = client.get(url.clone()).send().await?;

  if !response.status().is_success() {
    return Ok(Err(format!("{url} returned {}", response.status())));
  }

  if !response
    .headers()
    .get(CONTENT_TYPE)
    .and_then(|v| v.to_str().ok())
    .is_some_and(|v| v.contains("text/html"))
  {
    return Ok(Ok(None));
  }

  let final_url = response.url().clone();
  Ok(Ok(Some((final_url, response.text().await?))))
}

#[derive(Debug, Deserialize)]
struct SitemapUrlSet {
  #[serde(rename = "url", default)]
  urls: Vec<SitemapEntry>,
}

#[derive(Debug, Deserialize)]
struct SitemapIndex {
  #[serde(rename = "sitemap", default)]
  sitemaps: Vec<SitemapEntry>,
}

#[derive(Debug, Deserialize)]
struct SitemapEntry {
  loc: String,
}

async fn load_sitemap(
  client: &Client,
  sitemap: &str,
) -> Result<Vec<Url>, Box<dyn std::error::Error>> {
  // Sitemap 可以是 URL 集合，也可以是 Sitemap Index；pending 将索引递归展开为页面 URL。
  let mut pending = VecDeque::from([sitemap.to_string()]);
  let mut urls = Vec::new();

  while let Some(current) = pending.pop_front() {
    let response = client.get(&current).send().await?.error_for_status()?;
    let xml = response.text().await?;

    if let Ok(set) = from_str::<SitemapUrlSet>(&xml) {
      urls.extend(set.urls.into_iter().filter_map(|e| Url::parse(&e.loc).ok()));
      continue;
    }

    let index: SitemapIndex = from_str(&xml)?;

    for entry in index.sitemaps {
      let next = Url::parse(&entry.loc)
        .or_else(|_| Url::parse(&current).and_then(|base| base.join(&entry.loc)))?;
      pending.push_back(next.to_string());
    }
  }
  Ok(urls)
}

fn is_stop(config: &Config, url: &Url) -> bool {
  config.stop_urls.as_ref().is_some_and(|stops| {
    stops
      .iter()
      .any(|s| s == url.as_str() || s == &canonical_url(url))
  })
}

/// 去掉 fragment 后得到页面级 URL，用于队列去重和增量边界判断。
fn canonical_url(url: &Url) -> String {
  let mut value = url.clone();
  value.set_fragment(None);
  value.to_string()
}
