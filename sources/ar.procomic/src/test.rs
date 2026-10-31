#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = Procomic::new();
	let list = source
		.get_search_manga_list(Some("billionaire".into()), 1, vec![])
		.unwrap();
	assert!(!list.entries.is_empty(), "empty search");
	print(format!("search: {} results", list.entries.len()));
	let first = list
		.entries
		.into_iter()
		.find(|m| m.key == "107")
		.unwrap_or_else(|| Manga {
			key: "107".into(),
			..Default::default()
		});

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"{}: status={:?} viewer={:?} rating={:?} authors={:?} tags={:?} cover={:?}",
		manga.title,
		manga.status,
		manga.viewer,
		manga.content_rating,
		manga.authors,
		manga.tags,
		manga.cover
	));
	let chapters = manga.chapters.clone().unwrap();
	assert!(!chapters.is_empty(), "no chapters");
	print(format!(
		"{} unlocked chapters; newest #{:?} title={:?} key={}",
		chapters.len(),
		chapters[0].chapter_number,
		chapters[0].title,
		chapters[0].key
	));

	// the second chapter from the end, so reruns don't keep reading the same one; repeat views
	// make the site ask for a Turnstile check, which only the app's web view can answer
	let chapter = chapters[chapters.len().saturating_sub(2)].clone();
	let pages = match source.get_page_list(manga, chapter) {
		Ok(pages) => pages,
		Err(err) => {
			print(format!("page list blocked: {err:?}"));
			return;
		}
	};
	print(format!("{} pages", pages.len()));
	assert!(pages.len() > 3, "deferred pages missing");

	// a signed page image
	if let Some(Page {
		content: PageContent::Url(url, Some(context)),
		..
	}) = pages
		.iter()
		.find(|p| matches!(&p.content, PageContent::Url(_, Some(c)) if c.contains_key("sign")))
	{
		// the signing endpoint refuses requests outside the site's own reader, so only report it
		match source
			.get_image_request(url.clone(), Some(context.clone()))
			.and_then(|r| Ok(r.send()?))
		{
			Ok(response) => print(format!(
				"signed image: {} {:?}",
				response.status_code(),
				response.get_header("Content-Type")
			)),
			Err(err) => print(format!("signing failed: {err:?}")),
		}
	}
}

#[aidoku_test]
fn listings_filters_deeplink() {
	let source = Procomic::new();
	for id in ["popular", "latest_chapter"] {
		let listing = Listing {
			id: id.into(),
			..Default::default()
		};
		let result = source.get_manga_list(listing, 1).unwrap();
		print(format!(
			"{id}: {} entries, next={}",
			result.entries.len(),
			result.has_next_page
		));
		assert!(!result.entries.is_empty());
	}

	let filtered = source
		.get_search_manga_list(
			None,
			1,
			vec![FilterValue::Select {
				id: "type".into(),
				value: "manhwa".into(),
			}],
		)
		.unwrap();
	print(format!("manhwa: {} entries", filtered.entries.len()));
	assert!(!filtered.entries.is_empty());

	let link = source
		.handle_deep_link(
			"https://procomic.pro/ar/after-improperly-licking-a-dog-i-became-a-billionaire-107"
				.into(),
		)
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { ref key }) if key == "107"));

	let link = source
		.handle_deep_link(
			"https://procomic.pro/ar/chapter/after-improperly-licking-a-dog-i-became-a-billionaire-0-5477"
				.into(),
		)
		.unwrap();
	print(format!("chapter deep link: {link:?}"));
	assert!(matches!(
		link,
		Some(DeepLinkResult::Chapter { ref manga_key, .. }) if manga_key == "107"
	));
}
