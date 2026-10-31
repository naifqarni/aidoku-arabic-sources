#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element, Html},
		net::{Request, TimeUnit, set_rate_limit},
		std::send_partial_result,
	},
	prelude::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::Value;

const BASE_URL: &str = "https://mangadar.com";

struct MangaDar;

#[derive(Deserialize)]
struct SearchResponse {
	#[serde(default)]
	data: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
	title: String,
	url: String,
	cover: Option<String>,
}

fn get(url: &str) -> Result<Document> {
	Ok(Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.html()?)
}

/// List pages wrap every card in `<template x-if>` (rendered by Alpine.js); spec-compliant parsers
/// keep template contents out of the tree, so unwrap them before parsing.
fn get_unwrapped(url: &str) -> Result<Document> {
	let body = Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.string()?;
	let mut out = String::with_capacity(body.len());
	let mut rest = body.as_str();
	while let Some(start) = rest.find("<template") {
		out.push_str(&rest[..start]);
		rest = &rest[start..];
		let end = rest.find('>').map(|i| i + 1).unwrap_or(rest.len());
		rest = &rest[end..];
	}
	out.push_str(rest);
	let out = out.replace("</template>", "");
	Ok(Html::parse_with_url(out, url)?)
}

/// Returns the slug of a `/manga/{slug}/` url.
fn manga_slug(url: &str) -> Option<&str> {
	let path = url.strip_prefix(BASE_URL).unwrap_or(url);
	let mut segments = path
		.split(['?', '#'])
		.next()?
		.split('/')
		.filter(|s| !s.is_empty());
	match (segments.next(), segments.next(), segments.next()) {
		(Some("manga"), Some(slug), None) if slug != "page" => Some(slug),
		_ => None,
	}
}

fn parse_manga_list(html: &Document) -> MangaPageResult {
	let mut entries: Vec<Manga> = Vec::new();
	if let Some(links) = html.select("a[href*=\"/manga/\"]") {
		for link in links {
			let Some(href) = link.attr("abs:href") else {
				continue;
			};
			let Some(slug) = manga_slug(&href) else {
				continue;
			};
			if entries.iter().any(|m| m.key == slug) {
				continue;
			}
			let img = link.select_first("img");
			let title = img
				.as_ref()
				.and_then(|img| img.attr("alt"))
				.filter(|s| !s.trim().is_empty())
				.or_else(|| link.select_first("h3, h4").and_then(|el| el.text()));
			let Some(title) = title.map(|s| String::from(s.trim())) else {
				continue;
			};
			let cover = img.and_then(|img| {
				img.attr("abs:src")
					.filter(|s| !s.is_empty() && !s.starts_with("data:"))
					.or_else(|| img.attr("abs:data-src"))
			});
			entries.push(Manga {
				key: slug.into(),
				title,
				cover,
				url: Some(href),
				..Default::default()
			});
		}
	}
	MangaPageResult {
		entries,
		has_next_page: html
			.select_first("link[rel=next], a[rel=next], a.next")
			.is_some(),
	}
}

fn list_url(page: i32, params: &QueryParameters) -> String {
	let page = if page > 1 {
		format!("page/{page}/")
	} else {
		String::new()
	};
	let params = params.to_string();
	if params.is_empty() {
		format!("{BASE_URL}/manga/{page}")
	} else {
		format!("{BASE_URL}/manga/{page}?{params}")
	}
}

/// Finds the value of an info row (`<div><span>label</span> value</div>`) in the sidebar.
fn info_value(html: &Document, label: &str) -> Option<String> {
	html.select("aside div")?
		.filter_map(|row| {
			let name = row.select_first("span")?.text()?;
			if name.trim() != label {
				return None;
			}
			let text = row.text()?;
			Some(String::from(text.trim().strip_prefix(label)?.trim()))
		})
		// nested containers also match; the innermost row has the shortest text
		.min_by_key(|s| s.len())
		.filter(|s| !s.is_empty())
}

fn script_text(el: &Element) -> Option<String> {
	el.data()
		.filter(|s| !s.trim().is_empty())
		.or_else(|| el.html())
}

fn ld_json(html: &Document) -> Option<Value> {
	html.select("script[type=\"application/ld+json\"]")?
		.filter_map(|el| serde_json::from_str::<Value>(&script_text(&el)?).ok())
		.find(|v| v.get("@type").and_then(|t| t.as_str()) == Some("ComicSeries"))
}

