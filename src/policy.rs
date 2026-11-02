use clap::ValueEnum;
use serde::Serialize;

/// The declared side-effect class of an operation. Unknown effects must not be
/// silently downgraded to read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum SideEffect {
    ReadOnly,
    ReversibleWrite,
    Destructive,
    Administrator,
    ExternalCommunication,
    Financial,
    PaidInference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", content = "reason", rename_all = "snake_case")]
pub enum AuthorizationDecision {
    Allowed,
    ApprovalRequired(String),
    Denied(String),
}

/// Evaluate an operation against explicit capability grants and a scoped,
/// unconsumed approval. Paid inference and financial actions are hard-denied
/// until a separate explicit spend-authorization design exists.
pub fn evaluate(
    capability_granted: bool,
    effect: SideEffect,
    approval_valid: bool,
) -> AuthorizationDecision {
    if !capability_granted {
        return AuthorizationDecision::Denied(
            "required capability has not been explicitly granted".to_owned(),
        );
    }

    match effect {
        SideEffect::Financial | SideEffect::PaidInference => AuthorizationDecision::Denied(
            "blocked by the zero-spend policy; explicit spend authorization is not configured"
                .to_owned(),
        ),
        SideEffect::Destructive | SideEffect::Administrator | SideEffect::ExternalCommunication => {
            if approval_valid {
                AuthorizationDecision::Allowed
            } else {
                AuthorizationDecision::ApprovalRequired(
                    "this side effect requires an approved, action-scoped approval".to_owned(),
                )
            }
        }
        SideEffect::ReadOnly | SideEffect::ReversibleWrite => AuthorizationDecision::Allowed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_must_be_explicit() {
        assert!(matches!(
            evaluate(false, SideEffect::ReadOnly, false),
            AuthorizationDecision::Denied(_)
        ));
    }

    #[test]
    fn reversible_write_needs_grant_but_not_interactive_approval() {
        assert_eq!(
            evaluate(true, SideEffect::ReversibleWrite, false),
            AuthorizationDecision::Allowed
        );
    }

    #[test]
    fn destructive_and_external_effects_need_approval() {
        assert!(matches!(
            evaluate(true, SideEffect::Destructive, false),
            AuthorizationDecision::ApprovalRequired(_)
        ));
        assert_eq!(
            evaluate(true, SideEffect::ExternalCommunication, true),
            AuthorizationDecision::Allowed
        );
    }

    #[test]
    fn paid_routes_and_financial_actions_stay_blocked() {
        assert!(matches!(
            evaluate(true, SideEffect::PaidInference, true),
            AuthorizationDecision::Denied(_)
        ));
        assert!(matches!(
            evaluate(true, SideEffect::Financial, true),
            AuthorizationDecision::Denied(_)
        ));
    }
}
