#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap,
	ImageRequestProvider, ImageResponse, Listing, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, PageContext, PageImageProcessor, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		canvas::ImageRef,
		defaults::{DefaultValue, defaults_get, defaults_set},
		html::{Document, Element},
		net::Request,
	},
	prelude::*,
};
use serde::de::DeserializeOwned;

mod crypto;
mod models;
mod painter;
mod seeds;

use models::*;

pub const BASE_URL: &str = "https://mangatek.com";
const API_BASE: &str = "https://api.mangatek.com";
const SEEDS_KEY: &str = "readerSeeds";
const OVERLAYS_KEY: &str = "overlays";
const SIZE_KEY: &str = "overlaySize";

struct MangaTek;

fn request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
}

fn strip_base(url: String) -> String {
	url.strip_prefix(BASE_URL).map(String::from).unwrap_or(url)
}

fn img_attr(el: &Element) -> Option<String> {
	[
		"abs:data-src",
		"abs:data-url",
		"abs:data-zoom-src",
		"abs:data-lazy-src",
		"abs:data-cfsrc",
		"abs:src",
	]
	.iter()
	.find_map(|attr| {
		if el.has_attr(&attr[4..]) {
			el.attr(attr).filter(|s| !s.is_empty())
		} else {
			None
		}
	})
}

