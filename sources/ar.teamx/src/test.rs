#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = TeamX::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert!(!list.entries.is_empty(), "empty list");
	let first = list.entries.into_iter().next().unwrap();
	print(format!("first: {} ({})", first.title, first.key));

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"status={:?} viewer={:?} tags={:?} desc={:?}",
		manga.status,
		manga.viewer,
		manga.tags,
		manga
			.description
			.as_deref()
			.map(|d| d.chars().take(60).collect::<aidoku::alloc::String>())
	));
	let chapters = manga.chapters.clone().unwrap_or_default();
	assert!(!chapters.is_empty(), "no chapters");
	let newest = chapters.first().unwrap();
	print(format!(
		"newest: {:?} #{:?} date={:?} key={}",
		newest.title, newest.chapter_number, newest.date_uploaded, newest.key
	));
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
fn search_listing_deeplink() {
	let source = TeamX::new();
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
	print(format!(
		"latest: {} entries, next={}",
		latest.entries.len(),
		latest.has_next_page
	));

	let link = source
		.handle_deep_link("https://olympustaff.com/series/fast-break/15".into())
		.unwrap();
	assert!(matches!(link, Some(DeepLinkResult::Chapter { .. })));
}

#[aidoku_test]
fn paginated_chapters() {
	let source = TeamX::new();
	let manga = Manga {
		key: "/series/SMN".into(),
		..Default::default()
	};
	let chapters = source
		.get_manga_update(manga, false, true)
		.unwrap()
		.chapters
		.unwrap();
	print(format!("SMN: {} chapters", chapters.len()));
	assert!(chapters.len() > 40, "pagination not followed");
}
