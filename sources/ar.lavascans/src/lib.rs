#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://lavascans.com";

struct LavaScans;

impl Impl for LavaScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			date_format: "yyyy/MM/dd".into(),
			search_manga_selector: ".listupd .manga-card-v".into(),
			search_manga_title_selector: ".card-v-title a, .bigor .tt, h3 a".into(),
			series_details_selector: "div.lh-container".into(),
			series_title_selector: ".lh-title".into(),
			series_cover_selector: ".lh-poster img".into(),
			series_description_selector: "#manga-story".into(),
			series_genre_selector: ".lh-genres a".into(),
			series_status_selector: ".status-badge-lux".into(),
			// locked chapters are paid and can't be read
			chapter_list_selector: "#chapters-list-container .ch-item:not(.locked)".into(),
			chapter_title_selector: ".ch-num".into(),
			chapter_date_selector: ".ch-date".into(),
			..Default::default()
		}
	}
}

register_source!(MangaThemesia<LavaScans>, Home, DeepLinkHandler);

#[cfg(test)]
mod test;
