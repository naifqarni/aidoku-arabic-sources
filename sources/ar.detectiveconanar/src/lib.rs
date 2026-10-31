#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://manga.detectiveconanar.com";

struct DetectiveConanAr;

impl Impl for DetectiveConanAr {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_new_chapter_endpoint: true,
			datetime_format: "MMMM dd, yyyy".into(),
			datetime_locale: "ar".into(),
			details_status_selector: "div.summary-heading:contains(Status) + div, div.summary-heading:contains(الحالة) + div".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<DetectiveConanAr>,
	DynamicFilters,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);

#[cfg(test)]
mod test;
