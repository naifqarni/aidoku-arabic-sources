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

const BASE_URL: &str = "https://despair-world.com";
const API_URL: &str = "https://despair-world.com/supabase-api/rest/v1";
const ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJyb2xlIjoiYW5vbiIsImlzcyI6InN1cGFiYXNlIiwiaWF0IjoxNzkwMzczMDE4LCJleHAiOjE5NDgwNTMwMTh9.xYgAlC_9SuUCdzdff6wBYGWzYFmAI9apP_mk4DCPaVI";
const WORK_COLUMNS: &str =
	"id,slug,title_ar,title_en,cover_url,genres,synopsis,status,author_or_studio,type";
const PAGE_SIZE: usize = 24;

struct DespairManga;

#[derive(Deserialize)]
struct WorkDto {
	id: String,
	slug: Option<String>,
	title_ar: Option<String>,
	title_en: Option<String>,
	cover_url: Option<String>,
	genres: Option<Vec<String>>,
	synopsis: Option<String>,
	status: Option<String>,
	author_or_studio: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
}

#[derive(Deserialize)]
struct ChapterDto {
	id: String,
	chapter_number: f64,
	title: Option<String>,
	is_free: Option<bool>,
	created_at: Option<String>,
}

#[derive(Deserialize)]
struct PagesDto {
	pages: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct IdDto {
	id: String,
}

fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("apikey", ANON_KEY)
		.header("Authorization", &format!("Bearer {ANON_KEY}"))
		.header("Accept", "application/json")
		.header("Referer", &format!("{BASE_URL}/"))
		.json_owned()
}

fn non_blank(s: Option<String>) -> Option<String> {
	s.map(|s| s.trim().into())
		.filter(|s: &String| !s.is_empty())
}

fn format_number(n: f64) -> String {
	let s = n.to_string();
	s.strip_suffix(".0").map(String::from).unwrap_or(s)
}

/// Parses `yyyy-MM-ddTHH:mm:ss[.fraction](Z|±HH:MM)` into a unix timestamp (seconds).
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
	let mut ts = days * 86400 + hh * 3600 + mm * 60 + ss;
	// timezone offset after the seconds/fraction
	let rest = s
		.get(19..)?
		.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
	if let Some(sign) = rest.chars().next().filter(|c| *c == '+' || *c == '-') {
		let oh = rest.get(1..3)?.parse::<i64>().ok()?;
		let om = rest
			.get(4..6)
			.and_then(|v| v.parse::<i64>().ok())
			.unwrap_or(0);
		let offset = oh * 3600 + om * 60;
		ts += if sign == '+' { -offset } else { offset };
	}
	Some(ts)
}

impl WorkDto {
	fn into_manga(self) -> Option<Manga> {
		let slug = non_blank(self.slug)?;
		let title_en = non_blank(self.title_en);
		let title = non_blank(self.title_ar)
			.or_else(|| title_en.clone())
			.unwrap_or_default();
		let mut description = non_blank(self.synopsis).unwrap_or_default();
		if let Some(alt) = title_en.filter(|t| *t != title) {
			if !description.is_empty() {
				description.push_str("\n\n");
			}
			description.push_str("أسماء أخرى:\n");
			description.push_str(&alt);
		}
		Some(Manga {
			url: Some(format!("{BASE_URL}/series/{slug}")),
			key: slug,
			title,
			cover: self.cover_url,
			authors: non_blank(self.author_or_studio).map(|a| aidoku::alloc::vec![a]),
			description: Some(description).filter(|d| !d.is_empty()),
			tags: self.genres,
			status: match self.status.as_deref() {
				Some("مستمر") => MangaStatus::Ongoing,
				Some("مكتمل") => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			},
			content_rating: ContentRating::Safe,
			viewer: match self.kind.as_deref() {
				Some("manga") => Viewer::RightToLeft,
				Some("manhwa") | Some("manhua") | Some("webtoon") => Viewer::Webtoon,
				_ => Viewer::Unknown,
			},
			..Default::default()
		})
	}
}

