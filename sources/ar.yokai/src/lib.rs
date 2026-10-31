#![no_std]
use aidoku::{
	Chapter, Result, Source,
	alloc::{String, Vec},
	imports::html::Document,
	prelude::*,
};
use zeistmanga::{Impl, Params, ZeistManga, helpers};

struct Yokai;

impl Impl for Yokai {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://yokai-team.blogspot.com".into(),
			prefer_chapter_updated_date: true,
			..Default::default()
		}
	}

	// some series also list chapters as static pages below the feed
	fn get_chapter_list(
		&self,
		params: &Params,
		feed_url: &str,
		html: &Document,
	) -> Result<Vec<Chapter>> {
		let mut chapters = zeistmanga::fetch_chapter_list(params, feed_url)?;
		if let Some(links) = html.select("div#download > div.index-list > a") {
			for link in links {
				let (Some(url), Some(title)) = (link.attr("abs:href"), link.text()) else {
					continue;
				};
				let key: String = helpers::url_path(&url).into();
				let path = key.split('?').next().unwrap_or_default();
				if chapters
					.iter()
					.any(|c| c.key.split('?').next() == Some(path))
				{
					continue;
				}
				let (chapter_number, volume_number) = helpers::parse_chapter_numbers(&title);
				chapters.push(Chapter {
					key,
					title: Some(title),
					chapter_number,
					volume_number,
					url: Some(url),
					..Default::default()
				});
			}
		}
		Ok(chapters)
	}
}

register_source!(ZeistManga<Yokai>, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod test;
