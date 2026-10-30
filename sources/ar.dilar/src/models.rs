use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::{Deserialize, Serialize};

use crate::BASE_URL;

const NOVEL_TYPE: &str = "رواية";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesList {
	pub series: Vec<Series>,
	pub current_page: i32,
	pub total_pages: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rankings {
	pub top_series: Vec<Series>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchList {
	pub rows: Vec<Series>,
	pub total: i32,
	pub page: i32,
	pub per_page: i32,
}

#[derive(Deserialize)]
pub struct Series {
	id: String,
	title: String,
	cover: Option<String>,
	summary: Option<String>,
	#[serde(default)]
	staff: Vec<Staff>,
	#[serde(default)]
	categories: Vec<Name>,
	translation_status: Option<String>,
	#[serde(default)]
	over17: bool,
	#[serde(rename = "seriesType")]
	series_type: Option<SeriesType>,
}

#[derive(Deserialize)]
struct Staff {
	name: String,
	#[serde(rename = "Staff")]
	staff: Option<Role>,
}

#[derive(Deserialize)]
struct Role {
	role: Option<String>,
}

#[derive(Deserialize)]
struct SeriesType {
	title: String,
	reading_direction: Option<String>,
}

#[derive(Deserialize)]
struct Name {
	name: String,
}

impl Series {
	pub fn is_novel(&self) -> bool {
		self.series_type
			.as_ref()
			.is_some_and(|t| t.title == NOVEL_TYPE)
	}

	fn staff_with_role(&self, role: &str) -> Option<Vec<String>> {
		let names: Vec<String> = self
			.staff
			.iter()
			.filter(|s| s.staff.as_ref().and_then(|s| s.role.as_deref()) == Some(role))
			.map(|s| s.name.clone())
			.collect();
		if names.is_empty() { None } else { Some(names) }
	}

	pub fn into_manga(self) -> Manga {
		let viewer = match self.series_type.as_ref() {
			Some(t) => match t.reading_direction.as_deref() {
				Some("rtl") => Viewer::RightToLeft,
				Some("ltr") => Viewer::LeftToRight,
				Some("ttb") | Some("vertical") => Viewer::Webtoon,
				_ if t.title.contains("مانهوا") || t.title.contains("مانها") => {
					Viewer::Webtoon
				}
				_ => Viewer::Unknown,
			},
			None => Viewer::Unknown,
		};
		Manga {
			cover: self.cover.as_deref().map(|cover| {
				let name = cover.rsplit_once('.').map_or(cover, |(name, _)| name);
				format!(
					"{BASE_URL}/uploads/manga/cover/{}/large_{name}.webp",
					self.id
				)
			}),
			url: Some(format!("{BASE_URL}/series/{}", self.id)),
			authors: self.staff_with_role("Author"),
			artists: self.staff_with_role("Artist"),
			description: self.summary,
			tags: Some(self.categories.into_iter().map(|c| c.name).collect()),
			status: match self.translation_status.as_deref() {
				Some("ongoing") => MangaStatus::Ongoing,
				Some("completed") => MangaStatus::Completed,
				Some("dropped") | Some("cancelled") => MangaStatus::Cancelled,
				Some("hiatus") => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			},
			content_rating: if self.over17 {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			},
			viewer,
			key: self.id,
			title: self.title,
			..Default::default()
		}
	}
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest<'a> {
	pub query: &'a str,
	pub page: i32,
	pub series_type: IncludeExclude<String>,
	pub categories: IncludeExclude<i32>,
	pub oneshot: bool,
	pub story_status: IncludeExclude<String>,
	pub translation_status: IncludeExclude<String>,
	pub over17: Option<bool>,
}

#[derive(Serialize, Default)]
pub struct IncludeExclude<T> {
	pub include: Vec<T>,
	pub exclude: Vec<T>,
}

#[derive(Deserialize)]
pub struct ChapterList {
	pub chapters: Vec<ChapterReleases>,
}

#[derive(Deserialize)]
pub struct ChapterReleases {
	chapter: String,
	title: Option<String>,
	releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
	id: String,
	created_at: Option<String>,
	#[serde(default)]
	teams: Vec<Name>,
}

/// "32.00" -> "32", "32.50" -> "32.5"
fn chapter_label(number: &str) -> &str {
	if number.contains('.') {
		number.trim_end_matches('0').trim_end_matches('.')
	} else {
		number
	}
}

pub fn reader_url(series_id: &str, number: &str, release_id: &str) -> String {
	// the reader ignores the title segment, so any placeholder works
	format!("{BASE_URL}/reader/{series_id}/_/{number}?release={release_id}")
}

impl ChapterReleases {
	/// Each release is a separate upload of the chapter, usually by a different team.
	pub fn into_chapters(self, series_id: &str) -> impl Iterator<Item = Chapter> {
		let number = String::from(chapter_label(&self.chapter));
		let chapter_number = self.chapter.parse::<f32>().ok();
		let title = self.title.filter(|t| !t.trim().is_empty());
		let series_id = String::from(series_id);
		self.releases.into_iter().map(move |release| Chapter {
			url: Some(reader_url(&series_id, &number, &release.id)),
			key: release.id,
			title: title.clone(),
			chapter_number,
			date_uploaded: release
				.created_at
				.and_then(|d| parse_date(d, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'")),
			scanlators: if release.teams.is_empty() {
				None
			} else {
				Some(release.teams.into_iter().map(|t| t.name).collect())
			},
			..Default::default()
		})
	}
}

/// The page urls read off the site's reader in the web view.
#[derive(Deserialize)]
pub struct ReaderPages {
	/// Whether the urls came from the reader's own decrypted chapter data, rather than the
	/// images it has put on the page so far.
	pub exact: bool,
	pub urls: Vec<String>,
}
