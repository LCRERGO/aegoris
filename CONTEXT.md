# aegoris

aegoris turns a **Profile** and a **Job Description** into a curated **Resume** and **Cover Letter**, where every generated claim traces back to a source **Fact**.

## Language

### Source material

**Profile**:
The candidate's source data, normalized from JSON, plain text, a LinkedIn export, or a PDF. A Profile is raw material, never the finished document.
_Avoid_: CV, resume, account

**Job Description (JD)**:
The target posting, kept verbatim alongside the **Requirements** extracted from it.
_Avoid_: job post, listing, role

**Requirement**:
An atomic need extracted from a JD.
_Avoid_: keyword, ask, criterion

**Role**:
One job entry. The resume section that contains Roles is **Experience**.
_Avoid_: position, job, experience

**Fact**:
An atomic, profile-sourced, groundable statement. Facts are the only legitimate content for generated text.
_Avoid_: bullet, data point

### Generated material

**Claim**:
Generated text that must cite at least one Fact. A Claim cannot exist without **Evidence**.
_Avoid_: bullet, line, sentence

**Evidence**:
The typed link from a Claim to the Facts it derives from.
_Avoid_: citation, source

**Bullet**:
A rendered Claim inside a Role.
_Avoid_: line item

**Resume**:
The generated resume document. Distinct from the Profile it was derived from.
_Avoid_: CV

**Cover Letter**:
The generated cover letter document.
_Avoid_: application letter

**Artifact**:
A rendered Resume or Cover Letter in a specific output format.
_Avoid_: document, output, file

### Process

**Curation**:
Selecting and ordering Facts for a target JD. Deterministic and independent of any language model.
_Avoid_: tailoring, filtering, ranking

**Curation Plan**:
The selected Facts, their relevance scores, and their ordering.
_Avoid_: selection

**Relevance Score**:
A Fact's computed fit against the JD's Requirements.
_Avoid_: match, weight

**Grounding**:
The invariant that every Claim traces to Facts, enforced structurally and verified semantically.
_Avoid_: verification, hallucination check

**Mode**:
Whether phrasing is done by a language model (`llm`) or by deterministic templates (`template`).
_Avoid_: backend, engine
