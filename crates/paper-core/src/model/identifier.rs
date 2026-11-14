use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Doi(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct ArxivId(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CitationKey(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash([u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaperIdentifier {
    Doi(Doi),
    Arxiv(ArxivId),
    Url(String),
}

#[derive(Debug, Error)]
pub enum IdentifierError {
    #[error("Invalid DOI: {0}")]
    InvalidDoi(String),

    #[error("Invalid arXiv identifier: {0}")]
    InvalidArxivId(String),

    #[error("Invalid citation key: {0}")]
    InvalidCitationKey(String),

    #[error("Invalid content hash: {0}")]
    InvalidContentHash(String),
    #[error("An identifier has both a validated and a raw value")]
    AmbiguousIdentifier,
    #[error("Unvalidated legacy identifiers cannot be used as new input")]
    UnvalidatedIdentifier,
}

impl ContentHash {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl CitationKey {
    pub fn parse(value: &str) -> Result<Self, IdentifierError> {
        Self::try_from(value.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CitationKey {
    type Error = IdentifierError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let valid = !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'));

        if !valid {
            return Err(IdentifierError::InvalidCitationKey(value));
        }

        Ok(Self(value))
    }
}

impl std::str::FromStr for CitationKey {
    type Err = IdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl std::fmt::Display for CitationKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::{Error, StringDeserializer};

    fn deserialize<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, Error> {
        T::deserialize(StringDeserializer::<Error>::new(value.to_owned()))
    }

    #[test]
    fn citation_key_deserialization_preserves_validation() {
        for value in ["Smith2024", "smith_2024-paper:v1.2"] {
            let key: CitationKey = deserialize(value).unwrap();
            assert_eq!(key.as_str(), value);
            assert_eq!(key, CitationKey::parse(value).unwrap());
        }
        for value in ["", "Smith 2024", "key/part", "日本語"] {
            let error = deserialize::<CitationKey>(value).unwrap_err();
            assert_eq!(
                error.to_string(),
                CitationKey::parse(value).unwrap_err().to_string()
            );
        }
    }

    #[test]
    fn validated_identifiers_deserialize_as_strings() {
        assert_eq!(
            deserialize::<Doi>("10.1000/example").unwrap().0,
            "10.1000/example"
        );
        assert_eq!(
            deserialize::<ArxivId>("2401.12345v2").unwrap().0,
            "2401.12345v2"
        );
    }
}

impl Doi {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl ArxivId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[path = "identifier_parsing.rs"]
mod parsing;
