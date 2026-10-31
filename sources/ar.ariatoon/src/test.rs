#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = AriaToon::new();
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
		"status={:?} authors={:?} cover={:?}",
		manga.status, manga.authors, manga.cover
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
fn search_filters_deeplink() {
	let source = AriaToon::new();
	let found = source
		.get_search_manga_list(Some("زنوبيا".into()), 1, vec![])
		.unwrap();
	assert!(!found.entries.is_empty(), "no search results");
	print(format!("search: {} results", found.entries.len()));

	let genre = source
		.get_search_manga_list(
			None,
			1,
			vec![FilterValue::Select {
				id: "genre".into(),
				value: "38c19662-9798-4c60-8f24-5c8d5331fa57".into(),
			}],
		)
		.unwrap();
	assert!(!genre.entries.is_empty(), "no genre results");
	print(format!("genre: {} results", genre.entries.len()));

	let latest = source.get_manga_list(Listing::default(), 1).unwrap();
	assert!(!latest.entries.is_empty(), "empty latest listing");

	// a series with paid episodes: they must be filtered out
	let manga = source
		.get_manga_update(
			Manga {
				key: "bb379def-18d5-486d-9cff-e96d0303fc19".into(),
				..Default::default()
			},
			false,
			true,
		)
		.unwrap();
	print(format!(
		"free chapters: {}",
		manga.chapters.unwrap_or_default().len()
	));

	assert_eq!(
		parse_iso_date("2025-07-14T16:30:02.498699Z"),
		Some(1752510602)
	);

	let link = source
		.handle_deep_link(
			"https://ariatoon.com/series/manga/bc6024d1-e099-4e95-a7dc-d0399a54009f".into(),
		)
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
}
