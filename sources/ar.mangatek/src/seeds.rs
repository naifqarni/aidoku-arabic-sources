//! Reads the reader's two rotating seeds (proof salt, reader seed) out of the obfuscated
//! `/_astro/ChapterImageViewer.<hash>.js`. The bundle keeps its strings in one string array,
//! rotated at load, read through a decoder behind chains of offset wrappers. Each seed is a
//! concatenation of such lookups: the reader seed at top level, the salt inside a function. The
//! array, decoder and rotation run in a JS context; the wrappers are resolved here because their
//! names are reused per scope. The bundle's WASM module is a concatenation too; its hash checks
//! the decoding and that the algorithm this port implements is still the one the bundle uses.
use aidoku::{
	Result,
	alloc::{String, Vec, collections::BTreeMap, string::ToString, vec},
	error,
	imports::js::JsContext,
	prelude::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;

use crate::crypto::{hex, sha256};

pub struct Seeds {
	pub proof_salt: String,
	pub reader_seed: String,
}

const WASM_SHA256: &str = "7ae024b07b1264939707262242c6810b570b0bc1051c01c35dcd864e9bdecd2e";
const WASM_BASE64_PREFIX: &str = "AGFzbQ"; // "\0asm"
const MAX_WRAPPER_DEPTH: usize = 32;
const ROTATION_LOOP: &str = "for(;;)";
const ROTATION_GUARD: &str = "for(let rotationGuard=0;rotationGuard<10000;rotationGuard++)";
const IDENTIFIER_OR_DOT: &[u8] =
	b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_$.";
const REGEX_PRECEDERS: &[u8] = b"(,=:[!&|?{};+-*%<>~^";

const ARG: &str = r#"-?\d+(?:e\d+)?|"[^"\\]*"|'[^'\\]*'"#;
const STRING: &str = r#""[^"\\]*"|'[^'\\]*'"#;

/// Compiles a pattern with ASCII `\w`, `\d` and `\s` (the regex crate is built without
/// Unicode tables).
fn re(pattern: &str) -> Result<Regex> {
	let pattern = pattern
		.replace(r"\w", "A-Za-z0-9_")
		.replace(r"\d", "[0-9]")
		.replace(r"\s", r"[ \t\r\n]");
	Regex::new(&pattern).map_err(|_| error!("Invalid pattern"))
}

pub fn extract(js: &str) -> Result<Seeds> {
	Bundle::new(js)?.seeds()
}

/// A call `wrapper(p0, p1)` forwards to `target(p[index] + delta, …)`.
struct Wrapper {
	position: usize,
	scope: (usize, usize),
	target: String,
	args: Vec<(usize, i64)>,
}

struct Site {
	position: usize,
	expression: String,
}

struct Bundle<'a> {
	js: &'a str,
	blocks: Vec<(usize, usize)>,
	close_of: BTreeMap<usize, usize>,
	open_of: BTreeMap<usize, usize>,
	wrappers: BTreeMap<String, Vec<Wrapper>>,
	decoder_name: String,
	prelude: String,
}

impl<'a> Bundle<'a> {
	fn new(js: &'a str) -> Result<Self> {
		let blocks = blocks(js.as_bytes());
		let close_of = blocks.iter().map(|&(o, c)| (o, c)).collect();
		let open_of = blocks.iter().map(|&(o, c)| (c, o)).collect();
		let mut bundle = Self {
			js,
			blocks,
			close_of,
			open_of,
			wrappers: BTreeMap::new(),
			decoder_name: String::new(),
			prelude: String::new(),
		};

		// functions shaped like a wrapper but with other arguments are not wrappers
		let wrapper_re =
			re(r"function ([\w$]+)\(([\w$]+),([\w$]+)\)\{return ([\w$]+)\(([^()]*)\)\}")?;
		let arg_re = re(r"^([\w$]+)(?:\s*([+-])\s*(-?\d+))?$")?;
		let mut wrappers: BTreeMap<String, Vec<Wrapper>> = BTreeMap::new();
		'outer: for caps in wrapper_re.captures_iter(js) {
			let whole = caps.get(0).unwrap_or_else(|| unreachable!());
			let first = &caps[2];
			let second = &caps[3];
			let mut args = Vec::new();
			for arg in caps[5].split(',') {
				let Some(parts) = arg_re.captures(arg.trim()) else {
					continue 'outer;
				};
				let index = match &parts[1] {
					p if p == first => 0,
					p if p == second => 1,
					_ => continue 'outer,
				};
				let delta = match (parts.get(2), parts.get(3)) {
					(Some(sign), Some(amount)) => {
						let amount: i64 = amount.as_str().parse().unwrap_or(0);
						if sign.as_str() == "-" {
							-amount
						} else {
							amount
						}
					}
					_ => 0,
				};
				args.push((index, delta));
			}
			let position = whole.start();
			wrappers.entry(caps[1].into()).or_default().push(Wrapper {
				position,
				scope: bundle.scope_of(position),
				target: caps[4].into(),
				args,
			});
		}
		bundle.wrappers = wrappers;

