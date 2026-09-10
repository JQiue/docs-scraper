use std::collections::HashMap;

use scraper::{ElementRef, Html, Selector};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
  config::{Config, SelectorEntry},
  model::{HierarchyFields, SearchRecord},
};

pub fn get_level_weight(record_type: &str) -> i32 {
  record_type
    .strip_prefix("lvl")
    .and_then(|value| value.parse::<i32>().ok())
    .map_or(0, |level| 100 - level * 10)
}

pub fn extract_page(
  html: &str,
  page_url: &str,
  config: &Config,
) -> Result<Vec<SearchRecord>, String> {
  // 选择器只解析一次；后续遍历 DOM 时复用已编译的 Selector，避免重复解析配置。
  let document = Html::parse_document(html);
  let mut selectors = HashMap::new();

  for (name, entry) in &config.selectors {
    let raw_selector = entry.raw_selector().trim();
    if raw_selector.is_empty() {
      continue;
    }
    selectors.insert(
      name.clone(),
      Selector::parse(raw_selector)
        .map_err(|error| format!("selectors.{name}: invalid CSS selector: {error}"))?,
    );
  }

  let body_selector = Selector::parse("body").unwrap();
  let lang = document
    .select(&Selector::parse("html[lang]").unwrap())
    .next()
    .and_then(|element| element.value().attr("lang"))
    .unwrap_or("zh-CN")
    .to_string();
  let mut hierarchy = vec![None; 7];
  let mut records = Vec::new();
  let mut position = 0;
  let all_elements_selector = Selector::parse("*").unwrap();

  // global 标题是页面上下文，不参与 DOM 遍历；它们先建立初始层级并生成记录。
  for level in 0..=6 {
    let name = format!("lvl{level}");
    let Some(entry) = config.selectors.get(&name) else {
      continue;
    };
    if !entry.is_global() {
      continue;
    }
    let value = selectors
      .get(&name)
      .and_then(|selector| document.select(selector).next())
      .map(|element| (element_anchor(element), clean_text(element)))
      .filter(|(_, text)| !text.is_empty())
      .map(|(anchor, text)| anchor.unwrap_or(text))
      .filter(|text| !text.is_empty())
      .or_else(|| entry.default_value().map(str::to_owned));
    if let Some(value) = value {
      hierarchy[level] = Some(value.clone());
      records.push(make_record(
        page_url,
        lang.clone(),
        Some(value),
        None,
        format!("lvl{level}"),
        position,
        &hierarchy,
        Some(level),
        config.page_rank_for(page_url),
      ));
      position += 1;
    }
  }

  // 按 DOM 顺序处理元素：标题更新当前层级，正文继承最近的层级路径。
  for body in document.select(&body_selector) {
    for element in body.select(&all_elements_selector) {
      let matched_level = (0..=6).find(|level| {
        selectors
          .get(&format!("lvl{level}"))
          .is_some_and(|selector| selector.matches(&element))
      });

      if matched_level.is_some_and(|level| {
        config
          .selectors
          .get(&format!("lvl{level}"))
          .is_some_and(SelectorEntry::is_global)
      }) {
        continue;
      }

      let is_content = selectors
        .get("content")
        .is_some_and(|selector| selector.matches(&element));

      if let Some(level) = matched_level {
        let text = clean_text(element);
        if text.is_empty() {
          continue;
        }
        // 新标题会覆盖本层，并清除更深层级，防止后续正文继承旧的兄弟节点路径。
        hierarchy[level] = Some(text.clone());

        for item in hierarchy.iter_mut().skip(level + 1) {
          *item = None;
        }

        let anchor = Some(element_anchor(element).unwrap_or(text.clone()));
        // 正文使用当前最深层级作为 anchor；没有标题时该字段保持为空。
        records.push(make_record(
          page_url,
          lang.clone(),
          anchor,
          None,
          format!("lvl{level}"),
          position,
          &hierarchy,
          Some(level),
          config.page_rank_for(page_url),
        ));
        position += 1;
      } else if is_content {
        let text = clean_text(element);

        if text.is_empty() {
          continue;
        }

        records.push(make_record(
          page_url,
          lang.clone(),
          hierarchy.iter().rev().find_map(Clone::clone),
          Some(text),
          "content".to_string(),
          position,
          &hierarchy,
          None,
          config.page_rank_for(page_url),
        ));
        position += 1;
      }
    }
  }
  Ok(records)
}

