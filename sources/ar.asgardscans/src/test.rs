#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = AsgardScans::new();
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
	print(format!(
		"first: {} ({}) cover={:?}",
		first.title, first.key, first.cover
	));

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"status={:?} viewer={:?} tags={:?} desc={:?}",
		manga.status,
		manga.viewer,
		manga.tags,
		manga
			.description
			.as_deref()
			.map(|d| d.chars().take(60).collect::<String>())
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
fn search_filters_deeplink() {
	let source = AsgardScans::new();
	let found = source
		.get_search_manga_list(Some("solo".into()), 1, vec![])
		.unwrap();
	assert!(!found.entries.is_empty(), "no search results");
	print(format!("search: {} results", found.entries.len()));

	let filtered = source
		.get_search_manga_list(
			None,
			1,
			vec![FilterValue::Select {
				id: "type".into(),
				value: "MANGA".into(),
			}],
		)
		.unwrap();
	assert!(!filtered.entries.is_empty(), "no filter results");
	print(format!("type=MANGA: {} results", filtered.entries.len()));

	let latest = source.get_manga_list(Listing::default(), 2).unwrap();
	assert!(!latest.entries.is_empty(), "empty latest page 2");
	print(format!("latest p2: {} entries", latest.entries.len()));

	assert_eq!(parse_date("13/9/2026"), Some(1789257600));
	assert_eq!(
		parse_chapter_name("الفصل 12.5"),
		(Some(12.5), None::<String>)
	);

	let link = source
		.handle_deep_link("https://asgardscans.com/manga/mia-is-back/44".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
}
