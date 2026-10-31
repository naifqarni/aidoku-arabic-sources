#![no_std]
mod helpers;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, ImageResponse,
	Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext,
	PageImageProcessor, Result, Source,
	alloc::{String, Vec, vec},
	imports::{canvas::ImageRef, html::Document, net::Request},
	prelude::*,
};
use core::cell::RefCell;
use serde::Deserialize;

use helpers::*;

const BASE_URL: &str = "https://stellarsaber.pro";
const AJAX_URL: &str = "https://stellarsaber.pro/wp-admin/admin-ajax.php";

#[derive(Default)]
struct StellarSaber {
	/// The nonce the site's ajax search wants, read off any listing page.
	nonce: RefCell<Option<String>>,
}

#[derive(Deserialize)]
struct SearchResponse {
	data: SearchData,
}

#[derive(Deserialize)]
struct SearchData {
	results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
	title: String,
	url: String,
	cover: String,
	#[serde(rename = "type")]
	kind: String,
}

#[derive(Deserialize)]
struct CdnKeyResponse {
	data: CdnKey,
}

#[derive(Deserialize)]
struct CdnKey {
	key: String,
}

fn get(url: &str) -> Result<Document> {
	Ok(Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.html()?)
}

impl StellarSaber {
	fn remember_nonce(&self, html: &Document) {
		if self.nonce.borrow().is_some() {
			return;
		}
		let nonce = html
			.select_first("#flavor-ajax-js-extra")
			.and_then(|s| script_text(&s))
			.and_then(|s| extract(&s, "nonce\":\"", '"').map(String::from));
		*self.nonce.borrow_mut() = nonce;
	}

	fn nonce(&self) -> Result<String> {
		if let Some(nonce) = self.nonce.borrow().clone() {
			return Ok(nonce);
		}
		let html = get(&format!("{BASE_URL}/manga/"))?;
		self.remember_nonce(&html);
		self.nonce
			.borrow()
			.clone()
			.ok_or_else(|| error!("Search nonce not found"))
	}

