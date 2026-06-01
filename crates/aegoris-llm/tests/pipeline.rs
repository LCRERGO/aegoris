use aegoris_core::curate::{curate, CurationConfig};
use aegoris_core::domain::artifact::ArtifactKind;
use aegoris_core::domain::fact::FactStore;
use aegoris_core::grounding::{apply_policy, ClaimVerifier, GroundingPolicy, StructuralVerifier};
use aegoris_core::matching::LexicalScorer;
use aegoris_core::parse::{parse_job_description, parse_profile_json};
use aegoris_core::phrase::PhraseContext;
use aegoris_llm::{FakeLanguageModel, LlmPhraser};

const PROFILE: &str = r#"{
    "name": "Ada Lovelace",
    "summary": "Pioneering programmer.",
    "roles": [{"id":"r_1","title":"Engineer","organization":"Babbage",
               "bullets":["Wrote the first algorithm for the Analytical Engine"]}],
    "skills": [{"name":"Algorithms"}]
}"#;

fn setup() -> (
    aegoris_core::domain::Profile,
    aegoris_core::domain::JobDescription,
    FactStore,
) {
    let profile = parse_profile_json(PROFILE).unwrap();
    let jd = parse_job_description("Requirements:\n- Algorithms");
    let store = FactStore::from_profile(&profile);
    (profile, jd, store)
}

#[tokio::test]
async fn llm_phrasing_produces_structurally_grounded_claims() {
    let (profile, jd, store) = setup();
    let plan = curate(
        &profile,
        &jd,
        &store,
        &LexicalScorer::new(),
        &CurationConfig::default(),
    )
    .unwrap();

    let model = FakeLanguageModel::always(
        r#"{"claims":[{"text":"Wrote the first algorithm","fact_ids":["f_1"],
            "section":"experience","role_id":"r_1"}]}"#,
    );
    let phraser = LlmPhraser::new(model);
    let claims = phraser
        .phrase(&PhraseContext {
            kind: ArtifactKind::Resume,
            profile: &profile,
            jd: &jd,
            plan: &plan,
            store: &store,
        })
        .await
        .unwrap();

    assert_eq!(claims.len(), 1);
    let verifications = StructuralVerifier.verify(&claims, &store).unwrap();
    assert!(verifications.iter().all(|v| v.supported));
}

#[tokio::test]
async fn fabricated_claim_is_rejected_under_strict_grounding() {
    let (profile, jd, store) = setup();
    let plan = curate(
        &profile,
        &jd,
        &store,
        &LexicalScorer::new(),
        &CurationConfig::default(),
    )
    .unwrap();

    let model = FakeLanguageModel::always(
        r#"{"claims":[{"text":"Increased revenue by 900%","fact_ids":["f_999"],
            "section":"experience","role_id":"r_1"}]}"#,
    );
    let claims = LlmPhraser::new(model)
        .phrase(&PhraseContext {
            kind: ArtifactKind::Resume,
            profile: &profile,
            jd: &jd,
            plan: &plan,
            store: &store,
        })
        .await
        .unwrap();

    let verifications = StructuralVerifier.verify(&claims, &store).unwrap();
    let error = apply_policy(claims, &verifications, GroundingPolicy::Strict).unwrap_err();
    assert!(error.to_string().contains("grounding"));
}
