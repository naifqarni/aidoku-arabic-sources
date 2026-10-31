#![no_std]
use aidoku::{Source, prelude::*};
use zeistmanga::{Impl, Params, ZeistManga};

struct XSanoManga;

impl Impl for XSanoManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://www.xsano-manga.com".into(),
			details_selector: "main".into(),
			details_genres_selector: "dl a[rel=tag]".into(),
			details_info_selector: "aside dl".into(),
			details_info_title_selector: "dt".into(),
			details_info_description_selector: "dd".into(),
			page_list_selector: "#reader div.separator".into(),
			..Default::default()
		}
	}
}

register_source!(ZeistManga<XSanoManga>, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
