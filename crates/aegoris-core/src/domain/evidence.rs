use serde::{Deserialize, Serialize};

use crate::domain::ids::FactId;

/// The typed link from a generated `Claim` back to the `Fact`s it derives from.
///
/// Evidence is what makes grounding a structural property rather than a prompt
/// request: a claim cannot be constructed without at least one of these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub fact_id: FactId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

impl Evidence {
    pub fn new(fact_id: FactId) -> Self {
        Self {
            fact_id,
            quote: None,
        }
    }

    pub fn with_quote(mut self, quote: impl Into<String>) -> Self {
        self.quote = Some(quote.into());
        self
    }
}
