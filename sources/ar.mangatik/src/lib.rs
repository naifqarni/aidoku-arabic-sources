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
		std::parse_date,
	},
	prelude::*,
};
use serde::Deserialize;

const BASE_URL: &str = "https://mangatik.com";

struct MangaTik;

#[derive(Deserialize)]
struct MangaJsonLd {
	name: String,
	description: Option<String>,
	image: Option<ImageObject>,
	genre: Option<Vec<String>>,
	author: Option<Author>,
}

#[derive(Deserialize)]
struct ImageObject {
	url: Option<String>,
}

#[derive(Deserialize)]
struct Author {
	name: Option<String>,
}

fn get(url: &str) -> Result<Document> {
	Ok(Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.html()?)
}

fn strip_base(url: String) -> String {
	url.strip_prefix(BASE_URL).map(String::from).unwrap_or(url)
}

fn img_src(el: &Element) -> Option<String> {
	el.attr("abs:data-src")
		.filter(|s| !s.is_empty())
		.or_else(|| el.attr("abs:src"))
		.filter(|s| !s.is_empty())
}

fn parse_manga_list(html: &Document) -> MangaPageResult {
	let entries = html
		.select("article.manga-card")
		.map(|els| {
			els.filter_map(|el| {
				let link = el.select_first(".manga-title > a")?;
				Some(Manga {
					key: strip_base(link.attr("abs:href")?),
					title: link.text()?.trim().into(),
					cover: el
						.select_first("img.manga-poster")
						.and_then(|i| img_src(&i)),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default();
	let has_next_page = html
		.select("a.btn")
		.map(|mut els| {
			els.any(|el| {
				el.text().is_some_and(|t| t.contains("التالي"))
					&& el.attr("href").is_some_and(|h| h.starts_with("?page="))
			})
		})
		.unwrap_or(false);
	MangaPageResult {
		entries,
		has_next_page,
	}
}

impl Source for MangaTik {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		if let Some(query) = query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
			qs.push("q", Some(query));
		}
		let mut sort = String::from("popular");
		for filter in filters {
			if let FilterValue::Select { id, value } = filter
				&& !value.is_empty()
			{
				if id == "sort" {
					sort = value;
				} else {
					qs.push(&id, Some(&value));
				}
			}
		}
		qs.push("sort", Some(&sort));
		qs.push("page", Some(&page.to_string()));
		let html = get(&format!("{BASE_URL}/manga?{qs}"))?;
		Ok(parse_manga_list(&html))
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
			let data = html
				.select_first("script[type=\"application/ld+json\"]")
				.and_then(|el| el.data().filter(|s| !s.is_empty()).or_else(|| el.html()))
				.and_then(|s| serde_json::from_str::<MangaJsonLd>(&s).ok())
				.ok_or_else(|| error!("Unable to find manga data"))?;
			manga.title = data.name.trim().into();
			manga.description = data
				.description
				.map(|s| s.trim().into())
				.filter(|s: &String| !s.is_empty());
			manga.cover = data.image.and_then(|i| i.url).or(manga.cover);
			manga.tags = data.genre.filter(|g| !g.is_empty());
			manga.authors = data
				.author
				.and_then(|a| a.name)
				.map(|s| String::from(s.trim()))
				.filter(|s| {
					!s.is_empty()
						&& !s.eq_ignore_ascii_case("unknown")
						&& !s.eq_ignore_ascii_case("updating")
				})
				.map(|s| vec![s]);
			let badges: Vec<Element> = html
				.select("span.badge")
				.map(|els| els.collect())
				.unwrap_or_default();
			manga.status = badges
				.iter()
				.find_map(|el| match el.text()?.trim() {
					"مستمر" => Some(MangaStatus::Ongoing),
					"مكتمل" => Some(MangaStatus::Completed),
					"متوقف" | "متوقف مؤقتاً" => Some(MangaStatus::Hiatus),
					_ => None,
				})
				.unwrap_or_default();
			manga.viewer = badges
				.iter()
				.find_map(|el| {
					if el.has_class("badge-type-manga") {
						Some(Viewer::RightToLeft)
					} else if el.has_class("badge-type-manhwa") || el.has_class("badge-type-manhua")
					{
						Some(Viewer::Webtoon)
					} else {
						None
					}
				})
				.unwrap_or_default();
			let suggestive = manga.tags.as_ref().is_some_and(|tags| {
				tags.iter()
					.any(|t| t.eq_ignore_ascii_case("ecchi") || t == "إيتشي" || t == "ايتشي")
			});
			manga.content_rating = if suggestive {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.url = Some(url.clone());
		}

		if needs_chapters {
			manga.chapters = html.select("a.chapter-item-card").map(|els| {
				els.filter_map(|el| {
					let href = el.attr("abs:href")?;
					let number = el.attr("data-chapter").unwrap_or_default();
					let spans: Vec<String> = el
						.select("span")
						.map(|s| {
							s.filter_map(|s| s.text().map(|t| String::from(t.trim())))
								.collect()
						})
						.unwrap_or_default();
					let name = spans
						.iter()
						.find(|s| s.starts_with("الفصل"))
						.cloned()
						.unwrap_or_else(|| format!("الفصل {number}"));
					let chapter_number = number.parse::<f32>().ok();
					// drop the title when it only repeats the chapter number
					let title = name
						.strip_prefix("الفصل")
						.map(str::trim)
						.filter(|rest| {
							rest.parse::<f32>().ok() != chapter_number || chapter_number.is_none()
						})
						.map(|_| name.clone());
					Some(Chapter {
						key: strip_base(href.clone()),
						title,
						chapter_number,
						date_uploaded: spans.last().and_then(|d| parse_date(d, "yyyy-MM-dd")),
						url: Some(href),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = get(&format!("{BASE_URL}{}", chapter.key))?;
		Ok(html
			.select("img.webtoon-image")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(el.attr("abs:src").filter(|s| !s.is_empty())?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}
}

impl ListingProvider for MangaTik {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let html = get(&format!("{BASE_URL}/latest?page={page}"))?;
		Ok(parse_manga_list(&html))
	}
}

impl ImageRequestProvider for MangaTik {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for MangaTik {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.mangatik.com"))
		else {
			return Ok(None);
		};
		let mut segments = path.split(['?', '#']).next().unwrap_or_default().split('/');
		segments.next(); // leading empty segment
		match (segments.next(), segments.next(), segments.next()) {
			(Some("manga"), Some(slug), None | Some("")) if !slug.is_empty() => {
				Ok(Some(DeepLinkResult::Manga {
					key: format!("/manga/{slug}"),
				}))
			}
			(Some("manga"), Some(slug), Some(chapter)) if !slug.is_empty() => {
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: format!("/manga/{slug}"),
					key: format!("/manga/{slug}/{chapter}"),
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(
	MangaTik,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
