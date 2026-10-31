#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::{alloc::vec, imports::std::print};
use aidoku_test::aidoku_test;

/// Every content page requires an account; without one the source must
/// report a login error instead of returning an empty list.
#[aidoku_test]
fn requires_login() {
	let source = Duskoryvile::new();
	let result = source.get_search_manga_list(None, 1, vec![]);
	match result {
		Ok(list) => print(format!("logged in: {} entries", list.entries.len())),
		Err(err) => {
			print(format!("error: {err:?}"));
			assert!(format!("{err:?}").contains("Please log in"));
		}
	}
	assert!(
		!source
			.handle_basic_login("login".into(), "".into(), "".into())
			.unwrap()
	);
}

#[aidoku_test]
fn deeplink() {
	let source = Duskoryvile::new();
	let link = source
		.handle_deep_link("https://duskoryvile.com/series-page/?series_id=42".into())
		.unwrap();
	assert!(matches!(
		link,
		Some(DeepLinkResult::Manga { key }) if key == "/series-page/?series_id=42"
	));
	let none = source
		.handle_deep_link("https://duskoryvile.com/subscribe/".into())
		.unwrap();
	assert!(none.is_none());
}
