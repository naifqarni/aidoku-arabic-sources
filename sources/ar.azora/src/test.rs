#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = Iken::<Azora>::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert!(!list.entries.is_empty(), "empty list");
	// brand new series can have no chapters yet, so use the first one that has some
	let manga = list
		.entries
		.into_iter()
		.map(|entry| source.get_manga_update(entry, true, true).unwrap())
		.find(|manga| manga.chapters.as_ref().is_some_and(|c| !c.is_empty()))
		.expect("no series with chapters");
	print(format!("first: {} ({})", manga.title, manga.key));
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
