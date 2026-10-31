use aidoku::alloc::{String, Vec};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct EncryptedResponse {
	pub data: String,
}

#[derive(Deserialize)]
pub struct MangaDataAction<T> {
	#[serde(rename = "mangaDataAction")]
	pub manga_data_action: T,
}

#[derive(Deserialize)]
pub struct LatestChaptersDto {
	pub releases: Vec<LatestReleaseDto>,
}

#[derive(Deserialize)]
pub struct LatestReleaseDto {
	pub manga: BrowseManga,
}

#[derive(Deserialize)]
pub struct SearchMangaDto {
	pub mangas: Vec<BrowseManga>,
}

#[derive(Deserialize)]
pub struct BrowseManga {
	pub id: i64,
	pub title: String,
	pub cover: Option<String>,
	#[serde(default)]
	pub is_novel: bool,
}

#[derive(Deserialize)]
pub struct MangaDetailsDto {
	#[serde(rename = "mangaData")]
	pub manga_data: MangaDetails,
}

#[derive(Deserialize)]
pub struct MangaDetails {
	pub id: i64,
	pub cover: Option<String>,
	pub title: String,
	pub summary: Option<String>,
	#[serde(default)]
	pub artists: Vec<NameDto>,
	#[serde(default)]
	pub authors: Vec<NameDto>,
	#[serde(default)]
	pub story_status: i32,
	#[serde(rename = "type")]
	pub manga_type: TypeDto,
	#[serde(default)]
	pub categories: Vec<NameDto>,
	#[serde(default = "unknown_status")]
	pub translation_status: i32,
	pub synonyms: Option<String>,
	pub arabic_title: Option<String>,
	pub japanese: Option<String>,
	pub english: Option<String>,
	pub reading_direction: Option<String>,
	#[serde(default)]
	pub over17: bool,
}

fn unknown_status() -> i32 {
	-1
}

#[derive(Deserialize)]
pub struct NameDto {
	pub name: String,
}

#[derive(Deserialize)]
pub struct TypeDto {
	pub name: String,
	pub title: Option<String>,
	pub reading_direction: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterListDto {
	#[serde(rename = "mangaReleases")]
	pub manga_releases: Vec<ChapterRelease>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum NumberOrString {
	Number(f32),
	String(String),
}

impl NumberOrString {
	pub fn as_f32(&self) -> Option<f32> {
		match self {
			Self::Number(n) => Some(*n),
			Self::String(s) => s.trim().parse().ok(),
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterRelease {
	pub id: i64,
	pub chapter: NumberOrString,
	#[serde(default)]
	pub title: String,
	#[serde(default)]
	pub team_name: String,
	#[serde(default)]
	pub created_at: String,
}

#[derive(Deserialize)]
pub struct ReaderDto {
	#[serde(rename = "readerDataAction")]
	pub reader_data_action: ReaderData,
	pub globals: Globals,
}

#[derive(Deserialize)]
pub struct Globals {
	#[serde(rename = "mediaKey")]
	pub media_key: String,
}

#[derive(Deserialize)]
pub struct ReaderData {
	#[serde(rename = "readerData")]
	pub reader_data: ReaderChapter,
}

#[derive(Deserialize)]
pub struct ReaderChapter {
	pub release: ReaderPages,
}

#[derive(Deserialize)]
pub struct ReaderPages {
	pub hq_pages: String,
	pub manga_id: Option<i64>,
}

#[derive(Serialize)]
pub struct SearchPayload {
	pub oneshot: OneShot,
	pub title: String,
	pub page: i32,
	pub manga_types: IncludeExclude,
	pub story_status: IncludeExclude,
	pub translation_status: IncludeExclude,
	pub categories: IncludeExclude,
	pub chapters: MinMax,
	pub dates: StartEnd,
}

#[derive(Serialize)]
pub struct OneShot {
	pub value: bool,
}

#[derive(Serialize)]
pub struct IncludeExclude {
	pub include: Vec<Option<String>>,
	pub exclude: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct MinMax {
	pub min: String,
	pub max: String,
}

#[derive(Serialize, Default)]
pub struct StartEnd {
	pub start: String,
	pub end: String,
}
