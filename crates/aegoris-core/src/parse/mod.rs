//! Input parsing: profiles and job descriptions into the domain model.

pub mod jd;
pub mod linkedin;
pub mod linkedin_html;
pub mod profile_json;
pub mod profile_text;

pub use jd::parse_job_description;
pub use linkedin::{parse_linkedin_export, LinkedInExport};
pub use linkedin_html::parse_linkedin_html;
pub use profile_json::{parse_profile, parse_profile_json};
pub use profile_text::parse_profile_text;
