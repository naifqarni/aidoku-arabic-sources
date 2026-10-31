use aes_gcm::{
	Aes128Gcm, Aes256Gcm, KeyInit, Nonce,
	aead::{Aead, generic_array::GenericArray},
};
use aidoku::{
	Result, Viewer,
	alloc::{String, Vec},
	imports::{html::Element, std::current_date},
	prelude::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};

use crate::BASE_URL;

const IV_SIZE: usize = 12;

pub fn strip_base(url: &str) -> String {
	url.strip_prefix(BASE_URL).unwrap_or(url).into()
}

pub fn script_text(script: &Element) -> Option<String> {
	// the test runner only exposes script contents through `html`
	script.data().or_else(|| script.html())
}

/// The text between `start` and the next `end` after it.
pub fn extract<'a>(text: &'a str, start: &str, end: char) -> Option<&'a str> {
	let from = text.find(start)? + start.len();
	let rest = &text[from..];
	Some(&rest[..rest.find(end)?])
}

pub fn first_number(text: &str) -> Option<f32> {
	let start = text.find(|c: char| c.is_ascii_digit())?;
	let rest = &text[start..];
	let end = rest
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(rest.len());
	rest[..end].trim_end_matches('.').parse().ok()
}

pub fn viewer_for(kind: &str) -> Viewer {
	if kind.contains("مانجا") {
		Viewer::RightToLeft
	} else if kind.contains("مانهوا") || kind.contains("مانها") || kind.contains("ويب")
	{
		Viewer::Webtoon
	} else {
		Viewer::Unknown
	}
}

/// "منذ 3 ساعات", "منذ يومين", ... to a unix timestamp in seconds.
pub fn parse_relative_date(date: &str) -> Option<i64> {
	const MINUTE: i64 = 60;
	const HOUR: i64 = 60 * MINUTE;
	const DAY: i64 = 24 * HOUR;
	let has = |words: &[&str]| words.iter().any(|w| date.contains(w));
	let seconds = match first_number(date) {
		None => {
			if has(&["دقيقتين"]) {
				2 * MINUTE
			} else if has(&["دقيقة"]) {
				MINUTE
			} else if has(&["ساعتين"]) {
				2 * HOUR
			} else if has(&["ساعة"]) {
				HOUR
			} else if has(&["يومين"]) {
				2 * DAY
			} else if has(&["يوم"]) {
				DAY
			} else if has(&["أسبوعين"]) {
				14 * DAY
			} else if has(&["أسبوع"]) {
				7 * DAY
			} else if has(&["شهرين"]) {
				60 * DAY
			} else if has(&["شهر"]) {
				30 * DAY
			} else if has(&["سنتين"]) {
				730 * DAY
			} else if has(&["سنة"]) {
				365 * DAY
			} else {
				return None;
			}
		}
		Some(n) => {
			let n = n as i64;
			if has(&["دقيقة", "دقائق"]) {
				n * MINUTE
			} else if has(&["ساعة", "ساعات"]) {
				n * HOUR
			} else if has(&["يوم", "أيام"]) {
				n * DAY
			} else if has(&["أسبوع", "أسابيع"]) {
				n * 7 * DAY
			} else if has(&["شهر", "أشهر"]) {
				n * 30 * DAY
			} else if has(&["سنة", "سنوات"]) {
				n * 365 * DAY
			} else {
				return None;
			}
		}
	};
	Some(current_date() - seconds)
}

/// Page images are a 12-byte IV followed by AES-GCM ciphertext, keyed per chapter.
pub fn decrypt_image(key: &str, data: &[u8]) -> Result<Vec<u8>> {
	let key = STANDARD
		.decode(key.trim())
		.map_err(|_| error!("Invalid image key"))?;
	let (iv, ciphertext) = data
		.split_at_checked(IV_SIZE)
		.ok_or_else(|| error!("Truncated image"))?;
	let nonce = Nonce::from_slice(iv);
	let result = match key.len() {
		16 => Aes128Gcm::new(GenericArray::from_slice(&key)).decrypt(nonce, ciphertext),
		32 => Aes256Gcm::new(GenericArray::from_slice(&key)).decrypt(nonce, ciphertext),
		_ => bail!("Unsupported image key length"),
	};
	result.map_err(|_| error!("Failed to decrypt image"))
}

/// A multipart/form-data body, as the site's ajax search expects.
pub fn multipart(fields: &[(&str, &str)]) -> (String, Vec<u8>) {
	const BOUNDARY: &str = "----AidokuFormBoundary7MA4YWxkTrZu0gW";
	let mut body = String::new();
	for (name, value) in fields {
		body.push_str(&format!(
			"--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
		));
	}
	body.push_str(&format!("--{BOUNDARY}--\r\n"));
	(
		format!("multipart/form-data; boundary={BOUNDARY}"),
		body.into_bytes(),
	)
}
