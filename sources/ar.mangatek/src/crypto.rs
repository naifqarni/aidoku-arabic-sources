//! The site reader's overlay unlock (proof, esid, pow, overlay key unwrap); its WASM module is
//! plain SHA-256 over these inputs and AES-GCM is done in JS.
use aes_gcm::{
	AesGcm, KeyInit,
	aead::{Aead, consts::U12, consts::U16, generic_array::GenericArray},
	aes::{Aes128, Aes256},
};
use aidoku::{
	Result,
	alloc::{String, Vec},
	error,
	prelude::*,
};
use base64::{
	Engine,
	alphabet::URL_SAFE,
	engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use sha2::{Digest, Sha256};

const KEY: &str = "ff453871399fe268588a0936b45376022d85ed0fd1292001d5102f6a30291dc1";

pub fn sha256(data: &[u8]) -> [u8; 32] {
	Sha256::digest(data).into()
}

pub fn hex(bytes: &[u8]) -> String {
	const DIGITS: &[u8; 16] = b"0123456789abcdef";
	let mut out = String::with_capacity(bytes.len() * 2);
	for &b in bytes {
		out.push(DIGITS[(b >> 4) as usize] as char);
		out.push(DIGITS[(b & 0xf) as usize] as char);
	}
	out
}

pub fn decode_hex(s: &str) -> Option<Vec<u8>> {
	let s = s.trim();
	if !s.len().is_multiple_of(2) {
		return None;
	}
	(0..s.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
		.collect()
}

pub fn proof(proof_salt: &str, token: &str, chapter_id: i64) -> String {
	hex(&sha256(
		format!("{proof_salt}|{token}|{chapter_id}").as_bytes(),
	))
}

pub fn esid(reader_seed: &str) -> String {
	hex(&sha256(format!("esid|{reader_seed}").as_bytes()))[..8].into()
}

fn to_base36(mut n: u64) -> String {
	if n == 0 {
		return "0".into();
	}
	let mut digits = Vec::new();
	while n > 0 {
		digits.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
		n /= 36;
	}
	digits.reverse();
	String::from_utf8(digits).unwrap_or_default()
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
	let mut bits = 0;
	for &b in bytes {
		if b == 0 {
			bits += 8;
		} else {
			return bits + b.leading_zeros();
		}
	}
	bits
}

/// Smallest base-36 counter whose SHA-256("<nonce>:<counter>") starts with `bits` zero bits;
/// the reader gives up after 5M tries.
pub fn pow(nonce: &str, bits: u32) -> Option<String> {
	if bits == 0 {
		return None;
	}
	(0..5_000_000u64)
		.map(to_base36)
		.find(|answer| leading_zero_bits(&sha256(format!("{nonce}:{answer}").as_bytes())) >= bits)
}

fn aes_gcm_decrypt(key: &[u8], iv: &[u8], ciphertext_and_tag: &[u8]) -> Option<Vec<u8>> {
	macro_rules! run {
		($aes:ty, $nonce:ty) => {
			AesGcm::<$aes, $nonce>::new_from_slice(key)
				.ok()?
				.decrypt(GenericArray::from_slice(iv), ciphertext_and_tag)
				.ok()
		};
	}
	match (key.len(), iv.len()) {
		(32, 12) => run!(Aes256, U12),
		(32, 16) => run!(Aes256, U16),
		(16, 12) => run!(Aes128, U12),
		(16, 16) => run!(Aes128, U16),
		_ => None,
	}
}

/// `ivHex:ctHex:tagHex`, AES-GCM.
pub fn decrypt_hex_blob(blob: &str, key_hex: Option<&str>) -> Result<String> {
	let parts: Vec<Vec<u8>> = blob
		.split(':')
		.map(decode_hex)
		.collect::<Option<_>>()
		.ok_or_else(|| error!("Unexpected overlay format"))?;
	let [iv, ciphertext, tag] = parts.as_slice() else {
		bail!("Unexpected overlay format");
	};
	if tag.len() != 16 {
		bail!("Unsupported overlay tag length");
	}
	let key = decode_hex(key_hex.unwrap_or(KEY)).ok_or_else(|| error!("Invalid overlay key"))?;
	let mut data = ciphertext.clone();
	data.extend_from_slice(tag);
	let plain =
		aes_gcm_decrypt(&key, iv, &data).ok_or_else(|| error!("Overlay decryption failed"))?;
	String::from_utf8(plain).map_err(|_| error!("Invalid overlay text"))
}

/// Version byte, then `[tagLen u16][ivLen u16][tag][ct][iv]` XORed with a SHA-256 keystream.
pub fn decrypt_envelope(
	reader_seed: &str,
	overlay: &str,
	ks: &str,
	token: &str,
	chapter_id: i64,
) -> Result<String> {
	let engine = GeneralPurpose::new(
		&URL_SAFE,
		GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
	);
	let bytes = engine
		.decode(overlay.trim())
		.map_err(|_| error!("Invalid overlay envelope"))?;
	let version = *bytes
		.first()
		.ok_or_else(|| error!("Empty overlay envelope"))?;
	if version != 3 {
		bail!("Unsupported overlay version {version}");
	}
	let mut data = bytes[1..].to_vec();
	for (block_index, chunk) in data.chunks_mut(32).enumerate() {
		let block = sha256(format!("{reader_seed}|v{version}|{ks}|{block_index}").as_bytes());
		for (byte, key) in chunk.iter_mut().zip(block.iter()) {
			*byte ^= key;
		}
	}
	let read_u16 = |at: usize| -> Option<usize> {
		Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]) as usize)
	};
	let err = || error!("Invalid overlay envelope");
	let tag_len = read_u16(0).ok_or_else(err)?;
	let iv_len = read_u16(2).ok_or_else(err)?;
	if tag_len != 16 || data.len() < 4 + tag_len + iv_len {
		return Err(err());
	}
	let tag = &data[4..4 + tag_len];
	let ciphertext = &data[4 + tag_len..data.len() - iv_len];
	let iv = &data[data.len() - iv_len..];

	let key = sha256(format!("{reader_seed}|v{version}|{ks}|{token}|{chapter_id}").as_bytes());
	let mut sealed = ciphertext.to_vec();
	sealed.extend_from_slice(tag);
	let plain =
		aes_gcm_decrypt(&key, iv, &sealed).ok_or_else(|| error!("Overlay decryption failed"))?;
	String::from_utf8(plain).map_err(|_| error!("Invalid overlay text"))
}
