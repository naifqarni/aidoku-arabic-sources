#![no_std]
use aidoku::{Source, prelude::*};
use zeistmanga::{Impl, Params, ZeistManga};

struct Manhatok;

impl Impl for Manhatok {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://manhatok.blogspot.com".into(),
			rate_limit: Some(3),
			..Default::default()
		}
	}
}

register_source!(ZeistManga<Manhatok>, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