	fn manga_list(&self, sort: &str, page: i32) -> Result<MangaPageResult> {
		let page_path = if page > 1 {
			format!("page/{page}/")
		} else {
			String::new()
		};
		let html = get(&format!("{BASE_URL}/manga/{page_path}?sort={sort}"))?;
		let entries = html
			.select(".card-grid .card")
			.map(|els| {
				els.filter_map(|el| {
					let href = el.select_first("a")?.attr("abs:href")?;
					Some(Manga {
						key: strip_base(&href),
						title: el.select_first(".card__title")?.text()?,
						cover: el.select_first("img").and_then(|img| img.attr("abs:src")),
						url: Some(href),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();
		self.remember_nonce(&html);
		Ok(MangaPageResult {
			entries,
			has_next_page: html.select_first(".nav-links .next").is_some(),
		})
	}
}

impl Source for StellarSaber {
	fn new() -> Self {
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let Some(query) = query.filter(|q| !q.trim().is_empty()) else {
			return self.manga_list("latest", page);
		};
		let nonce = self.nonce()?;
		let page = format!("{page}");
		let (content_type, body) = multipart(&[
			("action", "flavor_ajax_filter_content"),
			("nonce", &nonce),
			("page", &page),
			("keyword", query.trim()),
		]);
		let data: SearchResponse = Request::post(AJAX_URL)?
			.header("Content-Type", &content_type)
			.header("Referer", &format!("{BASE_URL}/"))
			.body(body)
			.json_owned()?;
		let entries: Vec<Manga> = data
			.data
			.results
			.into_iter()
			.filter(|r| r.kind != "novel" && r.kind != "anime")
			.map(|r| Manga {
				key: strip_base(&r.url),
				title: r.title,
				cover: Some(r.cover),
				url: Some(r.url),
				..Default::default()
			})
			.collect();
		Ok(MangaPageResult {
			has_next_page: !entries.is_empty(),
			entries,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = get(&url)?;

		if needs_details && let Some(info) = html.select_first(".detail-content") {
			if let Some(title) = info
				.select_first(".detail-info__title")
				.and_then(|el| el.text())
				.filter(|t| !t.is_empty())
			{
				manga.title = title;
			}
			manga.cover = info
				.select_first(".detail-poster img")
				.and_then(|img| img.attr("abs:src"))
				.or(manga.cover);
			manga.tags = info.select(".detail-genres a").map(|els| {
				els.filter_map(|el| el.text())
					.filter(|t| !t.is_empty())
					.collect()
			});
			let mut description = info
				.select("#detail-desc p")
				.map(|els| {
					els.filter_map(|el| el.text())
						.filter(|t| !t.is_empty())
						.collect::<Vec<_>>()
						.join("\n\n")
				})
				.unwrap_or_default();
			if let Some(alt) = info
				.select_first(".detail-info__alt-title")
				.and_then(|el| el.text())
				.filter(|t| !t.is_empty())
			{
				if !description.is_empty() {
					description.push_str("\n\n");
				}
				description.push_str(&alt);
			}
			manga.description = Some(description).filter(|d| !d.is_empty());

			for label in info.select(".detail-meta__label").into_iter().flatten() {
				let name = label.text().unwrap_or_default();
				let Some(value) = label.next().and_then(|el| el.text()) else {
					continue;
				};
				if name.contains("المؤلف") {
					manga.authors = Some(vec![value]);
				} else if name.contains("الرسّام") || name.contains("الرسام") {
					manga.artists = Some(vec![value]);
				} else if name.contains("الحالة") {
					manga.status = if value.contains("مستمر") {
						MangaStatus::Ongoing
					} else if value.contains("مكتمل") {
						MangaStatus::Completed
					} else if value.contains("ملغا") || value.contains("ملغي") {
						MangaStatus::Cancelled
					} else if value.contains("متوقف") {
						MangaStatus::Hiatus
					} else {
						MangaStatus::Unknown
					};
				} else if name.contains("النوع") {
					manga.viewer = viewer_for(&value);
				}
			}
			manga.url = Some(url.clone());
		}

		if needs_chapters {
			let mut chapters = Vec::new();
			for volume in html.select(".volume-group").into_iter().flatten() {
				let volume_number = volume
					.select_first(".volume-group__label")
					.and_then(|el| el.text())
					.and_then(|label| first_number(&label));
				for el in volume.select(".chapter-item").into_iter().flatten() {
					let Some(href) = el.attr("abs:href") else {
						continue;
					};
					let title = el
						.select_first(".chapter-item__title")
						.and_then(|e| e.text())
						.filter(|t| !t.is_empty());
					let number = el
						.select_first(".chapter-item__number")
						.and_then(|e| e.text())
						.and_then(|n| first_number(&n))
						.or_else(|| title.as_deref().and_then(first_number));
					chapters.push(Chapter {
						key: strip_base(&href),
						title,
						chapter_number: number,
						volume_number,
						date_uploaded: el
							.select_first(".chapter-item__date")
							.and_then(|e| e.text())
							.and_then(|d| parse_relative_date(&d)),
						scanlators: el
							.select_first(".chapter-item__team")
							.and_then(|e| e.text())
							.filter(|t| !t.is_empty())
							.map(|t| vec![t]),
						url: Some(href),
						..Default::default()
					});
				}
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = get(&format!("{BASE_URL}{}", chapter.key))?;
		let script = html
			.select("script")
			.and_then(|scripts| {
				scripts
					.filter_map(|s| script_text(&s))
					.find(|s| s.contains("flavorReaderData"))
			})
			.ok_or_else(|| error!("Reader script not found"))?;
		let nonce = extract(&script, "cdnNonce: '", '\'')
			.ok_or_else(|| error!("Reader nonce not found"))?;
		let chapter_id = extract(&script, "chapterId: ", ',')
			.ok_or_else(|| error!("Chapter id not found"))?
			.trim();

		let mut form = aidoku::helpers::uri::QueryParameters::new();
		form.push("action", Some("flavor_cdn_get_key"));
		form.push("nonce", Some(nonce));
		form.push("chapter_id", Some(chapter_id));
		let key: CdnKeyResponse = Request::post(AJAX_URL)?
			.header("Content-Type", "application/x-www-form-urlencoded")
			.header("Referer", &format!("{BASE_URL}/"))
			.body(format!("{form}"))
			.json_owned()?;

		Ok(html
			.select("img.reader__page[data-cdn-url]")
			.map(|els| {
				els.filter_map(|img| {
					let url = img.attr("abs:data-cdn-url")?;
					let mut context = PageContext::new();
					context.insert("key".into(), key.data.key.clone());
					Some(Page {
						content: PageContent::url_context(url, context),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}
}

impl ListingProvider for StellarSaber {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.manga_list(&listing.id, page)
	}
}

impl ImageRequestProvider for StellarSaber {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl PageImageProcessor for StellarSaber {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(key) = context.as_ref().and_then(|c| c.get("key")) else {
			return Ok(response.image);
		};
		let data = decrypt_image(key, &response.image.data())?;
		Ok(ImageRef::new(&data))
	}
}

impl DeepLinkHandler for StellarSaber {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
		match segments.as_slice() {
			[kind, slug] => Ok(Some(DeepLinkResult::Manga {
				key: format!("/{kind}/{slug}/"),
			})),
			[kind, slug, chapter, ..] => Ok(Some(DeepLinkResult::Chapter {
				manga_key: format!("/{kind}/{slug}/"),
				key: format!("/{kind}/{slug}/{chapter}/"),
			})),
			_ => Ok(None),
		}
	}
}

register_source!(
	StellarSaber,
	ListingProvider,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