fn get_works(page: i32, order: &str, filter: Option<(&str, String)>) -> Result<MangaPageResult> {
	let mut qs = QueryParameters::new();
	qs.push("select", Some(WORK_COLUMNS));
	if let Some((key, value)) = filter {
		qs.push(key, Some(&value));
	}
	qs.push("order", Some(order));
	qs.push("limit", Some(&PAGE_SIZE.to_string()));
	qs.push(
		"offset",
		Some(&((page - 1).max(0) as usize * PAGE_SIZE).to_string()),
	);
	let works: Vec<WorkDto> = get_json(&format!("{API_URL}/manga_works?{qs}"))?;
	Ok(MangaPageResult {
		has_next_page: works.len() == PAGE_SIZE,
		entries: works.into_iter().filter_map(WorkDto::into_manga).collect(),
	})
}

fn work_url(slug: &str, columns: &str) -> String {
	let mut qs = QueryParameters::new();
	qs.push("select", Some(columns));
	qs.push("slug", Some(&format!("eq.{slug}")));
	qs.push("limit", Some("1"));
	format!("{API_URL}/manga_works?{qs}")
}

impl Source for DespairManga {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let term = query.unwrap_or_default();
		let term = term.trim();
		if term.is_empty() {
			return get_works(page, "views_count.desc", None);
		}
		let term = term.replace('\\', "\\\\").replace('"', "\\\"");
		get_works(
			page,
			"views_count.desc",
			Some((
				"or",
				format!("(title_ar.ilike.\"*{term}*\",title_en.ilike.\"*{term}*\")"),
			)),
		)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut work_id = None;
		if needs_details {
			let works: Vec<WorkDto> = get_json(&work_url(&manga.key, WORK_COLUMNS))?;
			let work = works
				.into_iter()
				.next()
				.ok_or_else(|| error!("Series not found"))?;
			work_id = Some(work.id.clone());
			if let Some(details) = work.into_manga() {
				manga.copy_from(details);
			}
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let work_id = match work_id {
				Some(id) => id,
				None => {
					let ids: Vec<IdDto> = get_json(&work_url(&manga.key, "id"))?;
					ids.into_iter()
						.next()
						.ok_or_else(|| error!("Series not found"))?
						.id
				}
			};
			let mut qs = QueryParameters::new();
			qs.push("select", Some("id,chapter_number,title,is_free,created_at"));
			qs.push("work_id", Some(&format!("eq.{work_id}")));
			qs.push("order", Some("chapter_number.desc,id.desc"));
			let chapters: Vec<ChapterDto> = get_json(&format!("{API_URL}/chapters?{qs}"))?;
			let slug = &manga.key;
			manga.chapters = Some(
				chapters
					.into_iter()
					.map(|c| {
						let number = format_number(c.chapter_number);
						// titles are usually just "<series> - الفصل <n>" / "<series> - Chapter <n>"
						let title = non_blank(c.title).filter(|t| {
							!t.strip_suffix(number.as_str()).is_some_and(|rest| {
								!rest.ends_with(|ch: char| ch.is_ascii_digit() || ch == '.')
							})
						});
						Chapter {
							url: Some(format!("{BASE_URL}/series/{slug}/main/ch/{number}")),
							key: c.id,
							title,
							chapter_number: Some(c.chapter_number as f32),
							date_uploaded: c.created_at.as_deref().and_then(parse_iso_date),
							// paid chapters need coins on the site
							locked: c.is_free == Some(false),
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
		qs.push("select", Some("pages"));
		qs.push("id", Some(&format!("eq.{}", chapter.key)));
		let pages: Vec<PagesDto> = get_json(&format!("{API_URL}/chapters?{qs}"))?;
		Ok(pages
			.into_iter()
			.next()
			.and_then(|p| p.pages)
			.unwrap_or_default()
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for DespairManga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "popular" {
			get_works(page, "views_count.desc", None)
		} else {
			get_works(page, "updated_at.desc", None)
		}
	}
}

impl DeepLinkHandler for DespairManga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.despair-world.com"))
		else {
			return Ok(None);
		};
		let mut segments = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.split('/')
			.filter(|s| !s.is_empty());
		match (segments.next(), segments.next()) {
			(Some("series"), Some(slug)) => Ok(Some(DeepLinkResult::Manga { key: slug.into() })),
			_ => Ok(None),
		}
	}
}

register_source!(DespairManga, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
