use std::collections::{BTreeMap, HashMap};

use crate::domain::ids::RoleId;
use crate::domain::profile::{Certification, Contact, Education, Profile, Project, Role, Skill};
use crate::error::{CoreError, CoreResult};

/// A LinkedIn data-export, unpacked into `file name -> CSV contents`.
///
/// Reading the archive is I/O and lives in the CLI; this adapter only sees
/// strings, which keeps `aegoris-core` free of filesystem access.
#[derive(Debug, Clone, Default)]
pub struct LinkedInExport {
    files: BTreeMap<String, String>,
}

impl LinkedInExport {
    pub fn new(files: BTreeMap<String, String>) -> Self {
        Self { files }
    }

    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            files: pairs.into_iter().collect(),
        }
    }

    pub fn file(&self, name: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Normalize the export into a `Profile`.
    pub fn parse(&self) -> CoreResult<Profile> {
        parse_export(self)
    }
}

/// Normalize a LinkedIn data-export into a `Profile`.
pub fn parse_linkedin_export(files: &BTreeMap<String, String>) -> CoreResult<Profile> {
    LinkedInExport::new(files.clone()).parse()
}

fn parse_export(export: &LinkedInExport) -> CoreResult<Profile> {
    let profile_rows = rows(export.file("Profile.csv"))?;
    let profile_row = profile_rows.first();

    let name = match profile_row {
        Some(row) => {
            let first = field(row, &["first name", "firstname"]).unwrap_or_default();
            let last = field(row, &["last name", "lastname"]).unwrap_or_default();
            format!("{first} {last}").trim().to_string()
        }
        None => String::new(),
    };
    if name.is_empty() {
        return Err(CoreError::parse(
            "LinkedIn export is missing Profile.csv or a name",
        ));
    }

    let mut profile = Profile {
        name,
        headline: profile_row.and_then(|row| field(row, &["headline"])),
        location: profile_row.and_then(|row| field(row, &["geo location", "location", "address"])),
        summary: profile_row.and_then(|row| field(row, &["summary"])),
        contact: Contact {
            website: profile_row
                .and_then(|row| field(row, &["websites", "website"]))
                .map(|value| first_url(&value)),
            ..Default::default()
        },
        ..Default::default()
    };

    for (index, row) in rows(export.file("Positions.csv"))?.iter().enumerate() {
        let title = field(row, &["title", "position"]).unwrap_or_default();
        let organization = field(row, &["company name", "company"]).unwrap_or_default();
        if title.is_empty() && organization.is_empty() {
            continue;
        }
        let end = field(row, &["finished on", "end date"]);
        profile.roles.push(Role {
            id: RoleId::generate(index + 1),
            title,
            organization,
            location: field(row, &["location"]),
            start: field(row, &["started on", "start date"]),
            current: end.is_none(),
            end,
            bullets: split_description(&field(row, &["description"]).unwrap_or_default()),
            skills: Vec::new(),
        });
    }

    for row in rows(export.file("Education.csv"))?.iter() {
        let institution = field(row, &["school name", "school", "institution"]).unwrap_or_default();
        if institution.is_empty() {
            continue;
        }
        profile.education.push(Education {
            degree: field(row, &["degree name", "degree"]),
            institution,
            field: field(row, &["field of study", "notes"]),
            start: field(row, &["start date"]),
            end: field(row, &["end date"]),
        });
    }

    for row in rows(export.file("Skills.csv"))?.iter() {
        if let Some(name) = field(row, &["name", "skill"]) {
            profile.skills.push(Skill { name, level: None });
        }
    }

    for row in rows(export.file("Certifications.csv"))?.iter() {
        if let Some(name) = field(row, &["name", "certification"]) {
            profile.certifications.push(Certification {
                name,
                issuer: field(row, &["authority", "issuer"]),
                year: field(row, &["time period", "date"]),
            });
        }
    }

    for row in rows(export.file("Projects.csv"))?.iter() {
        let name = field(row, &["title", "name"]).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        profile.projects.push(Project {
            name,
            description: field(row, &["description"]),
            bullets: Vec::new(),
            skills: Vec::new(),
            url: field(row, &["url"]),
        });
    }

    Ok(profile)
}

