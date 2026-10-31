use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{BASE_URL, SITE_HOST};

#[derive(Deserialize)]
pub struct ContentPage {
	pub data: Vec<Content>,
	pub meta: Meta,
}

#[derive(Deserialize)]
pub struct Meta {
	pub page: i32,
	pub pages: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
	id: i64,
	title: String,
	pub slug: String,
	#[serde(rename = "type")]
	pub kind: String,
	description: Option<String>,
	progress: Option<String>,
	thumbnail: Option<String>,
	cover_image: Option<String>,
	#[serde(rename = "cdn_path")]
	cdn_path: Option<String>,
	metadata: Option<Metadata>,
	#[serde(default)]
	is_blocked_series: bool,
}

/// A string, or a list of strings joined together.
fn string_or_list<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
	Ok(
		match Option::<serde_json::Value>::deserialize(deserializer)? {
			Some(serde_json::Value::String(s)) => Some(s),
			Some(serde_json::Value::Array(items)) => Some(
				items
					.iter()
					.filter_map(|i| i.as_str())
					.collect::<Vec<_>>()
					.join(", "),
			),
			_ => None,
		}
		.filter(|s| !s.trim().is_empty()),
	)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
	#[serde(default, deserialize_with = "string_or_list")]
	author: Option<String>,
	#[serde(default, deserialize_with = "string_or_list")]
	artist: Option<String>,
	origin: Option<String>,
	year: Option<String>,
	genres: Option<Vec<String>>,
	tags: Option<Vec<String>>,
	alt_titles: Option<Vec<String>>,
}

fn capitalize(s: &str) -> String {
	let mut chars = s.chars();
	match chars.next() {
		Some(first) => first.to_uppercase().chain(chars).collect(),
		None => String::new(),
	}
}

pub fn manga_url(slug: &str, id: &str) -> String {
	format!("{BASE_URL}/ar/{slug}-{id}")
}

impl Content {
	/// Novels are text-only and blocked series need a login.
	pub fn is_readable(&self) -> bool {
		self.kind != "novel" && !self.is_blocked_series
	}

	pub fn into_manga(self) -> Manga {
		let id = self.id.to_string();
		let cover = match self.thumbnail {
			Some(thumb) if thumb.starts_with('/') => Some(format!(
				"https://{}.{SITE_HOST}{thumb}",
				self.cdn_path.as_deref().unwrap_or("cdn")
			)),
			Some(thumb) => Some(thumb),
			None => self.cover_image,
		};
		Manga {
			url: Some(manga_url(&self.slug, &id)),
			key: id,
			title: self.title,
			cover,
			..Default::default()
		}
	}

