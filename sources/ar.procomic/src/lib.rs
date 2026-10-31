#![no_std]
mod models;
mod nextjs;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, ImageResponse,
	Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext,
	PageImageProcessor, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		canvas::{Canvas, ImageRef, Rect},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};
use core::cell::RefCell;

use models::*;

const BASE_URL: &str = "https://procomic.pro";
const SITE_HOST: &str = "procomic.pro";
const MIRROR_HOST: &str = "procomic.net";
const LIST_LIMIT: i32 = 50;
const CHAPTER_LIMIT: i32 = 100;

#[derive(Default)]
struct Procomic {
	/// The api page each listing has reached, since pages with nothing readable get skipped.
	cursors: RefCell<Vec<(String, i32)>>,
	/// Stitching plans fetched for page images, until the image gets processed.
	plans: RefCell<Vec<(String, MapPlan)>>,
}

/// Sensitive series render the reader only with the site's safe browsing switched off.
fn request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.header("Cookie", "safe_browsing=off"))
}

fn post_json(url: &str, body: Vec<u8>) -> Result<Request> {
	Ok(Request::post(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.header("Cookie", "safe_browsing=off")
		.header("Content-Type", "application/json")
		.body(body))
}

fn content(id: &str) -> Result<Content> {
	request(&format!("{BASE_URL}/api/public/content/{id}"))?.json_owned()
}

fn fetch_plan(chapter_id: &str, body: &str) -> Result<MapPlan> {
	let response: MapPlanResponse = post_json(
		&format!("{BASE_URL}/chapter-map-proxy-plan/{chapter_id}"),
		body.as_bytes().to_vec(),
	)?
	.json_owned()?;
	Ok(response.data.map)
}

impl Procomic {
	fn listing(
		&self,
		page: i32,
		cursor_id: &str,
		path_and_query: String,
	) -> Result<MangaPageResult> {
		let mut cursor = if page == 1 {
			1
		} else {
			self.cursors
				.borrow()
				.iter()
				.find(|(id, _)| id == cursor_id)
				.map(|(_, c)| *c)
				.unwrap_or(page)
		};
		let separator = if path_and_query.contains('?') {
			'&'
		} else {
			'?'
		};
		let result = loop {
			let data: ContentPage = request(&format!(
				"{BASE_URL}{path_and_query}{separator}limit={LIST_LIMIT}&page={cursor}"
			))?
			.json_owned()?;
			cursor += 1;
			let has_next_page = data.meta.page < data.meta.pages;
			let entries: Vec<Manga> = data
				.data
				.into_iter()
				.filter(Content::is_readable)
				.map(Content::into_manga)
				.collect();
			if !entries.is_empty() || !has_next_page {
				break MangaPageResult {
					entries,
					has_next_page,
				};
			}
		};
		let mut cursors = self.cursors.borrow_mut();
		cursors.retain(|(id, _)| id != cursor_id);
		cursors.push((cursor_id.into(), cursor));
		Ok(result)
	}
}

impl Source for Procomic {
	fn new() -> Self {
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
			qs.push("search", Some(query.trim()));
			return self.listing(page, "search", format!("/api/public/series/search?{qs}"));
		}
		let mut sort = String::from("popular");
		for filter in filters {
			if let FilterValue::Select { id, value } = filter
				&& !value.is_empty()
			{
				match id.as_str() {
					"sort" => sort = value,
					"type" | "progress" | "year" => qs.push(&id, Some(&value)),
					_ => {}
				}
			}
		}
		qs.push("sort", Some(&sort));
		self.listing(page, "search", format!("/api/public/content?{qs}"))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// the chapter list lives under the series type, so the series is always needed
		let series = content(&manga.key)?;
		let kind = series.kind.clone();
		let slug = series.slug.clone();

		if needs_details {
			let chapters = manga.chapters.take();
			manga = Manga {
				chapters,
				..series.into_manga_details()
			};
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let url = format!(
				"{BASE_URL}/api/public/{kind}/{}/chapters?limit={CHAPTER_LIMIT}&order=desc",
				manga.key
			);
			let first: ChapterPage = request(&format!("{url}&page=1"))?.json_owned()?;
			let pages = (first.total + CHAPTER_LIMIT - 1) / CHAPTER_LIMIT;
			let mut lists = Vec::from([first]);
			if pages > 1 {
				let requests = (2..=pages)
					.map(|page| request(&format!("{url}&page={page}")))
					.collect::<Result<Vec<_>>>()?;
				for response in Request::send_all(requests) {
					lists.push(response?.get_json_owned()?);
				}
			}
			manga.chapters = Some(
				lists
					.into_iter()
					.flat_map(|list| list.data)
					.filter(|c| c.is_arabic() && !c.is_locked())
					.map(|c| c.into_chapter(&slug))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = request(&format!("{BASE_URL}{}", chapter.key))?.html()?;
		let payload = nextjs::payload(&html);
		let reader: Reader = match nextjs::find(&payload, &["chapterId", "appImages"]) {
			Some(reader) => reader,
			None => {
				// series the site moved to its other domain render a bounce page instead
				let moved: Moved = nextjs::find(&payload, &["targetUrl"])
					.ok_or_else(|| error!("Chapter reader not found"))?;
				let html = request(&moved.target_url)?.html()?;
				nextjs::find(&nextjs::payload(&html), &["chapterId", "appImages"])
					.ok_or_else(|| error!("Chapter reader not found"))?
			}
		};

		let mut pages: Vec<Page> = reader
			.app_images
			.into_iter()
			.filter_map(|image| image.mobile.or(image.desktop))
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect();

		if let Some(deferred) = reader.deferred_media {
			let mut qs = QueryParameters::new();
			qs.push("token", Some(&deferred.token));
			let response: DeferredPage = request(&format!(
				"{BASE_URL}/chapter-deferred-media/{}?{qs}",
				reader.chapter_id
			))?
			.header("Referer", &format!("{BASE_URL}{}", chapter.key))
			.json_owned()?;
			let Some(data) = response.data else {
				bail!(
					"{}; open the chapter in the web view first",
					response
						.error
						.as_deref()
						.unwrap_or("The rest of the chapter didn't load")
				);
			};

			// the site signs each of these right before loading it
			for image in data.images {
				let mut context = PageContext::new();
				context.insert("sign".into(), format!("{BASE_URL}{}", chapter.key));
				pages.push(Page {
					content: PageContent::url_context(image, context),
					..Default::default()
				});
			}

			// these pages are put together from shuffled pieces
			let chapter_id = reader.chapter_id.to_string();
			let offset = pages.len();
			for (index, entry) in data.maps.into_iter().enumerate() {
				let body = MapPlanBody {
					token: entry.token,
					method: entry.method,
					cdn_path: reader.cdn_path.clone(),
					page_index: offset + index,
				};
				let body = serde_json::to_string(&body).map_err(|_| error!("Bad map plan"))?;
				let mut context = PageContext::new();
				context.insert("chapter".into(), chapter_id.clone());
				context.insert("map".into(), body);
				pages.push(Page {
					content: PageContent::url_context(
						format!(
							"{BASE_URL}/chapter-map-proxy-plan/{chapter_id}#{}",
							offset + index
						),
						context,
					),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl ListingProvider for Procomic {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let path = format!("/api/public/content?sort={}", listing.id);
		self.listing(page, &listing.id, path)
	}
}

impl ImageRequestProvider for Procomic {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		let Some(context) = context else {
			return request(&url);
		};
		if let Some(referer) = context.get("sign") {
			let body = serde_json::to_vec(&SignRequest { url: &url })
				.map_err(|_| error!("Bad sign request"))?;
			let signed: SignedImage = post_json(&format!("{BASE_URL}/api/cdn-image/sign"), body)?
				.header("Referer", referer)
				.json_owned()?;
			let mut qs = QueryParameters::new();
			qs.push("url", Some(&url));
			qs.push("token", Some(&signed.token));
			qs.push("expires", Some(&signed.expires.to_string()));
			return request(&format!("{BASE_URL}/api/cdn-image?{qs}"));
		}
		if let (Some(chapter_id), Some(body)) = (context.get("chapter"), context.get("map")) {
			// the first piece is loaded as the page image, the rest while stitching
			let plan = fetch_plan(chapter_id, body)?;
			let first = plan
				.order
				.first()
				.and_then(|i| plan.pieces.get(*i))
				.cloned()
				.ok_or_else(|| error!("Empty map plan"))?;
			let mut plans = self.plans.borrow_mut();
			plans.retain(|(key, _)| key != body);
			plans.push((body.clone(), plan));
			return request(&first);
		}
		request(&url)
	}
}

impl PageImageProcessor for Procomic {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let (Some(chapter_id), Some(body)) = (context.get("chapter"), context.get("map")) else {
			return Ok(response.image);
		};
		let cached = {
			let mut plans = self.plans.borrow_mut();
			let index = plans.iter().position(|(key, _)| key == body);
			index.map(|i| plans.remove(i).1)
		};
		let (plan, first_loaded) = match cached {
			Some(plan) => (plan, true),
			None => (fetch_plan(chapter_id, body)?, false),
		};
		let (Some(&width), Some(&height)) = (plan.dim.first(), plan.dim.get(1)) else {
			bail!("Bad map plan size");
		};

		let mut canvas = Canvas::new(width, height);
		for (index, rect) in plan.rects.iter().enumerate() {
			let destination = Rect::new(rect.left, rect.top, rect.width, rect.height);
			if index == 0 && first_loaded {
				canvas.draw_image(&response.image, destination);
				continue;
			}
			let url = plan
				.order
				.get(index)
				.and_then(|i| plan.pieces.get(*i))
				.ok_or_else(|| error!("Missing map piece"))?;
			let piece = request(url)?.image()?;
			canvas.draw_image(&piece, destination);
		}
		Ok(canvas.get_image())
	}
}

impl DeepLinkHandler for Procomic {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(rest) = url
			.strip_prefix("https://")
			.or_else(|| url.strip_prefix("http://"))
		else {
			return Ok(None);
		};
		let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
		let host = host.strip_prefix("www.").unwrap_or(host);
		if host != SITE_HOST && host != MIRROR_HOST {
			return Ok(None);
		}
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let segments: Vec<&str> = path
			.split('/')
			.filter(|s| !s.is_empty() && *s != "ar" && *s != "en")
			.collect();
		let Some(last) = segments.last() else {
			return Ok(None);
		};
		let Some(id) = last
			.rsplit_once('-')
			.and_then(|(_, id)| id.parse::<i64>().ok())
		else {
			return Ok(None);
		};
		if segments.first() == Some(&"chapter") {
			let chapter: ChapterRef =
				request(&format!("{BASE_URL}/api/public/chapters/{id}"))?.json_owned()?;
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: chapter.content_id.to_string(),
				key: format!("/ar/chapter/{last}"),
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga {
				key: id.to_string(),
			}))
		}
	}
}

register_source!(
	Procomic,
	ListingProvider,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
