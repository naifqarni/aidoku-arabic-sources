#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};
use serde::Deserialize;

const BASE_URL: &str = "https://eshadow.net";

struct Eshadow;

#[derive(Deserialize)]
struct MangaList {
	data: Vec<MangaDto>,
	limit: i32,
	page: i32,
	total: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MangaDto {
	id: String,
	slug: String,
	title: String,
	description: Option<String>,
	cover_image: Option<String>,
	author: Option<String>,
	status: Option<String>,
	#[serde(default)]
	genres: Vec<serde_json::Value>,
	#[serde(default)]
	chapters: Vec<ChapterDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterDto {
	// missing in list responses
	id: Option<String>,
	title: Option<String>,
	number: f32,
	published_at: Option<String>,
	#[serde(default)]
	images: Vec<String>,
}

#[derive(Deserialize)]
struct ChapterResponse {
	chapter: ChapterDto,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterMangaId {
	manga_id: String,
}

#[derive(Deserialize)]
struct ChapterMangaResponse {
	chapter: ChapterMangaId,
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.json_owned()
}

/// Parses an ISO 8601 UTC timestamp like `2026-06-08T13:10:13.403Z` into unix seconds.
fn parse_iso_date(date: &str) -> Option<i64> {
	let num = |s: Option<&str>| s?.parse::<i64>().ok();
	let (date, time) = date.split_once('T')?;
	let mut d = date.split('-');
	let (y, m, day) = (num(d.next())?, num(d.next())?, num(d.next())?);
	let mut t = time.trim_end_matches('Z').split(':');
	let (h, min) = (num(t.next())?, num(t.next())?);
	let sec = num(t.next()?.split('.').next())?;
	// days from civil (Howard Hinnant)
	let y = if m <= 2 { y - 1 } else { y };
	let era = y.div_euclid(400);
	let yoe = y - era * 400;
	let mp = (m + 9) % 12;
	let doy = (153 * mp + 2) / 5 + day - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	let days = era * 146097 + doe - 719468;
	Some(days * 86400 + h * 3600 + min * 60 + sec)
}

/// Drops empty values and the dot-only placeholders the site uses.
fn valid(s: Option<String>) -> Option<String> {
	s.map(|s| s.trim().into())
		.filter(|s: &String| !s.chars().all(|c| c == '.' || c == '…'))
}

impl MangaDto {
	fn into_manga(self) -> Manga {
		let tags = self
			.genres
			.into_iter()
			.filter_map(|g| match g {
				serde_json::Value::String(s) => Some(s),
				serde_json::Value::Object(o) => o
					.get("name")
					.or_else(|| o.get("genre").and_then(|g| g.get("name")))
					.and_then(|n| n.as_str())
					.map(String::from),
				_ => None,
			})
			.collect::<Vec<_>>();
		Manga {
			key: self.id,
			title: self.title,
			cover: self.cover_image,
			description: valid(self.description),
			authors: valid(self.author).map(|a| vec![a]),
			tags: (!tags.is_empty()).then_some(tags),
			status: match self.status.as_deref() {
				Some("ONGOING") => MangaStatus::Ongoing,
				Some("COMPLETED") => MangaStatus::Completed,
				Some("HIATUS") => MangaStatus::Hiatus,
				Some("CANCELLED") | Some("DROPPED") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			url: Some(format!("{BASE_URL}/manga/{}", self.slug)),
			content_rating: ContentRating::Safe,
			viewer: Viewer::RightToLeft,
			..Default::default()
		}
	}
}

impl ChapterDto {
	fn into_chapter(self) -> Option<Chapter> {
		let id = self.id?;
		let title = self.title.map(|t| t.trim().into()).filter(|t: &String| {
			// the site often fills the title with "Chapter"/"الفصل" and a number
			let rest = t
				.trim_start_matches("Chapter")
				.trim_start_matches("الفصل")
				.trim();
			!rest.is_empty() && rest.parse::<f32>().is_err()
		});
		Some(Chapter {
			url: Some(format!("{BASE_URL}/read/{id}")),
			key: id,
			title,
			chapter_number: Some(self.number),
			date_uploaded: self.published_at.as_deref().and_then(parse_iso_date),
			..Default::default()
		})
	}
}

impl Source for Eshadow {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		if let Some(query) = query.filter(|q| !q.is_empty()) {
			qs.push("query", Some(&query));
		}
		let data: MangaList = get_json(&format!("{BASE_URL}/api/manga?{qs}"))?;
		Ok(MangaPageResult {
			has_next_page: data.page * data.limit < data.total,
			entries: data.data.into_iter().map(|m| m.into_manga()).collect(),
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut data: MangaDto = get_json(&format!("{BASE_URL}/api/manga/{}", manga.key))?;
		let chapters = core::mem::take(&mut data.chapters);

		if needs_details {
			manga.copy_from(data.into_manga());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = chapters
				.into_iter()
				.filter_map(ChapterDto::into_chapter)
				.collect();
			chapters.sort_by(|a, b| {
				b.chapter_number
					.unwrap_or_default()
					.total_cmp(&a.chapter_number.unwrap_or_default())
			});
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let data: ChapterResponse = get_json(&format!("{BASE_URL}/api/chapters/{}", chapter.key))?;
		Ok(data
			.chapter
			.images
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl DeepLinkHandler for Eshadow {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let mut segments = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.split('/')
			.skip(1);
		match (segments.next(), segments.next()) {
			(Some("manga"), Some(slug)) if !slug.is_empty() => {
				let data: MangaDto = get_json(&format!("{BASE_URL}/api/manga/slug/{slug}"))?;
				Ok(Some(DeepLinkResult::Manga { key: data.id }))
			}
			(Some("read"), Some(id)) if !id.is_empty() => {
				let data: ChapterMangaResponse =
					get_json(&format!("{BASE_URL}/api/chapters/{id}"))?;
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: data.chapter.manga_id,
					key: id.into(),
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(Eshadow, DeepLinkHandler);

#[cfg(test)]
mod test;
