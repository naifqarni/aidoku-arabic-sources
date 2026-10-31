use crate::{
	Params, helpers,
	models::{Entry, EntryResponse, FeedResponse},
};
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, Listing, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::encode_uri_component,
	imports::{
		html::{Document, Html},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};

const MANGA_PAGE_SIZE: usize = 20;
const CHAPTER_PAGE_SIZE: usize = 150;

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	/// Base of the label feeds, e.g. `https://x.blogspot.com/feeds/posts/default/-/`.
	fn feed_url(&self, params: &Params) -> String {
		format!("{}/feeds/posts/default/-/", params.base_url)
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut url = self.feed_url(params) + &helpers::encode_label(&params.manga_category);
		let query = query.filter(|q| !q.trim().is_empty());

		// filters are blogger labels and can't be combined with a text search
		if query.is_none() {
			for filter in filters {
				match filter {
					FilterValue::Select { value, .. } if !value.is_empty() => {
						url.push('/');
						url.push_str(&helpers::encode_label(&value));
					}
					FilterValue::MultiSelect { included, .. } => {
						for value in included {
							url.push('/');
							url.push_str(&helpers::encode_label(&value));
						}
					}
					_ => {}
				}
			}
		}

		let start_index = MANGA_PAGE_SIZE * (page.max(1) as usize - 1) + 1;
		url = format!(
			"{url}?alt=json&max-results={}&start-index={start_index}",
			MANGA_PAGE_SIZE + 1
		);
		match query {
			Some(query) => {
				let q = format!("label:{} {}", params.manga_category, query.trim());
				url.push_str("&q=");
				url.push_str(&encode_uri_component(q));
			}
			None => url.push_str("&orderby=published"),
		}

		self.parse_manga_feed(params, &url)
	}

	fn parse_manga_feed(&self, params: &Params, url: &str) -> Result<MangaPageResult> {
		let mut entries = Request::get(url)?.json_owned::<FeedResponse>()?.feed.entry;
		let has_next_page = entries.len() > MANGA_PAGE_SIZE;
		entries.truncate(MANGA_PAGE_SIZE);
		Ok(MangaPageResult {
			entries: entries
				.into_iter()
				.filter(|e| {
					e.has_category(&params.manga_category)
						&& !params.excluded_categories.iter().any(|c| e.has_category(c))
				})
				.filter_map(Entry::into_manga)
				.collect(),
			has_next_page,
		})
	}

	/// The "popular" listing scrapes the home page's popular posts widget.
	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		if listing.id != "popular" {
			return self.get_search_manga_list(params, None, page, Vec::new());
		}
		let html = Request::get(params.base_url.as_ref())?.html()?;
		let entries = html
			.select("div.PopularPosts figure")
			.map(|els| {
				els.filter_map(|el| {
					let link = el.select_first("figcaption > a")?;
					Some(Manga {
						key: helpers::url_path(&link.attr("abs:href")?).into(),
						title: link.text()?,
						cover: el.select_first("img").and_then(|img| img.attr("abs:src")),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();
		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			self.parse_manga_details(params, &html, &mut manga)?;
			manga.url = Some(url);
			send_partial_result(&manga);
		}

		if needs_chapters {
			let feed_url = self.chapter_feed_url(params, &html, &manga.title);
			manga.chapters = Some(self.get_chapter_list(params, &feed_url, &html)?);
		}

		Ok(manga)
	}

	fn parse_manga_details(
		&self,
		params: &Params,
		html: &Document,
		manga: &mut Manga,
	) -> Result<()> {
		let details = html
			.select_first(&params.details_selector)
			.ok_or_else(|| error!("Unable to find details"))?;

		if manga.title.is_empty() {
			manga.title = html
				.select_first("meta[property='og:title']")
				.and_then(|el| el.attr("content"))
				.map(|title| title.split('-').next().unwrap_or_default().trim().into())
				.unwrap_or_default();
		}
		manga.cover = details
			.select_first("img")
			.and_then(|img| img.attr("abs:src"))
			.or(manga.cover.take());

		let synopsis = details
			.select("#synopsis")
			.and_then(|els| els.text())
			.unwrap_or_default();
		let alt_names = details
			.select_first("header > p")
			.and_then(|el| el.text())
			.filter(|s| !s.is_empty());
		let description = match alt_names {
			Some(names) => format!("{synopsis}\n\nأسماء أخرى: {names}"),
			None => synopsis,
		};
		manga.description = Some(description.trim().into()).filter(|s: &String| !s.is_empty());

		let tags: Vec<String> = details
			.select(&params.details_genres_selector)
			.map(|els| els.filter_map(|el| el.text()).collect())
			.unwrap_or_default();

		let text_of = |selector: &str| {
			details
				.select_first(selector)
				.and_then(|el| el.text())
				.filter(|s| !s.is_empty())
		};
		let mut author = text_of("span#author");
		let mut artist = text_of("span#artist");
		let mut status = text_of("span[data-status]")
			.map(|s| self.parse_status(&s))
			.unwrap_or_default();
		let mut kind = None;

		if let Some(rows) = details.select(&params.details_info_selector) {
			for row in rows {
				let label = row
					.own_text()
					.map(|s| s.trim().to_string())
					.filter(|s| !s.is_empty())
					.or_else(|| {
						row.select_first(&params.details_info_title_selector)
							.and_then(|el| el.text())
					})
					.unwrap_or_default();
				let Some(value) = row
					.select(&params.details_info_description_selector)
					.and_then(|els| els.text())
					.filter(|s| !s.is_empty())
				else {
					continue;
				};
				let is = |words: &[&str]| words.iter().any(|w| label.contains(w));
				if status == MangaStatus::Unknown && is(&["Status", "Estado", "الحالة"]) {
					status = self.parse_status(&value);
				} else if author.is_none()
					&& is(&["Author", "Autor", "Mangaka", "الكاتب", "المؤلف"])
				{
					author = Some(value);
				} else if artist.is_none() && is(&["Artist", "Artista", "الرسام"]) {
					artist = Some(value);
				} else if kind.is_none() && is(&["Type", "النوع"]) {
					kind = Some(value);
				}
			}
		}

		// status and type are often plain genre labels
		if status == MangaStatus::Unknown {
			status = tags
				.iter()
				.map(|tag| self.parse_status(tag))
				.find(|s| *s != MangaStatus::Unknown)
				.unwrap_or_default();
		}
		manga.viewer = kind
			.iter()
			.chain(tags.iter())
			.map(|s| match s.trim().to_lowercase().as_str() {
				"manhwa" | "manhua" | "webtoon" | "مانهوا" | "مانها" | "ويبتون" => {
					Viewer::Webtoon
				}
				"manga" | "مانجا" => Viewer::RightToLeft,
				"comic" | "comics" | "كوميكس" | "كوميك" => Viewer::LeftToRight,
				_ => Viewer::Unknown,
			})
			.find(|v| *v != Viewer::Unknown)
			.unwrap_or_default();
		manga.content_rating = if tags.iter().any(|t| {
			matches!(
				t.to_lowercase().as_str(),
				"adult" | "smut" | "hentai" | "بالغ" | "+18" | "18+"
			)
		}) {
			ContentRating::NSFW
		} else if tags
			.iter()
			.any(|t| matches!(t.to_lowercase().as_str(), "ecchi" | "إيتشي" | "ايتشي"))
		{
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};
		manga.status = status;
		manga.authors = author.map(|a| vec![a]);
		manga.artists = artist.map(|a| vec![a]);
		manga.tags = Some(tags).filter(|t| !t.is_empty());
		Ok(())
	}

	/// Label to look up chapters by when the page has no chapter feed script.
	fn chapter_label(&self, _params: &Params, _html: &Document, title: &str) -> String {
		title.into()
	}

	/// The JSON feed holding a series' chapters, found from the chapter list script.
	fn chapter_feed_url(&self, params: &Params, html: &Document, title: &str) -> String {
		let feed_url = self.feed_url(params);
		let script = |selector: &str| {
			html.select_first(selector)
				.and_then(|el| el.data().filter(|s| !s.is_empty()).or_else(|| el.html()))
		};
		let category = helpers::encode_label(&params.chapter_category);

		if let Some(script) = script("#clwd > script") {
			if let Some(label) = helpers::extract_quoted_after(&script, "clwd.run(") {
				return format!(
					"{feed_url}{category}/{}?alt=json",
					helpers::encode_label(&label)
				);
			}
		} else if let Some(src) = html
			.select_first("#myUL > script")
			.and_then(|el| el.attr("src"))
			.and_then(|src| src.split('?').next().map(String::from))
			.filter(|src| !src.is_empty())
		{
			// older themes load the feed of the series label directly
			let base = if src.starts_with('/') {
				params.base_url.as_ref()
			} else {
				""
			};
			return format!("{base}{src}?alt=json");
		} else if let Some(label) =
			script("#latest > script").and_then(|s| helpers::extract_quoted_after(&s, "label"))
		{
			return format!("{feed_url}{}?alt=json", helpers::encode_label(&label));
		}

		let label = self.chapter_label(params, html, title);
		format!(
			"{feed_url}{category}/{}?alt=json",
			helpers::encode_label(&label)
		)
	}

	fn get_chapter_list(
		&self,
		params: &Params,
		feed_url: &str,
		_html: &Document,
	) -> Result<Vec<Chapter>> {
		fetch_chapter_list(params, feed_url)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", params.base_url, chapter.key);
		let body = Request::get(&url)?.string()?;
		let html = Html::parse_with_url(&body, &url)?;

		let selector = params
			.page_list_selector
			.split(',')
			.map(|s| format!("{} img[src]", s.trim()))
			.collect::<Vec<_>>()
			.join(", ");
		let mut images = image_urls(&html, &selector);

		// some themes only load the post body with javascript
		if images.is_empty()
			&& let Some(id) = helpers::find_post_id(&body)
		{
			let feed_url = format!("{}/feeds/posts/default/{id}?alt=json", params.base_url);
			if let Some(content) = Request::get(feed_url)?
				.json_owned::<EntryResponse>()?
				.entry
				.content
			{
				let content = Html::parse_fragment_with_url(content.t, &url)?;
				images = image_urls(&content, "img[src]");
			}
		}

		if images.is_empty() {
			bail!("No pages found");
		}
		Ok(images
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let host = |url: &str| {
			url.split_once("://").map(|(_, rest)| {
				rest.split('/')
					.next()
					.unwrap_or_default()
					.trim_start_matches("www.")
					.to_string()
			})
		};
		if host(&url) != host(&params.base_url) || !url.contains(".html") {
			return Ok(None);
		}
		let key: String = helpers::url_path(&url).into();
		let html = Request::get(format!("{}{key}", params.base_url))?.html()?;
		Ok(html
			.select_first(&params.details_selector)
			.map(|_| DeepLinkResult::Manga { key }))
	}

	fn parse_status(&self, status: &str) -> MangaStatus {
		match status.trim().to_lowercase().as_str() {
			"ongoing" | "en curso" | "en emisión" | "em lançamento" | "activo" | "ativo"
			| "lançando" | "مستمر" | "مستمرة" => MangaStatus::Ongoing,
			"completed" | "completo" | "finalizado" | "مكتمل" | "مكتملة" => {
				MangaStatus::Completed
			}
			"hiatus" | "pausado" | "متوقف" | "متوقفة" => MangaStatus::Hiatus,
			"cancelled" | "dropped" | "dropado" | "abandonado" | "cancelado" | "ملغي" | "ملغية" => {
				MangaStatus::Cancelled
			}
			_ => MangaStatus::Unknown,
		}
	}
}

/// Fetches every chapter in a chapter feed, paging through it in parallel.
pub fn fetch_chapter_list(params: &Params, feed_url: &str) -> Result<Vec<Chapter>> {
	let page_url = |start: usize, count: usize| {
		format!("{feed_url}&orderby=published&start-index={start}&max-results={count}")
	};

	let feed = Request::get(page_url(1, CHAPTER_PAGE_SIZE))?
		.json_owned::<FeedResponse>()?
		.feed;
	let total = feed.total_results().unwrap_or(0);
	// the server may return fewer entries than requested
	let per_page = feed
		.items_per_page()
		.filter(|n| *n > 0)
		.unwrap_or(CHAPTER_PAGE_SIZE);
	let mut entries = feed.entry;

	let requests = (entries.len()..total)
		.step_by(per_page)
		.map(|start| Request::get(page_url(start + 1, per_page)))
		.collect::<core::result::Result<Vec<_>, _>>()?;
	for response in Request::send_all(requests) {
		entries.extend(response?.get_json_owned::<FeedResponse>()?.feed.entry);
	}

	Ok(entries
		.into_iter()
		.filter(|e| e.has_category(&params.chapter_category))
		.filter_map(|e| e.into_chapter(params.prefer_chapter_updated_date))
		.collect())
}

fn image_urls(html: &Document, selector: &str) -> Vec<String> {
	html.select(selector)
		.map(|els| {
			els.filter_map(|img| img.attr("abs:src"))
				.filter(|src| src.starts_with("http"))
				.collect()
		})
		.unwrap_or_default()
}