	pub fn into_manga_details(mut self) -> Manga {
		let metadata = self.metadata.take();
		let kind = self.kind.clone();
		let progress = self.progress.take();
		let mut description = self
			.description
			.take()
			.map(|d| String::from(d.trim()))
			.unwrap_or_default();
		let mut tags = vec![capitalize(&kind)];
		let mut authors = None;
		let mut artists = None;
		if let Some(meta) = metadata {
			if let Some(alt) = meta.alt_titles.filter(|a| !a.is_empty()) {
				if !description.is_empty() {
					description.push_str("\n\n");
				}
				description.push_str("عناوين بديلة\n");
				for title in alt {
					description.push_str("- ");
					description.push_str(&title);
					description.push('\n');
				}
			}
			tags.extend(meta.origin.map(|o| capitalize(&o)));
			tags.extend(meta.year);
			tags.extend(meta.genres.unwrap_or_default());
			tags.extend(meta.tags.unwrap_or_default());
			authors = meta.author.map(|a| vec![a]);
			artists = meta.artist.map(|a| vec![a]);
		}
		let description = String::from(description.trim());
		let lower: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
		let has = |words: &[&str]| lower.iter().any(|t| words.contains(&t.as_str()));
		let content_rating = if has(&["adult", "smut", "hentai", "mature", "18+"]) {
			ContentRating::NSFW
		} else if has(&["ecchi"]) {
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};
		Manga {
			authors,
			artists,
			description: if description.is_empty() {
				None
			} else {
				Some(description)
			},
			tags: Some(tags),
			status: match progress.as_deref().map(str::trim) {
				Some("مستمر") => MangaStatus::Ongoing,
				Some("مكتمل") => MangaStatus::Completed,
				Some("متوقف") => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			},
			content_rating,
			viewer: match kind.as_str() {
				"manga" => Viewer::RightToLeft,
				"manhwa" | "manhua" => Viewer::Webtoon,
				_ => Viewer::Unknown,
			},
			..self.into_manga()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterPage {
	pub data: Vec<SeriesChapter>,
	pub total: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesChapter {
	id: i64,
	#[serde(rename = "chapter_number")]
	number: String,
	title: Option<String>,
	language: String,
	translator: Option<String>,
	#[serde(rename = "published_at")]
	published_at: Option<String>,
	#[serde(default)]
	locked_forever: bool,
	#[serde(default)]
	locked_by_coins: bool,
	#[serde(default)]
	locked_by_shortlink: bool,
	#[serde(default)]
	locked_by_exclusive: bool,
}

impl SeriesChapter {
	pub fn is_arabic(&self) -> bool {
		self.language == "AR"
	}

	pub fn is_locked(&self) -> bool {
		self.locked_forever
			|| self.locked_by_coins
			|| self.locked_by_shortlink
			|| self.locked_by_exclusive
	}

	pub fn into_chapter(self, series_slug: &str) -> Chapter {
		let key = format!("/ar/chapter/{series_slug}-{}-{}", self.number, self.id);
		Chapter {
			url: Some(format!("{BASE_URL}{key}")),
			key,
			title: self
				.title
				.filter(|t| !t.trim().is_empty() && *t != self.number),
			chapter_number: self.number.parse().ok(),
			date_uploaded: self
				.published_at
				.and_then(|d| parse_date(d, "yyyy-MM-dd'T'HH:mm:ss.SSSX")),
			scanlators: self.translator.filter(|t| !t.is_empty()).map(|t| vec![t]),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterRef {
	pub content_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reader {
	pub chapter_id: i64,
	pub app_images: Vec<AppImage>,
	pub cdn_path: String,
	pub deferred_media: Option<DeferredMedia>,
}

#[derive(Deserialize)]
pub struct AppImage {
	pub mobile: Option<String>,
	pub desktop: Option<String>,
}

#[derive(Deserialize)]
pub struct DeferredMedia {
	pub token: String,
}

#[derive(Deserialize)]
pub struct DeferredPage {
	pub data: Option<DeferredData>,
	pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct DeferredData {
	#[serde(default)]
	pub images: Vec<String>,
	#[serde(default)]
	pub maps: Vec<MapEntry>,
}

#[derive(Deserialize)]
pub struct MapEntry {
	pub token: String,
	pub method: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Moved {
	pub target_url: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPlanBody {
	pub token: String,
	pub method: String,
	pub cdn_path: String,
	pub page_index: usize,
}

#[derive(Deserialize)]
pub struct MapPlanResponse {
	pub data: MapPlanWrapper,
}

#[derive(Deserialize)]
pub struct MapPlanWrapper {
	pub map: MapPlan,
}

#[derive(Deserialize)]
pub struct MapPlan {
	pub dim: Vec<f32>,
	pub pieces: Vec<String>,
	pub order: Vec<usize>,
	pub rects: Vec<MapRect>,
}

#[derive(Deserialize)]
pub struct MapRect {
	pub left: f32,
	pub top: f32,
	pub width: f32,
	pub height: f32,
}

#[derive(Serialize)]
pub struct SignRequest<'a> {
	pub url: &'a str,
}

#[derive(Deserialize)]
pub struct SignedImage {
	pub token: String,
	pub expires: i64,
}
