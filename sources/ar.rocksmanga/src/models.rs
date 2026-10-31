use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::Deserialize;

use crate::{API_URL, BASE_URL};

#[derive(Deserialize)]
pub struct Details {
	pub manga: Series,
}

#[derive(Deserialize)]
pub struct Series {
	slug: String,
	title: String,
	alternative_titles: Option<String>,
	synopsis: Option<String>,
	cover_url: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
	status: Option<String>,
	#[serde(default)]
	is_nsfw: bool,
	#[serde(default)]
	authors: Vec<Name>,
	#[serde(default)]
	artists: Vec<Name>,
	#[serde(default)]
	genres: Vec<Name>,
}

#[derive(Deserialize)]
struct Name {
	name: String,
}

#[derive(Deserialize)]
struct AltTitle {
	title: String,
}

fn names(list: Vec<Name>) -> Option<Vec<String>> {
	let names: Vec<String> = list.into_iter().map(|n| n.name).collect();
	if names.is_empty() { None } else { Some(names) }
}

impl Series {
	/// Some entries hold a JSON array of `{lang, title}` in this string instead of plain text.
	fn alt_titles(&self) -> Vec<String> {
		let Some(titles) = self
			.alternative_titles
			.as_deref()
			.map(str::trim)
			.filter(|t| !t.is_empty())
		else {
			return Vec::new();
		};
		if titles.starts_with('[') {
			serde_json::from_str::<Vec<AltTitle>>(titles)
				.map(|t| t.into_iter().map(|t| t.title).collect())
				.unwrap_or_default()
		} else {
			vec![titles.into()]
		}
	}

	pub fn into_manga(self) -> Manga {
		let mut description = self.synopsis.clone().unwrap_or_default();
		let alt_titles = self.alt_titles();
		if !alt_titles.is_empty() {
			if !description.is_empty() {
				description.push_str("\n\n");
			}
			description.push_str("أسماء أخرى:\n");
			description.push_str(&alt_titles.join("\n"));
		}
		let key = format!("/manga/{}", self.slug);
		Manga {
			url: Some(format!("{BASE_URL}{key}")),
			key,
			title: self.title,
			cover: self.cover_url.map(|c| format!("{API_URL}{c}")),
			authors: names(self.authors),
			artists: names(self.artists),
			description: if description.is_empty() {
				None
			} else {
				Some(description)
			},
			tags: names(self.genres),
			status: match self.status.as_deref() {
				Some("RELEASING") => MangaStatus::Ongoing,
				Some("COMPLETED") => MangaStatus::Completed,
				Some("CANCELED") => MangaStatus::Cancelled,
				Some("ON_HOLD") => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			},
			content_rating: if self.is_nsfw {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			},
			viewer: match self.kind.as_deref() {
				Some("MANGA") => Viewer::RightToLeft,
				Some("MANHWA") | Some("MANHUA") => Viewer::Webtoon,
				Some("COMIC") => Viewer::LeftToRight,
				_ => Viewer::Unknown,
			},
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterList {
	pub manga_slug: String,
	pub chapters: Vec<SeriesChapter>,
}

#[derive(Deserialize)]
pub struct SeriesChapter {
	pub id: i64,
	number: String,
	title: Option<String>,
	created_at: Option<String>,
}

impl SeriesChapter {
	pub fn into_chapter(self, slug: &str) -> Chapter {
		let key = format!("/manga/{slug}/chapter/{}", self.number);
		Chapter {
			url: Some(format!("{BASE_URL}{key}")),
			key,
			title: self.title.filter(|t| !t.trim().is_empty()),
			chapter_number: self.number.parse().ok(),
			date_uploaded: self
				.created_at
				.and_then(|d| parse_date(d, "yyyy-MM-dd'T'HH:mm:ss.SSSX")),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct Reader {
	pub chapter: ReaderChapter,
}

#[derive(Deserialize)]
pub struct ReaderChapter {
	pub pages: Vec<String>,
}
