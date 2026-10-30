#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		net::Request,
		std::{parse_date, send_partial_result},
	},
	prelude::*,
};
use serde::Deserialize;

const BASE_URL: &str = "https://meshmanga.com";
const API_URL: &str = "https://meshmanga.com/v2/api/v2";

struct MangaSwat;

#[derive(Deserialize)]
struct MangaList {
	results: Vec<MangaEntry>,
	next: Option<String>,
}

#[derive(Deserialize)]
struct MangaEntry {
	id: Option<i64>,
	serie_id: Option<i64>,
	title: String,
	poster: Poster,
}

#[derive(Deserialize)]
struct Poster {
	medium: Option<String>,
}

#[derive(Deserialize)]
struct MangaDetails {
	title: String,
	poster: Poster,
	#[serde(default)]
	genres: Vec<Named>,
	story: Option<String>,
	author: Option<Named>,
	artist: Option<Named>,
	status: Option<Named>,
}

#[derive(Deserialize)]
struct Named {
	name: String,
}

#[derive(Deserialize)]
struct ChapterList {
	results: Vec<ChapterEntry>,
	next: Option<String>,
}

#[derive(Deserialize)]
struct ChapterEntry {
	id: i64,
	slug: String,
	chapter: String,
	created_at: Option<String>,
}

#[derive(Deserialize)]
struct PageList {
	images: Vec<PageImage>,
}

#[derive(Deserialize)]
struct PageImage {
	image: String,
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.json_owned()
}

fn manga_page(url: &str) -> Result<MangaPageResult> {
	let data: MangaList = get_json(url)?;
	Ok(MangaPageResult {
		entries: data
			.results
			.into_iter()
			.filter_map(|m| {
				Some(Manga {
					key: m.id.or(m.serie_id)?.to_string(),
					title: m.title,
					cover: m.poster.medium,
					..Default::default()
				})
			})
			.collect(),
		has_next_page: data.next.is_some(),
	})
}

fn parse_iso_date(date: &str) -> Option<i64> {
	parse_date(date, "yyyy-MM-dd'T'HH:mm:ss.SSSSSSXXX")
		.or_else(|| parse_date(date, "yyyy-MM-dd'T'HH:mm:ssXXX"))
		.or_else(|| parse_date(date, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'"))
}

impl Source for MangaSwat {
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
		qs.push("search", Some(query.as_deref().unwrap_or_default()));
		qs.push("page", Some(&page.to_string()));
		manga_page(&format!("{API_URL}/series/?{qs}"))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let details: MangaDetails = get_json(&format!("{API_URL}/series/{}/", manga.key))?;
			manga.title = details.title;
			manga.cover = details.poster.medium.or(manga.cover);
			manga.description = details.story;
			manga.tags = Some(details.genres.into_iter().map(|g| g.name).collect());
			manga.authors = details.author.map(|a| vec![a.name]);
			manga.artists = details.artist.map(|a| vec![a.name]);
			manga.status = match details.status.as_ref().map(|s| s.name.as_str()) {
				Some("ongoing") => MangaStatus::Ongoing,
				Some("completed") => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.url = Some(format!("{BASE_URL}/series/{}", manga.key));
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut qs = QueryParameters::new();
			qs.push("serie", Some(&manga.key));
			qs.push("order_by", Some("-order"));
			qs.push("page_size", Some("200"));
			let mut next = Some(format!("{API_URL}/chapters/?{qs}"));
			let mut chapters = Vec::new();
			while let Some(url) = next {
				let data: ChapterList = get_json(&url)?;
				chapters.extend(data.results.into_iter().map(|c| Chapter {
					key: c.id.to_string(),
					chapter_number: c.chapter.trim().parse().ok(),
					title: if c.chapter.trim().parse::<f32>().is_ok() {
						None
					} else {
						Some(c.chapter)
					},
					date_uploaded: c.created_at.as_deref().and_then(parse_iso_date),
					url: Some(format!("{BASE_URL}/chapters/{}/{}/", c.id, c.slug)),
					..Default::default()
				}));
				next = data.next;
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let data: PageList = get_json(&format!("{API_URL}/chapters/{}/", chapter.key))?;
		Ok(data
			.images
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(p.image),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for MangaSwat {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "popular" {
			manga_page(&format!(
				"{API_URL}/series/?order_by=-followers_count&page={page}"
			))
		} else {
			manga_page(&format!("{API_URL}/series/releases/?page={page}"))
		}
	}
}

impl DeepLinkHandler for MangaSwat {
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
			(Some("series"), Some(id)) if id.parse::<i64>().is_ok() => {
				Ok(Some(DeepLinkResult::Manga { key: id.into() }))
			}
			_ => Ok(None),
		}
	}
}

register_source!(MangaSwat, ListingProvider, DeepLinkHandler);
