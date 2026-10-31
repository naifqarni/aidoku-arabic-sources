#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, borrow::ToOwned, string::ToString, vec},
	imports::{html::Document, net::Request, std::send_partial_result},
	prelude::*,
};
use serde::de::DeserializeOwned;

mod crypto;
mod models;

use models::*;

const BASE_URL: &str = "https://www.mangatales.com";
const CDN_URL: &str = "https://media.mangatales.com";

struct MangaTales;

fn cover_url(id: i64, cover: &str) -> String {
	format!("{CDN_URL}/uploads/manga/cover/{id}/large_{cover}")
}

/// The site embeds its page state as JSON in `.js-react-on-rails-component` scripts.
fn react_props<T: DeserializeOwned>(html: &Document) -> Result<T> {
	html.select(".js-react-on-rails-component")
		.and_then(|els| {
			els.filter_map(|el| el.data().filter(|s| !s.is_empty()).or_else(|| el.html()))
				.find_map(|s| serde_json::from_str::<T>(&s).ok())
		})
		.ok_or_else(|| error!("Unable to find page data"))
}

fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	let data = Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.data()?;
	serde_json::from_slice(&data).map_err(|_| error!("Invalid JSON response"))
}

fn include_exclude(
	filter: Option<(Vec<String>, Vec<String>)>,
	default: (&[&str], &[&str]),
) -> IncludeExclude {
	let (include, exclude) = filter.unwrap_or_else(|| {
		(
			default.0.iter().map(|s| (*s).to_owned()).collect(),
			default.1.iter().map(|s| (*s).to_owned()).collect(),
		)
	});
	IncludeExclude {
		include: include.into_iter().map(Some).collect(),
		exclude,
	}
}

fn valid_date(date: &str) -> bool {
	let parts: Vec<&str> = date.split('/').collect();
	parts.len() == 3
		&& parts[0].len() == 4
		&& parts
			.iter()
			.all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
		&& parts[1].parse::<u8>().is_ok_and(|m| (1..=12).contains(&m))
		&& parts[2].parse::<u8>().is_ok_and(|d| (1..=31).contains(&d))
}

impl Source for MangaTales {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut types = None;
		let mut story = None;
		let mut tl = None;
		let mut categories = None;
		let mut oneshot = false;
		let mut chapters = MinMax::default();
		let mut dates = StartEnd::default();

