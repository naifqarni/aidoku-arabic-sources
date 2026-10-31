#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = KawiiManga::new();
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
		"status={:?} viewer={:?} authors={:?} tags={:?}",
		manga.status, manga.viewer, manga.authors, manga.tags
	));
	let chapters = manga.chapters.clone().unwrap_or_default();
	assert!(!chapters.is_empty(), "no chapters");
	let ch = chapters.first().unwrap().clone();
	print(format!(
		"{} chapters; newest: {:?} #{:?} date={:?} key={} url={:?}",
		chapters.len(),
		ch.title,
		ch.chapter_number,
		ch.date_uploaded,
		ch.key,
		ch.url
	));

	let pages = source.get_page_list(manga, ch).unwrap();
	assert!(!pages.is_empty(), "no pages");
	print(format!("{} pages", pages.len()));
}

#[aidoku_test]
fn search_listing_deeplink() {
	let source = KawiiManga::new();
	let found = source
		.get_search_manga_list(Some("duke".into()), 1, vec![])
		.unwrap();
	assert!(!found.entries.is_empty(), "no search results");
	print(format!("search: {} results", found.entries.len()));

	let latest = source
		.get_manga_list(
			Listing {
				id: "latest".into(),
				..Default::default()
			},
			2,
		)
		.unwrap();
	assert!(!latest.entries.is_empty(), "empty latest listing");
	print(format!("latest p2: {} entries", latest.entries.len()));

	let link = source
		.handle_deep_link("https://kawaiimanga.org/manga/i-only-need-the-dukes-child".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
	let link = source
		.handle_deep_link("https://kawaiimanga.org/reader/i-only-need-the-dukes-child/33".into())
		.unwrap();
	print(format!("{link:?}"));
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
}
