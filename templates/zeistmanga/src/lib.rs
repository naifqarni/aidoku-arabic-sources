#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, Result, Source,
	alloc::{String, Vec, borrow::Cow},
	imports::net::{TimeUnit, set_rate_limit},
};

pub mod helpers;
mod imp;
pub mod models;

pub use imp::{Impl, fetch_chapter_list};

pub struct Params {
	pub base_url: Cow<'static, str>,
	/// Blogger label shared by every series post.
	pub manga_category: Cow<'static, str>,
	/// Blogger label shared by every chapter post.
	pub chapter_category: Cow<'static, str>,
	/// Series carrying any of these labels are hidden from listings.
	pub excluded_categories: &'static [&'static str],
	/// Prefer a chapter's last update date over its publish date.
	pub prefer_chapter_updated_date: bool,
	/// Requests allowed per second.
	pub rate_limit: Option<i32>,
	pub details_selector: Cow<'static, str>,
	pub details_genres_selector: Cow<'static, str>,
	/// Rows of "label: value" info, e.g. status or author.
	pub details_info_selector: Cow<'static, str>,
	/// Label of an info row, used when the row has no own text.
	pub details_info_title_selector: Cow<'static, str>,
	pub details_info_description_selector: Cow<'static, str>,
	/// Container of the chapter images.
	pub page_list_selector: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			manga_category: "Series".into(),
			chapter_category: "Chapter".into(),
			excluded_categories: &["Anime", "Novel", "Novela"],
			prefer_chapter_updated_date: false,
			rate_limit: None,
			details_selector: ".grid.gtc-235fr".into(),
			details_genres_selector: "div.mt-15 > a[rel=tag]".into(),
			details_info_selector: ".y6x11p".into(),
			details_info_title_selector: "strong".into(),
			details_info_description_selector: "span.dt".into(),
			page_list_selector: "div.check-box div.separator".into(),
		}
	}
}

pub struct ZeistManga<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for ZeistManga<T> {
	fn new() -> Self {
		let inner = T::new();
		let params = inner.params();
		if let Some(permits) = params.rate_limit {
			set_rate_limit(permits, 1, TimeUnit::Seconds);
		}
		Self { inner, params }
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		self.inner
			.get_search_manga_list(&self.params, query, page, filters)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		self.inner
			.get_manga_update(&self.params, manga, needs_details, needs_chapters)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		self.inner.get_page_list(&self.params, manga, chapter)
	}
}

impl<T: Impl> ListingProvider for ZeistManga<T> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> DeepLinkHandler for ZeistManga<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
