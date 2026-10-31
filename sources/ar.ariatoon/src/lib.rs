#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};
use serde::{Deserialize, de::DeserializeOwned};

const BASE_URL: &str = "https://ariatoon.com";
const API_URL: &str = "https://api.ariatoon.com/v1";
const CDN_URL: &str = "https://api.ariatoon.com/uploads";
const PAGE_SIZE: usize = 20;
const CHAPTER_PAGE_SIZE: usize = 100;

struct AriaToon;

#[derive(Deserialize)]
struct Paginated<T> {
	data: Option<Vec<T>>,
}

#[derive(Deserialize)]
struct Item<T> {
	data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MangaDto {
	id: String,
	title: String,
	cover_path: Option<String>,
	author: Option<String>,
	summary: Option<String>,
	status: Option<String>,
	announce: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterDto {
	id: String,
	#[serde(rename = "mangaID")]
	manga_id: String,
	created_at: Option<String>,
	title: Option<String>,
	number: Option<f32>,
	is_free: Option<bool>,
}

#[derive(Deserialize)]
struct EpisodeDetails {
	#[serde(default)]
	images: Vec<String>,
}

fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("Accept", "application/json")
		.header("Referer", &format!("{BASE_URL}/"))
		.json_owned()
}

fn non_blank(s: Option<String>) -> Option<String> {
	s.map(|s| s.trim().into())
		.filter(|s: &String| !s.is_empty())
}

impl MangaDto {
	fn into_manga(self) -> Manga {
		let mut description = non_blank(self.summary).unwrap_or_default();
		if let Some(announce) = non_blank(self.announce) {
			if !description.is_empty() {
				description.push_str("\n\n");
			}
			description.push_str("إعلان:\n");
			description.push_str(&announce);
		}
		Manga {
			url: Some(format!("{BASE_URL}/series/manga/{}", self.id)),
			key: self.id,
			title: self.title.trim().into(),
			cover: self.cover_path.map(|p| format!("{CDN_URL}/{p}")),
			authors: non_blank(self.author).map(|a| aidoku::alloc::vec![a]),
			description: Some(description).filter(|d| !d.is_empty()),
			status: match self.status.as_deref() {
				Some("ongoing") => MangaStatus::Ongoing,
				Some("completed") => MangaStatus::Completed,
				Some("hiatus") => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			},
			content_rating: ContentRating::Safe,
			viewer: Viewer::Webtoon,
			..Default::default()
		}
	}
}

/// Parses `yyyy-MM-ddTHH:mm:ss[.fraction]Z` into a unix timestamp (seconds, UTC).
fn parse_iso_date(s: &str) -> Option<i64> {
	let num = |r: core::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
	let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
	let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
	// days from civil (Howard Hinnant)
	let y = if m <= 2 { y - 1 } else { y };
	let era = y.div_euclid(400);
	let yoe = y - era * 400;
	let mp = (m + 9) % 12;
	let doy = (153 * mp + 2) / 5 + d - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	let days = era * 146097 + doe - 719468;
	Some(days * 86400 + hh * 3600 + mm * 60 + ss)
}

impl ChapterDto {
	fn into_chapter(self) -> Chapter {
		let key = format!("{}/episodes/{}", self.manga_id, self.id);
		Chapter {
			url: Some(format!("{BASE_URL}/series/manga/{key}")),
			key,
			title: non_blank(self.title),
			chapter_number: self.number,
			date_uploaded: self.created_at.as_deref().and_then(parse_iso_date),
			..Default::default()
		}
	}
}

fn manga_page(url: &str) -> Result<MangaPageResult> {
	let data: Paginated<MangaDto> = get_json(url)?;
	let entries: Vec<Manga> = data
		.data
		.unwrap_or_default()
		.into_iter()
		.map(MangaDto::into_manga)
		.collect();
	Ok(MangaPageResult {
		has_next_page: entries.len() == PAGE_SIZE,
		entries,
	})
}

impl Source for AriaToon {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// the API has separate endpoints for text search and genre filtering
		if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
			let mut qs = QueryParameters::new();
			qs.push("page", Some(&page.to_string()));
			qs.push("limit", Some(&PAGE_SIZE.to_string()));
			qs.push("search", Some(query.trim()));
			return manga_page(&format!("{API_URL}/mangas/search?{qs}"));
		}
		let genre = filters.into_iter().find_map(|f| match f {
			FilterValue::Select { id, value } if id == "genre" && !value.is_empty() => Some(value),
			_ => None,
		});
		match genre {
			Some(genre) => manga_page(&format!(
				"{API_URL}/mangas/filters/{genre}?page={page}&limit={PAGE_SIZE}&language=ar"
			)),
			None => manga_page(&format!("{API_URL}/mangas?page={page}&limit={PAGE_SIZE}")),
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let details: Item<MangaDto> = get_json(&format!("{API_URL}/mangas/{}", manga.key))?;
			manga.copy_from(details.data.into_manga());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters = Vec::new();
			let mut page = 1;
			loop {
				let data: Paginated<ChapterDto> = get_json(&format!(
					"{API_URL}/mangas/{}/episodes?direction=desc&publishStatus=published&limit={CHAPTER_PAGE_SIZE}&page={page}",
					manga.key
				))?;
				let batch = data.data.unwrap_or_default();
				let len = batch.len();
				// paid episodes return 401 without a purchase
				chapters.extend(
					batch
						.into_iter()
						.filter(|c| c.is_free != Some(false))
						.map(ChapterDto::into_chapter),
				);
				if len < CHAPTER_PAGE_SIZE {
					break;
				}
				page += 1;
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let details: Item<EpisodeDetails> = get_json(&format!("{API_URL}/mangas/{}", chapter.key))?;
		Ok(details
			.data
			.images
			.into_iter()
			.map(|path| Page {
				content: PageContent::url(format!("{CDN_URL}/{path}")),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for AriaToon {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "popular" {
			manga_page(&format!(
				"{API_URL}/feed/mangas/popular?page={page}&limit={PAGE_SIZE}"
			))
		} else {
			manga_page(&format!("{API_URL}/mangas?page={page}&limit={PAGE_SIZE}"))
		}
	}
}

impl DeepLinkHandler for AriaToon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.ariatoon.com"))
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
			["series", "manga", id] if *id != "tags" => {
				Ok(Some(DeepLinkResult::Manga { key: (*id).into() }))
			}
			["series", "manga", id, "episodes", episode] => Ok(Some(DeepLinkResult::Chapter {
				manga_key: (*id).into(),
				key: format!("{id}/episodes/{episode}"),
			})),
			_ => Ok(None),
		}
	}
}

register_source!(AriaToon, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
