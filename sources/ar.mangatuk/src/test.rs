#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = MangaTuk::new();
	let list = source
		.get_search_manga_list(Some("solo".into()), 1, vec![])
		.unwrap();
	assert!(!list.entries.is_empty(), "empty search");
	print(format!("search: {} results", list.entries.len()));
	let first = list.entries.into_iter().next().unwrap();
	print(format!("first: {} ({})", first.title, first.key));

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"status={:?} viewer={:?} rating={:?} authors={:?} tags={:?} url={:?}",
		manga.status, manga.viewer, manga.content_rating, manga.authors, manga.tags, manga.url
	));
	let chapters = manga.chapters.clone().unwrap();
	assert!(!chapters.is_empty(), "no chapters");
	let ch = chapters.first().unwrap().clone();
	print(format!(
		"{} chapters; newest #{:?} title={:?} date={:?} url={:?}",
		chapters.len(),
		ch.chapter_number,
		ch.title,
		ch.date_uploaded,
		ch.url
	));

	let pages = source.get_page_list(manga, ch).unwrap();
	assert!(!pages.is_empty(), "no pages");
	print(format!("{} pages", pages.len()));
}

#[aidoku_test]
fn listings_filters_deeplink() {
	let source = MangaTuk::new();
	for id in ["popular", "latest_updates"] {
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
			2,
			vec![FilterValue::Select {
				id: "status".into(),
				value: "completed".into(),
			}],
		)
		.unwrap();
	print(format!("completed p2: {} entries", filtered.entries.len()));
	assert!(!filtered.entries.is_empty());

	let link = source
		.handle_deep_link("https://mangatuk.com/series/solo-leveling".into())
		.unwrap();
	print(format!("deep link: {link:?}"));
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));

	let link = source
		.handle_deep_link("https://mangatuk.com/series/solo-leveling/200".into())
		.unwrap();
	print(format!("chapter deep link: {link:?}"));
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
}
