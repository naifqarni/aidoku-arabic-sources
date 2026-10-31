//! Reads the data a Next.js app streams into its page through `self.__next_f.push` scripts.
use aidoku::{alloc::String, imports::html::Document};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// The concatenated flight payload of the page.
pub fn payload(html: &Document) -> String {
	let mut payload = String::new();
	if let Some(scripts) = html.select("script") {
		for script in scripts {
			// the test runner only exposes script contents through `html`
			let Some(data) = script.data().or_else(|| script.html()) else {
				continue;
			};
			let Some(args) = data
				.trim()
				.strip_prefix("self.__next_f.push(")
				.and_then(|s| s.trim_end_matches(';').strip_suffix(')'))
			else {
				continue;
			};
			if let Ok((1, chunk)) = serde_json::from_str::<(i32, String)>(args) {
				payload.push_str(&chunk);
			}
		}
	}
	payload
}

fn find_value<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
	match value {
		Value::Object(map) => {
			if keys.iter().all(|k| map.contains_key(*k)) {
				return Some(value);
			}
			map.values().find_map(|v| find_value(v, keys))
		}
		Value::Array(items) => items.iter().find_map(|v| find_value(v, keys)),
		_ => None,
	}
}

/// The first object in the payload that has all of `keys`.
pub fn find<T: DeserializeOwned>(payload: &str, keys: &[&str]) -> Option<T> {
	payload.lines().find_map(|line| {
		let (_, row) = line.split_once(':')?;
		let value: Value = serde_json::from_str(row).ok()?;
		T::deserialize(find_value(&value, keys)?).ok()
	})
}
