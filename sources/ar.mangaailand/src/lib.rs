#![no_std]
use aidoku::{Source, prelude::*};
use zeistmanga::{Impl, Params, ZeistManga};

struct MangaAiLand;

impl Impl for MangaAiLand {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://manga-ai-land.blogspot.com".into(),
			chapter_category: "فصل".into(),
			..Default::default()
		}
	}
}

register_source!(ZeistManga<MangaAiLand>, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