		for filter in filters {
			match filter {
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					let value = Some((included, excluded));
					match id.as_str() {
						"manga_types" => types = value,
						"story_status" => story = value,
						"translation_status" => tl = value,
						"categories" => categories = value,
						_ => {}
					}
				}
				FilterValue::Check { id, value } if id == "oneshot" => oneshot = value == 1,
				FilterValue::Range { id, from, to } if id == "chapters" => {
					chapters.min = from.map(|v| (v as i32).to_string()).unwrap_or_default();
					chapters.max = to.map(|v| (v as i32).to_string()).unwrap_or_default();
				}
				FilterValue::Text { id, value } => {
					let value = value.trim();
					if value.is_empty() {
						continue;
					}
					match id.as_str() {
						"start" if valid_date(value) => dates.start = value.into(),
						"end" if valid_date(value) => dates.end = value.into(),
						"start" => bail!("تاريخ بداية غير صالح"),
						"end" => bail!("تاريخ نهاية غير صالح"),
						_ => {}
					}
				}
				_ => {}
			}
		}

		let mut categories = include_exclude(categories, (&[], &[]));
		// the site always sends a leading null
		categories.include.insert(0, None);

		let payload = SearchPayload {
			oneshot: OneShot { value: oneshot },
			title: query.unwrap_or_default().trim().into(),
			page,
			manga_types: include_exclude(types, (&["1", "2"], &[])),
			story_status: include_exclude(story, (&[], &[])),
			translation_status: include_exclude(tl, (&[], &["3"])),
			categories,
			chapters,
			dates,
		};
		let body = serde_json::to_vec(&payload).map_err(|_| error!("Invalid search payload"))?;
		let response: EncryptedResponse = {
			let data = Request::post(format!("{BASE_URL}/api/mangas/search"))?
				.header("Content-Type", "application/json")
				.header("Referer", &format!("{BASE_URL}/mangas/"))
				.body(body)
				.data()?;
			serde_json::from_slice(&data).map_err(|_| error!("Invalid search response"))?
		};
		let decrypted =
			crypto::decrypt(&response.data).ok_or_else(|| error!("Decryption failed"))?;
		let data: SearchMangaDto =
			serde_json::from_str(&decrypted).map_err(|_| error!("Invalid search data"))?;

		Ok(MangaPageResult {
			has_next_page: data.mangas.len() == 50,
			entries: data
				.mangas
				.into_iter()
				.filter(|m| !m.is_novel)
				.map(Into::into)
				.collect(),
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{BASE_URL}{}", manga.key);
			let html = Request::get(&url)?
				.header("Referer", &format!("{BASE_URL}/"))
				.html()?;
			let data: MangaDataAction<MangaDetailsDto> = react_props(&html)?;
			let details = data.manga_data_action.manga_data;
			details.apply(&mut manga);
			manga.url = Some(url);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let data: ChapterListDto = get_json(&format!("{BASE_URL}/api{}", manga.key))?;
			let mut chapters: Vec<Chapter> =
				data.manga_releases.into_iter().map(Into::into).collect();
			chapters.sort_by(|a, b| {
				let num = b
					.chapter_number
					.unwrap_or(-1.0)
					.total_cmp(&a.chapter_number.unwrap_or(-1.0));
				num.then(b.date_uploaded.cmp(&a.date_uploaded))
			});
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Request::get(format!("{BASE_URL}{}", chapter.key))?
			.header("Referer", &format!("{BASE_URL}/"))
			.html()?;
		let data: ReaderDto = react_props(&html)?;
		let key = data.globals.media_key;
		Ok(data
			.reader_data_action
			.reader_data
			.release
			.hq_pages
			.split(['\r', '\n'])
			.map(str::trim)
			.filter(|s| !s.is_empty())
			.map(|img| Page {
				content: PageContent::url(format!(
					"{CDN_URL}/uploads/releases/{}?ak={key}",
					encode_path(img)
				)),
				..Default::default()
			})
			.collect())
	}
}

/// Percent-encodes everything in a path except unreserved characters and `/`.
fn encode_path(path: &str) -> String {
	let mut out = String::with_capacity(path.len());
	for b in path.bytes() {
		if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
			out.push(b as char);
		} else {
			out.push_str(&format!("%{b:02X}"));
		}
	}
	out
}

impl ListingProvider for MangaTales {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let data: LatestChaptersDto = get_json(&format!("{BASE_URL}/api/releases?page={page}"))?;
		let has_next_page = data.releases.len() >= 30;
		let mut entries: Vec<Manga> = Vec::new();
		for release in data.releases {
			if release.manga.is_novel {
				continue;
			}
			let manga: Manga = release.manga.into();
			if !entries.iter().any(|m| m.key == manga.key) {
				entries.push(manga);
			}
		}
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

impl DeepLinkHandler for MangaTales {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://mangatales.com"))
		else {
			return Ok(None);
		};
		let mut segments = path.split(['?', '#']).next().unwrap_or_default().split('/');
		segments.next(); // leading empty segment
		match (segments.next(), segments.next()) {
			(Some("mangas"), Some(id))
				if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) =>
			{
				Ok(Some(DeepLinkResult::Manga {
					key: format!("/mangas/{id}"),
				}))
			}
			(Some("r"), Some(id)) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => {
				// the reader page knows which manga it belongs to
				let html = Request::get(format!("{BASE_URL}/r/{id}"))?.html()?;
				let manga_id = react_props::<ReaderDto>(&html)
					.ok()
					.and_then(|d| d.reader_data_action.reader_data.release.manga_id);
				Ok(Some(match manga_id {
					Some(manga_id) => DeepLinkResult::Chapter {
						manga_key: format!("/mangas/{manga_id}"),
						key: format!("/r/{id}"),
					},
					None => return Ok(None),
				}))
			}
			_ => Ok(None),
		}
	}
}

