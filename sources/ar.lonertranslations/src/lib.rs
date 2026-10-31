#![no_std]
use aidoku::{Source, prelude::*};
use zeistmanga::{Impl, Params, ZeistManga};

struct LonerTranslations;

impl Impl for LonerTranslations {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://loner-tl.blogspot.com".into(),
			..Default::default()
		}
	}
}

register_source!(
	ZeistManga<LonerTranslations>,
	ListingProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
