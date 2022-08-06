use anyhow::{anyhow, Error};

use aes_gcm::aead::{Aead, NewAead};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{password_hash::Salt, Argon2};
use base64ct::{Base64, Encoding};
use log::warn;

use crate::util::global_scope;

const AWS_CREDS_KEY_NAME: &'static str = "aws_credentials";
const KEY_LENGTH: usize = 256 / 8;
pub(crate) const SALT_LENGTH: usize = Salt::RECOMMENDED_LENGTH as usize;
const NONCE_LENGTH: usize = 96 / 8;

pub(crate) struct KeyData {
    key: [u8; KEY_LENGTH],
    pub(crate) salt: [u8; SALT_LENGTH],
}

impl KeyData {
    pub(crate) fn new_with_salt(b64salt: &str) -> Result<Self, Error> {
        let mut kd = Self {
            key: [0u8; KEY_LENGTH],
            salt: [0u8; SALT_LENGTH],
        };
        let decoded = Base64::decode_vec(b64salt)?;
        if decoded.len() == SALT_LENGTH {
            for i in 0..SALT_LENGTH {
                kd.salt[i] = decoded[i];
            }
        } else {
            return Err(anyhow!("incorrect salt length"));
        }
        Ok(kd)
    }

    pub(crate) fn new() -> Self {
        let mut kd = Self {
            key: [0u8; KEY_LENGTH],
            salt: [0u8; SALT_LENGTH],
        };

        let crypto = global_scope().crypto().unwrap();
        crypto
            .get_random_values_with_u8_array(&mut kd.salt[..])
            .unwrap();
        let b64salt = Base64::encode_string(&kd.salt[..]);

        kd
    }

    // we do this by-hand cloning to make sure the cloned key is zeroed
    pub(crate) fn new_from(input: &mut KeyData) -> Self {
        let mut kd = Self {
            key: [0u8; KEY_LENGTH],
            salt: [0u8; SALT_LENGTH],
        };

        for i in 0..KEY_LENGTH {
            kd.key[i] = input.key[i];
            input.key[i] = 0;
        }

        for i in 0..SALT_LENGTH {
            kd.salt[i] = input.salt[i];
            input.salt[i] = 0;
        }
        kd
    }

    pub(crate) fn fill_from_password(&mut self, password: &str) {
        // we use default parameters, but explicitly set the output length
        // so we don't have to count on defaults
        let argon2_params = argon2::Params::new(
            argon2::Params::DEFAULT_M_COST,
            argon2::Params::DEFAULT_T_COST,
            argon2::Params::DEFAULT_P_COST,
            Some(KEY_LENGTH),
        )
        .unwrap();

        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2_params,
        );

        argon2
            .hash_password_into(password.as_bytes(), &self.salt[..], &mut self.key[..])
            .unwrap();
    }

    pub(crate) fn encrypt_string(&self, plaintext: &str) -> Result<String, Error> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|e| anyhow!(e))?;
        let mut nonce = [0u8; NONCE_LENGTH];
        let crypto = global_scope().crypto().unwrap();
        crypto
            .get_random_values_with_u8_array(&mut nonce[..])
            .unwrap();

        let mut result = Base64::encode_string(&nonce[..]) + ":";
        let nonce = Nonce::from_slice(&nonce[..]);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!(e))?;
        result = result + &Base64::encode_string(&ciphertext);
        Ok(result)
    }

    pub(crate) fn decrypt_string(&self, plaintext: &str) -> Result<String, Error> {
        let mut split_iter = plaintext.split(':');
        let b64nonce = split_iter
            .next()
            .ok_or(anyhow!("bad data for decryption: no nonce"))?;
        let b64ciphertext = split_iter
            .next()
            .ok_or(anyhow!("bad data for decryption: no ciphertext"))?;

        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|e| anyhow!(e))?;

        let nonce = Base64::decode_vec(b64nonce)?;
        let nonce = Nonce::from_slice(&nonce);
        let ciphertext = Base64::decode_vec(b64ciphertext)?;

        let plaintext = cipher
            .decrypt(nonce, &*ciphertext)
            .map_err(|e| anyhow!(e))?;
        Ok(String::from_utf8(plaintext)?)
    }
}
