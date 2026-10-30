#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

// the page list runs in a web view, which the test runner doesn't support
#[aidoku_test]
fn smoke() {
	let source = Dilar::new();
	let list = source
		.get_search_manga_list(Some("Satsuriku".into()), 1, vec![])
		.unwrap();
	assert!(!list.entries.is_empty(), "empty search");
	let first = list.entries.into_iter().next().unwrap();
	print(format!(
		"first: {} ({}) cover={:?}",
		first.title, first.key, first.cover
	));

	let manga = source.get_manga_update(first, true, true).unwrap();
	print(format!(
		"status={:?} viewer={:?} rating={:?} authors={:?} tags={:?}",
		manga.status, manga.viewer, manga.content_rating, manga.authors, manga.tags
	));
	let chapters = manga.chapters.unwrap();
	assert!(!chapters.is_empty(), "no chapters");
	let ch = &chapters[0];
	print(format!(
		"{} chapters; newest #{:?} title={:?} date={:?} teams={:?} url={:?}",
		chapters.len(),
		ch.chapter_number,
		ch.title,
		ch.date_uploaded,
		ch.scanlators,
		ch.url
	));
}

#[aidoku_test]
fn listings_and_filters() {
	let source = Dilar::new();
	for id in ["popular", "latest"] {
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
			1,
			vec![FilterValue::MultiSelect {
				id: "type".into(),
				included: vec!["1".into()],
				excluded: vec![],
			}],
		)
		.unwrap();
	print(format!("manga-only: {} entries", filtered.entries.len()));
	assert!(!filtered.entries.is_empty());

	let link = source
		.handle_deep_link("https://dilar.tube/reader/13908/x/32?release=195895".into())
		.unwrap();
	assert!(matches!(
		link,
		Some(DeepLinkResult::Chapter { ref manga_key, ref key }) if manga_key == "13908" && key == "195895"
	));
}
