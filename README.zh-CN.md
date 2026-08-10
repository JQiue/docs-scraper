# docs-scraper

![GitHub Release](https://img.shields.io/github/v/release/JQiue/docs-scraper)
![GitHub Issues or Pull Requests](https://img.shields.io/github/issues/JQiue/docs-scraper)
![GitHub commit activity](https://img.shields.io/github/commit-activity/t/JQiue/docs-scraper)
![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/JQiue/docs-scraper/total)
![GitHub License](https://img.shields.io/github/license/JQiue/docs-scraper)
![Code Lines](https://img.shields.io/endpoint?url=https://ghloc.vercel.app/api/JQiue/docs-scraper/badge?filter=.rs$)
![Docker Image Size](https://img.shields.io/docker/image-size/jqiue/docs-scraper)
![Docker Image Version](https://img.shields.io/docker/v/jqiue/docs-scraper?label=docker)

[English](./README.md) | 简体中文

> 基于 Rust 的结构化文档爬虫，面向文档站点和 Meilisearch。

## 功能

- 递归发现链接和 Sitemap
- 受控并发
- 标题层级抽取
- 支持全量重建和增量更新
- 页面 `page_rank` 规则
- Meilisearch 批量写入

## 用法

### 从可执行文件中运行

默认读取根目录的 `config.json`：

```sh
export MEILISEARCH_API_KEY=your_meilisearch_api_key

./docs-scraper
```

### Docker

```sh
docker run -t --rm \
    -e MEILISEARCH_API_KEY=<your-meilisearch-api-key> \
    -v <absolute-path-to-your-config-file>:/app/config.json:ro \
    jqiue/docs-scraper:1
```

### Docker Compose

```yml
services:
  docs-scraper:
    image: jqiue/docs-scraper:1
    container_name: docs-scraper
    environment:
      - MEILISEARCH_API_KEY=${MEILISEARCH_API_KEY}
    volumes:
      - ${PWD}/config.json:/app/config.json:ro
```

## 配置

### `crawl`

- `max_pages` 是最多尝试访问的页面数。
- `concurrency` 是并发请求数。
- `timeout_seconds` 是请求超时。

```json
{
 "crawl": {
  "concurrency": 8,
  "max_pages": 1000,
  "timeout_seconds": 60
 }
}
```

### `start_urls`

如果指定 `start_urls`，抓取器会从这些 URL 开始，并继续发现同域名页面。普通模式下，`start_urls` 会与 `sitemap_urls` 中解析出的 URL 合并。

```json
{
  "start_urls": ["https://example.com/docs/"]
}
```

全量写入 Meilisearch 前会删除索引中的旧文档，然后分批导入本次结果。

### `sitemap_urls`

```json
{
  "sitemap_urls": ["https://example.com/sitemap.xml"]
}
```

### `only_urls`

当 `only_urls` 非空时，`start_urls` 和 `sitemap_urls` 都不会生效。抓取器将将严格按照指定 url 进行增量抓取，写入 Meilisearch 前，会按以下条件删除这些页面的所有旧记录：

```text
url_without_anchor = 页面 URL
```

然后再导入新记录，因此页面内容块减少时不会留下旧块。

```json
{
 "only_urls": [
  "https://example.com/docs/intro.html",
  "https://example.com/docs/api.html"
 ]
}
```

### `stop_urls`

```json
{
  "stop_urls": []
}
```

### `selectors`

`global: true` 的 Selector 每页只执行一次。找到有效文本时使用实际文本；找不到时使用 `default_value`；之后正文记录会继承该层级。

```json
{
 "selectors": {
  "lvl0": {
   "selector": ".vp-page-title h1",
   "global": true,
   "default_value": "文档"
  },
  "lvl1": "[vp-content] h1",
  "lvl2": "[vp-content] h2",
  "lvl3": "[vp-content] h3",
  "lvl4": "[vp-content] h4",
  "lvl5": "[vp-content] h5",
  "lvl6": "[vp-content] h6",
  "content": "[vp-content] p, [vp-content] li"
 }
}
```

### `page_rank`

没有匹配规则时默认是 `0`，不需要额外配置默认值：

```json
{
 "page_rank": [
  {"pattern": "https://example.com/docs/**", "rank": 80},
  {"pattern": "https://example.com/about.html", "rank": 10}
 ]
}
```

以 `**` 结尾按 URL 前缀匹配，否则按完整 URL 匹配；后匹配的规则覆盖前面的规则。

### `meilisearch`

典型配置：

```json
{
 "meilisearch": {
  "patch_size": 500,
  "host": "https://search.example.com",
  "index_uid": "docs",
  "index_setting": {
   "filterable_attributes": ["lang", "url_without_anchor"],
   "sortable_attributes": [],
   "ranking_rules": [
    "words", "typo", "attribute", "proximity", "exactness",
    "page_rank:desc", "level:desc", "position:asc"
   ]
  }
 }
}
```

抓取器使用 `objectID` 作为主键，增量删除依赖 `url_without_anchor`，因此该字段必须配置为 `filterable_attributes`。

## 数据字段说明

这里解释推送到 MeiliSearch 文档的字段：

| 字段                 | 含义                                              |
| -------------------- | ------------------------------------------------- |
| objectID             | 用于 Meilisearch 主键的稳定哈希，使用 sha256 算法 |
| type                 | lvl0 到 lvl6 或 content                           |
| content              | 正文内容，标题记录通常为 `null`                   |
| anchor               | HTML 元素的 id 或当前层级文本                     |
| url                  | 可能包含 anchor 的记录 URL                        |
| url_without_anchor   | 去掉 fragment 的页面 URL                          |
| level                | 搜索排序权重                                      |
| position             | 页面内记录的 DOM 顺序                             |
| page_rank            | 页面级排序权重                                    |
| hierarchy_lvlN       | 当前记录的标题层级                                |
| hierarchy_radio_lvlN | 当前标题的导航层级值                              |

## 注意事项

- 全量模式会清空当前索引，运行前确认 `index_uid` 正确。
- 增量模式只删除 `only_urls` 对应页面的旧记录。
- Sitemap 中的 404 或非 HTML URL 会被跳过，不会中止其他页面抓取。
