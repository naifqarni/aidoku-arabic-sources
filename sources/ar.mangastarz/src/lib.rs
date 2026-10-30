#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://starzmanga.com";

struct MangaStarz;

impl Impl for MangaStarz {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			datetime_format: "d MMMM\u{060c} yyyy".into(),
			datetime_locale: "ar".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<MangaStarz>,
	DynamicFilters,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