		let array = re(r"function ([\w$]+)\(\)\{const [\w$]+=\[")?
			.captures(js)
			.ok_or_else(|| error!("No string array in the reader script"))?;
		let array_name = array[1].to_string();
		let array_start = array.get(0).map(|m| m.start()).unwrap_or_default();

		// the regex crate has no backreferences, so the parameter check is done here
		let decoder = re(
			r"function ([\w$]+)\(([\w$]+),[\w$]+\)\{([\w$]+)=([\w$]+)-[^;]+;const [\w$]+=([\w$]+)\(\)",
		)?
		.captures_iter(js)
		.find(|c| c[5] == *array_name && c[2] == c[3] && c[2] == c[4])
		.ok_or_else(|| error!("No string decoder in the reader script"))?;
		bundle.decoder_name = decoder[1].into();
		let decoder_start = decoder.get(0).map(|m| m.start()).unwrap_or_default();

		let rotation = re(r"\}\)\(([\w$]+),")?
			.captures_iter(js)
			.find(|c| c[1] == *array_name)
			.ok_or_else(|| error!("No string array rotation in the reader script"))?;
		let rotation_start = rotation.get(0).map(|m| m.start()).unwrap_or_default();

		let parts = [
			bundle.function_at(array_start)?,
			bundle.function_at(decoder_start)?,
			bundle.rotation_at(rotation_start)?,
		];
		bundle.prelude = format!("{};\n", parts.join(";\n"));
		Ok(bundle)
	}

	fn seeds(&self) -> Result<Seeds> {
		let sites = self.concat_sites()?;
		let expressions: Vec<String> = sites
			.iter()
			.map(|s| {
				format!(
					"(() => {{ try {{ return String({}) }} catch (e) {{ return null }} }})()",
					s.expression
				)
			})
			.collect();
		let script = format!(
			"{} JSON.stringify([{}])",
			self.prelude,
			expressions.join(",")
		);
		let result = JsContext::new()
			.eval(&script)
			.map_err(|_| error!("Reader script strings did not evaluate"))?;
		let values: Vec<Option<String>> = serde_json::from_str(&result)
			.map_err(|_| error!("Reader script strings did not evaluate"))?;
		let values: Vec<(usize, String)> = values
			.into_iter()
			.zip(sites.iter())
			.filter_map(|(v, s)| v.map(|v| (s.position, v)))
			.collect();

		let wasm = values
			.iter()
			.find(|(_, v)| v.starts_with(WASM_BASE64_PREFIX))
			.map(|(_, v)| v)
			.ok_or_else(|| error!("No WASM module in the reader script"))?;
		let wasm = STANDARD
			.decode(wasm)
			.map_err(|_| error!("Invalid WASM module in the reader script"))?;
		if hex(&sha256(&wasm)) != WASM_SHA256 {
			bail!("The reader script's WASM module did not match");
		}

		let is_hex48 =
			|s: &str| s.len() == 48 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
		let (top, nested): (Vec<_>, Vec<_>) = values
			.iter()
			.filter(|(_, v)| is_hex48(v))
			.partition(|(p, _)| self.depth(*p) == 0);
		if top.len() != 1 {
			bail!(
				"Expected one top-level seed in the reader script, found {}",
				top.len()
			);
		}
		if nested.len() != 1 {
			bail!(
				"Expected one nested seed in the reader script, found {}",
				nested.len()
			);
		}
		Ok(Seeds {
			proof_salt: nested[0].1.clone(),
			reader_seed: top[0].1.clone(),
		})
	}

	/// `function name(…){…}` starting at `start`.
	fn function_at(&self, start: usize) -> Result<String> {
		let open = self.js[start..]
			.find('{')
			.map(|i| i + start)
			.ok_or_else(|| error!("Unbalanced function in the reader script"))?;
		let close = *self
			.close_of
			.get(&open)
			.ok_or_else(|| error!("Unbalanced function in the reader script"))?;
		Ok(self.js[start..=close].into())
	}

	/// `(function(…){…})(array,…)` whose body closes at `close`.
	fn rotation_at(&self, close: usize) -> Result<String> {
		let err = || error!("Unexpected rotation in the reader script");
		let open = *self.open_of.get(&close).ok_or_else(err)?;
		let start = self.js[..open].rfind("(function").ok_or_else(err)?;
		let bytes = self.js.as_bytes();
		let mut index = close + 2;
		let mut parens = 0i32;
		loop {
			match bytes.get(index) {
				Some(b'(') => parens += 1,
				Some(b')') => parens -= 1,
				None => return Err(err()),
				_ => {}
			}
			index += 1;
			if parens <= 0 {
				break;
			}
		}
		// the rotation loops until its checksum matches; bound it so a changed array cannot
		// hang the JS context (a wrong rotation then fails the WASM check)
		let rotation = &self.js[start..index];
		if !rotation.contains(ROTATION_LOOP) {
			bail!("Unexpected rotation loop in the reader script");
		}
		Ok(rotation.replacen(ROTATION_LOOP, ROTATION_GUARD, 1))
	}

	/// Runs of lookups joined by `+`, optionally grouped in parentheses:
	/// `a(1,2)+(b(3,4)+"x")+…`, as decoder(…) calls. A run starts at a call.
	fn concat_sites(&self) -> Result<Vec<Site>> {
		let call_re = re(&format!(r"([\w$]+)\(({ARG}),({ARG})\)"))?;
		let piece_re = re(&format!(r"^(?:([\w$]+)\(({ARG}),({ARG})\)|{STRING})"))?;
		let js = self.js;
		let bytes = js.as_bytes();
		let mut sites = Vec::new();
		let mut search_from = 0;
		while let Some(start) = call_re.captures_at(js, search_from) {
			let whole = start.get(0).unwrap_or_else(|| unreachable!());
			if whole.start() > 0 && IDENTIFIER_OR_DOT.contains(&bytes[whole.start() - 1]) {
				search_from = whole.end();
				continue;
			}
			let mut pieces = vec![(
				whole.start(),
				Some((
					start[1].to_string(),
					start[2].to_string(),
					start[3].to_string(),
				)),
				String::new(),
			)];
			let mut end = whole.end();
			loop {
				let mut index = end;
				while index < bytes.len() && bytes[index] == b')' {
					index += 1;
				}
				if index >= bytes.len() || bytes[index] != b'+' {
					break;
				}
				index += 1;
				while index < bytes.len() && bytes[index] == b'(' {
					index += 1;
				}
				let Some(next) = piece_re.captures(&js[index..]) else {
					break;
				};
				let matched = next.get(0).unwrap_or_else(|| unreachable!());
				let call = next.get(1).map(|name| {
					(
						name.as_str().to_string(),
						next[2].to_string(),
						next[3].to_string(),
					)
				});
				pieces.push((index, call, matched.as_str().to_string()));
				end = index + matched.end();
			}
			if pieces.len() > 1 {
				let mut parts = Vec::with_capacity(pieces.len());
				for (position, call, raw) in pieces {
					match call {
						Some((name, a, b)) => match self.resolve(&name, [a, b], position) {
							Some(expr) => parts.push(expr),
							None => {
								parts.clear();
								break;
							}
						},
						None => parts.push(raw),
					}
				}
				// a run with a call that is not a string lookup is not a seed
				if !parts.is_empty() {
					sites.push(Site {
						position: whole.start(),
						expression: parts.join("+"),
					});
				}
			}
			search_from = end;
		}
		Ok(sites)
	}

	/// `name(args)` at `position`, followed through the innermost wrapper in scope to the decoder.
	fn resolve(&self, name: &str, args: [String; 2], position: usize) -> Option<String> {
		let mut target: &str = name;
		let mut current: Vec<String> = args.into();
		let mut from = position;
		for _ in 0..MAX_WRAPPER_DEPTH {
			let wrapper = self.wrappers.get(target).and_then(|ws| {
				ws.iter()
					.filter(|w| from >= w.scope.0 && from <= w.scope.1)
					.min_by_key(|w| w.scope.1 - w.scope.0)
			});
			let Some(wrapper) = wrapper else {
				return (target == self.decoder_name)
					.then(|| format!("{}({})", self.decoder_name, current.join(",")));
			};
			let mut next = Vec::with_capacity(wrapper.args.len());
			for &(index, delta) in &wrapper.args {
				next.push(offset(current.get(index)?, delta)?);
			}
			current = next;
			target = &wrapper.target;
			from = wrapper.position;
		}
		None
	}

	fn scope_of(&self, position: usize) -> (usize, usize) {
		self.blocks
			.iter()
			.filter(|&&(o, c)| position > o && position < c)
			.min_by_key(|&&(o, c)| c - o)
			.copied()
			.unwrap_or((0, self.js.len().saturating_sub(1)))
	}

	fn depth(&self, position: usize) -> usize {
		self.blocks
			.iter()
			.filter(|&&(o, c)| position > o && position < c)
			.count()
	}
}

