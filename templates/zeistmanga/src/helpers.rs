use aidoku::{
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::encode_uri_component,
};

const CHAPTER_WORDS: &[&str] = &[
	"chapter",
	"ch",
	"ch.",
	"episode",
	"ep",
	"ep.",
	"الفصل",
	"فصل",
	"العدد",
	"عدد",
];
const VOLUME_WORDS: &[&str] = &["volume", "vol", "vol.", "المجلد", "مجلد"];

/// The path (with query) of an absolute URL, e.g. `/2024/01/chapter-1.html`.
pub fn url_path(url: &str) -> &str {
	url.split_once("://")
		.and_then(|(_, rest)| rest.find('/').map(|idx| &rest[idx..]))
		.unwrap_or(url)
}

/// Encodes each segment of a Blogger label path.
pub fn encode_label(label: &str) -> String {
	label
		.split('/')
		.map(encode_uri_component)
		.collect::<Vec<_>>()
		.join("/")
}

/// Requests a 600px wide thumbnail instead of the 72px square in feeds.
pub fn resize_thumbnail(url: &str) -> String {
	// `.../s72-c/image.jpg`
	if let Some(end) = url.rfind("-c/")
		&& let Some(start) = url[..end].rfind("/s")
	{
		return format!("{}/w600/{}", &url[..start], &url[end + 3..]);
	}
	// `...=s72-c`
	if url.ends_with("-c")
		&& let Some(start) = url.rfind("=s")
	{
		return format!("{}=w600", &url[..start]);
	}
	url.into()
}

/// Finds a quoted string after `prefix`, as in `clwd.run('Label')` or `label = 'Label'`.
pub fn extract_quoted_after(text: &str, prefix: &str) -> Option<String> {
	let rest = text[text.find(prefix)? + prefix.len()..].trim_start();
	let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
	let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
	let rest = &rest[1..];
	let value = &rest[..rest.find(quote)?];
	(!value.is_empty()).then(|| value.into())
}

fn tokens(title: &str) -> Vec<String> {
	title
		.split(|c: char| c.is_whitespace() || c == '#' || c == ':')
		.filter(|s| !s.is_empty())
		.map(|s| s.to_lowercase())
		.collect()
}

fn leading_number(token: &str) -> Option<f32> {
	let end = token
		.find(|c: char| !(c.is_ascii_digit() || c == '.'))
		.unwrap_or(token.len());
	token[..end].trim_end_matches('.').parse().ok()
}

/// Parses `(chapter, volume)` numbers from titles like "المجلد 14 الفصل 146" or "العدد#6".
pub fn parse_chapter_numbers(title: &str) -> (Option<f32>, Option<f32>) {
	let tokens = tokens(title);
	let after = |words: &[&str]| {
		tokens
			.windows(2)
			.find(|w| words.contains(&w[0].as_str()))
			.and_then(|w| leading_number(&w[1]))
	};
	let volume = after(VOLUME_WORDS);
	let chapter = after(CHAPTER_WORDS).or_else(|| {
		if volume.is_some() {
			None
		} else {
			tokens.iter().find_map(|t| leading_number(t))
		}
	});
	(chapter, volume)
}

/// Whether a title is only a chapter label and number, adding nothing over the number.
pub fn is_generic_chapter_title(title: &str) -> bool {
	let tokens = tokens(title);
	!tokens.is_empty()
		&& tokens.iter().any(|t| leading_number(t).is_some())
		&& tokens.iter().all(|t| {
			CHAPTER_WORDS.contains(&t.as_str())
				|| t.parse::<f32>().is_ok()
				|| t.chars().all(|c| c == '-')
		})
}

/// Parses an ISO 8601 timestamp such as `2023-04-01T23:16:00.004+01:00` into seconds.
pub fn parse_iso_date(date: &str) -> Option<i64> {
	let num = |from: usize, to: usize| date.get(from..to)?.parse::<i64>().ok();
	let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
	let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);

	let zone = date[19..].trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
	let offset = match zone.chars().next() {
		Some(sign @ ('+' | '-')) => {
			let digits = zone[1..].replace(':', "");
			let hours = digits.get(0..2)?.parse::<i64>().ok()?;
			let minutes = digits.get(2..4).and_then(|m| m.parse::<i64>().ok());
			let offset = hours * 3600 + minutes.unwrap_or(0) * 60;
			if sign == '-' { -offset } else { offset }
		}
		_ => 0,
	};

	// days from civil date (proleptic Gregorian calendar)
	let y = if month <= 2 { year - 1 } else { year };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = y - era * 400;
	let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	let days = era * 146097 + doe - 719468;

	Some(days * 86400 + hour * 3600 + minute * 60 + second - offset)
}

/// Finds the Blogger post id embedded in a post page.
pub fn find_post_id(html: &str) -> Option<String> {
	let rest = &html[html.find("'postId': '")? + 11..];
	let id = &rest[..rest.find('\'')?];
	(!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn chapter_numbers() {
		assert_eq!(parse_chapter_numbers("الفصل 97"), (Some(97.0), None));
		assert_eq!(parse_chapter_numbers("العدد#6"), (Some(6.0), None));
		assert_eq!(
			parse_chapter_numbers("المجلد 14 الفصل 146 - واجب"),
			(Some(146.0), Some(14.0))
		);
		assert_eq!(
			parse_chapter_numbers("مانجا Blue lock الفصل Chapter 286"),
			(Some(286.0), None)
		);
		assert_eq!(
			parse_chapter_numbers("الفصل 25: تانِكَ الفتاتان."),
			(Some(25.0), None)
		);
		assert_eq!(parse_chapter_numbers("Chapter 12.5"), (Some(12.5), None));
		assert_eq!(parse_chapter_numbers("الخاتمة"), (None, None));
		assert!(is_generic_chapter_title("الفصل 97"));
		assert!(is_generic_chapter_title("Chapter 04"));
		assert!(!is_generic_chapter_title("الخاتمة"));
		assert!(!is_generic_chapter_title("الفصل 217 BE BLUES"));
	}

	#[aidoku_test]
	fn dates() {
		assert_eq!(parse_iso_date("1970-01-01T00:00:00Z"), Some(0));
		assert_eq!(
			parse_iso_date("2023-04-01T23:16:00.004+01:00"),
			Some(1680387360)
		);
		assert_eq!(
			parse_iso_date("2026-10-28T01:09:48.532-07:00"),
			Some(1793174988)
		);
	}

	#[aidoku_test]
	fn urls() {
		assert_eq!(
			url_path("https://a.blogspot.com/2023/04/x.html"),
			"/2023/04/x.html"
		);
		assert_eq!(
			resize_thumbnail("https://b.com/img/b/R29v/AVv/s72-c/001.webp"),
			"https://b.com/img/b/R29v/AVv/w600/001.webp"
		);
		assert_eq!(
			resize_thumbnail("https://b.com/img/a/AVvXs=s72-c"),
			"https://b.com/img/a/AVvXs=w600"
		);
		assert_eq!(
			extract_quoted_after("clwd.run('Blue Lock');", "clwd.run("),
			Some("Blue Lock".into())
		);
	}
}
