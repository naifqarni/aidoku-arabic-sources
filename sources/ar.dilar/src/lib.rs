#![no_std]
mod models;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec},
	imports::{
		js::{WebView, WebViewUserScript},
		net::Request,
		std::{send_partial_result, sleep},
	},
	prelude::*,
};

use models::*;

const BASE_URL: &str = "https://dilar.tube";
/// How long the reader in the web view gets to load a chapter.
const READER_TIMEOUT_SECONDS: i32 = 20;

struct Dilar;

fn series_page(series: Vec<Series>, has_next_page: bool) -> MangaPageResult {
	MangaPageResult {
		entries: series
			.into_iter()
			.filter(|s| !s.is_novel())
			.map(Series::into_manga)
			.collect(),
		has_next_page,
	}
}

/// The chapter data the reader receives is encrypted with a scheme the site keeps rotating, so
/// the site's own reader decrypts it and the result is caught as it gets parsed.
fn capture_script(release_id: &str) -> String {
	format!(
		r#"(() => {{
	const parse = JSON.parse;
	JSON.parse = function () {{
		const result = parse.apply(this, arguments);
		if (result && String(result.id) === "{release_id}" && result.storage_key && Array.isArray(result.pages)) {{
			window.__aidokuPages = result.pages
				.slice()
				.sort((a, b) => a.order - b.order)
				.map((p) => `${{location.origin}}/uploads/releases/${{result.storage_key}}/hq/${{p.url}}?t=${{result.media_token}}`);
		}}
		return result;
	}};
}})();"#
	)
}

const READ_PAGES_SCRIPT: &str = r#"(() => {
	if (window.__aidokuPages) return JSON.stringify({ exact: true, urls: window.__aidokuPages });
	const urls = [...document.querySelectorAll("img")]
		.map((img) => img.src)
		.filter((src) => src.includes("/uploads/releases/") && src.includes("/hq/"));
	return JSON.stringify({ exact: false, urls: [...new Set(urls)] });
})()"#;

impl Source for Dilar {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut series_type = IncludeExclude::default();
		let mut categories = IncludeExclude::default();
		let mut story_status = IncludeExclude::default();
		let mut translation_status = IncludeExclude::default();
		let mut oneshot = false;
		let mut over17 = None;

		for filter in filters {
			match filter {
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"type" => {
						series_type.include = included;
						series_type.exclude = excluded;
					}
					"story" => story_status.include = included,
					"translation" => translation_status.include = included,
					_ if id.starts_with("category") => {
						categories
							.include
							.extend(included.iter().filter_map(|c| c.parse::<i32>().ok()));
						categories
							.exclude
							.extend(excluded.iter().filter_map(|c| c.parse::<i32>().ok()));
					}
					_ => {}
				},
				FilterValue::Check { id, value } if id == "oneshot" => oneshot = value != 0,
				FilterValue::Select { id, value } if id == "over17" => {
					over17 = value.parse::<bool>().ok();
				}
				_ => {}
			}
		}

		let body = SearchRequest {
			query: query.as_deref().unwrap_or_default(),
			page,
			series_type,
			categories,
			oneshot,
			story_status,
			translation_status,
			over17,
		};
		let body = serde_json::to_vec(&body).map_err(|_| error!("Failed to encode search"))?;
		let data: SearchList = Request::post(format!("{BASE_URL}/api/search/filter"))?
			.header("Content-Type", "application/json")
			.body(body)
			.json_owned()?;
		let has_next_page = data.total > data.page * data.per_page;
		Ok(series_page(data.rows, has_next_page))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let series: Series =
				Request::get(format!("{BASE_URL}/api/series/{}", manga.key))?.json_owned()?;
			let chapters = manga.chapters.take();
			manga = Manga {
				chapters,
				..series.into_manga()
			};
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let data: ChapterList =
				Request::get(format!("{BASE_URL}/api/series/{}/chapters", manga.key))?
					.json_owned()?;
			let mut chapters: Vec<Chapter> = data
				.chapters
				.into_iter()
				.flat_map(|c| c.into_chapters(&manga.key))
				.collect();
			chapters.sort_by(|a, b| {
				b.chapter_number
					.unwrap_or(-1.0)
					.total_cmp(&a.chapter_number.unwrap_or(-1.0))
			});
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = chapter.url.unwrap_or_else(|| {
			let number = chapter
				.chapter_number
				.map(|n| format!("{n}"))
				.unwrap_or_default();
			reader_url(&manga.key, &number, &chapter.key)
		});

		let web_view = WebView::new();
		web_view.add_user_script(WebViewUserScript::new(capture_script(&chapter.key)))?;
		web_view.load(Request::get(url)?)?;

		// fall back to the images the reader puts on the page once their count settles
		let mut last_count = 0;
		for _ in 0..READER_TIMEOUT_SECONDS {
			sleep(1);
			let Ok(result) = web_view.eval(READ_PAGES_SCRIPT) else {
				continue;
			};
			let Ok(pages) = serde_json::from_str::<ReaderPages>(&result) else {
				continue;
			};
			let count = pages.urls.len();
			if pages.exact || (count > 0 && count == last_count) {
				return Ok(pages
					.urls
					.into_iter()
					.map(|url| Page {
						content: PageContent::url(url),
						..Default::default()
					})
					.collect());
			}
			last_count = count;
		}
		bail!("The chapter didn't load in time")
	}
}

impl ListingProvider for Dilar {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "popular" {
			if page > 1 {
				return Ok(MangaPageResult::default());
			}
			let data: Rankings = Request::get(format!("{BASE_URL}/api/rankings"))?.json_owned()?;
			Ok(series_page(data.top_series, false))
		} else {
			let data: SeriesList =
				Request::get(format!("{BASE_URL}/api/series?page={page}"))?.json_owned()?;
			let has_next_page = data.current_page < data.total_pages;
			Ok(series_page(data.series, has_next_page))
		}
	}
}

impl DeepLinkHandler for Dilar {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let (path, query) = path.split_once('?').unwrap_or((path, ""));
		let mut segments = path.split('/').skip(1);
		match (segments.next(), segments.next()) {
			(Some("series"), Some(id)) if !id.is_empty() => {
				Ok(Some(DeepLinkResult::Manga { key: id.into() }))
			}
			(Some("reader"), Some(id)) if !id.is_empty() => {
				let release = query
					.split('&')
					.find_map(|pair| pair.strip_prefix("release="));
				Ok(Some(match release {
					Some(release) => DeepLinkResult::Chapter {
						manga_key: id.into(),
						key: release.into(),
					},
					None => DeepLinkResult::Manga { key: id.into() },
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(Dilar, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