fn offset(arg: &str, delta: i64) -> Option<String> {
	if delta == 0 {
		return Some(arg.into());
	}
	let value: f64 = arg.parse().ok()?;
	Some(((value as i64) + delta).to_string())
}

/// Brace pairs of the source, skipping strings, templates, regex literals and comments.
fn blocks(js: &[u8]) -> Vec<(usize, usize)> {
	let mut pairs = Vec::new();
	// open-brace index, or None for a template `${`
	let mut stack: Vec<Option<usize>> = Vec::new();
	let mut i = 0usize;
	let mut previous = b';';
	let len = js.len();

	let skip_template = |i: &mut usize, stack: &mut Vec<Option<usize>>| {
		while *i < len {
			match js[*i] {
				b'\\' => *i += 2,
				b'`' => {
					*i += 1;
					return;
				}
				b'$' if js.get(*i + 1) == Some(&b'{') => {
					stack.push(None);
					*i += 2;
					return;
				}
				_ => *i += 1,
			}
		}
	};

	while i < len {
		let c = js[i];
		let next = js.get(i + 1).copied();
		if c == b'"' || c == b'\'' {
			i += 1;
			while i < len && js[i] != c {
				i += if js[i] == b'\\' { 2 } else { 1 };
			}
			i += 1;
			previous = b'a';
		} else if c == b'`' {
			i += 1;
			skip_template(&mut i, &mut stack);
			previous = b'a';
		} else if c == b'/' && next == Some(b'/') {
			i = js[i..]
				.iter()
				.position(|&b| b == b'\n')
				.map_or(len, |p| p + i);
		} else if c == b'/' && next == Some(b'*') {
			i = js[i + 2..]
				.windows(2)
				.position(|w| w == b"*/")
				.map_or(len, |p| p + i + 4);
		} else if c == b'/' && REGEX_PRECEDERS.contains(&previous) {
			i += 1;
			let mut in_class = false;
			while i < len {
				match js[i] {
					b'\\' => i += 1,
					b'[' => in_class = true,
					b']' => in_class = false,
					b'/' if !in_class => break,
					_ => {}
				}
				i += 1;
			}
			i += 1;
			while i < len && js[i].is_ascii_alphabetic() {
				i += 1;
			}
			previous = b'a';
		} else if c == b'{' {
			stack.push(Some(i));
			i += 1;
			previous = c;
		} else if c == b'}' {
			let open = stack.pop();
			i += 1;
			match open {
				Some(None) => skip_template(&mut i, &mut stack),
				Some(Some(open)) => pairs.push((open, i - 1)),
				None => {}
			}
			previous = c;
		} else {
			if !c.is_ascii_whitespace() {
				previous = c;
			}
			i += 1;
		}
	}
	pairs
}
