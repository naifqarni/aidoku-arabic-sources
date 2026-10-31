#![no_std]
use aidoku::{
	Chapter, Manga, Page, PageContent, Result, Source,
	alloc::{String, Vec},
	imports::{html::Html, net::Request},
	prelude::*,
};
use mangathemesia::{Impl, MangaThemesia, Params, helpers::ElementImageAttr};
use serde::Deserialize;

const BASE_URL: &str = "https://ar.kenmanga.com";

struct AreaManga;

#[derive(Deserialize)]
struct SecureChapterResponse {
	success: bool,
	data: SecureChapterData,
}

#[derive(Deserialize)]
struct SecureChapterData {
	status: String,
	content: Option<String>,
}

impl Impl for AreaManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			date_format: "yyyy/MM/dd".into(),
			search_manga_selector: ".listupd .manga-card-v".into(),
			search_manga_title_selector: ".bigor .tt, h3 a".into(),
			series_details_selector: "div.legendary-single-page".into(),
			series_title_selector: ".manga-title-large".into(),
			series_cover_selector: ".manga-poster img".into(),
			series_description_selector: "div.story-text".into(),
			series_genre_selector: "div.filter-tags a".into(),
			series_status_selector: ".info-label:contains(الحالة) + span".into(),
			series_author_selector: ".info-label:contains(المؤلف) + span".into(),
			series_artist_selector: ".info-label:contains(الرسام) + span".into(),
			chapter_list_selector: "#chapters-list-container .ch-item".into(),
			chapter_title_selector: ".chap-num".into(),
			chapter_date_selector: ".chap-date".into(),
			..Default::default()
		}
	}

	// chapter images are served by an ajax endpoint instead of the reader page
	fn get_page_list(
		&self,
		_params: &Params,
		_manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(&url)?.html()?;
		let chapter_id = html
			.select_first("#comment_post_ID")
			.and_then(|el| el.attr("value"))
			.filter(|id| !id.trim().is_empty())
			.ok_or_else(|| error!("Chapter ID not found."))?;

		let response: SecureChapterResponse =
			Request::post(format!("{BASE_URL}/wp-admin/admin-ajax.php"))?
				.header("Referer", &url)
				.header("Content-Type", "application/x-www-form-urlencoded")
				.header("X-Requested-With", "XMLHttpRequest")
				.body(format!(
					"action=get_secure_chapter_images&chapter_id={}",
					chapter_id.trim()
				))
				.json_owned()?;
		if !response.success {
			bail!("Failed to load chapter.");
		}

		match response.data.status.as_str() {
			"unlocked" => {
				let content = response
					.data
					.content
					.ok_or_else(|| error!("Chapter content missing."))?;
				let doc = Html::parse_fragment_with_url(content, BASE_URL)?;
				Ok(doc
					.select("img")
					.map(|els| {
						els.filter_map(|img| {
							Some(Page {
								content: PageContent::url(img.img_attr()?),
								..Default::default()
							})
						})
						.collect()
					})
					.unwrap_or_default())
			}
			"locked" => bail!("Chapter locked. Open in WebView to unlock."),
			_ => Ok(Vec::new()),
		}
	}
}

register_source!(
	MangaThemesia<AreaManga>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
