use super::{ArxivId, Doi, IdentifierError};

fn strip_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &s[prefix.len()..])
}
fn wrapped(value: &str, hosts: &[&str], paths: &[&str], prefix: &str) -> Option<String> {
    let value = value.trim();
    if value.contains("://") {
        let url = url::Url::parse(value).ok()?;
        if !["http", "https"].contains(&url.scheme())
            || !hosts.contains(&url.host_str()?)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        // Read the original path: URL parser path normalization must not alter DOI suffixes.
        if value.contains('\\') {
            return None;
        }
        let after_authority = value.split_once("://")?.1;
        let path = &after_authority[after_authority.find('/')?..];
        if path.as_bytes().iter().enumerate().any(|(i, c)| {
            *c == b'%'
                && (i + 2 >= path.len()
                    || !path.as_bytes()[i + 1].is_ascii_hexdigit()
                    || !path.as_bytes()[i + 2].is_ascii_hexdigit())
        }) {
            return None;
        }
        let path = paths.iter().find_map(|p| path.strip_prefix(p))?;
        let decoded = percent_encoding::percent_decode_str(path)
            .decode_utf8()
            .ok()?;
        Some(decoded.into_owned())
    } else {
        Some(strip_prefix(value, prefix).unwrap_or(value).trim().into())
    }
}
impl TryFrom<String> for Doi {
    type Error = IdentifierError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let value = wrapped(&raw, &["doi.org", "dx.doi.org"], &["/"], "doi:")
            .ok_or_else(|| IdentifierError::InvalidDoi(raw.clone()))?;
        let valid = value.split_once('/').is_some_and(|(prefix, suffix)| {
            prefix.strip_prefix("10.").is_some_and(|code| {
                code.split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
            }) && !suffix.is_empty()
                && !value.chars().any(|c| c.is_whitespace() || c.is_control())
        });
        if !valid {
            return Err(IdentifierError::InvalidDoi(raw));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}
impl TryFrom<String> for ArxivId {
    type Error = IdentifierError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let mut value = wrapped(
            &raw,
            &["arxiv.org", "www.arxiv.org"],
            &["/abs/", "/pdf/"],
            "arxiv:",
        )
        .ok_or_else(|| IdentifierError::InvalidArxivId(raw.clone()))?;
        if raw.contains("://") && raw.contains("/pdf/") {
            value = value.strip_suffix(".pdf").unwrap_or(&value).into();
        }
        if !valid_arxiv(&value) {
            return Err(IdentifierError::InvalidArxivId(raw));
        }
        Ok(Self(value))
    }
}
fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}
fn valid_arxiv(value: &str) -> bool {
    let (base, version) = if let Some((a, b)) = value.rsplit_once('v').filter(|(_, b)| digits(b)) {
        (a, Some(b))
    } else {
        (value, None)
    };
    if version.is_some_and(|v| !digits(v) || v.starts_with('0')) {
        return false;
    }
    if let Some((archive, date)) = base.split_once('/') {
        let archive_valid = archive.split_once('.').map_or_else(
            || !archive.is_empty() && archive.bytes().all(|c| c.is_ascii_lowercase() || c == b'-'),
            |(name, category)| {
                !name.is_empty()
                    && name.bytes().all(|c| c.is_ascii_lowercase() || c == b'-')
                    && category.len() == 2
                    && category.bytes().all(|c| c.is_ascii_uppercase())
            },
        );
        if !archive_valid || date.len() != 7 || !digits(date) {
            return false;
        }
        let y = date[..2].parse::<u16>().unwrap();
        let month = date[2..4].parse::<u16>().unwrap();
        let year = if y >= 91 { 1900 + y } else { 2000 + y };
        (1991 * 12 + 7..=2007 * 12 + 3).contains(&(year * 12 + month))
            && (1..=12).contains(&month)
            && date[4..].parse::<u16>().unwrap() > 0
    } else if let Some((date, sequence)) = base.split_once('.') {
        if date.len() != 4 || !digits(date) || !digits(sequence) {
            return false;
        }
        let year = 2000 + date[..2].parse::<u16>().unwrap();
        let month = date[2..].parse::<u16>().unwrap();
        if !(1..=12).contains(&month) || year * 12 + month < 2007 * 12 + 4 {
            return false;
        }
        let width = if year * 12 + month <= 2014 * 12 + 12 {
            4
        } else {
            5
        };
        sequence.len() == width && sequence.bytes().any(|c| c != b'0')
    } else {
        false
    }
}
macro_rules! api {
    ($ty:ident) => {
        impl $ty {
            pub fn parse(value: &str) -> Result<Self, IdentifierError> {
                Self::try_from(value.to_owned())
            }
        }
        impl std::str::FromStr for $ty {
            type Err = IdentifierError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}
api!(Doi);
api!(ArxivId);
