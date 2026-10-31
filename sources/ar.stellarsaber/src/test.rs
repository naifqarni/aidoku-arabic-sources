#![expect(clippy::unwrap_used)]

use super::*;
use aes_gcm::{
	Aes256Gcm, KeyInit, Nonce,
	aead::{Aead, generic_array::GenericArray},
};
use aidoku::imports::std::print;
use aidoku_test::aidoku_test;
use base64::{Engine, engine::general_purpose::STANDARD};

// The site sits behind a Cloudflare challenge that only the app can pass, so the live test
// reports the block instead of failing.
#[aidoku_test]
fn live_smoke() {
	let source = StellarSaber::new();
	let listing = Listing {
		id: "latest".into(),
		..Default::default()
	};
	let list = match source.get_manga_list(listing, 1) {
		Ok(list) if !list.entries.is_empty() => list,
		Ok(_) => {
			print("latest listing came back empty (likely Cloudflare)");
			return;
		}
		Err(err) => {
			print(format!(
				"latest listing failed (likely Cloudflare): {err:?}"
			));
			return;
		}
	};
	print(format!("latest: {} entries", list.entries.len()));
	let first = list.entries.into_iter().next().unwrap();
	let manga = source.get_manga_update(first, true, true).unwrap();
	let chapters = manga.chapters.clone().unwrap();
	print(format!("{}: {} chapters", manga.title, chapters.len()));
	let pages = source
		.get_page_list(manga, chapters.into_iter().next().unwrap())
		.unwrap();
	print(format!("{} pages", pages.len()));
}

#[aidoku_test]
fn decrypts_images() {
	let key = [7u8; 32];
	let iv = [3u8; 12];
	let plain = b"\xff\xd8\xff\xe0 fake jpeg";
	let mut data = iv.to_vec();
	data.extend(
		Aes256Gcm::new(GenericArray::from_slice(&key))
			.encrypt(Nonce::from_slice(&iv), plain.as_slice())
			.unwrap(),
	);
	let decrypted = decrypt_image(&STANDARD.encode(key), &data).unwrap();
	assert_eq!(decrypted, plain);
}

#[aidoku_test]
fn parses_helpers() {
	let now = aidoku::imports::std::current_date();
	let three_hours = parse_relative_date("منذ 3 ساعات").unwrap();
	assert!((now - three_hours - 3 * 3600).abs() < 5);
	let two_days = parse_relative_date("منذ يومين").unwrap();
	assert!((now - two_days - 2 * 86400).abs() < 5);
	assert_eq!(first_number("الفصل 12.5"), Some(12.5));
	assert_eq!(
		extract(
			"var x = {chapterId: 991, cdnNonce: 'ab12'}",
			"cdnNonce: '",
			'\''
		),
		Some("ab12")
	);
	let (content_type, body) = multipart(&[("action", "a"), ("page", "1")]);
	assert!(content_type.starts_with("multipart/form-data; boundary="));
	assert!(
		String::from_utf8(body)
			.unwrap()
			.contains("name=\"page\"\r\n\r\n1\r\n")
	);
}

#[aidoku_test]
fn deep_links() {
	let source = StellarSaber::new();
	let link = source
		.handle_deep_link("https://stellarsaber.pro/manga/some-series/".into())
		.unwrap();
	assert!(
		matches!(link, Some(DeepLinkResult::Manga { ref key }) if key == "/manga/some-series/")
	);
	let link = source
		.handle_deep_link("https://stellarsaber.pro/manga/some-series/chapter-3/".into())
		.unwrap();
	assert!(matches!(
		link,
		Some(DeepLinkResult::Chapter { ref manga_key, ref key })
			if manga_key == "/manga/some-series/" && key == "/manga/some-series/chapter-3/"
	));
}
