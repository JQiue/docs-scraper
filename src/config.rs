use std::{collections::HashMap, path::Path};

use serde::Deserialize;

fn default_max_pages() -> usize {
  1000
}

fn default_concurrency() -> usize {
  4
}

fn default_timeout_seconds() -> u64 {
  60
}

fn default_patch_size() -> usize {
  500
}

#[derive(Deserialize, Debug)]
pub struct MeilisearchConfig {
  pub host: String,
  #[serde(default)]
  pub api_key: String,
  pub index_uid: String,
  pub index_setting: IndexSetting,
  #[serde(default = "default_patch_size")]
  pub patch_size: usize,
}

#[derive(Default, Deserialize, Debug)]
pub struct IndexSetting {
  #[serde(default)]
  pub displayed_attributes: Vec<String>,
  #[serde(default)]
  pub searchable_attributes: Vec<String>,
  #[serde(default)]
  pub filterable_attributes: Vec<String>,
  #[serde(default)]
  pub sortable_attributes: Vec<String>,
  #[serde(default)]
  pub ranking_rules: Vec<String>,
}

/// 根据页面 URL 覆盖搜索结果的页面级排序权重。
#[derive(Debug, Deserialize)]
pub struct PageRankRule {
  pub pattern: String,
  pub rank: i32,
}

/// HTTP 抓取的资源限制和请求参数。
#[derive(Deserialize, Debug, Default)]
pub struct CrawlConfig {
  #[serde(default = "default_concurrency")]
  pub concurrency: usize,
  #[serde(default = "default_max_pages")]
  pub max_pages: usize,
  #[serde(default = "default_timeout_seconds")]
  pub timeout_seconds: u64,
  #[serde(default)]
  pub user_agent: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Config {
  #[serde(default)]
  pub only_urls: Vec<String>,
  #[serde(default)]
  pub start_urls: Vec<String>,
  #[serde(default)]
  pub sitemap_urls: Vec<String>,
  #[serde(default)]
  pub stop_urls: Option<Vec<String>>,
  #[serde(default)]
  pub selectors: HashMap<String, SelectorEntry>,
  #[serde(default)]
  pub page_rank: Vec<PageRankRule>,
  #[serde(default)]
  pub crawl: CrawlConfig,
  pub meilisearch: MeilisearchConfig,
  // pub allowed_domains: Vec<String>,
}

impl Config {
  pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, config::ConfigError> {
    let mut cfg: Config = config::Config::builder()
      .add_source(config::File::from(path.as_ref()))
      .build()?
      .try_deserialize()?;
    let api_key = std::env::var("MEILISEARCH_API_KEY").unwrap_or_default();

    if api_key.is_empty() {
      panic!("meilisearch api key not set: set MEILISEARCH_API_KEY first!")
    }

    cfg.meilisearch.api_key = api_key;
    Ok(cfg)
  }

  pub fn page_rank_for(&self, page_url: &str) -> i32 {
    let mut rank = 0;
    for rule in &self.page_rank {
      if matches_page_pattern(page_url, &rule.pattern) {
        rank = rule.rank;
      }
    }
    rank
  }
}

fn matches_page_pattern(page_url: &str, pattern: &str) -> bool {
  if let Some(prefix) = pattern.strip_suffix("**") {
    page_url.starts_with(prefix)
  } else {
    page_url == pattern
  }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SelectorEntry {
  Simple(String),
  Detailed {
    selector: String,
    #[serde(default)]
    global: bool,
    default_value: Option<String>,
    #[allow(dead_code)]
    attribute: Option<String>,
  },
}

impl SelectorEntry {
  pub fn raw_selector(&self) -> &str {
    match self {
      Self::Simple(s) => s,
      Self::Detailed { selector, .. } => selector,
    }
  }

  pub fn is_global(&self) -> bool {
    match self {
      Self::Simple(_) => false,
      Self::Detailed { global, .. } => *global,
    }
  }

  pub fn default_value(&self) -> Option<&str> {
    match self {
      Self::Simple(_) => None,
      Self::Detailed { default_value, .. } => default_value.as_deref(),
    }
  }
}
