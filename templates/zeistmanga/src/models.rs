use crate::helpers;
use aidoku::{
	Chapter, Manga,
	alloc::{String, Vec},
	imports::html::Html,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct FeedResponse {
	pub feed: Feed,
}

#[derive(Deserialize)]
pub struct EntryResponse {
	pub entry: Entry,
}

#[derive(Deserialize)]
pub struct Feed {
	#[serde(rename = "openSearch$totalResults")]
	total_results: Option<Text>,
	#[serde(rename = "openSearch$itemsPerPage")]
	items_per_page: Option<Text>,
	#[serde(default)]
	pub entry: Vec<Entry>,
}

impl Feed {
	pub fn total_results(&self) -> Option<usize> {
		self.total_results.as_ref().and_then(|t| t.t.parse().ok())
	}

	pub fn items_per_page(&self) -> Option<usize> {
		self.items_per_page.as_ref().and_then(|t| t.t.parse().ok())
	}
}

#[derive(Deserialize)]
pub struct Text {
	#[serde(rename = "$t")]
	pub t: String,
}

#[derive(Deserialize)]
pub struct Category {
	pub term: String,
}

#[derive(Deserialize)]
pub struct Link {
	pub rel: String,
	pub href: String,
}

#[derive(Deserialize)]
pub struct Thumbnail {
	pub url: String,
}

#[derive(Deserialize)]
pub struct Entry {
	pub title: Text,
	pub published: Option<Text>,
	pub updated: Option<Text>,
	#[serde(default)]
	pub category: Vec<Category>,
	#[serde(default)]
	pub link: Vec<Link>,
	pub content: Option<Text>,
	#[serde(rename = "media$thumbnail")]
	pub thumbnail: Option<Thumbnail>,
}

impl Entry {
	pub fn has_category(&self, term: &str) -> bool {
		self.category.iter().any(|c| c.term == term)
	}

	/// The post's public URL.
	pub fn url(&self) -> Option<&str> {
		self.link
			.iter()
			.find(|l| l.rel == "alternate")
			.map(|l| l.href.as_str())
	}

	pub fn into_manga(self) -> Option<Manga> {
		let url = self.url()?;
		let key = helpers::url_path(url).into();
		let cover = match self.thumbnail {
			Some(thumbnail) => Some(helpers::resize_thumbnail(&thumbnail.url)),
			None => self.content.and_then(|content| {
				Html::parse_fragment(content.t)
					.ok()?
					.select_first("img")?
					.attr("src")
			}),
		};
		Some(Manga {
			key,
			title: self.title.t.trim().into(),
			cover,
			..Default::default()
		})
	}

	pub fn into_chapter(self, prefer_updated: bool) -> Option<Chapter> {
		let url: String = self.url()?.into();
		let (published, updated) = (self.published, self.updated);
		let date = if prefer_updated {
			updated.or(published)
		} else {
			published.or(updated)
		};
		let title = self.title.t.trim();
		let (chapter_number, volume_number) = helpers::parse_chapter_numbers(title);
		Some(Chapter {
			key: helpers::url_path(&url).into(),
			title: (!helpers::is_generic_chapter_title(title)).then(|| title.into()),
			chapter_number,
			volume_number,
			date_uploaded: date.and_then(|d| helpers::parse_iso_date(&d.t)),
			url: Some(url),
			..Default::default()
		})
	}
}
