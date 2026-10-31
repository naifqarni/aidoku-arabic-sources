#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::imports::std::print;
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = Oduto::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert_eq!(list.entries.len(), 1);
	let manga = list.entries.into_iter().next().unwrap();

	let manga = source.get_manga_update(manga, true, true).unwrap();
	let chapters = manga.chapters.clone().unwrap_or_default();
	assert!(!chapters.is_empty(), "no chapters");
	for ch in [chapters.first().unwrap(), chapters.last().unwrap()] {
		print(format!(
			"{:?} #{:?} date={:?} scanlators={:?} key={}",
			ch.title, ch.chapter_number, ch.date_uploaded, ch.scanlators, ch.key
		));
	}
	print(format!("{} chapters", chapters.len()));

	let ch = chapters.first().unwrap().clone();
	let pages = source.get_page_list(manga, ch).unwrap();
	assert!(!pages.is_empty(), "no pages");
	print(format!("{} pages", pages.len()));
}