fn clean_text(element: ElementRef<'_>) -> String {
  element
    .text()
    .collect::<Vec<_>>()
    .join(" ")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

fn element_anchor(element: ElementRef<'_>) -> Option<String> {
  element
    .value()
    .attr("id")
    .map(str::to_owned)
    .filter(|id| !id.trim().is_empty())
}

#[allow(clippy::too_many_arguments)]
fn make_record(
  url: &str,
  lang: String,
  anchor: Option<String>,
  content: Option<String>,
  record_type: String,
  position: usize,
  hierarchy: &[Option<String>],
  radio_level: Option<usize>,
  page_rank: i32,
) -> SearchRecord {
  let parsed = Url::parse(url).ok();
  let url_without_anchor = parsed
    .as_ref()
    .map(|value| {
      let mut clone = value.clone();
      clone.set_fragment(None);
      clone.to_string()
    })
    .unwrap_or_else(|| url.to_string());
  let record_url = match (&anchor, Url::parse(&url_without_anchor).ok()) {
    (Some(anchor), Some(mut parsed)) if !anchor.is_empty() => {
      parsed.set_fragment(Some(anchor));
      parsed.to_string()
    }
    _ => url.to_string(),
  };
  let identity = format!(
    "{url_without_anchor}|{record_type}|{}|{position}",
    anchor.as_deref().unwrap_or("")
  );
  let object_id = Sha256::digest(identity.as_bytes())
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect::<String>();
  let mut fields = HierarchyFields::default();
  let levels = [
    &mut fields.lvl0,
    &mut fields.lvl1,
    &mut fields.lvl2,
    &mut fields.lvl3,
    &mut fields.lvl4,
    &mut fields.lvl5,
    &mut fields.lvl6,
  ];

  for (target, value) in levels.into_iter().zip(hierarchy.iter()) {
    *target = value.clone();
  }

  if let Some(level) = radio_level {
    match level {
      0 => fields.radio_lvl0 = anchor.clone(),
      1 => fields.radio_lvl1 = anchor.clone(),
      2 => fields.radio_lvl2 = anchor.clone(),
      3 => fields.radio_lvl3 = anchor.clone(),
      4 => fields.radio_lvl4 = anchor.clone(),
      5 => fields.radio_lvl5 = anchor.clone(),
      6 => fields.radio_lvl6 = anchor.clone(),
      _ => {}
    }
  }

  SearchRecord {
    anchor,
    content,
    level: get_level_weight(&record_type),
    record_type,
    tags: Vec::new(),
    url: record_url,
    url_without_variables: url.to_string(),
    lang,
    url_without_anchor,
    no_variables: true,
    object_id,
    page_rank,
    position,
    hierarchy: fields,
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::{extract_page, get_level_weight};
  use crate::config::{Config, CrawlConfig, IndexSetting, MeilisearchConfig, SelectorEntry};

  fn example_config() -> Config {
    Config {
      only_urls: Vec::new(),
      start_urls: Vec::new(),
      sitemap_urls: Vec::new(),
      stop_urls: None,
      page_rank: Vec::new(),
      selectors: HashMap::from([
        ("lvl0".to_string(), SelectorEntry::Simple("h1".to_string())),
        ("lvl1".to_string(), SelectorEntry::Simple("h2".to_string())),
        (
          "content".to_string(),
          SelectorEntry::Simple("p".to_string()),
        ),
      ]),
      crawl: CrawlConfig::default(),
      meilisearch: MeilisearchConfig {
        host: "http://127.0.0.1:7700".to_string(),
        api_key: "test_api_key".to_string(),
        index_uid: "test_index".to_string(),
        index_setting: IndexSetting::default(),
        patch_size: 100,
      },
    }
  }

  #[test]
  fn level_weight_matches_docsearch_rules() {
    assert_eq!(get_level_weight("lvl0"), 100);
    assert_eq!(get_level_weight("lvl2"), 80);
    assert_eq!(get_level_weight("lvl6"), 40);
    assert_eq!(get_level_weight("content"), 0);
    assert_eq!(get_level_weight("unknown"), 0);
  }

  #[test]
  fn extracts_example_document_in_dom_order() {
    let html = include_str!("../example/example.html");
    let records = extract_page(
      html,
      "https://example.test/tree.html#old",
      &example_config(),
    )
    .unwrap();

    assert_eq!(records.len(), 10);
    assert_eq!(
      records
        .iter()
        .map(|record| record.position)
        .collect::<Vec<_>>(),
      (0..10).collect::<Vec<_>>()
    );

    assert_eq!(records[0].record_type, "lvl0");
    assert_eq!(records[0].level, 100);
    assert_eq!(records[0].content, None);
    assert_eq!(records[0].hierarchy.lvl0.as_deref(), Some("标题"));
    assert_eq!(records[0].hierarchy.radio_lvl0.as_deref(), Some("标题"));

    assert_eq!(records[1].record_type, "lvl1");
    assert_eq!(records[1].level, 90);
    assert_eq!(records[1].hierarchy.lvl0.as_deref(), Some("标题"));
    assert_eq!(records[1].hierarchy.lvl1.as_deref(), Some("副标题一"));
    assert_eq!(records[1].hierarchy.radio_lvl1.as_deref(), Some("副标题一"));

    assert_eq!(records[2].record_type, "content");
    assert_eq!(records[2].level, 0);
    assert_eq!(records[2].content.as_deref(), Some("段落一"));
    assert_eq!(records[2].hierarchy.lvl1.as_deref(), Some("副标题一"));
    assert_eq!(records[2].hierarchy.radio_lvl1, None);
    assert_eq!(records[2].lang, "en");
    assert_eq!(
      records[2].url_without_anchor,
      "https://example.test/tree.html"
    );
    assert!(records[2].no_variables);
  }

  #[test]
  fn object_id_is_stable_for_same_input() {
    let html = include_str!("../example/example.html");
    let first = extract_page(
      html,
      "https://example.test/tree.html#old",
      &example_config(),
    )
    .unwrap();
    let second = extract_page(
      html,
      "https://example.test/tree.html#old",
      &example_config(),
    )
    .unwrap();

    assert_eq!(first, second);
    assert_eq!(first[0].object_id.len(), 64);
    assert_ne!(first[0].object_id, first[1].object_id);
  }

  #[test]
  fn invalid_css_selector_returns_field_name() {
    let mut config = example_config();
    config.selectors.insert(
      "content".to_string(),
      SelectorEntry::Simple("[".to_string()),
    );

    let error = extract_page("<p>text</p>", "https://example.test/", &config).unwrap_err();
    assert!(error.starts_with("selectors.content: invalid CSS selector:"));
  }
}
