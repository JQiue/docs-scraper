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

### 运行之前

1. 确认 MeiliSearch 服务是否启动
2. 用于抓取器的 api_key 是否拥有以下权限：`indexes.get, indexes.create, settings.update, documents.add, documents.delete, tasks.get`，建议使用最小权限，避免使用 master key
3. 创建`config.json`，配置如下：

```json
{
  "start_urls": [
    "https://example.com"
  ],
  "sitemap_urls": [
    "https://example.com/sitemap.xml"
  ],
  "selectors": {
    "lvl0": {
      "selector": ".vp-page-title h1",
      "global": true,
      "default_value": "文档"
    },
    "lvl1": "[vp-content] h2",
    "lvl2": "[vp-content] h3",
    "lvl3": "[vp-content] h4",
    "lvl4": "[vp-content] h5",
    "lvl5": "[vp-content] h6",
    "content": "[vp-content] p, [vp-content] li"
  },
  "meilisearch": {
    "host": "https://search.example.com",
    "index_uid": "index-name"
  }
}
```

### 从可执行文件中运行

在运行命令的当前目录下放置`config.json`，并通过环境变量传入 API key（会覆盖配置文件中的 `meilisearch.api_key` 字段）：

```sh
export MEILISEARCH_API_KEY=your_meilisearch_api_key

./docs-scraper
```

### Docker

```sh
docker run -t --rm \
    -e MEILISEARCH_API_KEY=<your-meilisearch-api-key> \
    -v <absolute-path-to-your-config-file>:/config.json:ro \
    jqiue/docs-scraper:next
```

### Docker Compose

```yml
services:
  docs-scraper:
    image: jqiue/docs-scraper:next
    container_name: docs-scraper
    environment:
      - MEILISEARCH_API_KEY=${MEILISEARCH_API_KEY}
    volumes:
      - ${PWD}/config.json:/config.json:ro
```

## 配置

### 字段总览

| 字段                        | 必填 | 默认值   | 说明                                                                        |
| --------------------------- | ---- | -------- | --------------------------------------------------------------------------- |
| `meilisearch.host`          | ✅    | -        | MeiliSearch 服务地址                                                        |
| `meilisearch.index_uid`     | ✅    | -        | 索引名称                                                                    |
| `meilisearch.api_key`       | ❌    | 空       | 会被 `MEILISEARCH_API_KEY` 环境变量覆盖，推荐用环境变量                     |
| `meilisearch.patch_size`    | ❌    | `500`    | 每批导入的记录数                                                            |
| `meilisearch.index_setting` | ❌    | 内置默认 | 索引设置，见下文                                                            |
| `start_urls`                | ⚠️    | `[]`     | 起始页面，与 `sitemap_urls`、`only_urls` 至少配置其一，否则不会抓取任何页面 |
| `sitemap_urls`              | ⚠️    | `[]`     | Sitemap 地址，同上                                                          |
| `only_urls`                 | ⚠️    | `[]`     | 增量白名单，配置后 `start_urls`、`sitemap_urls`、`stop_urls` 均不生效       |
| `stop_urls`                 | ❌    | `[]`     | 精确匹配跳过的 URL                                                          |
| `selectors`                 | ⚠️    | `{}`     | 抽取规则，至少配置 `content`，否则无法抽取正文                              |
| `page_rank`                 | ❌    | `[]`     | 页面排序权重，未匹配时默认 `0`                                              |
| `crawl`                     | ❌    | 内置默认 | 并发、上限、超时、UA，见下文                                                |

> ✅ = 必须配置；⚠️ = 可省略但功能上需要；❌ = 可选，省略时使用默认值。

### `crawl`

- `concurrency` 并发请求数，默认 `4`。
- `max_pages` 最多尝试访问的页面数，默认 `1000`。
- `timeout_seconds` 请求超时秒数，默认 `60`。
- `user_agent` 自定义请求 UA，不设置时默认 `docs-scraper`。

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

### `sitemap_urls`

```json
{
  "sitemap_urls": ["https://example.com/sitemap.xml"]
}
```

### `only_urls`

当 `only_urls` 非空时，`start_urls` 和 `sitemap_urls` 都不会生效。抓取器将严格按照指定 url 进行增量抓取，不扩展页面链接（`stop_urls` 同样不生效）。写入 Meilisearch 前，会按以下条件删除这些页面的所有旧记录：

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

与待抓取 URL 完全相等时跳过该页面（忽略 fragment 差异）

```json
{
  "stop_urls": []
}
```

### `selectors`

每个 Selector 的 `selector` 可以是纯字符串，也可以是带 `global` 和 `default_value` 的对象。`global: true` 的 Selector 每页只执行一次：找到有效文本时使用实际文本，找不到时使用 `default_value`；之后正文记录会继承该层级。

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

没有匹配规则时默认为 `0`：

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

- `displayed_attributes` 控制搜索结果返回的字段，默认包含 `hierarchy_*`、`content`、`url`、`anchor`、`lang`、`objectID`、`page_rank`、`level`、`position` 等全部字段。
- `searchable_attributes` 控制参与搜索的字段，默认包含 `hierarchy_*` 与 `content`。
- `filterable_attributes` 默认包含 `lang` 与 `url_without_anchor`。**增量删除依赖 `url_without_anchor`，因此该字段不能从过滤属性中移除。**
- `sortable_attributes` 默认为空。
- `ranking_rules` 默认按 `words → typo → attribute → proximity → exactness → page_rank:desc → level:desc → position:asc` 排序。

以上字段均可省略，省略时使用默认值。

## 数据字段说明

推送到 MeiliSearch 文档的字段：

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
