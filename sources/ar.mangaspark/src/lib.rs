#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://sparkmanga.net";

struct MangaSpark;

impl Impl for MangaSpark {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			datetime_format: "d MMMM\u{060c} yyyy".into(),
			datetime_locale: "ar".into(),
			details_status_selector: "div.summary-heading:contains(Status) + div, div.summary-heading:contains(الحالة) + div".into(),
			page_list_selector: "div.wp-manga-chapter-img img, div.reading-content img".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<MangaSpark>,
	DynamicFilters,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);

#[cfg(test)]
mod test;
