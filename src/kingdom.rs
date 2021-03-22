use chrono::{FixedOffset, TimeZone};
use encoding::{all::WINDOWS_949, DecoderTrap, Encoding};
use once_cell::sync::Lazy;
use reqwest::Response;
use scraper::Selector;
use anyhow::Context;

const URL_PREFIX: &str = "https://cafe.naver.com/crkingdom";
const NOTICE_PAGE: &str =
    "/ArticleList.nhn?search.clubid=30291108&search.menuid=6&search.boardtype=L";

#[derive(Debug)]
pub struct Notice {
    pub number: i64,
    pub title: String,
    pub url: String,
    pub datetime: chrono::DateTime<chrono::Utc>,
}

static NOTICE_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse(".article-board").unwrap());
static NOTICE_ITEM_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse(".td_article").unwrap());
static ARTICLE_NUMBER_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".board-number .inner_number").unwrap());
static ARTICLE_TITLE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".board-list a").unwrap());
static DATE_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse(".date").unwrap());

async fn get_body(resp: Response) -> anyhow::Result<String> {
    let content_type = resp.headers().get("content-type").ok_or_else(|| anyhow::anyhow!("Empty content-type"))?;
    let content_type = content_type.to_str()?.to_ascii_lowercase();
    let body = resp.bytes().await.map_err(|e| anyhow::anyhow!("Failed to fetch html, {}", e))?;

    if content_type.contains("utf-8") {
        Ok(std::str::from_utf8(&body)?.to_string())
    } else if content_type.contains("ms949") {
        Ok(WINDOWS_949
            .decode(&body, DecoderTrap::Strict)
            .map_err(|_| anyhow::anyhow!("Failed to decode html"))?)
    } else {
        Err(anyhow::anyhow!("Unknown  {}", content_type))
    }
}

pub async fn scrap_notice(last_checked_id: Option<i64>) -> anyhow::Result<Vec<Notice>> {
    let html = get_body(reqwest::get(format!("{}{}", URL_PREFIX, NOTICE_PAGE))
        .await?).await?;

    let doc = scraper::Html::parse_document(&html);
    let mut notice_item_selector = doc.select(&NOTICE_SELECTOR);
    if notice_item_selector.next().is_none() {
        return Err(anyhow::anyhow!("Notice selector error - no match"));
    }
    let elem = notice_item_selector
        .next()
        .ok_or_else(|| anyhow::anyhow!("Notice selector error - second item is not found"))?;
    let item_nodes = elem.select(&NOTICE_ITEM_SELECTOR);
    let mut ret = Vec::new();
    let last_checked_id = last_checked_id.unwrap_or(0);
    for (idx, item_node) in item_nodes.enumerate() {
        let number_item = item_node
            .select(&ARTICLE_NUMBER_SELECTOR)
            .next()
            .ok_or_else(|| anyhow::anyhow!("article number cannot found at {}th item", idx))?;
        let article_number: i64 = format!(
            "{}",
            super::util::NodeIterFormatter::new(number_item.children())
        )
        .parse()?;
        if article_number <= last_checked_id {
            break;
        }

        let title_item = item_node
            .select(&ARTICLE_TITLE_SELECTOR)
            .next()
            .ok_or_else(|| anyhow::anyhow!("article number cannot found at {}th item", idx))?;
        let title = format!(
            "{}",
            super::util::NodeIterFormatter::new(title_item.children())
        );
        let href = title_item
            .value()
            .attr("href")
            .ok_or_else(|| anyhow::anyhow!("href attr not found at {}th item", idx))?;
        let url = format!("{}{}", URL_PREFIX, href);

        let html = get_body(reqwest::get(&url)
            .await?).await?;

        let doc = scraper::Html::parse_document(&html);
        let datetime = doc
            .select(&DATE_SELECTOR)
            .next()
            .ok_or_else(|| anyhow::anyhow!("date cannot found at {}th item", idx))?;
        let datetime = format!(
            "{}",
            super::util::NodeIterFormatter::new(datetime.children())
        );
        let datetime =
            FixedOffset::east(3600 * 9).datetime_from_str(&datetime, "%Y.%m.%d. %H:%M").context(datetime)?;

        ret.push(Notice {
            number: article_number,
            title: title.trim().to_string(),
            url,
            datetime: datetime.into(),
        })
    }

    Ok(ret)
}
