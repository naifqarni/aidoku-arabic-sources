#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://3asq.online";

struct Manga3asq;

impl Impl for Manga3asq {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_new_chapter_endpoint: true,
			use_paginated_chapter_endpoint: true,
			// arabic has no abbreviated month names, so MMM and MMMM are the same
			datetime_format: "d MMMM\u{060c} yyyy".into(),
			datetime_locale: "ar".into(),
			search_manga_url_selector: "div.post-title a:not([target])".into(),
			search_manga_title_selector: "div.post-title a:not([target])".into(),
			details_description_selector: "div.description-summary div.summary__content, div.manga-excerpt".into(),
			details_status_selector: "div.summary-heading:contains(Status) + div, div.summary-heading:contains(الحالة) + div".into(),
			chapter_date_selector: "span.chapter-release-date .timediff".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<Manga3asq>,
	DynamicFilters,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);

#[cfg(test)]
mod test;
