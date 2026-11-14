use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContributorName {
    Person {
        given: Option<String>,
        family: String,
    },

    Organization(String),

    Literal(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contributor {
    pub name: ContributorName,
    pub orcid: Option<String>,
}