#![no_std]
mod models;
mod nextjs;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{html::Document, net::Request},
	prelude::*,
};

use models::*;

const BASE_URL: &str = "https://rocksmanga.com";
const API_URL: &str = "https://api.rocksmanga.com";

struct RocksManga;

#[derive(Default)]
struct Browse {
	sort: Option<String>,
	query: Option<String>,
	kind: Option<String>,
	status: Option<String>,
	include: Vec<String>,
	exclude: Vec<String>,
}

fn get(url: &str) -> Result<Document> {
	Ok(Request::get(url)?.html()?)
}

fn browse(page: i32, options: Browse) -> Result<MangaPageResult> {
	let mut qs = QueryParameters::new();
	if page > 1 {
		qs.push("page", Some(&page.to_string()));
	}
	if let Some(sort) = options.sort.filter(|s| !s.is_empty()) {
		if sort == "alphabetical" {
			qs.push("direction", Some("asc"));
		}
		qs.push("sort", Some(&sort));
	}
	if let Some(query) = options
		.query
		.as_deref()
		.map(str::trim)
		.filter(|q| !q.is_empty())
	{
		qs.push("search", Some(query));
	}
	if let Some(kind) = options.kind.filter(|s| !s.is_empty()) {
		qs.push("type", Some(&kind));
	}
	if let Some(status) = options.status.filter(|s| !s.is_empty()) {
		qs.push("status", Some(&status));
	}
	if !options.include.is_empty() {
		qs.push("genre", Some(&options.include.join(",")));
	}
	if !options.exclude.is_empty() {
		qs.push("genre_exclude", Some(&options.exclude.join(",")));
	}

	let html = get(&format!("{BASE_URL}/browse?{qs}"))?;
	let mut entries: Vec<Manga> = Vec::new();
	for el in html.select("main .unit").into_iter().flatten() {
		let Some(link) = el.select_first(".info > a[href]") else {
			continue;
		};
		let Some(href) = link.attr("abs:href") else {
			continue;
		};
		let key = href
			.strip_prefix(BASE_URL)
			.map(String::from)
			.unwrap_or(href);
		if entries.iter().any(|m| m.key == key) {
			continue;
		}
		entries.push(Manga {
			url: Some(format!("{BASE_URL}{key}")),
			key,
			title: link.text().unwrap_or_default(),
			cover: el.select_first("img").and_then(|img| img.attr("abs:src")),
			..Default::default()
		});
	}
	Ok(MangaPageResult {
		entries,
		has_next_page: html
			.select_first("li.page-item > a.page-link[rel=next]")
			.is_some(),
	})
}

impl Source for RocksManga {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut options = Browse {
			query,
			..Default::default()
		};
		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"sort" => options.sort = Some(value),
					"type" => options.kind = Some(value),
					"status" => options.status = Some(value),
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genre" => {
					options.include = included;
					options.exclude = excluded;
				}
				_ => {}
			}
		}
		browse(page, options)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// details and chapters come from the same page
		let html = get(&format!("{BASE_URL}{}", manga.key))?;
		let payload = nextjs::payload(&html);

		if needs_details {
			let details: Details = nextjs::find(&payload, &["readHref", "manga"])
				.ok_or_else(|| error!("Missing series details"))?;
			let chapters = manga.chapters.take();
			manga = Manga {
				chapters,
				..details.manga.into_manga()
			};
		}

		if needs_chapters {
			let list: ChapterList = nextjs::find(&payload, &["mangaSlug", "chapters"])
				.ok_or_else(|| error!("Missing chapter list"))?;
			let mut chapters = list.chapters;
			chapters.sort_by_key(|c| core::cmp::Reverse(c.id));
			manga.chapters = Some(
				chapters
					.into_iter()
					.map(|c| c.into_chapter(&list.manga_slug))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = get(&format!("{BASE_URL}{}", chapter.key))?;
		let reader: Reader = nextjs::find(&nextjs::payload(&html), &["chapter", "manga"])
			.ok_or_else(|| error!("Missing chapter pages"))?;
		Ok(reader
			.chapter
			.pages
			.into_iter()
			.map(|path| Page {
				content: PageContent::url(format!("{API_URL}{path}")),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for RocksManga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		browse(
			page,
			Browse {
				sort: Some(listing.id),
				..Default::default()
			},
		)
	}
}

impl DeepLinkHandler for RocksManga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let mut segments = path.split('/').filter(|s| !s.is_empty());
		let (Some("manga"), Some(slug)) = (segments.next(), segments.next()) else {
			return Ok(None);
		};
		let manga_key = format!("/manga/{slug}");
		Ok(Some(match (segments.next(), segments.next()) {
			(Some("chapter"), Some(number)) => DeepLinkResult::Chapter {
				key: format!("{manga_key}/chapter/{number}"),
				manga_key,
			},
			_ => DeepLinkResult::Manga { key: manga_key },
		}))
	}
}

register_source!(RocksManga, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
