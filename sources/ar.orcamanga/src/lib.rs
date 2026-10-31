#![no_std]
use aidoku::{Source, prelude::*};
use zeistmanga::{Impl, Params, ZeistManga};

struct OrcaManga;

impl Impl for OrcaManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://infinity896.blogspot.com".into(),
			..Default::default()
		}
	}
}

register_source!(ZeistManga<OrcaManga>, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
