#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://manga-lionz.org";

struct MangaLionz;

impl Impl for MangaLionz {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			datetime_format: "MMMM d, yyyy".into(),
			datetime_locale: "ar".into(),
			details_status_selector: "div.summary-heading:contains(Status) + div, div.summary-heading:contains(الحالة) + div".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<MangaLionz>,
	DynamicFilters,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);

#[cfg(test)]
mod test;
