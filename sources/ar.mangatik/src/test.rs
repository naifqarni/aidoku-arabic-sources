#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = MangaTik::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert!(!list.entries.is_empty(), "empty list");
	print(format!(
		"popular: {} entries, next={}",
		list.entries.len(),
		list.has_next_page
	));
	let first = list.entries.into_iter().next().unwrap();
	print(format!("first: {} ({})", first.title, first.key));

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"title={} status={:?} viewer={:?} tags={:?} authors={:?} cover={:?}",
		manga.title, manga.status, manga.viewer, manga.tags, manga.authors, manga.cover
	));
	let chapters = manga.chapters.clone().unwrap_or_default();
	assert!(!chapters.is_empty(), "no chapters");
	let ch = chapters.first().unwrap().clone();
	print(format!(
		"{} chapters; newest: {:?} #{:?} date={:?} key={}",
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
fn search_listing_deeplink() {
	let source = MangaTik::new();
	let found = source
		.get_search_manga_list(Some("one".into()), 1, vec![])
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
				id: "type".into(),
				value: "manhwa".into(),
			}],
		)
		.unwrap();
	assert!(!filtered.entries.is_empty(), "no filtered results");

	let latest = source.get_manga_list(Listing::default(), 1).unwrap();
	assert!(!latest.entries.is_empty(), "empty latest listing");
	print(format!(
		"latest: {} entries, next={}",
		latest.entries.len(),
		latest.has_next_page
	));

	let link = source
		.handle_deep_link("https://mangatik.com/manga/one-punch-man/235.0".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
	let link = source
		.handle_deep_link("https://mangatik.com/manga/one-punch-man".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
}
