#![no_std]
mod models;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};

use models::*;

const BASE_URL: &str = "https://mangatuk.com";
const API_URL: &str = "https://api.mangatuk.com/api";
const PAGE_SIZE: i32 = 30;

struct MangaTuk;

fn search(
	page: i32,
	query: Option<&str>,
	sort: &str,
	status: Option<&str>,
) -> Result<MangaPageResult> {
	let mut qs = QueryParameters::new();
	if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
		qs.push("q", Some(query.trim()));
	}
	qs.push("sort", Some(sort));
	if let Some(status) = status {
		qs.push("status", Some(status));
	}
	qs.push("limit", Some(&PAGE_SIZE.to_string()));
	qs.push("offset", Some(&((page - 1) * PAGE_SIZE).to_string()));
	let data: SeriesList = Request::get(format!("{API_URL}/catalog/search?{qs}"))?.json_owned()?;
	Ok(MangaPageResult {
		entries: data.data.into_iter().map(Series::into_manga).collect(),
		has_next_page: page * PAGE_SIZE < data.total,
	})
}

fn series_by_slug(slug: &str) -> Result<Series> {
	Request::get(format!("{API_URL}/catalog/series/by-slug/{slug}"))?.json_owned()
}

impl Source for MangaTuk {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut sort = String::from("popular");
		let mut status = None;
		for filter in filters {
			if let FilterValue::Select { id, value } = filter
				&& !value.is_empty()
			{
				match id.as_str() {
					"sort" => sort = value,
					"status" => status = Some(value),
					_ => {}
				}
			}
		}
		search(page, query.as_deref(), &sort, status.as_deref())
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// the series endpoint includes the whole chapter list, so one request covers both
		let mut series: Series =
			Request::get(format!("{API_URL}/catalog/series/{}", manga.key))?.json_owned()?;
		if needs_chapters {
			let slug = series.slug.clone();
			// oldest first in the api; coin-locked chapters can't be read
			manga.chapters = Some(
				core::mem::take(&mut series.chapters)
					.into_iter()
					.rev()
					.filter(|c| !c.locked())
					.map(|c| c.into_chapter(&slug))
					.collect(),
			);
		}
		if needs_details {
			let chapters = manga.chapters.take();
			manga = Manga {
				chapters,
				..series.into_manga_details()
			};
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let pages: Vec<PageImage> =
			Request::get(format!("{API_URL}/catalog/chapters/{}/pages", chapter.key))?
				.json_owned()?;
		Ok(pages
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(p.image_url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for MangaTuk {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		search(page, None, &listing.id, None)
	}
}

impl DeepLinkHandler for MangaTuk {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let mut segments = path.split('/').filter(|s| !s.is_empty());
		let (Some("series" | "manga"), Some(slug)) = (segments.next(), segments.next()) else {
			return Ok(None);
		};
		let chapter_slug = segments.next();
		let series = series_by_slug(slug)?;
		let chapter = chapter_slug.and_then(|chapter_slug| {
			series
				.chapters
				.iter()
				.find(|c| c.slug == chapter_slug)
				.map(|c| c.id.clone())
		});
		Ok(Some(match chapter {
			Some(key) => DeepLinkResult::Chapter {
				manga_key: series.id,
				key,
			},
			None => DeepLinkResult::Manga { key: series.id },
		}))
	}
}

register_source!(MangaTuk, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
