//! Recursion prevention: which analyzer categories a child may not run.

use std::collections::BTreeSet;

use crate::params::ChildDetonation;

/// The category that stands for detonations.
pub(crate) const DYNAMIC: &str = "dynamic";

#[derive(Debug, Clone, Default)]
pub(crate) struct ParentContext {
    /// Categories forbidden for the parent itself.
    pub(crate) forbidden: BTreeSet<String>,
    /// `recursion_prevention` of every analyzer with a job on the parent,
    /// plus `"dynamic"` when the parent was detonated in `extracted` mode.
    pub(crate) prevention_from_jobs: BTreeSet<String>,
}

/// Switches that relax recursion prevention for one child.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RecursionOverrides {
    /// The submission disabled recursion prevention entirely.
    pub(crate) ignore_prevention: bool,
    /// The child was explicitly requested for detonation despite inheritance.
    pub(crate) dynamic_bypass: bool,
}

pub(crate) fn forbidden_for_child(
    parents: &[ParentContext],
    mode: ChildDetonation,
    overrides: RecursionOverrides,
) -> BTreeSet<String> {
    let RecursionOverrides {
        ignore_prevention,
        dynamic_bypass: bypass,
    } = overrides;
    let mut out: BTreeSet<String> = if ignore_prevention {
        BTreeSet::new()
    } else {
        parents
            .iter()
            .flat_map(|p| p.forbidden.iter().chain(&p.prevention_from_jobs))
            .cloned()
            .collect()
    };
    match mode {
        ChildDetonation::None => {
            out.insert(DYNAMIC.to_string());
        }
        ChildDetonation::All => {
            out.remove(DYNAMIC);
        }
        ChildDetonation::Extracted => {
            if bypass {
                out.remove(DYNAMIC);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(ToString::to_string).collect()
    }

    fn detonated_parent() -> ParentContext {
        ParentContext {
            forbidden: set(&["extract"]),
            prevention_from_jobs: set(&[DYNAMIC]),
        }
    }

    const NONE: RecursionOverrides = RecursionOverrides {
        ignore_prevention: false,
        dynamic_bypass: false,
    };
    const BYPASS: RecursionOverrides = RecursionOverrides {
        ignore_prevention: false,
        dynamic_bypass: true,
    };
    const IGNORE: RecursionOverrides = RecursionOverrides {
        ignore_prevention: true,
        dynamic_bypass: false,
    };

    #[test]
    fn extracted_inherits_parent_prevention() {
        let f = forbidden_for_child(&[detonated_parent()], ChildDetonation::Extracted, NONE);
        assert_eq!(f, set(&["dynamic", "extract"]));
    }

    #[test]
    fn bypass_allows_dynamic() {
        let f = forbidden_for_child(&[detonated_parent()], ChildDetonation::Extracted, BYPASS);
        assert_eq!(f, set(&["extract"]));
    }

    #[test]
    fn none_mode_always_forbids_dynamic() {
        let f = forbidden_for_child(&[], ChildDetonation::None, BYPASS);
        assert_eq!(f, set(&["dynamic"]));
        let f = forbidden_for_child(&[], ChildDetonation::None, IGNORE);
        assert_eq!(f, set(&["dynamic"]));
    }

    #[test]
    fn all_mode_never_forbids_dynamic() {
        let f = forbidden_for_child(&[detonated_parent()], ChildDetonation::All, NONE);
        assert_eq!(f, set(&["extract"]));
    }

    #[test]
    fn ignore_recursion_prevention_clears() {
        let f = forbidden_for_child(&[detonated_parent()], ChildDetonation::Extracted, IGNORE);
        assert!(f.is_empty());
    }

    #[test]
    fn unions_over_parents() {
        let a = ParentContext {
            forbidden: set(&["a"]),
            prevention_from_jobs: BTreeSet::new(),
        };
        let b = ParentContext {
            forbidden: BTreeSet::new(),
            prevention_from_jobs: set(&["b"]),
        };
        let f = forbidden_for_child(&[a, b], ChildDetonation::Extracted, NONE);
        assert_eq!(f, set(&["a", "b"]));
    }
}
