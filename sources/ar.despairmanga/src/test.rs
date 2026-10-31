#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = DespairManga::new();
	let list = source
		.get_manga_list(
			Listing {
				id: "popular".into(),
				..Default::default()
			},
			1,
		)
		.unwrap();
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
		"status={:?} viewer={:?} tags={:?} authors={:?}",
		manga.status, manga.viewer, manga.tags, manga.authors
	));
	let chapters = manga.chapters.clone().unwrap_or_default();
	assert!(!chapters.is_empty(), "no chapters");
	let locked = chapters.iter().filter(|c| c.locked).count();
	let ch = chapters.iter().find(|c| !c.locked).unwrap().clone();
	print(format!(
		"{} chapters ({locked} locked); newest free: {:?} #{:?} date={:?} key={}",
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
	let source = DespairManga::new();
	let found = source
		.get_search_manga_list(Some("solo".into()), 1, vec![])
		.unwrap();
	assert!(!found.entries.is_empty(), "no search results");
	print(format!(
		"search: {} results, first {}",
		found.entries.len(),
		found.entries[0].key
	));

	let latest = source.get_manga_list(Listing::default(), 1).unwrap();
	assert!(!latest.entries.is_empty(), "empty latest listing");
	print(format!("latest: {} entries", latest.entries.len()));

	// chapters only (no details request)
	let manga = source
		.get_manga_update(
			Manga {
				key: "solo-leveling".into(),
				..Default::default()
			},
			false,
			true,
		)
		.unwrap();
	print(format!(
		"solo-leveling: {} chapters",
		manga.chapters.unwrap_or_default().len()
	));

	assert_eq!(
		parse_iso_date("2025-01-25T19:28:02+00:00"),
		Some(1737833282)
	);

	let link = source
		.handle_deep_link("https://despair-world.com/series/solo-leveling/main/ch/1".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
}
