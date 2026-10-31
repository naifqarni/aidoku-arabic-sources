use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString},
	imports::std::parse_date,
	prelude::*,
};
use serde::{Deserialize, Deserializer};

use crate::BASE_URL;

#[derive(Deserialize)]
pub struct SeriesList {
	pub data: Vec<Series>,
	pub total: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
	pub id: String,
	pub slug: String,
	title: String,
	cover_image: Option<String>,
	associated_names: Option<String>,
	description: Option<String>,
	author: Option<String>,
	status: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
	#[serde(default)]
	genres: Vec<Genre>,
	#[serde(default)]
	pub chapters: Vec<SeriesChapter>,
}

#[derive(Deserialize)]
struct Genre {
	name: String,
	slug: Option<String>,
}

/// The api sends chapter numbers as strings in some places and as numbers in others.
fn string_or_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
	Ok(match serde_json::Value::deserialize(deserializer)? {
		serde_json::Value::String(s) => s,
		other => other.to_string(),
	})
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesChapter {
	pub id: String,
	#[serde(deserialize_with = "string_or_number")]
	number: String,
	title: Option<String>,
	pub slug: String,
	published_at: Option<String>,
	coin_access: Option<CoinAccess>,
}

#[derive(Deserialize)]
struct CoinAccess {
	#[serde(default)]
	locked: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageImage {
	pub image_url: String,
}

pub fn series_url(slug: &str) -> String {
	format!("{BASE_URL}/series/{slug}")
}

impl Series {
	pub fn into_manga(self) -> Manga {
		Manga {
			url: Some(series_url(&self.slug)),
			key: self.id,
			title: self.title,
			cover: self.cover_image,
			..Default::default()
		}
	}

	pub fn into_manga_details(self) -> Manga {
		let mut description = self.description.unwrap_or_default();
		if let Some(names) = self.associated_names.filter(|n| !n.trim().is_empty()) {
			if !description.is_empty() {
				description.push_str("\n\n");
			}
			description.push_str("أسماء أخرى:\n");
			description.push_str(&names);
		}
		let adult = self
			.genres
			.iter()
			.any(|g| g.slug.as_deref() == Some("adult"));
		let viewer = match self.kind.as_deref() {
			Some("manhwa") | Some("manhua") | Some("webtoon") => Viewer::Webtoon,
			Some("manga") => Viewer::RightToLeft,
			Some("comic") => Viewer::LeftToRight,
			_ => Viewer::Unknown,
		};
		let mut tags: Vec<String> = self.kind.clone().into_iter().collect();
		tags.extend(self.genres.into_iter().map(|g| g.name));
		Manga {
			url: Some(series_url(&self.slug)),
			key: self.id,
			title: self.title,
			cover: self.cover_image,
			authors: self
				.author
				.map(|a| {
					a.split(',')
						.map(|s| String::from(s.trim()))
						.filter(|s| !s.is_empty())
						.collect::<Vec<_>>()
				})
				.filter(|a| !a.is_empty()),
			description: if description.is_empty() {
				None
			} else {
				Some(description)
			},
			tags: Some(tags),
			status: match self.status.as_deref() {
				Some("ongoing") => MangaStatus::Ongoing,
				Some("completed") => MangaStatus::Completed,
				Some("hiatus") => MangaStatus::Hiatus,
				Some("cancelled") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			content_rating: if adult {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			},
			viewer,
			..Default::default()
		}
	}
}

impl SeriesChapter {
	pub fn locked(&self) -> bool {
		self.coin_access.as_ref().is_some_and(|c| c.locked)
	}

	pub fn into_chapter(self, series_slug: &str) -> Chapter {
		let date_uploaded = self.published_at.as_deref().and_then(|d| {
			parse_date(d, "yyyy-MM-dd'T'HH:mm:ss.SSSX")
				.or_else(|| parse_date(d, "yyyy-MM-dd HH:mm:ss.SSS"))
		});
		Chapter {
			url: Some(format!("{}/{}", series_url(series_slug), self.slug)),
			key: self.id,
			title: self.title.filter(|t| !t.trim().is_empty()),
			chapter_number: self.number.parse().ok(),
			date_uploaded,
			..Default::default()
		}
	}
}