fn parse_chapters(html: &Document) -> Vec<Chapter> {
	// rows: [[id, "number", url, unix timestamp, number], ...]
	let Some(data) = html
		.select_first("script#mdg-manga-chapters-json")
		.and_then(|el| script_text(&el))
		.and_then(|data| serde_json::from_str::<Value>(&data).ok())
	else {
		return Vec::new();
	};
	data.get("rows")
		.and_then(|rows| rows.as_array())
		.map(|rows| {
			rows.iter()
				.filter_map(|row| {
					let row = row.as_array()?;
					let number = match row.get(1)? {
						Value::String(s) => s.clone(),
						other => other.to_string(),
					};
					let url = row.get(2)?.as_str()?;
					let key = url.strip_prefix(BASE_URL).unwrap_or(url);
					Some(Chapter {
						key: key.into(),
						chapter_number: number.trim().parse().ok(),
						title: number
							.trim()
							.parse::<f32>()
							.is_err()
							.then(|| number.clone()),
						date_uploaded: row.get(3).and_then(|t| t.as_i64()).filter(|t| *t > 0),
						url: Some(url.into()),
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

impl Source for MangaDar {
	fn new() -> Self {
		set_rate_limit(2, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
			// the search endpoint isn't paginated
			if page > 1 {
				return Ok(MangaPageResult::default());
			}
			let mut qs = QueryParameters::new();
			qs.push("action", Some("mangaverse_search"));
			qs.push("q", Some(&query));
			let data: SearchResponse =
				Request::get(format!("{BASE_URL}/wp-admin/admin-ajax.php?{qs}"))?
					.header("Referer", &format!("{BASE_URL}/"))
					.json_owned()?;
			return Ok(MangaPageResult {
				entries: data
					.data
					.into_iter()
					.filter_map(|item| {
						Some(Manga {
							key: manga_slug(&item.url)?.into(),
							title: item.title,
							cover: item.cover,
							url: Some(item.url),
							..Default::default()
						})
					})
					.collect(),
				has_next_page: false,
			});
		}

		let mut qs = QueryParameters::new();
		for filter in filters {
			if let FilterValue::Select { id, value } = filter
				&& !value.is_empty()
			{
				qs.push(&id, Some(&value));
			}
		}
		Ok(parse_manga_list(&get_unwrapped(&list_url(page, &qs))?))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}/manga/{}/", manga.key);
		let html = get(&url)?;

		if needs_details {
			let ld = ld_json(&html);
			let ld_str = |key: &str| -> Option<String> {
				ld.as_ref()?
					.get(key)?
					.as_str()
					.map(|s| String::from(s.trim()))
					.filter(|s| !s.is_empty())
			};

			if let Some(title) = html
				.select_first("main h1")
				.and_then(|el| el.text())
				.map(|s| String::from(s.trim()))
				.filter(|s| !s.is_empty())
				.or_else(|| ld_str("name"))
			{
				manga.title = title;
			}
			manga.cover = html
				.select_first("main picture img")
				.and_then(|img| img.attr("abs:src"))
				.filter(|s| !s.starts_with("data:"))
				.or_else(|| ld_str("image"))
				.or(manga.cover);
			manga.description = ld_str("description").or_else(|| {
				html.select_first("meta[name=description]")
					.and_then(|el| el.attr("content"))
			});
			let tags: Vec<String> = ld
				.as_ref()
				.and_then(|v| v.get("genre"))
				.and_then(|g| g.as_array())
				.map(|g| {
					g.iter()
						.filter_map(|s| s.as_str().map(|s| String::from(s.trim())))
						.collect()
				})
				.unwrap_or_default();
			manga.authors = info_value(&html, "المؤلف")
				.or_else(|| {
					ld.as_ref()?
						.get("author")?
						.get("name")?
						.as_str()
						.map(String::from)
				})
				.map(|a| vec![a]);
			manga.artists = info_value(&html, "الرسام").map(|a| vec![a]);
			let status = info_value(&html, "الحالة").unwrap_or_default();
			manga.status = if status.contains("مستمر") {
				MangaStatus::Ongoing
			} else if status.contains("مكتمل") {
				MangaStatus::Completed
			} else if status.contains("متوقف") {
				MangaStatus::Hiatus
			} else if status.contains("ملغ") || status.contains("متروك") {
				MangaStatus::Cancelled
			} else {
				MangaStatus::Unknown
			};
			// type badge above the title: مانغا / مانهوا / مانهوا صيني
			let kind = html
				.select("main span")
				.and_then(|spans| {
					spans
						.filter_map(|el| el.text())
						.map(|s| String::from(s.trim()))
						.find(|s| s == "مانغا" || s == "مانهوا" || s == "مانهوا صيني")
				})
				.unwrap_or_default();
			manga.viewer = if tags.iter().any(|t| t == "ويبتون") || kind.starts_with("مانهوا")
			{
				Viewer::Webtoon
			} else if kind == "مانغا" {
				Viewer::RightToLeft
			} else {
				Viewer::Unknown
			};
			manga.tags = Some(tags);
			manga.content_rating = ContentRating::Safe;
			manga.url = Some(url.clone());

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(parse_chapters(&html));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = get(&format!("{BASE_URL}{}", chapter.key))?;
		Ok(html
			.select(".reader-page img")
			.map(|imgs| {
				imgs.filter_map(|img| {
					// the real url is base64 encoded; src only holds a placeholder
					let url = img
						.attr("data-mds")
						.and_then(|s| STANDARD.decode(s.trim()).ok())
						.and_then(|bytes| String::from_utf8(bytes).ok())
						.or_else(|| img.attr("abs:data-src").filter(|s| !s.is_empty()))
						.or_else(|| img.attr("abs:src"))
						.filter(|s| s.starts_with("http"))?;
					Some(Page {
						content: PageContent::url(url),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}
}

impl ListingProvider for MangaDar {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push(
			"sort",
			Some(if listing.id == "popular" {
				"popular"
			} else {
				"new"
			}),
		);
		Ok(parse_manga_list(&get_unwrapped(&list_url(page, &qs))?))
	}
}

impl DeepLinkHandler for MangaDar {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let mut segments = path.split('/').filter(|s| !s.is_empty());
		match (segments.next(), segments.next(), segments.next()) {
			(Some("manga"), Some(slug), None) if slug != "page" => {
				Ok(Some(DeepLinkResult::Manga { key: slug.into() }))
			}
			(Some("manga"), Some(slug), Some(chapter)) if slug != "page" => {
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: slug.into(),
					key: format!("/manga/{slug}/{chapter}/"),
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(MangaDar, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
