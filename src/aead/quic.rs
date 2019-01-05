// Copyright 2018 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! QUIC Header Protection.
//!
//! See draft-ietf-quic-tls.

use crate::{
    aead::{aes, block::Block, chacha},
    cpu, error,
    polyfill::convert::*,
};

/// A key for generating QUIC Header Protection masks.
pub struct HeaderProtectionKey {
    inner: KeyInner,
    algorithm: &'static Algorithm,
}

#[allow(variant_size_differences)]
enum KeyInner {
    Aes(aes::Key),
    ChaCha20(chacha::Key),
}

impl HeaderProtectionKey {
    /// Create a new header protection key.
    ///
    /// `key_bytes` must be exactly `algorithm.key_len` bytes long.
    pub fn new(
        algorithm: &'static Algorithm, key_bytes: &[u8],
    ) -> Result<Self, error::Unspecified> {
        cpu::cache_detected_features();
        Ok(HeaderProtectionKey {
            inner: (algorithm.init)(key_bytes)?,
            algorithm,
        })
    }

    /// Generate a new QUIC Header Protection mask.
    ///
    /// `sample` must be exactly 16 bytes long.
    pub fn new_mask(&self, sample: &[u8]) -> Result<[u8; 5], error::Unspecified> {
        let sample = <&[u8; SAMPLE_LEN]>::try_from_(sample)?;
        let sample = Block::from(sample);

        let out = (self.algorithm.new_mask)(&self.inner, sample);
        Ok(out)
    }
}

const SAMPLE_LEN: usize = super::TAG_LEN;

/// A QUIC Header Protection Algorithm.
pub struct Algorithm {
    init: fn(key: &[u8]) -> Result<KeyInner, error::Unspecified>,

    new_mask: fn(key: &KeyInner, sample: Block) -> [u8; 5],

    key_len: usize,
    id: AlgorithmID,
}

impl Algorithm {
    /// The length of the key.
    #[inline(always)]
    pub fn key_len(&self) -> usize { self.key_len }
}

derive_debug_via_self!(Algorithm, self.id);

#[derive(Debug, Eq, PartialEq)]
enum AlgorithmID {
    AES_128,
    AES_256,
    CHACHA20,
}

impl PartialEq for Algorithm {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}

impl Eq for Algorithm {}

/// AES-128.
pub static AES_128: Algorithm = Algorithm {
    key_len: 16,
    init: aes_init_128,
    new_mask: aes_new_mask,
    id: AlgorithmID::AES_128,
};

/// AES-256.
pub static AES_256: Algorithm = Algorithm {
    key_len: 32,
    init: aes_init_256,
    new_mask: aes_new_mask,
    id: AlgorithmID::AES_256,
};

fn aes_init_128(key: &[u8]) -> Result<KeyInner, error::Unspecified> {
    let aes_key = aes::Key::new(key, aes::Variant::AES_128)?;
    Ok(KeyInner::Aes(aes_key))
}

fn aes_init_256(key: &[u8]) -> Result<KeyInner, error::Unspecified> {
    let aes_key = aes::Key::new(key, aes::Variant::AES_256)?;
    Ok(KeyInner::Aes(aes_key))
}

fn aes_new_mask(key: &KeyInner, sample: Block) -> [u8; 5] {
    let aes_key = match key {
        KeyInner::Aes(key) => key,
        _ => unreachable!(),
    };

    aes_key.new_mask(sample)
}

/// ChaCha20.
pub static CHACHA20: Algorithm = Algorithm {
    key_len: chacha::KEY_LEN,
    init: chacha20_init,
    new_mask: chacha20_new_mask,
    id: AlgorithmID::CHACHA20,
};

fn chacha20_init(key: &[u8]) -> Result<KeyInner, error::Unspecified> {
    let chacha20_key: &[u8; chacha::KEY_LEN] = key.try_into_()?;
    Ok(KeyInner::ChaCha20(chacha::Key::from(chacha20_key)))
}

fn chacha20_new_mask(key: &KeyInner, sample: Block) -> [u8; 5] {
    let chacha20_key = match key {
        KeyInner::ChaCha20(key) => key,
        _ => unreachable!(),
    };

    chacha20_key.new_mask(sample)
}