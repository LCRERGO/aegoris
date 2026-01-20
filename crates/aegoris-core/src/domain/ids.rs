use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Deterministic, positional identifier, e.g. `f_3`.
            pub fn generate(position: usize) -> Self {
                Self(format!("{}{}", $prefix, position))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_newtype!(FactId, "f_");
id_newtype!(RoleId, "r_");
id_newtype!(RequirementId, "rq_");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_positional_and_deterministic() {
        assert_eq!(FactId::generate(1).as_str(), "f_1");
        assert_eq!(FactId::generate(42).as_str(), "f_42");
        assert_eq!(RoleId::generate(2).as_str(), "r_2");
        assert_eq!(RequirementId::generate(7).as_str(), "rq_7");
    }

    #[test]
    fn ids_round_trip_through_json_as_strings() {
        let id = FactId::generate(3);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"f_3\"");
        let back: FactId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
