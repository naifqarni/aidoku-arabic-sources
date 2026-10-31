#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		net::Request,
		std::{current_date, send_partial_result},
	},
	prelude::*,
};
use core::cell::RefCell;
use serde::{Deserialize, de::DeserializeOwned};

const BASE_URL: &str = "https://kawaiimanga.org";
const API_URL: &str = "https://manga-api.kawaii-anime.com/api/manga";
const APP_KEY: &str = "km_2026_live";

struct KawiiManga {
	/// short-lived api token and the unix time it should be refreshed at
	token: RefCell<Option<(String, i64)>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MangaList {
	results: Vec<MangaDto>,
	#[serde(default)]
	has_more: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MangaDto {
	slug: String,
	title: String,
	description: Option<String>,
	cover_url: Option<String>,
	author: Option<String>,
	artist: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
	status: Option<String>,
	#[serde(default)]
	genres: Vec<String>,
	#[serde(default)]
	chapters: Vec<ChapterDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterDto {
	id: String,
	title: Option<String>,
	number: f32,
	created_at: Option<String>,
}

#[derive(Deserialize)]
struct Pages {
	pages: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Token {
	token: String,
	expires_in: i64,
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

fn valid(s: Option<String>) -> Option<String> {
	s.map(|s| s.trim().into())
		.filter(|s: &String| !s.is_empty() && !s.eq_ignore_ascii_case("unknown"))
}

fn format_number(n: f32) -> String {
	let s = n.to_string();
	s.strip_suffix(".0").map(String::from).unwrap_or(s)
}

impl MangaDto {
	fn into_manga(self) -> Manga {
		let mut tags = Vec::new();
		match self.kind.as_deref() {
			Some("manga") => tags.push("Manga".into()),
			Some("manhwa") => tags.push("Manhwa".into()),
			Some("manhua") => tags.push("Manhua".into()),
			_ => {}
		}
		for genre in self.genres {
			if !tags.contains(&genre) {
				tags.push(genre);
			}
		}
		Manga {
			url: Some(format!("{BASE_URL}/manga/{}", self.slug)),
			key: self.slug,
			title: self.title,
			cover: self.cover_url,
			description: valid(self.description),
			authors: valid(self.author).map(|a| vec![a]),
			artists: valid(self.artist).map(|a| vec![a]),
			tags: Some(tags),
			status: match self.status.as_deref() {
				Some("ongoing") | Some("coming_soon") => MangaStatus::Ongoing,
				Some("completed") => MangaStatus::Completed,
				Some("cancelled") | Some("dropped") => MangaStatus::Cancelled,
				Some("hiatus") => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			},
			viewer: match self.kind.as_deref() {
				Some("manga") => Viewer::RightToLeft,
				Some("manhwa") | Some("manhua") | Some("webtoon") => Viewer::Webtoon,
				_ => Viewer::Unknown,
			},
			content_rating: ContentRating::Safe,
			..Default::default()
		}
	}
}

impl KawiiManga {
	fn token(&self, refresh: bool) -> Result<String> {
		let now = current_date();
		if !refresh
			&& let Some((token, expiry)) = self.token.borrow().as_ref()
			&& now < *expiry
		{
			return Ok(token.clone());
		}
		let data: Token = Request::get(format!("{API_URL}/token"))?
			.header("x-app-key", APP_KEY)
			.json_owned()?;
		*self.token.borrow_mut() = Some((data.token.clone(), now + data.expires_in - 120));
		Ok(data.token)
	}

	fn api<T: DeserializeOwned>(&self, query: &str) -> Result<T> {
		let url = format!("{API_URL}/own?{query}");
		let request = |token: &str| -> Result<_> {
			Ok(Request::get(&url)?
				.header("x-app-key", APP_KEY)
				.header("x-app-token", token)
				.header("Referer", &format!("{BASE_URL}/"))
				.send()?)
		};
		let mut response = request(&self.token(false)?)?;
		if response.status_code() == 401 {
			response = request(&self.token(true)?)?;
		}
		if !(200..300).contains(&response.status_code()) {
			bail!("HTTP error {}", response.status_code());
		}
		response.get_json_owned()
	}

	fn manga_page(&self, query: &str) -> Result<MangaPageResult> {
		let data: MangaList = self.api(query)?;
		Ok(MangaPageResult {
			entries: data.results.into_iter().map(|m| m.into_manga()).collect(),
			has_next_page: data.has_more,
		})
	}

	fn series(&self, slug: &str) -> Result<MangaDto> {
		let mut qs = QueryParameters::new();
		qs.push("action", Some("series"));
		qs.push("slug", Some(slug));
		self.api(&qs.to_string())
	}
}

impl Source for KawiiManga {
	fn new() -> Self {
		Self {
			token: RefCell::new(None),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		match query.filter(|q| !q.is_empty()) {
			Some(query) => {
				// search isn't paginated
				if page > 1 {
					return Ok(MangaPageResult::default());
				}
				let mut qs = QueryParameters::new();
				qs.push("action", Some("search"));
				qs.push("q", Some(&query));
				let mut result = self.manga_page(&qs.to_string())?;
				result.has_next_page = false;
				Ok(result)
			}
			None => self.manga_page(&format!("action=browse&page={page}&sort=views")),
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut data = self.series(&manga.key)?;
		let chapters = core::mem::take(&mut data.chapters);

		if needs_details {
			manga.copy_from(data.into_manga());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let slug = &manga.key;
			manga.chapters = Some(
				chapters
					.into_iter()
					.map(|c| {
						let number = format_number(c.number);
						let title = c
							.title
							.map(|t| t.trim().into())
							.filter(|t: &String| !t.is_empty() && *t != format!("الفصل {number}"));
						Chapter {
							key: c.id,
							title,
							chapter_number: Some(c.number),
							date_uploaded: c.created_at.as_deref().and_then(parse_iso_date),
							url: Some(format!("{BASE_URL}/reader/{slug}/{number}")),
							..Default::default()
						}
					})
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let mut qs = QueryParameters::new();
		qs.push("action", Some("pages"));
		qs.push("chapterId", Some(&chapter.key));
		let data: Pages = self.api(&qs.to_string())?;
		Ok(data
			.pages
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for KawiiManga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "popular" {
			self.manga_page(&format!("action=browse&page={page}&sort=views"))
		} else {
			self.manga_page(&format!("action=browse&page={page}"))
		}
	}
}

impl DeepLinkHandler for KawiiManga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.kawaiimanga.org"))
		else {
			return Ok(None);
		};
		let mut segments = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.split('/')
			.skip(1);
		match (segments.next(), segments.next(), segments.next()) {
			(Some("manga"), Some(slug), _) if !slug.is_empty() => {
				Ok(Some(DeepLinkResult::Manga { key: slug.into() }))
			}
			(Some("reader"), Some(slug), Some(number)) if !slug.is_empty() => {
				// chapter keys are api ids, so look the chapter up by its number
				let number = number.parse::<f32>().ok();
				let key = self
					.series(slug)?
					.chapters
					.into_iter()
					.find(|c| Some(c.number) == number)
					.map(|c| c.id);
				Ok(Some(match key {
					Some(key) => DeepLinkResult::Chapter {
						manga_key: slug.into(),
						key,
					},
					None => DeepLinkResult::Manga { key: slug.into() },
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(KawiiManga, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
