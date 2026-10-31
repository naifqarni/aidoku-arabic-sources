#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = MangaTek::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert!(!list.entries.is_empty(), "empty list");
	print(format!(
		"popular: {} entries, next={}",
		list.entries.len(),
		list.has_next_page
	));
	let first = list.entries.into_iter().next().unwrap();
	print(format!(
		"first: {} ({}) {:?}",
		first.title, first.key, first.cover
	));

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"title={} status={:?} viewer={:?} rating={:?} tags={:?} authors={:?}",
		manga.title, manga.status, manga.viewer, manga.content_rating, manga.tags, manga.authors
	));
	let chapters = manga.chapters.clone().unwrap_or_default();
	assert!(!chapters.is_empty(), "no chapters");
	let ch = chapters.last().unwrap().clone();
	print(format!(
		"{} chapters; oldest: {:?} #{:?} date={:?} key={}",
		chapters.len(),
		ch.title,
		ch.chapter_number,
		ch.date_uploaded,
		ch.key
	));

	let pages = source.get_page_list(manga, ch).unwrap();
	assert!(!pages.is_empty(), "no pages");
	print(format!("{} pages", pages.len()));
}

#[aidoku_test]
fn overlay_chapter() {
	let source = MangaTek::new();
	let latest = source.get_manga_list(Listing::default(), 1).unwrap();
	assert!(!latest.entries.is_empty(), "empty latest listing");
	print(format!(
		"latest: {} entries, next={}",
		latest.entries.len(),
		latest.has_next_page
	));

	// the newest chapter of a recently updated series is usually an overlay chapter
	let manga = Manga {
		key: "/manga/magic-emperor".into(),
		..Default::default()
	};
	let chapter = Chapter {
		key: "/reader/magic-emperor/82".into(),
		..Default::default()
	};
	let pages = source.get_page_list(manga, chapter).unwrap();
	let with_overlays: Vec<&Page> = pages
		.iter()
		.filter(|p| matches!(&p.content, PageContent::Url(_, Some(_))))
		.collect();
	print(format!(
		"{} pages, {} with overlays",
		pages.len(),
		with_overlays.len()
	));
	assert!(!with_overlays.is_empty(), "no overlays unlocked");
	if let PageContent::Url(url, Some(context)) = &with_overlays[0].content {
		let json = context.get(OVERLAYS_KEY).unwrap();
		let bubbles: Vec<Bubble> = serde_json::from_str(json).unwrap();
		print(format!(
			"{} bubbles; first: {:?} font={} size={} authored at {:?}",
			bubbles.len(),
			bubbles[0].text,
			bubbles[0].font_family,
			bubbles[0].font_size_px,
			context.get(SIZE_KEY)
		));

		// paint the bubbles onto the real page
		let data = request(url).unwrap().data().unwrap();
		let image = ImageRef::new(&data);
		let painted = painter::paint(&image, &bubbles, Some((image.width(), image.height())));
		print(format!(
			"page {}x{} painted {}x{}",
			image.width(),
			image.height(),
			painted.width(),
			painted.height()
		));
		assert_eq!(painted.width(), image.width());
	}
}

#[aidoku_test]
fn search_deeplink() {
	let source = MangaTek::new();
	let found = source
		.get_search_manga_list(Some("solo".into()), 1, vec![])
		.unwrap();
	assert!(!found.entries.is_empty(), "no search results");
	print(format!(
		"search: {} results, first {}",
		found.entries.len(),
		found.entries[0].key
	));
	let filtered = source
		.get_search_manga_list(
			None,
			1,
			vec![FilterValue::Select {
				id: "status".into(),
				value: "completed".into(),
			}],
		)
		.unwrap();
	assert!(!filtered.entries.is_empty(), "no filtered results");

	let link = source
		.handle_deep_link("https://mangatek.com/reader/magic-emperor/929".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
	let link = source
		.handle_deep_link("https://mangatek.com/manga/magic-emperor".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
}

#[aidoku_test]
fn crypto_helpers() {
	assert_eq!(crypto::hex(&[0x0f, 0xa0]), "0fa0");
	assert_eq!(crypto::decode_hex("0fa0"), Some(vec![0x0f, 0xa0]));
}
