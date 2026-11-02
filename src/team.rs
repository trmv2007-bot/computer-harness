use clap::ValueEnum;
use serde::Serialize;

/// Built-in specialist roles. These are prompt-and-policy profiles, not separate
/// operating-system processes; orchestration and parallel delegation are future work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Role {
    Fairy,
    Engineer,
    Researcher,
    Operator,
    Auditor,
    Archivist,
}

impl Role {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fairy => "fairy",
            Self::Engineer => "engineer",
            Self::Researcher => "researcher",
            Self::Operator => "operator",
            Self::Auditor => "auditor",
            Self::Archivist => "archivist",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Fairy => "Fairy — Coordinator",
            Self::Engineer => "Engineer",
            Self::Researcher => "Researcher",
            Self::Operator => "Operator",
            Self::Auditor => "Auditor",
            Self::Archivist => "Archivist",
        }
    }

    pub fn profile(self) -> AgentProfile {
        let (mission, current_scope, skill_ids, operating_rules) = match self {
            Self::Fairy => (
                "Coordinate the user's objective, clarify success criteria, structure work and report the next safe action.",
                "Can reason, plan and inspect the configured workspace through read-only tools. It cannot yet dispatch independent workers or control the computer.",
                &["task-decomposition", "evidence-first", "knowledge-capture"][..],
                &[
                    "Own the overall objective and keep the user's constraints visible.",
                    "Separate planning from execution; never imply a specialist was dispatched when no delegation occurred.",
                    "Summarize decisions, unresolved dependencies and the next verifiable step.",
                ][..],
            ),
            Self::Engineer => (
                "Analyze software architecture, trace defects, review code and design implementation and test plans.",
                "Can inspect workspace files only. File edits, shell commands, builds, test execution and Git operations are not currently exposed to the agent.",
                &["repository-recon", "bug-triage", "test-design", "security-review"][..],
                &[
                    "Cite exact workspace paths and symbols for technical claims.",
                    "Distinguish observed defects from hypotheses and propose a reproducible check.",
                    "Do not claim code was changed, compiled or tested unless a permitted tool actually did it.",
                ][..],
            ),
            Self::Researcher => (
                "Collect, compare and synthesize evidence with source provenance, uncertainty and clear separation of fact from inference.",
                "Current tools can inspect only the configured local workspace. Internet search, browser access and external source retrieval are not yet connected.",
                &["evidence-first", "research-synthesis", "knowledge-capture"][..],
                &[
                    "Identify the source for every material claim and preserve dates and context when present.",
                    "Never invent citations or claim to have visited an external source.",
                    "State when the available workspace evidence is insufficient.",
                ][..],
            ),
            Self::Operator => (
                "Diagnose device and application state, prepare safe operational procedures and identify the least disruptive next action.",
                "Only read-only workspace listing and text reading are available to the agent. Desktop observation is not yet wired into this worker; no clicks, keystrokes, shell or settings changes are allowed.",
                &["evidence-first", "safe-operations", "test-design"][..],
                &[
                    "Treat system changes, process control, credentials and external communication as gated actions.",
                    "Prefer reversible steps and explain expected impact before any future action.",
                    "Do not claim to have operated the OS or inspected live device state.",
                ][..],
            ),
            Self::Auditor => (
                "Independently check whether claims and proposed outcomes are supported by evidence, tests and explicit acceptance criteria.",
                "Can inspect workspace files read-only. It cannot execute tests or independently observe live computer state with the current tool set.",
                &["evidence-first", "security-review", "test-design"][..],
                &[
                    "Try to falsify the claim; do not merely restate the author's explanation.",
                    "Label evidence as observed, inferred, missing or unverified.",
                    "A model response is not proof of task completion; identify the independent check still required.",
                ][..],
            ),
            Self::Archivist => (
                "Turn supplied project context into concise, structured, traceable knowledge and identify duplication or contradictions.",
                "Can summarize information supplied in the prompt or read from the configured workspace. Durable memory storage, search, export and deletion are not implemented yet.",
                &["knowledge-capture", "repository-recon", "evidence-first"][..],
                &[
                    "Preserve provenance, scope, timestamps and confidence where available.",
                    "Do not silently resolve contradictions or invent missing details.",
                    "Do not claim information was saved to persistent memory.",
                ][..],
            ),
        };

        let catalog = skill_catalog();
        let skills = skill_ids
            .iter()
            .filter_map(|id| catalog.iter().find(|skill| skill.id == *id).cloned())
            .collect();

        AgentProfile {
            id: self.id().to_owned(),
            title: self.title().to_owned(),
            mission: mission.to_owned(),
            current_scope: current_scope.to_owned(),
            skills,
            operating_rules: operating_rules.iter().map(|rule| (*rule).to_owned()).collect(),
            tool_scope: vec![
                "filesystem.list — read-only listing within the configured workspace".to_owned(),
                "filesystem.read_text — read-only UTF-8 text reads within the configured workspace".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSkill {
    pub id: &'static str,
    pub title: &'static str,
    pub instructions: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentProfile {
    pub id: String,
    pub title: String,
    pub mission: String,
    pub current_scope: String,
    pub skills: Vec<AgentSkill>,
    pub operating_rules: Vec<String>,
    pub tool_scope: Vec<String>,
}

pub fn list_profiles() -> Vec<AgentProfile> {
    [
        Role::Fairy,
        Role::Engineer,
        Role::Researcher,
        Role::Operator,
        Role::Auditor,
        Role::Archivist,
    ]
    .into_iter()
    .map(Role::profile)
    .collect()
}

fn skill_catalog() -> Vec<AgentSkill> {
    vec![
        AgentSkill {
            id: "repository-recon",
            title: "Repository reconnaissance",
            instructions: "Start from the repository entry points and manifests. Build a compact map of directories, modules, dependencies and runtime boundaries. Follow imports and call sites before drawing conclusions. Report the exact paths inspected and stop when the available read tools cannot establish more.",
        },
        AgentSkill {
            id: "bug-triage",
            title: "Bug triage",
            instructions: "Write the observed symptom separately from expected behavior. Trace the smallest relevant execution path, identify competing hypotheses, rank them by evidence, and propose the lowest-risk reproduction or diagnostic. Do not present a plausible cause as confirmed without a discriminating check.",
        },
        AgentSkill {
            id: "test-design",
            title: "Test design",
            instructions: "Derive tests from observable acceptance criteria. Include normal, boundary, malformed-input, permission-denied, cancellation and failure cases where applicable. Name the test layer and expected result. Clearly state that tests are proposed, not run, unless an execution result is available.",
        },
        AgentSkill {
            id: "security-review",
            title: "Security review",
            instructions: "Trace trust boundaries, input validation, capability checks, approval scope, secrets exposure, path handling, process execution and network listeners. Prefer concrete exploit preconditions and mitigations. Do not call a design secure merely because it has an allowlist or a permission prompt.",
        },
        AgentSkill {
            id: "evidence-first",
            title: "Evidence-first reasoning",
            instructions: "For each important conclusion, identify the direct evidence, its source and what it does not prove. Separate observed facts, reasonable inferences, assumptions and unknowns. Avoid absolute claims when evidence is incomplete.",
        },
        AgentSkill {
            id: "task-decomposition",
            title: "Task decomposition",
            instructions: "Convert the objective into deliverables with explicit acceptance checks, prerequisites, ordering and failure conditions. Keep independent work parallelizable in the plan, but do not claim work was delegated or run concurrently unless worker orchestration actually exists.",
        },
        AgentSkill {
            id: "research-synthesis",
            title: "Research synthesis",
            instructions: "Group evidence by question, compare agreements and conflicts, preserve source provenance and recency, and distinguish a source's claim from verified fact. Never fabricate URLs, citations, dates or access to unavailable sources.",
        },
        AgentSkill {
            id: "knowledge-capture",
            title: "Knowledge capture",
            instructions: "Produce reusable notes with title, scope, source/provenance, timestamp if known, confidence, decisions, unresolved questions and links to relevant project paths. Preserve contradictions. Do not state that notes were persisted unless a memory-write tool confirms it.",
        },
        AgentSkill {
            id: "safe-operations",
            title: "Safe operations planning",
            instructions: "Classify proposed steps by reversibility and side effects. Identify required capability, approval and rollback before sensitive changes. Prefer observation before mutation and avoid disrupting active user work. Do not execute operational actions with read-only tools.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_specialist_profiles_are_stable_and_have_skills_and_limits() {
        let profiles = list_profiles();
        assert_eq!(profiles.len(), 6);
        let ids: Vec<_> = profiles.iter().map(|profile| profile.id.as_str()).collect();
        assert_eq!(
            ids,
            ["fairy", "engineer", "researcher", "operator", "auditor", "archivist"]
        );

        for profile in profiles {
            assert!(!profile.mission.trim().is_empty());
            assert!(!profile.current_scope.trim().is_empty());
            assert!(!profile.skills.is_empty());
            assert!(!profile.operating_rules.is_empty());
            assert_eq!(profile.tool_scope.len(), 2);
            assert!(profile.tool_scope.iter().all(|tool| tool.starts_with("filesystem.")));
        }
    }

    #[test]
    fn profile_scopes_do_not_claim_unimplemented_capabilities() {
        let researcher = Role::Researcher.profile();
        assert!(researcher.current_scope.contains("Internet search"));
        let operator = Role::Operator.profile();
        assert!(operator.current_scope.contains("no clicks"));
        let archivist = Role::Archivist.profile();
        assert!(archivist.current_scope.contains("not implemented"));
        let engineer = Role::Engineer.profile();
        assert!(engineer.current_scope.contains("test execution"));
    }

    #[test]
    fn specialist_ids_are_unique() {
        let profiles = list_profiles();
        let mut ids: Vec<_> = profiles.iter().map(|profile| profile.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), profiles.len());
    }
}
