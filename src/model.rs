use serde::Serialize;

/// 单条 DocSearch 风格搜索记录；一个页面会产生多条标题或正文记录。
#[derive(Debug, Serialize, PartialEq)]
pub struct SearchRecord {
  pub anchor: Option<String>,
  pub content: Option<String>,
  #[serde(rename = "type")]
  pub record_type: String,
  pub tags: Vec<String>,
  pub url: String,
  pub url_without_variables: String,
  pub lang: String,
  pub url_without_anchor: String,
  pub no_variables: bool,
  #[serde(rename = "objectID")]
  pub object_id: String,
  pub page_rank: i32,
  pub level: i32,
  pub position: usize,
  #[serde(flatten)]
  pub hierarchy: HierarchyFields,
}

/// 当前记录所在的标题路径，以及用于导航的 radio 层级字段。
#[derive(Debug, Serialize, PartialEq, Default)]
pub struct HierarchyFields {
  #[serde(rename = "hierarchy_lvl0")]
  pub lvl0: Option<String>,
  #[serde(rename = "hierarchy_lvl1")]
  pub lvl1: Option<String>,
  #[serde(rename = "hierarchy_lvl2")]
  pub lvl2: Option<String>,
  #[serde(rename = "hierarchy_lvl3")]
  pub lvl3: Option<String>,
  #[serde(rename = "hierarchy_lvl4")]
  pub lvl4: Option<String>,
  #[serde(rename = "hierarchy_lvl5")]
  pub lvl5: Option<String>,
  #[serde(rename = "hierarchy_lvl6")]
  pub lvl6: Option<String>,
  #[serde(rename = "hierarchy_radio_lvl0")]
  pub radio_lvl0: Option<String>,
  #[serde(rename = "hierarchy_radio_lvl1")]
  pub radio_lvl1: Option<String>,
  #[serde(rename = "hierarchy_radio_lvl2")]
  pub radio_lvl2: Option<String>,
  #[serde(rename = "hierarchy_radio_lvl3")]
  pub radio_lvl3: Option<String>,
  #[serde(rename = "hierarchy_radio_lvl4")]
  pub radio_lvl4: Option<String>,
  #[serde(rename = "hierarchy_radio_lvl5")]
  pub radio_lvl5: Option<String>,
  #[serde(rename = "hierarchy_radio_lvl6")]
  pub radio_lvl6: Option<String>,
}
