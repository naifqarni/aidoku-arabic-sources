#![no_std]
use aidoku::{
	Chapter, ImageResponse, Manga, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec},
	imports::{
		canvas::{Canvas, ImageRef, Rect},
		html::Html,
		net::Request,
	},
	prelude::*,
};
use mangathemesia::{
	Impl, MangaThemesia, Params,
	helpers::{self, ElementImageAttr},
};

const BASE_URL: &str = "https://hijala.com";

// context key holding the url of the right half of a scrambled page
const RIGHT_IMAGE_KEY: &str = "rightImage";

struct Hijala;

impl Impl for Hijala {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			..Default::default()
		}
	}

	fn get_page_list(
		&self,
		_params: &Params,
		_manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let response = Request::get(&url)?.string()?;
		let html = Html::parse_fragment_with_url(&response, &url)?;

		let mut images: Vec<String> = html
			.select("div#readerarea img")
			.map(|els| els.filter_map(|el| el.img_attr()).collect())
			.unwrap_or_default();
		if images.is_empty() && response.contains("\"images\":") {
			images = helpers::extract_images(&response);
		}
		if images.is_empty() {
			bail!("No pages found");
		}

		// scrambled chapters split every page into a left and a right half
		if html.select_first("#chapter-pages-js-before").is_none() {
			return Ok(images
				.into_iter()
				.map(|url| Page {
					content: PageContent::url(url),
					..Default::default()
				})
				.collect());
		}
		Ok(images
			.as_chunks::<2>()
			.0
			.iter()
			.map(|[left, right]| {
				let mut context = PageContext::new();
				context.insert(RIGHT_IMAGE_KEY.into(), right.clone());
				Page {
					content: PageContent::url_context(left, context),
					..Default::default()
				}
			})
			.collect())
	}

	fn process_page_image(
		&self,
		params: &Params,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(right_url) = context.as_ref().and_then(|c| c.get(RIGHT_IMAGE_KEY)) else {
			return Ok(response.image);
		};
		let left = response.image;
		let right = self
			.get_image_request(params, right_url.clone(), None)?
			.image()?;

		let width = left.width() + right.width();
		let height = left.height().max(right.height());
		let mut canvas = Canvas::new(width, height);
		canvas.draw_image(&left, Rect::new(0.0, 0.0, left.width(), left.height()));
		canvas.draw_image(
			&right,
			Rect::new(left.width(), 0.0, right.width(), right.height()),
		);
		Ok(canvas.get_image())
	}
}

register_source!(
	MangaThemesia<Hijala>,
	Home,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
