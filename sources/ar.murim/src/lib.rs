#![no_std]
use aidoku::{Source, prelude::*};
use zeistmanga::{Impl, Params, ZeistManga};

struct Murim;

impl Impl for Murim {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://murim-manga.blogspot.com".into(),
			rate_limit: Some(2),
			..Default::default()
		}
	}
}

register_source!(ZeistManga<Murim>, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
