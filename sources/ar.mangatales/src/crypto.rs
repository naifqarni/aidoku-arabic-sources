use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use aidoku::alloc::String;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Decrypts the search API's `ciphertext|?|iv|passphrase` payload: AES-256-CBC keyed with
/// SHA-256(passphrase), base64 ciphertext and IV.
pub fn decrypt(data: &str) -> Option<String> {
	let parts: aidoku::alloc::Vec<&str> = data.split('|').collect();
	let ciphertext = STANDARD.decode(parts.first()?.trim()).ok()?;
	let iv = STANDARD.decode(parts.get(2)?.trim()).ok()?;
	let key = Sha256::digest(parts.get(3)?.as_bytes());
	let plain = Aes256CbcDec::new_from_slices(&key, &iv)
		.ok()?
		.decrypt_padded_vec_mut::<Pkcs7>(&ciphertext)
		.ok()?;
	String::from_utf8(plain).ok()
}
