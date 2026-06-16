use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_aegoris"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn base_command() -> Command {
    let mut command = Command::new(bin());
    command
        .env_remove("AEGORIS_API_KEY")
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("AEGORIS_MODE");
    command
}

#[test]
fn parse_emits_canonical_json() {
    let output = base_command()
        .args(["parse", "--profile"])
        .arg(fixture("ada-profile.json"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"name\": \"Ada Lovelace\""));
    assert!(stdout.contains("\"id\": \"r_2\""));
}

#[test]
fn generate_template_mode_writes_all_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let output = base_command()
        .args(["generate", "--profile"])
        .arg(fixture("ada-profile.json"))
        .arg("--jd")
        .arg(fixture("backend-jd.txt"))
        .args(["--out"])
        .arg(dir.path())
        .args(["--no-llm", "--format", "md,ats"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let resume_md = dir.path().join("ada-lovelace-resume.md");
    let resume_txt = dir.path().join("ada-lovelace-resume.txt");
    let letter_md = dir.path().join("ada-lovelace-cover-letter.md");
    let curation = dir.path().join("ada-lovelace-curation.json");
    assert!(resume_md.is_file());
    assert!(resume_txt.is_file());
    assert!(letter_md.is_file());
    assert!(curation.is_file());

    let markdown = std::fs::read_to_string(&resume_md).unwrap();
    assert!(markdown.contains("# Ada Lovelace"));
    assert!(markdown.contains("## Experience"));
    assert!(markdown.contains("Rust"));

    let curation_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&curation).unwrap()).unwrap();
    assert_eq!(curation_json["schema"], "aegoris/curation/v1");
    assert_eq!(curation_json["mode"], "template");
}

#[test]
fn generate_refuses_to_overwrite_without_force() {
    let dir = tempfile::tempdir().unwrap();
    let run = || {
        base_command()
            .args(["generate", "--profile"])
            .arg(fixture("ada-profile.json"))
            .arg("--jd")
            .arg(fixture("backend-jd.txt"))
            .args(["--out"])
            .arg(dir.path())
            .args(["--no-llm", "--format", "md"])
            .output()
            .unwrap()
    };
    assert!(run().status.success());
    let second = run();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("already exists"));

    let forced = base_command()
        .args(["generate", "--profile"])
        .arg(fixture("ada-profile.json"))
        .arg("--jd")
        .arg(fixture("backend-jd.txt"))
        .args(["--out"])
        .arg(dir.path())
        .args(["--no-llm", "--format", "md", "--force"])
        .output()
        .unwrap();
    assert!(forced.status.success());
}

#[test]
fn llm_mode_without_key_exits_with_config_code() {
    let dir = tempfile::tempdir().unwrap();
    let output = base_command()
        .args(["generate", "--profile"])
        .arg(fixture("ada-profile.json"))
        .arg("--jd")
        .arg(fixture("backend-jd.txt"))
        .args(["--out"])
        .arg(dir.path())
        .args(["--mode", "llm"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn score_reports_requirements() {
    let output = base_command()
        .args(["score", "--profile"])
        .arg(fixture("ada-profile.json"))
        .arg("--jd")
        .arg(fixture("backend-jd.txt"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Requirements:"));
    assert!(stdout.contains("Rust"));
    assert!(stdout.contains("Top facts:"));
}

#[test]
fn dry_run_prints_plan_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let output = base_command()
        .args(["generate", "--profile"])
        .arg(fixture("ada-profile.json"))
        .arg("--jd")
        .arg(fixture("backend-jd.txt"))
        .args(["--out"])
        .arg(dir.path())
        .args(["--no-llm", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("selected_fact_ids"));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn linkedin_export_zip_is_parsed() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("linkedin-export.zip");
    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("Profile.csv", options).unwrap();
        writer
            .write_all(
                b"First Name,Last Name,Headline,Summary,Geo Location,Websites\nAda,Lovelace,Backend Engineer,Pioneering programmer,London,https://ada.example\n",
            )
            .unwrap();
        writer.start_file("Positions.csv", options).unwrap();
        writer
            .write_all(
                b"Company Name,Title,Description,Location,Started On,Finished On\nAnalytical Engines,Senior Backend Engineer,\"Designed a Rust pipeline\",London,2022,\n",
            )
            .unwrap();
        writer.start_file("Skills.csv", options).unwrap();
        writer.write_all(b"Name\nRust\nPostgreSQL\n").unwrap();
        writer.finish().unwrap();
    }

    let output = base_command()
        .args(["parse", "--profile"])
        .arg(&zip_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"name\": \"Ada Lovelace\""));
    assert!(stdout.contains("Analytical Engines"));
    assert!(stdout.contains("Rust"));
}

#[cfg(feature = "pdf")]
#[test]
fn generate_produces_a_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let output = base_command()
        .args(["generate", "--profile"])
        .arg(fixture("ada-profile.json"))
        .arg("--jd")
        .arg(fixture("backend-jd.txt"))
        .args(["--out"])
        .arg(dir.path())
        .args(["--no-llm", "--format", "pdf"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pdf = dir.path().join("ada-lovelace-resume.pdf");
    assert!(pdf.is_file());
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}
