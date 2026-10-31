#![no_std]
use aidoku::{
	AidokuError, BasicLoginHandler, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult,
	FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		defaults::defaults_get,
		html::Document,
		net::Request,
		std::{parse_date_with_options, send_partial_result},
	},
	prelude::*,
};

const BASE_URL: &str = "https://duskoryvile.com";
const USERNAME_KEY: &str = "login.username";
const PASSWORD_KEY: &str = "login.password";
const LOGIN_REQUIRED: &str =
	"يرجى تسجيل الدخول من إعدادات المصدر (Please log in via source settings)";

struct Duskoryvile;

fn strip_base(url: String) -> String {
	url.strip_prefix(BASE_URL).map(String::from).unwrap_or(url)
}

/// The site redirects every content page to /login/ without a session.
fn is_login_page(html: &Document) -> bool {
	html.select_first("form#dap_sub_loginform").is_some()
}

fn fetch_once(url: &str) -> Result<(bool, Document)> {
	let html = Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.html()?;
	Ok((is_login_page(&html), html))
}

/// Logs in through the WordPress form; the session cookie is kept by the app.
fn login(username: &str, password: &str) -> Result<bool> {
	let mut body = QueryParameters::new();
	body.push("log", Some(username));
	body.push("pwd", Some(password));
	body.push("rememberme", Some("forever"));
	body.push("wp-submit", Some("Login"));
	body.push("redirect_to", Some(&format!("{BASE_URL}/")));
	Request::post(format!("{BASE_URL}/wp-login.php"))?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Referer", &format!("{BASE_URL}/login/"))
		.body(body.to_string())
		.send()?;
	let (needs_login, _) = fetch_once(&format!("{BASE_URL}/series-list/"))?;
	Ok(!needs_login)
}

fn get(url: &str) -> Result<Document> {
	let (needs_login, html) = fetch_once(url)?;
	if !needs_login {
		return Ok(html);
	}
	let username = defaults_get::<String>(USERNAME_KEY).filter(|s| !s.is_empty());
	let password = defaults_get::<String>(PASSWORD_KEY).filter(|s| !s.is_empty());
	let (Some(username), Some(password)) = (username, password) else {
		bail!("{LOGIN_REQUIRED}");
	};
	// the session may have expired; log in again with the saved credentials
	if !login(&username, &password)? {
		bail!("{LOGIN_REQUIRED}");
	}
	let (needs_login, html) = fetch_once(url)?;
	if needs_login {
		bail!("{LOGIN_REQUIRED}");
	}
	Ok(html)
}

fn parse_series_grid(html: &Document) -> MangaPageResult {
	let entries = html
		.select(".dap-series-grid a[href*=\"series_id=\"]")
		.map(|els| {
			els.filter_map(|el| {
				Some(Manga {
					key: strip_base(el.attr("abs:href")?),
					title: el
						.select_first("div[style*=\"font-weight\"]")?
						.text()?
						.trim()
						.into(),
					cover: el.select_first("img").and_then(|img| {
						img.attr("abs:data-src")
							.filter(|s| !s.is_empty())
							.or_else(|| img.attr("abs:src"))
					}),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default();
	MangaPageResult {
		entries,
		has_next_page: false,
	}
}

impl Source for Duskoryvile {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// neither the series list nor search is paginated
		if page > 1 {
			return Ok(MangaPageResult::default());
		}
		let html = match query.filter(|q| !q.trim().is_empty()) {
			Some(query) => {
				let mut qs = QueryParameters::new();
				qs.push("q", Some(query.trim()));
				get(&format!("{BASE_URL}/search/?{qs}"))?
			}
			None => get(&format!("{BASE_URL}/series-list/"))?,
		};
		Ok(parse_series_grid(&html))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = get(&url)?;

		if needs_details {
			if let Some(title) = html
				.select_first(".dk-series-hero h2")
				.and_then(|el| el.text())
			{
				manga.title = title.trim().into();
			}
			manga.cover = html
				.select_first(".dk-series-hero-img img")
				.and_then(|img| img.attr("abs:src"))
				.or(manga.cover);
			let description = html
				.select_first(".dk-series-hero div[style*=\"line-height:1.5\"]")
				.and_then(|el| el.text())
				.map(|s| String::from(s.trim()))
				.filter(|s| !s.is_empty());
			manga.tags = description
				.as_deref()
				.and_then(|d| d.split_once("التصنيف:"))
				.map(|(_, genres)| {
					genres
						.split(['،', ','])
						.map(|g| String::from(g.trim()))
						.filter(|g| !g.is_empty())
						.collect()
				});
			manga.description = description;
			manga.status = MangaStatus::Unknown;
			manga.content_rating = ContentRating::Suggestive;
			manga.url = Some(url.clone());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = html
				.select(".dk-ch-grid a.dk-ch-card")
				.map(|els| {
					els.filter_map(|el| {
						let url = el.attr("abs:href")?;
						let name = String::from(el.select_first(".dk-ch-title")?.text()?.trim());
						let chapter_number = name
							.split(|c: char| !c.is_ascii_digit() && c != '.')
							.find_map(|s| s.trim_matches('.').parse::<f32>().ok());
						Some(Chapter {
							key: strip_base(url.clone()),
							title: Some(name),
							chapter_number,
							date_uploaded: el
								.select_first(".dk-ch-date")
								.and_then(|el| el.text())
								.and_then(|d| {
									parse_date_with_options(
										d.trim(),
										"MMMM d, yyyy",
										"en_US_POSIX",
										"current",
									)
								}),
							url: Some(url),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default();
			// the site lists oldest first
			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = get(&format!("{BASE_URL}{}", chapter.key))?;
		Ok(html
			.select(".dap-pages img.dap-page")
			.map(|els| {
				els.filter_map(|el| {
					let url = el
						.attr("abs:data-src")
						.filter(|s| !s.is_empty())
						.or_else(|| el.attr("abs:src"))?;
					Some(Page {
						content: PageContent::url(url),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}
}

impl BasicLoginHandler for Duskoryvile {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		if username.is_empty() || password.is_empty() {
			return Ok(false);
		}
		login(&username, &password).or(Ok::<bool, AidokuError>(false))
	}
}

impl DeepLinkHandler for Duskoryvile {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.duskoryvile.com"))
		else {
			return Ok(None);
		};
		let Some(series_id) = path
			.split_once('?')
			.and_then(|(_, query)| {
				query
					.split(['&', '#'])
					.find_map(|kv| kv.strip_prefix("series_id="))
			})
			.filter(|id| !id.is_empty())
		else {
			return Ok(None);
		};
		Ok(Some(DeepLinkResult::Manga {
			key: format!("/series-page/?series_id={series_id}"),
		}))
	}
}

register_source!(Duskoryvile, BasicLoginHandler, DeepLinkHandler);

#[cfg(test)]
mod test;
