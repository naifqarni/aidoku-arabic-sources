#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

const BASE_URL: &str = "https://mangatime.org";
const PAGE_LIMIT: i32 = 24;
const SERIES_TYPES: [&str; 5] = ["manga", "manhwa", "manhua", "webtoon", "series"];

struct MangaTime;

#[derive(Deserialize)]
struct TrpcResponse<T> {
	result: TrpcResult<T>,
}

#[derive(Deserialize)]
struct TrpcResult<T> {
	data: TrpcData<T>,
}

#[derive(Deserialize)]
struct TrpcData<T> {
	json: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MangaListDto {
	results: Vec<SeriesItem>,
	#[serde(default)]
	has_more: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesItem {
	title: String,
	slug: String,
	cover_url: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
}

#[derive(Deserialize)]
struct Named {
	name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesDto {
	id: String,
	title: String,
	slug: String,
	cover_url: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
	genres: Option<Vec<Named>>,
	description: Option<String>,
	status: Option<String>,
	author: Option<String>,
	artist: Option<String>,
	content_rating: Option<String>,
}

#[derive(Deserialize)]
struct ChaptersDto {
	chapters: Vec<ChapterDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterDto {
	number: f32,
	volume: Option<f32>,
	title: Option<String>,
	published_at: Option<String>,
	#[serde(default)]
	is_premium: bool,
	is_unlocked: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PagesDto {
	pages: Vec<String>,
	is_unlocked: Option<bool>,
}

fn trpc<T: DeserializeOwned>(endpoint: &str, input: Value) -> Result<T> {
	let mut qs = QueryParameters::new();
	qs.push("batch", Some("1"));
	qs.push(
		"input",
		Some(&json!({ "0": { "json": input } }).to_string()),
	);
	let mut response: Vec<TrpcResponse<T>> =
		Request::get(format!("{BASE_URL}/api/trpc/{endpoint}?{qs}"))?
			.header("Referer", &format!("{BASE_URL}/"))
			.json_owned()?;
	if response.is_empty() {
		bail!("empty response");
	}
	Ok(response.remove(0).result.data.json)
}

fn to_image(url: &str) -> String {
	let url = url.replace(' ', "%20");
	if url.starts_with("http") {
		url
	} else {
		format!("{BASE_URL}{url}")
	}
}

fn format_number(n: f32) -> String {
	let s = n.to_string();
	s.strip_suffix(".0").map(String::from).unwrap_or(s)
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
		.filter(|s: &String| !s.is_empty())
}

fn series_url(kind: Option<&str>, slug: &str) -> String {
	format!("{BASE_URL}/{}/{slug}", kind.unwrap_or("manga"))
}

fn search(page: i32, sort_by: &str, query: Option<String>) -> Result<MangaPageResult> {
	let mut input = json!({
		"page": page,
		"limit": PAGE_LIMIT,
		"sortBy": sort_by,
		"sortOrder": "desc",
	});
	if let Some(query) = query {
		input["query"] = Value::String(query);
	}
	let data: MangaListDto = trpc("search.searchSeries", input)?;
	Ok(MangaPageResult {
		entries: data
			.results
			.into_iter()
			.map(|s| Manga {
				url: Some(series_url(s.kind.as_deref(), &s.slug)),
				key: s.slug,
				title: s.title,
				cover: s.cover_url.as_deref().map(to_image),
				..Default::default()
			})
			.collect(),
		has_next_page: data.has_more,
	})
}

impl Source for MangaTime {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		search(page, "popularity", query.filter(|q| !q.is_empty()))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// the series id (needed for chapters) only comes from the details endpoint
		let series: SeriesDto = trpc("content.getSeriesBySlug", json!({ "slug": manga.key }))?;
		let kind = series.kind.clone();

		if needs_details {
			let mut tags: Vec<String> = series
				.genres
				.unwrap_or_default()
				.into_iter()
				.map(|g| String::from(g.name.trim()))
				.filter(|g| !g.is_empty())
				.collect();
			if let Some(kind) = valid(kind.clone()) {
				tags.push(kind);
			}
			manga.title = series.title;
			manga.cover = series.cover_url.as_deref().map(to_image).or(manga.cover);
			manga.description = valid(series.description);
			manga.authors = valid(series.author).map(|a| vec![a]);
			manga.artists = valid(series.artist).map(|a| vec![a]);
			manga.tags = Some(tags);
			manga.status = match series.status.as_deref().map(str::to_lowercase).as_deref() {
				Some("ongoing") => MangaStatus::Ongoing,
				Some("completed") => MangaStatus::Completed,
				Some("hiatus") => MangaStatus::Hiatus,
				Some("cancelled") | Some("dropped") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = match series.content_rating.as_deref() {
				Some("suggestive") => ContentRating::Suggestive,
				Some("erotica") | Some("pornographic") | Some("nsfw") => ContentRating::NSFW,
				_ => ContentRating::Safe,
			};
			manga.viewer = match kind.as_deref() {
				Some("manga") => Viewer::RightToLeft,
				Some("manhwa") | Some("manhua") | Some("webtoon") => Viewer::Webtoon,
				_ => Viewer::Unknown,
			};
			manga.url = Some(series_url(kind.as_deref(), &series.slug));

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let data: ChaptersDto = trpc(
				"content.getChapters",
				json!({ "seriesId": series.id, "limit": -1 }),
			)?;
			let base = series_url(kind.as_deref(), &manga.key);
			manga.chapters = Some(
				data.chapters
					.into_iter()
					// paid chapters can't be read without buying them
					.filter(|c| c.is_unlocked.unwrap_or(!c.is_premium))
					.map(|c| {
						let number = format_number(c.number);
						Chapter {
							url: Some(format!("{base}/chapter/{number}")),
							key: number,
							title: valid(c.title),
							chapter_number: Some(c.number),
							volume_number: c.volume,
							date_uploaded: c.published_at.as_deref().and_then(parse_iso_date),
							..Default::default()
						}
					})
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let number: f32 = chapter.key.parse().unwrap_or_default();
		let data: PagesDto = trpc(
			"content.getChapterPages",
			json!({ "seriesSlug": manga.key, "chapterNumber": number }),
		)?;
		if data.is_unlocked == Some(false) {
			bail!("Chapter is locked");
		}
		Ok(data
			.pages
			.iter()
			.map(|url| Page {
				content: PageContent::url(to_image(url)),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for MangaTime {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "latest" {
			search(page, "recent", None)
		} else {
			search(page, "popularity", None)
		}
	}
}

impl DeepLinkHandler for MangaTime {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.mangatime.org"))
		else {
			return Ok(None);
		};
		let mut segments = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.split('/')
			.filter(|s| !s.is_empty());
		let (Some(kind), Some(slug)) = (segments.next(), segments.next()) else {
			return Ok(None);
		};
		if !SERIES_TYPES.contains(&kind) {
			return Ok(None);
		}
		match (segments.next(), segments.next()) {
			(Some("chapter"), Some(number)) => Ok(Some(DeepLinkResult::Chapter {
				manga_key: slug.into(),
				key: number.into(),
			})),
			_ => Ok(Some(DeepLinkResult::Manga { key: slug.into() })),
		}
	}
}

register_source!(MangaTime, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
