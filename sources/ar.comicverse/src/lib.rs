#![no_std]
use aidoku::{Source, alloc::String, imports::html::Document, prelude::*};
use zeistmanga::{Impl, Params, ZeistManga};

struct ComicVerse;

impl Impl for ComicVerse {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://arcomixverse.blogspot.com".into(),
			details_genres_selector: "a.label[rel=tag]".into(),
			page_list_selector: "#cvOriginal".into(),
			..Default::default()
		}
	}

	// chapters are labelled with the series' `data-label`, which can differ from its title
	fn chapter_label(&self, _params: &Params, html: &Document, title: &str) -> String {
		html.select_first("[data-label]")
			.and_then(|el| el.attr("data-label"))
			.unwrap_or_else(|| title.into())
	}
}

register_source!(ZeistManga<ComicVerse>, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
