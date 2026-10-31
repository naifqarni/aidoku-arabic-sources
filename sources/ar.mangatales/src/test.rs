#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = MangaTales::new();
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
	print(format!("desc={:?}", manga.description));
	let chapters = manga.chapters.clone().unwrap_or_default();
	assert!(!chapters.is_empty(), "no chapters");
	let ch = chapters.first().unwrap().clone();
	print(format!(
		"{} chapters; newest: {:?} #{:?} date={:?} scan={:?} key={}",
		chapters.len(),
		ch.title,
		ch.chapter_number,
		ch.date_uploaded,
		ch.scanlators,
		ch.key
	));

	let pages = source.get_page_list(manga, ch).unwrap();
	assert!(!pages.is_empty(), "no pages");
	print(format!("{} pages", pages.len()));
	if let PageContent::Url(url, _) = &pages[0].content {
		print(format!("first page: {url}"));
	}
}

#[aidoku_test]
fn search_listing_deeplink() {
	let source = MangaTales::new();
	let found = source
		.get_search_manga_list(Some("one".into()), 1, vec![])
		.unwrap();
	assert!(!found.entries.is_empty(), "no search results");
	print(format!(
		"search: {} results, first {} ({})",
		found.entries.len(),
		found.entries[0].title,
		found.entries[0].key
	));

	let filtered = source
		.get_search_manga_list(
			None,
			1,
			vec![
				FilterValue::MultiSelect {
					id: "categories".into(),
					included: vec!["2".into()],
					excluded: vec![],
				},
				FilterValue::MultiSelect {
					id: "story_status".into(),
					included: vec!["3".into()],
					excluded: vec![],
				},
			],
		)
		.unwrap();
	assert!(!filtered.entries.is_empty(), "no filtered results");
	print(format!("filtered: {} results", filtered.entries.len()));

	let latest = source.get_manga_list(Listing::default(), 1).unwrap();
	assert!(!latest.entries.is_empty(), "empty latest listing");
	print(format!(
		"latest: {} entries, next={}",
		latest.entries.len(),
		latest.has_next_page
	));

	let link = source
		.handle_deep_link("https://www.mangatales.com/mangas/2674".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Manga { .. })));
	let link = source
		.handle_deep_link("https://www.mangatales.com/r/3242".into())
		.unwrap();
	print(format!("chapter link: {link:?}"));
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
}

#[aidoku_test]
fn iso_date() {
	assert_eq!(
		parse_iso_date("2026-10-28T20:40:12.130Z"),
		Some(1_793_220_012)
	);
}
