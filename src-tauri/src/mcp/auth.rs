use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use uuid::Uuid;

/// Gera um token opaco no formato `bst_<hex>`.
pub fn generate_token() -> String {
    format!("bst_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// Hash Argon2id do token (nunca armazenamos o token em claro).
pub fn hash_token(token: &str) -> Result<String, String> {
    let salt =
        SaltString::encode_b64(Uuid::new_v4().as_bytes()).map_err(|error| error.to_string())?;
    Argon2::default()
        .hash_password(token.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| error.to_string())
}

pub fn verify_token(token: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(token.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_roundtrip() {
        let token = generate_token();
        assert!(token.starts_with("bst_"));
        let hash = hash_token(&token).expect("hash");
        assert!(verify_token(&token, &hash));
        assert!(!verify_token("bst_errado", &hash));
        assert_ne!(token, hash);
    }
}
