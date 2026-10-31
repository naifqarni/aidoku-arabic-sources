use aidoku::alloc::{String, Vec};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Astro serializes island props as `[type, value]` pairs; this strips the type tags.
pub fn unwrap_astro(value: Value) -> Value {
	match value {
		Value::Array(mut items) => {
			if items.len() == 2 && !items[0].is_array() && !items[0].is_object() {
				unwrap_astro(items.swap_remove(1))
			} else {
				Value::Array(items.into_iter().map(unwrap_astro).collect())
			}
		}
		Value::Object(map) => {
			Value::Object(map.into_iter().map(|(k, v)| (k, unwrap_astro(v))).collect())
		}
		other => other,
	}
}

#[derive(Deserialize)]
pub struct MangaDto {
	pub manga: MangaData,
}

#[derive(Deserialize)]
pub struct MangaData {
	pub title: String,
	pub title_ar: Option<String>,
	pub alternative_titles: Option<String>,
	pub description: Option<String>,
	pub cover_image: Option<String>,
	pub status: Option<String>,
	#[serde(rename = "type")]
	pub manga_type: Option<String>,
	#[serde(rename = "Tags", default)]
	pub tags: Vec<Tag>,
	pub author: Option<String>,
	#[serde(default)]
	pub is_mature: bool,
	#[serde(rename = "MangaChapters", default)]
	pub chapters: Vec<ChapterDto>,
}

#[derive(Deserialize)]
pub struct Tag {
	pub name: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum NumberOrString {
	Number(f64),
	String(String),
}

impl NumberOrString {
	pub fn as_string(&self) -> String {
		match self {
			Self::Number(n) => {
				if *n == (*n as i64) as f64 {
					aidoku::prelude::format!("{}", *n as i64)
				} else {
					aidoku::prelude::format!("{n}")
				}
			}
			Self::String(s) => s.trim().into(),
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterDto {
	pub title: Option<String>,
	pub chapter_number: NumberOrString,
	pub created_at: Option<String>,
	#[serde(rename = "isAI")]
	pub is_ai: Option<i32>,
	pub price: Option<f64>,
	pub visible: Option<i32>,
	pub access: Option<bool>,
}

#[derive(Deserialize)]
pub struct ChapterProps {
	/// Either a URL or `{"pending": true, "url": ""}` for pages the reader fetches later.
	#[serde(rename = "imageUrls", default)]
	pub image_urls: Vec<Value>,
	#[serde(rename = "mangaSlug")]
	pub manga_slug: Option<String>,
	#[serde(rename = "chapterNumber")]
	pub chapter_number: Option<Value>,
	#[serde(rename = "overlayBlob")]
	pub overlay_blob: Option<String>,
	#[serde(rename = "overlayPageOffset")]
	pub overlay_page_offset: Option<i32>,
	#[serde(rename = "textMode")]
	pub text_mode: Option<String>,
	#[serde(rename = "chapterId")]
	pub chapter_id: Option<i64>,
	#[serde(rename = "unlockToken")]
	pub unlock_token: Option<String>,
}

#[derive(Deserialize)]
pub struct OverlayData {
	#[serde(default)]
	pub pages: Vec<OverlayPage>,
}

#[derive(Deserialize)]
pub struct OverlayPage {
	pub page_number: i32,
	pub image_width: Option<f32>,
	pub image_height: Option<f32>,
	#[serde(default)]
	pub overlays: Vec<Value>,
}

#[derive(Deserialize)]
pub struct Bubble {
	#[serde(default)]
	pub text: String,
	#[serde(default)]
	pub x: f32,
	#[serde(default)]
	pub y: f32,
	#[serde(default)]
	pub w: f32,
	#[serde(default)]
	pub h: f32,
	pub color: Option<String>,
	pub stroke_color: Option<String>,
	#[serde(default = "default_font_size")]
	pub font_size_px: f32,
	#[serde(default = "default_line_height")]
	pub line_height: f32,
	#[serde(default)]
	pub stroke_width_px: f32,
	#[serde(default)]
	pub font_family: String,
	pub text_align: Option<String>,
}

fn default_font_size() -> f32 {
	16.0
}

fn default_line_height() -> f32 {
	1.2
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockRequest<'a> {
	pub chapter_id: i64,
	pub token: &'a str,
	pub proof: &'a str,
	pub esid: &'a str,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub nonce: Option<&'a str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pow: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagesRequest<'a> {
	pub chapter_id: i64,
	pub manga_slug: &'a str,
	pub chapter_number: &'a str,
	pub token: &'a str,
	pub proof: &'a str,
}

#[derive(Deserialize)]
pub struct PagesResponse {
	#[serde(default)]
	pub data: Option<Vec<Value>>,
}

/// A page entry: a URL, or an object with one.
pub fn page_url(value: &Value) -> Option<&str> {
	match value {
		Value::String(s) => Some(s.as_str()),
		Value::Object(map) => ["url", "src", "image", "imageUrl"]
			.iter()
			.find_map(|k| map.get(*k).and_then(Value::as_str)),
		_ => None,
	}
	.filter(|s| !s.is_empty())
}

#[derive(Deserialize)]
pub struct UnlockResponse {
	pub overlay: Option<String>,
	pub ks: Option<String>,
	pub key: Option<String>,
	#[serde(default)]
	pub needs_challenge: bool,
	pub nonce: Option<String>,
	pub pow_bits: Option<u32>,
}
