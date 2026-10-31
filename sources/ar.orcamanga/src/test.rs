#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{
	Listing, ListingProvider,
	alloc::{String, vec},
	imports::std::print,
};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = ZeistManga::<OrcaManga>::new();
	let popular = source
		.get_manga_list(
			Listing {
				id: "popular".into(),
				..Default::default()
			},
			1,
		)
		.unwrap();
	print(format!("popular: {} entries", popular.entries.len()));
	assert!(!popular.entries.is_empty(), "empty popular");

	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	print(format!(
		"latest: {} entries, next={}",
		list.entries.len(),
		list.has_next_page
	));
	assert!(!list.entries.is_empty(), "empty list");

	let query: String = list.entries[0]
		.title
		.split_whitespace()
		.max_by_key(|w| w.chars().count())
		.unwrap()
		.into();
	let found = source
		.get_search_manga_list(Some(query.clone()), 1, vec![])
		.unwrap();
	print(format!("search {query:?}: {} entries", found.entries.len()));
	assert!(!found.entries.is_empty(), "empty search");

	for entry in list.entries.into_iter().take(6) {
		let manga = source.get_manga_update(entry, true, true).unwrap();
		print(format!(
			"{} ({}): status={:?} viewer={:?} rating={:?} authors={:?} tags={:?} cover={:?} desc={:?}",
			manga.title,
			manga.key,
			manga.status,
			manga.viewer,
			manga.content_rating,
			manga.authors,
			manga.tags,
			manga.cover,
			manga
				.description
				.as_deref()
				.map(|d| d.chars().take(50).collect::<String>())
		));
		let chapters = manga.chapters.clone().unwrap_or_default();
		print(format!("  {} chapters", chapters.len()));
		let (Some(newest), Some(oldest)) = (chapters.first(), chapters.last()) else {
			continue;
		};
		for ch in [newest, oldest] {
			print(format!(
				"  {:?} #{:?} vol={:?} date={:?} key={}",
				ch.title, ch.chapter_number, ch.volume_number, ch.date_uploaded, ch.key
			));
		}
		match source.get_page_list(manga, newest.clone()) {
			Ok(pages) if !pages.is_empty() => {
				print(format!("  {} pages", pages.len()));
				return;
			}
			_ => print("  no pages"),
		}
	}
	panic!("no readable chapter found");
}
