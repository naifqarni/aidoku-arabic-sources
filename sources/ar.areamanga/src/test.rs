#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn smoke() {
	let source = MangaThemesia::<AreaManga>::new();
	let list = source.get_search_manga_list(None, 1, vec![]).unwrap();
	assert!(!list.entries.is_empty(), "empty list");
	print(format!(
		"{} entries, has_next_page={}",
		list.entries.len(),
		list.has_next_page
	));

	// some entries may be novels or have no chapters, so try a few
	for entry in list.entries.into_iter().take(12) {
		print(format!("entry: {} ({})", entry.title, entry.key));
		let manga = match source.get_manga_update(entry, true, true) {
			Ok(manga) => manga,
			Err(err) => {
				print(format!("error: {err:?}"));
				continue;
			}
		};
		print(format!(
			"status={:?} viewer={:?} authors={:?} tags={:?} desc={:?}",
			manga.status,
			manga.viewer,
			manga.authors,
			manga.tags,
			manga
				.description
				.as_deref()
				.map(|d| d.chars().take(60).collect::<aidoku::alloc::String>())
		));
		let chapters = manga.chapters.clone().unwrap_or_default();
		if chapters.is_empty() {
			print("no chapters");
			continue;
		}
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
		if pages.is_empty() {
			print("no pages");
			continue;
		}
		print(format!(
			"{} pages; first: {:?}",
			pages.len(),
			pages[0].content
		));
		return;
	}
	panic!("no entry with readable pages");
}
