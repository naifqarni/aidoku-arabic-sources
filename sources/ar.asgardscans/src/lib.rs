#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{html::Document, net::Request, std::send_partial_result},
	prelude::*,
};
use serde_json::Value;

const BASE_URL: &str = "https://asgardscans.com";

struct AsgardScans;

fn get(url: &str) -> Result<Document> {
	Ok(Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.html()?)
}

fn strip_base(url: String) -> String {
	url.strip_prefix(BASE_URL).map(String::from).unwrap_or(url)
}

fn get_manga_list(page: i32, mut qs: QueryParameters) -> Result<MangaPageResult> {
	if page > 1 {
		qs.push("page", Some(&page.to_string()));
	}
	let html = get(&format!("{BASE_URL}/manga?{qs}"))?;
	let entries = html
		.select(".grid a.group[href^=\"/manga/\"]")
		.map(|els| {
			els.filter_map(|el| {
				Some(Manga {
					key: strip_base(el.attr("abs:href")?),
					title: el.select_first("h3")?.text()?.trim().into(),
					cover: el.select_first("img").and_then(|img| img.attr("abs:src")),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default();
	Ok(MangaPageResult {
		entries,
		has_next_page: html.select_first("a[rel=next]").is_some(),
	})
}

fn find_series(json: Value) -> Option<Value> {
	if json.get("@type").and_then(Value::as_str) == Some("CreativeWorkSeries") {
		return Some(json);
	}
	match json {
		Value::Object(mut map) => match map.remove("@graph") {
			Some(Value::Array(items)) => items
				.into_iter()
				.find(|it| it.get("@type").and_then(Value::as_str) == Some("CreativeWorkSeries")),
			_ => None,
		},
		_ => None,
	}
}

fn string_list(value: Option<&Value>) -> Vec<String> {
	match value {
		Some(Value::String(s)) if !s.trim().is_empty() => aidoku::alloc::vec![s.trim().into()],
		Some(Value::Array(items)) => items
			.iter()
			.filter_map(Value::as_str)
			.map(|s| String::from(s.trim()))
			.filter(|s| !s.is_empty())
			.collect(),
		_ => Vec::new(),
	}
}

/// Value of a row in the details info table ("الحالة", "النوع", ...).
fn info_value(html: &Document, label: &str) -> Option<String> {
	html.select("span.text-ink-faint")?.find_map(|el| {
		if el.text()?.trim() != label {
			return None;
		}
		el.next()?.text().map(|s| s.trim().into())
	})
}

/// Parses `d/M/yyyy` into a unix timestamp (seconds, UTC).
fn parse_date(s: &str) -> Option<i64> {
	let mut parts = s.trim().split('/').map(|p| p.trim().parse::<i64>().ok());
	let (d, m, y) = (parts.next()??, parts.next()??, parts.next()??);
	let y = if m <= 2 { y - 1 } else { y };
	let era = y.div_euclid(400);
	let yoe = y - era * 400;
	let mp = (m + 9) % 12;
	let doy = (153 * mp + 2) / 5 + d - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	Some((era * 146097 + doe - 719468) * 86400)
}

/// Splits "الفصل 12.5 - title" into (number, title).
fn parse_chapter_name(name: &str) -> (Option<f32>, Option<String>) {
	let rest = name.trim().trim_start_matches("الفصل").trim_start();
	let end = rest
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(rest.len());
	let number = rest[..end].parse::<f32>().ok();
	if number.is_none() {
		return (None, Some(name.trim().into()));
	}
	let title = rest[end..]
		.trim_start_matches(|c: char| c.is_whitespace() || c == '-' || c == ':' || c == '–')
		.trim();
	(number, Some(String::from(title)).filter(|t| !t.is_empty()))
}

impl Source for AsgardScans {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
			qs.push("q", Some(query.trim()));
		}
		for filter in filters {
			if let FilterValue::Select { id, value } = filter
				&& !value.is_empty()
			{
				qs.push(&id, Some(&value));
			}
		}
		get_manga_list(page, qs)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = get(&url)?;

		if needs_details {
			let series = html
				.select("script[type=\"application/ld+json\"]")
				.and_then(|els| {
					els.filter_map(|el| el.data().filter(|d| !d.is_empty()).or_else(|| el.html()))
						.filter(|d| d.contains("\"CreativeWorkSeries\""))
						.find_map(|d| find_series(serde_json::from_str(&d).ok()?))
				});
			if let Some(title) = html.select_first("h1").and_then(|el| el.text()) {
				manga.title = title.trim().into();
			}
			manga.cover = html
				.select_first("meta[property=\"og:image\"]")
				.and_then(|el| el.attr("content"))
				.or(manga.cover);
			if let Some(series) = &series {
				let mut description = string_list(series.get("description")).join("\n");
				let alt = string_list(series.get("alternateName")).join(", ");
				if !alt.is_empty() {
					description.push_str("\n\nأسماء أخرى: ");
					description.push_str(&alt);
				}
				manga.description = Some(description.trim().into());
				let genres = string_list(series.get("genre"));
				if !genres.is_empty() {
					manga.tags = Some(genres);
				}
			}
			if manga.tags.is_none() {
				manga.tags = html.select("a[href^=\"/manga?genres=\"]").map(|els| {
					els.filter_map(|el| el.text().map(|s| s.trim().into()))
						.collect()
				});
			}
			manga.status = match info_value(&html, "الحالة") {
				Some(s) if s.contains("مستمر") => MangaStatus::Ongoing,
				Some(s) if s.contains("مكتمل") => MangaStatus::Completed,
				Some(s) if s.contains("متوقف") => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			};
			manga.viewer = match info_value(&html, "النوع") {
				Some(s) if s.contains("مانجا") => Viewer::RightToLeft,
				Some(s) if s.contains("مانهوا") || s.contains("مانها") => {
					Viewer::Webtoon
				}
				_ => Viewer::Unknown,
			};
			manga.content_rating = ContentRating::Safe;
			manga.url = Some(url.clone());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html.select("ol > li > a[dir=ltr]").map(|els| {
				els.filter_map(|el| {
					// skip chapters behind a paywall
					if el
						.select_first("svg.lucide-lock, svg.lucide-coins")
						.is_some()
					{
						return None;
					}
					let url = el.attr("abs:href")?;
					let name = el.select_first("div[dir=rtl] span")?.text()?;
					let (chapter_number, title) = parse_chapter_name(&name);
					Some(Chapter {
						key: strip_base(url.clone()),
						title,
						chapter_number,
						date_uploaded: el
							.select_first("div[dir=rtl] span.tabular")
							.and_then(|el| el.text())
							.and_then(|s| parse_date(&s)),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = get(&format!("{BASE_URL}{}", chapter.key))?;
		let Some(script) = html.select("script:not([src])").and_then(|els| {
			els.filter_map(|el| el.data().filter(|d| !d.is_empty()).or_else(|| el.html()))
				.find(|d| d.contains("pages:") && d.contains("/api/public/page/"))
		}) else {
			return Ok(Vec::new());
		};
		const MARKER: &str = "imageUrl:\"";
		Ok(script
			.split(MARKER)
			.skip(1)
			.filter_map(|part| part.split('"').next())
			.filter(|path| !path.is_empty())
			.map(|path| Page {
				content: PageContent::url(if path.starts_with("http") {
					path.into()
				} else {
					format!("{BASE_URL}{path}")
				}),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for AsgardScans {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		if listing.id == "popular" {
			qs.push("sort", Some("popular"));
		}
		get_manga_list(page, qs)
	}
}

impl DeepLinkHandler for AsgardScans {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.asgardscans.com"))
		else {
			return Ok(None);
		};
		let segments: Vec<&str> = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.split('/')
			.filter(|s| !s.is_empty())
			.collect();
		match segments.as_slice() {
			["manga", slug] => Ok(Some(DeepLinkResult::Manga {
				key: format!("/manga/{slug}"),
			})),
			["manga", slug, chapter] => Ok(Some(DeepLinkResult::Chapter {
				manga_key: format!("/manga/{slug}"),
				key: format!("/manga/{slug}/{chapter}"),
			})),
			_ => Ok(None),
		}
	}
}

register_source!(AsgardScans, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
