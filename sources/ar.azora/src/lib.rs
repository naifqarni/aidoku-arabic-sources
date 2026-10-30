#![no_std]
use aidoku::{Source, prelude::*};
use iken::{Iken, Impl, Params};

const BASE_URL: &str = "https://azorafly.com";
const API_URL: &str = "https://api.azorafly.com";

struct Azora;

impl Impl for Azora {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: Some(API_URL.into()),
			fetch_full_chapter_list: true,
			..Default::default()
		}
	}
}

register_source!(Iken<Azora>, Home, DeepLinkHandler);

#[cfg(test)]
mod test;
