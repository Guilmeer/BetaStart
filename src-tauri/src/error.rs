use serde::{Serialize, Serializer};

/// Erro unificado do backend. Implementa `Serialize` para poder ser devolvido
/// diretamente pelos comandos Tauri (`Result<T, AppError>`).
#[allow(dead_code)] // `Platform`/`NotFound` serão usados nas próximas fases.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("erro de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro de configuração: {0}")]
    Config(String),

    #[error("erro de banco de dados: {0}")]
    Database(String),

    #[error("erro de plataforma: {0}")]
    Platform(String),

    #[error("não encontrado: {0}")]
    NotFound(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Database(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::Config(value.to_string())
    }
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        AppError::Platform(value)
    }
}

pub type AppResult<T> = Result<T, AppError>;
