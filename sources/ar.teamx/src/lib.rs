#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext,
	Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};

const BASE_URL: &str = "https://olympustaff.com";

struct TeamX;

fn get(url: &str) -> Result<Document> {
	Ok(Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.html()?)
}

fn page_suffix(page: i32) -> String {
	if page > 1 {
		format!("?page={page}")
	} else {
		String::new()
	}
}

fn strip_base(url: String) -> String {
	url.strip_prefix(BASE_URL).map(String::from).unwrap_or(url)
}

/// Covers in lists point to a thumbnail; dropping the prefix gives the full image.
fn full_cover(url: String) -> String {
	url.replace("thumbnail_", "")
}

fn img_src(el: &Element) -> Option<String> {
	el.attr("abs:data-src")
		.filter(|s| !s.is_empty())
		.or_else(|| el.attr("abs:src"))
}

fn has_next_page(html: &Document) -> bool {
	html.select_first("a[rel=next]").is_some()
}

fn parse_series_grid(html: &Document) -> MangaPageResult {
	let entries = html
		.select("div.listupd div.bsx")
		.map(|els| {
			els.filter_map(|el| {
				let link = el.select_first("a")?;
				Some(Manga {
					key: strip_base(link.attr("abs:href")?),
					title: link.attr("title")?,
					cover: el.select_first("img").and_then(|img| img_src(&img)),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default();
	MangaPageResult {
		entries,
		has_next_page: has_next_page(html),
	}
}

fn info_value(html: &Document, label: &str) -> Option<String> {
	html.select(".full-list-info")?
		.find_map(|info| {
			let fields = info.select("small")?;
			let name = fields.get(0)?.text()?;
			if name.contains(label) {
				fields.get(1)?.text()
			} else {
				None
			}
		})
		.map(|s| s.trim().into())
		.filter(|s: &String| !s.is_empty() && s != "غير معروف")
}

fn parse_chapters(html: &Document) -> Vec<Chapter> {
	html.select("div.chapter-card")
		.map(|els| {
			els.filter_map(|el| {
				// paid chapters can't be read
				if el.select_first("span.locked").is_some() {
					return None;
				}
				let url = el.select_first("a")?.attr("abs:href")?;
				let number = el.attr("data-number").unwrap_or_default();
				let title = el
					.select_first("div.chapter-info div.chapter-title")
					.and_then(|el| el.text())
					.map(|s| String::from(s.trim()))
					.filter(|s| {
						!s.is_empty()
							&& *s != number && *s != format!("الفصل {number}")
							&& *s != format!("الفصل رقم {number}")
					});
				Some(Chapter {
					key: strip_base(url.clone()),
					title,
					chapter_number: number.parse().ok(),
					// unix timestamp in seconds
					date_uploaded: el.attr("data-date").and_then(|s| s.parse().ok()),
					url: Some(url),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default()
}

impl Source for TeamX {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// the search endpoint ignores filters and isn't paginated
		if let Some(query) = query.filter(|q| !q.is_empty()) {
			if page > 1 {
				return Ok(MangaPageResult::default());
			}
			let mut qs = QueryParameters::new();
			qs.push("keyword", Some(&query));
			let html = get(&format!("{BASE_URL}/search?{qs}"))?;
			let entries = html
				.select("div.tx-grid a.tx-card")
				.map(|els| {
					els.filter_map(|el| {
						Some(Manga {
							key: strip_base(el.attr("abs:href")?),
							title: el.select_first("h3")?.text()?,
							cover: el
								.select_first("img")
								.and_then(|img| img_src(&img))
								.map(full_cover),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default();
			return Ok(MangaPageResult {
				entries,
				has_next_page: false,
			});
		}

		let mut qs = QueryParameters::new();
		for filter in filters {
			if let FilterValue::Select { id, value } = filter
				&& !value.is_empty()
			{
				qs.push(&id, Some(&value));
			}
		}
		if page > 1 {
			qs.push("page", Some(&page.to_string()));
		}
		let html = get(&format!("{BASE_URL}/series?{qs}"))?;
		Ok(parse_series_grid(&html))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = get(&url)?;

		if needs_details {
			if let Some(title) = html
				.select_first("div.author-info-title h1, div.author-info-title h6")
				.and_then(|el| el.text())
			{
				manga.title = title.trim().into();
			}
			manga.cover = html
				.select_first("div.text-right img")
				.and_then(|img| img_src(&img))
				.or(manga.cover);
			manga.description = html
				.select_first("div.review-content")
				.and_then(|el| el.text())
				.map(|s| s.trim().into());
			manga.tags = html.select("div.review-author-info a").map(|els| {
				els.filter_map(|el| el.text().map(|s| s.trim().into()))
					.collect()
			});
			manga.authors = info_value(&html, "الرسام").map(|s| vec![s]);
			manga.status = match info_value(&html, "الحالة").as_deref() {
				Some("مستمرة") | Some("قادم قريبًا") => MangaStatus::Ongoing,
				Some("مكتمل") => MangaStatus::Completed,
				Some("متوقف") => MangaStatus::Hiatus,
				Some("متروك") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			};
			manga.viewer = match info_value(&html, "النوع") {
				Some(kind) if kind.contains("مانجا") => Viewer::RightToLeft,
				Some(kind)
					if kind.contains("مانهوا")
						|| kind.contains("مانها")
						|| kind.contains("ويب") =>
				{
					Viewer::Webtoon
				}
				_ => Viewer::Unknown,
			};
			manga.content_rating = ContentRating::Safe;
			manga.url = Some(url.clone());

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters = parse_chapters(&html);
			let last_page = html
				.select("ul.pagination a.page-link")
				.map(|els| {
					els.filter_map(|el| el.text()?.trim().parse::<i32>().ok())
						.max()
						.unwrap_or(1)
				})
				.unwrap_or(1);
			for page in 2..=last_page {
				let html = get(&format!("{url}{}", page_suffix(page)))?;
				chapters.extend(parse_chapters(&html));
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = get(&format!("{BASE_URL}{}", chapter.key))?;
		Ok(html
			.select("div.image_list canvas[data-src], div.image_list img[src]")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(img_src(&el)?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}
}

impl ListingProvider for TeamX {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let html = get(&format!("{BASE_URL}{}", page_suffix(page)))?;
		let entries = html
			.select("div.last-chapter div.box")
			.map(|els| {
				els.filter_map(|el| {
					let link = el.select_first("div.info a")?;
					Some(Manga {
						key: strip_base(link.attr("abs:href")?),
						title: link.select_first("h3")?.text()?,
						cover: el
							.select_first("div.imgu img")
							.and_then(|img| img_src(&img))
							.map(full_cover),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();
		Ok(MangaPageResult {
			entries,
			has_next_page: has_next_page(&html),
		})
	}
}

impl ImageRequestProvider for TeamX {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for TeamX {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let mut segments = path.split(['?', '#']).next().unwrap_or_default().split('/');
		segments.next(); // leading empty segment
		match (segments.next(), segments.next(), segments.next()) {
			(Some("series"), Some(slug), None | Some("")) if !slug.is_empty() => {
				Ok(Some(DeepLinkResult::Manga {
					key: format!("/series/{slug}"),
				}))
			}
			(Some("series"), Some(slug), Some(chapter)) => Ok(Some(DeepLinkResult::Chapter {
				manga_key: format!("/series/{slug}"),
				key: format!("/series/{slug}/{chapter}"),
			})),
			_ => Ok(None),
		}
	}
}

register_source!(
	TeamX,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
