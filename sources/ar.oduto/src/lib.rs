#![no_std]
use aidoku::{
	Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
	alloc::{String, Vec, vec},
	helpers::uri::encode_uri_component,
	imports::net::Request,
	prelude::*,
};
use serde::Deserialize;

const BASE_URL: &str = "https://nb19u.blogspot.com";
/// The site only translates Boruto; its chapters share this Blogger label.
const MANGA_LABEL: &str = "مانجا بوروتو";

#[derive(Deserialize)]
struct FeedResponse {
	feed: Feed,
}

#[derive(Deserialize)]
struct Feed {
	#[serde(default)]
	entry: Vec<Entry>,
}

#[derive(Deserialize)]
struct Text {
	#[serde(rename = "$t")]
	t: String,
}

#[derive(Deserialize)]
struct Link {
	rel: String,
	href: String,
}

#[derive(Deserialize)]
struct Author {
	name: Text,
}

#[derive(Deserialize)]
struct Entry {
	title: Text,
	published: Text,
	#[serde(default)]
	link: Vec<Link>,
	#[serde(default)]
	author: Vec<Author>,
}

impl Entry {
	fn into_chapter(self) -> Option<Chapter> {
		let url = self.link.into_iter().find(|l| l.rel == "alternate")?.href;
		let key = url.strip_prefix(BASE_URL)?.into();
		// titles look like "الفصل 25: تانِكَ الفتاتان."
		let title = self.title.t.trim();
		let (number, name) = title
			.strip_prefix("الفصل")
			.and_then(|rest| {
				let (number, name) = rest.split_once(':').unwrap_or((rest, ""));
				Some((number.trim().parse::<f32>().ok()?, name.trim()))
			})
			.unzip();
		Some(Chapter {
			key,
			title: match name {
				Some(name) => Some(name).filter(|n| !n.is_empty()).map(Into::into),
				None => Some(title.into()),
			},
			chapter_number: number,
			date_uploaded: parse_iso_date(&self.published.t),
			scanlators: self.author.into_iter().next().map(|a| vec![a.name.t]),
			url: Some(url),
			..Default::default()
		})
	}
}

/// Parses an ISO 8601 timestamp such as `2026-10-02T15:35:24.226+03:00` into seconds.
fn parse_iso_date(date: &str) -> Option<i64> {
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

	let y = if month <= 2 { year - 1 } else { year };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = y - era * 400;
	let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	let days = era * 146097 + doe - 719468;

	Some(days * 86400 + hour * 3600 + minute * 60 + second - offset)
}

fn boruto() -> Manga {
	Manga {
		key: MANGA_LABEL.into(),
		title: "BORUTO: Two Blue Vortex".into(),
		cover: Some("https://blogger.googleusercontent.com/img/b/R29vZ2xl/AVvXsEggWB9vWPMqjEvIoDsJSO29OmW-srULDQD3cS9HJ8cDk0vq2jLwDerUX-i61CqmZf62eBVmWZwU5CgXi0p2lxhKrh2_nZum3p-k3q9QJ2uozove0QAbOKtbd1QPjytjrJc9UsL65X4BbFdgcicLDYubD9LgY1Kco8wyhDGm4YEOim8u1TL42gOFe16NaaEP/s3464/4D55C3C5-9168-4103-B45C-99B52B58B6A5.jpeg".into()),
		artists: Some(vec!["Mikio Ikemoto".into()]),
		authors: Some(vec!["Masashi Kishimoto".into()]),
		url: Some(format!("{BASE_URL}/search/label/{}", encode_uri_component(MANGA_LABEL))),
		tags: Some(["شونين", "دراما", "خيال", "أكشن", "نينجا"].map(Into::into).into()),
		status: MangaStatus::Ongoing,
		content_rating: ContentRating::Safe,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

struct Oduto;

impl Source for Oduto {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		_query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		Ok(MangaPageResult {
			entries: vec![boruto()],
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			manga.copy_from(boruto());
		}
		if needs_chapters {
			let url = format!(
				"{BASE_URL}/feeds/posts/summary/-/{}?alt=json&max-results=500",
				encode_uri_component(&manga.key)
			);
			let feed = Request::get(url)?.json_owned::<FeedResponse>()?.feed;
			manga.chapters = Some(
				feed.entry
					.into_iter()
					.filter_map(Entry::into_chapter)
					.collect(),
			);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Request::get(format!("{BASE_URL}{}", chapter.key))?.html()?;
		Ok(html
			.select("div#post-body img")
			.map(|els| {
				els.filter_map(|img| img.attr("abs:src"))
					.map(|url| Page {
						content: PageContent::url(url),
						..Default::default()
					})
					.collect()
			})
			.unwrap_or_default())
	}
}

register_source!(Oduto);

#[cfg(test)]
mod test;
