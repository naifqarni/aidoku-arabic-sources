#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://mangalik.net";

struct Mangalek;

impl Impl for Mangalek {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			// the site labels most of its series as something other than manga
			filter_non_manga_items: false,
			datetime_format: "MMMM d, yyyy".into(),
			datetime_locale: "ar".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<Mangalek>,
	DynamicFilters,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