fn rows(contents: Option<&str>) -> CoreResult<Vec<HashMap<String, String>>> {
    let Some(contents) = contents else {
        return Ok(Vec::new());
    };
    let contents = contents.trim_start_matches('\u{feff}');
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(contents.as_bytes());

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| CoreError::parse(format!("invalid LinkedIn CSV header: {e}")))?
        .iter()
        .map(|header| header.trim().to_lowercase())
        .collect();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record =
            record.map_err(|e| CoreError::parse(format!("invalid LinkedIn CSV row: {e}")))?;
        let mut row = HashMap::new();
        for (header, value) in headers.iter().zip(record.iter()) {
            let value = value.trim();
            if !value.is_empty() {
                row.insert(header.clone(), value.to_string());
            }
        }
        rows.push(row);
    }
    Ok(rows)
}

fn field(row: &HashMap<String, String>, aliases: &[&str]) -> Option<String> {
    for alias in aliases {
        if let Some(value) = row.get(*alias) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn split_description(description: &str) -> Vec<String> {
    description
        .lines()
        .map(|line| line.trim().trim_start_matches(['-', '*', '•', '·']).trim())
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn first_url(value: &str) -> String {
    value
        .split(['\n', ',', ';'])
        .map(str::trim)
        .find(|part| !part.is_empty())
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE_CSV: &str = "First Name,Last Name,Headline,Summary,Geo Location,Websites\nAda,Lovelace,Backend Engineer,Pioneering programmer,London,https://ada.example\n";
    const POSITIONS_CSV: &str = "Company Name,Title,Description,Location,Started On,Finished On\nAnalytical Engines,Senior Backend Engineer,\"- Designed a pipeline\n- Reduced latency\",London,2022,\nBabbage & Co,Backend Engineer,Built APIs,London,2019,2022\n";
    const EDUCATION_CSV: &str =
        "School Name,Start Date,End Date,Notes,Degree Name\nUniversity of London,2016,2019,,BSc\n";
    const SKILLS_CSV: &str = "Name\nRust\nPostgreSQL\n";
    const CERTS_CSV: &str =
        "Name,Authority,Time Period\nCertified Kubernetes Administrator,CNCF,2023\n";
    const PROJECTS_CSV: &str =
        "Title,Description,URL\nAnalytical Engine,A general-purpose computer,https://example.com\n";

    fn export() -> LinkedInExport {
        LinkedInExport::from_pairs([
            ("Profile.csv".to_string(), PROFILE_CSV.to_string()),
            ("Positions.csv".to_string(), POSITIONS_CSV.to_string()),
            ("Education.csv".to_string(), EDUCATION_CSV.to_string()),
            ("Skills.csv".to_string(), SKILLS_CSV.to_string()),
            ("Certifications.csv".to_string(), CERTS_CSV.to_string()),
            ("Projects.csv".to_string(), PROJECTS_CSV.to_string()),
        ])
    }

    #[test]
    fn parses_a_full_export() {
        let profile = export().parse().unwrap();
        assert_eq!(profile.name, "Ada Lovelace");
        assert_eq!(profile.headline.as_deref(), Some("Backend Engineer"));
        assert_eq!(profile.location.as_deref(), Some("London"));
        assert_eq!(
            profile.contact.website.as_deref(),
            Some("https://ada.example")
        );
        assert_eq!(profile.roles.len(), 2);
        assert_eq!(profile.roles[0].id.as_str(), "r_1");
        assert_eq!(profile.roles[0].organization, "Analytical Engines");
        assert!(profile.roles[0].current);
        assert_eq!(profile.roles[0].bullets.len(), 2);
        assert_eq!(profile.roles[1].end.as_deref(), Some("2022"));
        assert_eq!(profile.skills.len(), 2);
        assert_eq!(profile.education[0].degree.as_deref(), Some("BSc"));
        assert_eq!(profile.certifications[0].issuer.as_deref(), Some("CNCF"));
        assert_eq!(profile.projects[0].name, "Analytical Engine");
    }

    #[test]
    fn missing_profile_csv_is_an_error() {
        let export =
            LinkedInExport::from_pairs([("Skills.csv".to_string(), SKILLS_CSV.to_string())]);
        assert!(export.parse().is_err());
    }

    #[test]
    fn handles_utf8_bom_and_missing_optional_files() {
        let with_bom = format!("\u{feff}{PROFILE_CSV}");
        let export = LinkedInExport::from_pairs([("Profile.csv".to_string(), with_bom)]);
        let profile = export.parse().unwrap();
        assert_eq!(profile.name, "Ada Lovelace");
        assert!(profile.roles.is_empty());
    }
}
