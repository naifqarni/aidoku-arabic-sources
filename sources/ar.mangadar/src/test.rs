#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = MangaDar::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert!(!list.entries.is_empty(), "empty list");
	print(format!(
		"list: {} entries, next={}",
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
		"status={:?} viewer={:?} authors={:?} tags={:?} cover={:?}",
		manga.status, manga.viewer, manga.authors, manga.tags, manga.cover
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
	print(format!(
		"{} pages, first {:?}",
		pages.len(),
		pages[0].content
	));
}

#[aidoku_test]
fn search_listing_deeplink() {
	let source = MangaDar::new();
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
			2,
			vec![FilterValue::Select {
				id: "type".into(),
				value: "manhwa".into(),
			}],
		)
		.unwrap();
	assert!(!filtered.entries.is_empty(), "empty filtered list");
	print(format!("manhwa p2: {} entries", filtered.entries.len()));

	let popular = source
		.get_manga_list(
			Listing {
				id: "popular".into(),
				..Default::default()
			},
			1,
		)
		.unwrap();
	assert!(!popular.entries.is_empty(), "empty popular listing");
	print(format!("popular: {} entries", popular.entries.len()));

	let one_piece = source
		.get_manga_update(
			Manga {
				key: "one-piece".into(),
				..Default::default()
			},
			true,
			false,
		)
		.unwrap();
	print(format!(
		"one piece: status={:?} viewer={:?} authors={:?}",
		one_piece.status, one_piece.viewer, one_piece.authors
	));
	assert_eq!(one_piece.status, MangaStatus::Ongoing);

	let link = source
		.handle_deep_link("https://mangadar.com/manga/one-piece/1197/".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
	let link = source
		.handle_deep_link("https://mangadar.com/manga/one-piece/".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
}