fn parse_manga_list(html: &Document) -> MangaPageResult {
	let entries = html
		.select("a.manga-card")
		.map(|els| {
			els.filter_map(|el| {
				let title = el.select_first("h3").and_then(|h| {
					h.attr("title")
						.filter(|t| !t.is_empty())
						.or_else(|| h.text())
				})?;
				Some(Manga {
					key: strip_base(el.attr("abs:href")?),
					title: title.trim().into(),
					cover: el.select_first("img").and_then(|img| img_attr(&img)),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default();
	let has_next_page = html
		.select_first("nav a[aria-disabled=false] .fa-chevron-left")
		.is_some();
	MangaPageResult {
		entries,
		has_next_page,
	}
}

/// Reads the props of the first astro island that has `key`.
fn astro_prop<T: DeserializeOwned>(html: &Document, key: &str) -> Result<T> {
	let needle = format!("\"{key}\"");
	html.select("astro-island[props]")
		.and_then(|els| {
			els.filter_map(|el| el.attr("props"))
				.filter(|props| props.contains(&needle))
				.find_map(|props| {
					let value = serde_json::from_str(&props).ok()?;
					serde_json::from_value(unwrap_astro(value)).ok()
				})
		})
		.ok_or_else(|| error!("Unable to find prop with {key}"))
}

fn parse_iso_date(date: &str) -> Option<i64> {
	let (day, time) = date.split_once('T')?;
	let mut d = day.split('-').map(|p| p.parse::<i64>().ok());
	let (y, m, dd) = (d.next()??, d.next()??, d.next()??);
	let time = time.trim_end_matches('Z');
	let time = time.split(['.', '+']).next()?;
	let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
	let (h, mi, s) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
	// days from civil (Howard Hinnant)
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = y - era * 400;
	let mp = (m + 9) % 12;
	let doy = (153 * mp + 2) / 5 + dd - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	let days = era * 146097 + doe - 719468;
	Some(days * 86400 + h * 3600 + mi * 60 + s)
}

impl Source for MangaTek {
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
			qs.push("search", Some(query));
		}
		qs.push("page", Some(&page.to_string()));
		let mut sorted = false;
		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				sorted |= id == "sort";
				if !value.is_empty() {
					qs.push(&id, Some(&value));
				}
			}
		}
		// popular is sorted by views, like the first sort option
		if !sorted {
			qs.push("sort", Some("views"));
		}
		let html = request(&format!("{BASE_URL}/manga-list?{qs}"))?.html()?;
		Ok(parse_manga_list(&html))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = request(&url)?.html()?;
		let data: MangaDto = astro_prop(&html, "manga")?;
		let data = data.manga;
		let slug = manga
			.key
			.trim_start_matches("/manga/")
			.trim_end_matches('/')
			.to_string();

		if needs_details {
			manga.title = data.title.trim().into();
			let mut description = data
				.description
				.as_deref()
				.map(str::trim)
				.unwrap_or_default()
				.to_string();
			let alt_titles: Vec<&str> =
				[data.title_ar.as_deref(), data.alternative_titles.as_deref()]
					.into_iter()
					.flatten()
					.map(str::trim)
					.filter(|s| !s.is_empty())
					.collect();
			if !alt_titles.is_empty() {
				if !description.is_empty() {
					description.push_str("\n\n");
				}
				description.push_str("أسماء أخرى: ");
				description.push_str(&alt_titles.join(" | "));
			}
			manga.description = Some(description).filter(|d| !d.is_empty());
			manga.cover = data
				.cover_image
				.clone()
				.filter(|c| !c.is_empty())
				.or(manga.cover);
			manga.tags = Some(data.tags.iter().map(|t| t.name.trim().into()).collect())
				.filter(|t: &Vec<String>| !t.is_empty());
			manga.authors = data
				.author
				.as_deref()
				.map(str::trim)
				.filter(|a| !a.is_empty() && !a.eq_ignore_ascii_case("unknown"))
				.map(|a| vec![a.into()]);
			manga.status = match data.status.as_deref() {
				Some("ongoing") => MangaStatus::Ongoing,
				Some("completed") => MangaStatus::Completed,
				Some("hiatus") => MangaStatus::Hiatus,
				Some("cancelled") | Some("dropped") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			};
			manga.viewer = match data.manga_type.as_deref() {
				Some("manga") => Viewer::RightToLeft,
				Some("manhwa") | Some("manhua") | Some("webtoon") => Viewer::Webtoon,
				_ => Viewer::Unknown,
			};
			manga.content_rating = if data.is_mature {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			};
			manga.url = Some(url);
		}

		if needs_chapters {
			manga.chapters = Some(
				data.chapters
					.into_iter()
					// paid chapters can't be read
					.filter(|c| {
						c.visible != Some(0)
							&& !(c.price.unwrap_or(0.0) > 0.0 && c.access == Some(false))
					})
					.map(|c| {
						let number = c.chapter_number.as_string();
						let mut title = c
							.title
							.map(|t| String::from(t.trim()))
							.filter(|t| {
								!t.is_empty() && *t != format!("Chapter {number}") && *t != number
							});
						if c.is_ai == Some(1) {
							title = Some(match title {
								Some(t) => format!("{t} (مترجم)"),
								None => String::from("(مترجم)"),
							});
						}
						let key = format!("/reader/{slug}/{number}");
						Chapter {
							url: Some(format!("{BASE_URL}{key}")),
							key,
							title,
							chapter_number: number.parse().ok(),
							date_uploaded: c.created_at.as_deref().and_then(parse_iso_date),
							..Default::default()
						}
					})
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = request(&format!("{BASE_URL}{}", chapter.key))?.html()?;
		let props: ChapterProps = astro_prop(&html, "imageUrls")?;
		let mut seeds = None;

		let mut urls: Vec<Option<String>> = props
			.image_urls
			.iter()
			.map(|v| page_url(v).map(String::from))
			.collect();
		if urls.iter().any(Option::is_none) {
			// with the gate on, only the first pages are in the HTML; the rest need a proof
			let seeds = seeds.get_or_insert(reader_seeds(&html)?);
			let fetched = fetch_pages(&props, seeds)?;
			if fetched.len() >= urls.len() {
				urls = fetched.into_iter().map(Some).collect();
			} else {
				let mut fetched = fetched.into_iter();
				for url in urls.iter_mut().filter(|u| u.is_none()) {
					*url = fetched.next();
				}
			}
		}

		let overlay_json = match props.overlay_blob.as_deref().filter(|b| !b.is_empty()) {
			Some(blob) => crypto::decrypt_hex_blob(blob, None).ok(),
			None if props.text_mode.as_deref() == Some("overlay") => {
				let seeds = match seeds {
					Some(seeds) => Some(seeds),
					None => reader_seeds(&html).ok(),
				};
				// without the overlay the pages are still readable, just untranslated
				seeds.and_then(|seeds| unlock_overlay(&props, &seeds).ok().flatten())
			}
			None => None,
		};
		let overlay_data =
			overlay_json.and_then(|json| serde_json::from_str::<OverlayData>(&json).ok());

		// page_number counts from 1 after the overlayPageOffset images
		let first_overlay_image = props.overlay_page_offset.unwrap_or(0);
		let mut overlays_by_image: HashMap<i32, OverlayPage> = HashMap::new();
		for page in overlay_data.map(|d| d.pages).unwrap_or_default() {
			overlays_by_image.insert(first_overlay_image + page.page_number - 1, page);
		}

		Ok(urls
			.into_iter()
			.enumerate()
			.filter_map(|(index, url)| Some((index, url?)))
			.map(|(index, url)| {
				let content = match overlays_by_image.remove(&(index as i32)) {
					Some(page) if !page.overlays.is_empty() => {
						let mut context = PageContext::new();
						context.insert(
							OVERLAYS_KEY.into(),
							serde_json::to_string(&page.overlays).unwrap_or_default(),
						);
						// overlay coordinates are in the size the editor saw the page at
						if let (Some(w), Some(h)) = (page.image_width, page.image_height) {
							context.insert(SIZE_KEY.into(), format!("{w}x{h}"));
						}
						PageContent::url_context(url, context)
					}
					_ => PageContent::url(url),
				};
				Page {
					content,
					..Default::default()
				}
			})
			.collect())
	}
}

fn fetch_pages(props: &ChapterProps, seeds: &seeds::Seeds) -> Result<Vec<String>> {
	let (Some(chapter_id), Some(token)) = (props.chapter_id, props.unlock_token.as_deref()) else {
		bail!("Missing chapter token");
	};
	let chapter_number = match &props.chapter_number {
		Some(serde_json::Value::String(s)) => s.clone(),
		Some(serde_json::Value::Number(n)) => n.to_string(),
		_ => String::new(),
	};
	let proof = crypto::proof(&seeds.proof_salt, token, chapter_id);
	let body = serde_json::to_vec(&PagesRequest {
		chapter_id,
		manga_slug: props.manga_slug.as_deref().unwrap_or_default(),
		chapter_number: &chapter_number,
		token,
		proof: &proof,
	})
	.map_err(|_| error!("Invalid pages request"))?;
	let data = Request::post(format!("{API_BASE}/api/reader/pages"))?
		.header("Content-Type", "application/json")
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/"))
		.body(body)
		.data()?;
	let response: PagesResponse =
		serde_json::from_slice(&data).map_err(|_| error!("Invalid pages response"))?;
	Ok(response
		.data
		.unwrap_or_default()
		.iter()
		.filter_map(|v| page_url(v).map(String::from))
		.collect())
}

fn unlock_overlay(props: &ChapterProps, seeds: &seeds::Seeds) -> Result<Option<String>> {
	let (Some(chapter_id), Some(token)) = (props.chapter_id, props.unlock_token.as_deref()) else {
		return Ok(None);
	};
	let proof = crypto::proof(&seeds.proof_salt, token, chapter_id);
	let esid = crypto::esid(&seeds.reader_seed);

	let post = |nonce: Option<&str>, pow: Option<&str>| -> Result<UnlockResponse> {
		let body = serde_json::to_vec(&UnlockRequest {
			chapter_id,
			token,
			proof: &proof,
			esid: &esid,
			nonce,
			pow,
		})
		.map_err(|_| error!("Invalid unlock request"))?;
		let data = Request::post(format!("{API_BASE}/api/reader/unlock"))?
			.header("Content-Type", "application/json")
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/"))
			.body(body)
			.data()?;
		serde_json::from_slice(&data).map_err(|_| error!("Invalid unlock response"))
	};

	let mut response = post(None, None)?;
	if response.needs_challenge
		&& let Some(nonce) = response.nonce.clone()
	{
		let pow = response.pow_bits.and_then(|bits| crypto::pow(&nonce, bits));
		response = post(Some(&nonce), pow.as_deref())?;
	}

	let Some(overlay) = response.overlay.as_deref() else {
		return Ok(None);
	};
	if let Some(ks) = response.ks.as_deref() {
		crypto::decrypt_envelope(&seeds.reader_seed, overlay, ks, token, chapter_id).map(Some)
	} else if let Some(key) = response.key.as_deref() {
		crypto::decrypt_hex_blob(overlay, Some(key)).map(Some)
	} else {
		Ok(None)
	}
}

/// The reader's seeds rotate with each build of its bundle, so they are cached per bundle URL.
fn reader_seeds(html: &Document) -> Result<seeds::Seeds> {
	let bundle_url = html
		.select("astro-island[component-url]")
		.and_then(|mut els| {
			els.find(|el| {
				el.attr("component-url")
					.is_some_and(|u| u.contains("ChapterImageViewer"))
			})
		})
		.and_then(|el| el.attr("abs:component-url"))
		.map(|u| {
			if u.starts_with('/') {
				format!("{BASE_URL}{u}")
			} else {
				u
			}
		})
		.ok_or_else(|| error!("Unable to find the reader script"))?;

	if let Some(cached) = defaults_get::<Vec<String>>(SEEDS_KEY)
		&& let [url, proof_salt, reader_seed] = cached.as_slice()
		&& *url == bundle_url
	{
		return Ok(seeds::Seeds {
			proof_salt: proof_salt.clone(),
			reader_seed: reader_seed.clone(),
		});
	}

	let script = request(&bundle_url)?.string()?;
	let seeds = seeds::extract(&script)?;
	defaults_set(
		SEEDS_KEY,
		DefaultValue::StringArray(vec![
			bundle_url,
			seeds.proof_salt.clone(),
			seeds.reader_seed.clone(),
		]),
	);
	Ok(seeds)
}

impl ListingProvider for MangaTek {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let html = request(&format!("{BASE_URL}/manga-list?page={page}"))?.html()?;
		Ok(parse_manga_list(&html))
	}
}

impl ImageRequestProvider for MangaTek {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		request(&url)
	}
}

impl PageImageProcessor for MangaTek {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let bubbles: Vec<Bubble> = context
			.as_ref()
			.and_then(|c| c.get(OVERLAYS_KEY))
			.and_then(|json| serde_json::from_str(json).ok())
			.unwrap_or_default();
		if bubbles.is_empty() || response.code >= 400 {
			return Ok(response.image);
		}
		let size = context
			.as_ref()
			.and_then(|c| c.get(SIZE_KEY))
			.and_then(|s| s.split_once('x'))
			.and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
			.filter(|(w, h)| *w > 0.0 && *h > 0.0);
		Ok(painter::paint(&response.image, &bubbles, size))
	}
}

impl DeepLinkHandler for MangaTek {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.mangatek.com"))
		else {
			return Ok(None);
		};
		let mut segments = path.split(['?', '#']).next().unwrap_or_default().split('/');
		segments.next(); // leading empty segment
		match (segments.next(), segments.next(), segments.next()) {
			(Some("manga"), Some(slug), _) if !slug.is_empty() => Ok(Some(DeepLinkResult::Manga {
				key: format!("/manga/{slug}"),
			})),
			(Some("reader"), Some(slug), Some(chapter))
				if !slug.is_empty() && !chapter.is_empty() =>
			{
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: format!("/manga/{slug}"),
					key: format!("/reader/{slug}/{chapter}"),
				}))
			}
			(Some("reader"), Some(slug), _) if !slug.is_empty() => {
				Ok(Some(DeepLinkResult::Manga {
					key: format!("/manga/{slug}"),
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(
	MangaTek,
	ListingProvider,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
