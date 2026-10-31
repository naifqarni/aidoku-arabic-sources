#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = Eshadow::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert!(!list.entries.is_empty(), "empty list");
	print(format!(
		"list: {} entries, next={}",
		list.entries.len(),
		list.has_next_page
	));

	let found = source
		.get_search_manga_list(Some("blue".into()), 1, vec![])
		.unwrap();
	assert!(!found.entries.is_empty(), "no search results");
	let first = found.entries.into_iter().next().unwrap();
	print(format!("search first: {} ({})", first.title, first.key));

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"status={:?} authors={:?} tags={:?} url={:?}",
		manga.status, manga.authors, manga.tags, manga.url
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
fn deep_link() {
	let source = Eshadow::new();
	let link = source
		.handle_deep_link("https://eshadow.net/manga/blue-lock".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
	let link = source
		.handle_deep_link("https://eshadow.net/read/cmq58alqy0005du1sskkaph9t".into())
		.unwrap();
	print(format!("{link:?}"));
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
	assert_eq!(parse_iso_date("2026-06-08T13:10:13.403Z"), Some(1780924213));
}
