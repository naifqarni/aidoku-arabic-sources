#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://anyonemanga.com";

struct AnyoneManga;

impl Impl for AnyoneManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_new_chapter_endpoint: true,
			datetime_format: "MM/dd/yyyy".into(),
			search_manga_selector: ".am-search-card, .am-manga-card".into(),
			search_manga_url_selector: ".am-search-card__title a, .am-manga-card__title a".into(),
			search_manga_title_selector: ".am-search-card__title a, .am-manga-card__title a".into(),
			details_title_selector: ".am-manga-hero__title".into(),
			details_cover_selector: ".am-manga-hero__cover img".into(),
			details_author_selector: ".am-manga-meta tr:has(td:contains(Author)) td:nth-child(2) a"
				.into(),
			details_artist_selector: ".am-manga-meta tr:has(td:contains(Artist)) td:nth-child(2) a"
				.into(),
			details_description_selector: ".am-manga-summary__text".into(),
			details_tag_selector: ".am-genres-wrap a".into(),
			details_status_selector: ".am-manga-meta tr:has(td:contains(Status)) td:nth-child(2)"
				.into(),
			details_type_selector: ".am-manga-meta tr:has(td:contains(Type)) td:nth-child(2)"
				.into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<AnyoneManga>,
	DynamicFilters,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);

#[cfg(test)]
mod test;