impl MangaDetails {
	fn apply(self, manga: &mut Manga) {
		manga.title = self.title.trim().into();
		manga.cover = self
			.cover
			.as_deref()
			.filter(|c| !c.is_empty())
			.map(|c| cover_url(self.id, c));
		manga.artists = Some(self.artists.into_iter().map(|a| a.name).collect())
			.filter(|v: &Vec<String>| !v.is_empty());
		manga.authors = Some(self.authors.into_iter().map(|a| a.name).collect())
			.filter(|v: &Vec<String>| !v.is_empty());
		manga.status = match self.story_status {
			2 => MangaStatus::Ongoing,
			3 => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};

		let mut tags = Vec::new();
		if let Some(title) = self.manga_type.title.filter(|t| !t.is_empty()) {
			tags.push(title);
		}
		tags.push(self.manga_type.name);
		let webtoon = self.categories.iter().any(|c| c.name.contains("ويب"));
		tags.extend(self.categories.into_iter().map(|c| c.name));
		manga.tags = Some(tags);

		let mut description = String::from(
			self.summary
				.as_deref()
				.map(str::trim)
				.filter(|s| !s.is_empty())
				.unwrap_or("لم يتم اضافة قصة بعد"),
		);
		description.push_str("\n\nحالة الترجمة:\n• ");
		description.push_str(match self.translation_status {
			0 => "منتهية",
			1 => "مستمرة",
			2 => "متوقفة",
			_ => "مجهول",
		});
		let titles: Vec<String> = [
			self.synonyms,
			self.arabic_title,
			self.japanese,
			self.english,
		]
		.into_iter()
		.flatten()
		.map(|s| String::from(s.trim()))
		.filter(|s| !s.is_empty())
		.collect();
		if !titles.is_empty() {
			description.push_str("\n\nمسميّات أخرى:\n• ");
			description.push_str(&titles.join("\n• "));
		}
		manga.description = Some(description);

		let direction = self
			.reading_direction
			.or(self.manga_type.reading_direction)
			.unwrap_or_default();
		manga.viewer = if webtoon {
			Viewer::Webtoon
		} else if direction == "ltr" {
			Viewer::LeftToRight
		} else if direction == "rtl" {
			Viewer::RightToLeft
		} else {
			Viewer::Unknown
		};
		manga.content_rating = if self.over17 {
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};
	}
}

impl From<BrowseManga> for Manga {
	fn from(value: BrowseManga) -> Self {
		Manga {
			key: format!("/mangas/{}", value.id),
			title: value.title.trim().into(),
			cover: value
				.cover
				.as_deref()
				.filter(|c| !c.is_empty())
				.map(|c| cover_url(value.id, c)),
			..Default::default()
		}
	}
}

impl From<ChapterRelease> for Chapter {
	fn from(value: ChapterRelease) -> Self {
		let key = format!("/r/{}", value.id);
		Chapter {
			url: Some(format!("{BASE_URL}{key}")),
			key,
			title: Some(String::from(value.title.trim())).filter(|s| !s.is_empty()),
			chapter_number: value.chapter.as_f32(),
			date_uploaded: parse_iso_date(&value.created_at),
			scanlators: Some(vec![String::from(value.team_name.trim())])
				.filter(|v: &Vec<String>| !v[0].is_empty()),
			..Default::default()
		}
	}
}

/// Parses `yyyy-MM-ddTHH:mm:ss(.SSS)Z` into a unix timestamp.
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

register_source!(MangaTales, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
