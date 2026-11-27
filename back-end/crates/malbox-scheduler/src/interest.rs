//! Ordering of detonation candidates when the budget is short.

use std::cmp::Reverse;

/// Higher is more interesting to detonate.
pub fn interest_class(artifact_type: &str) -> u8 {
    const CLASSES: [(&[&str], u8); 3] = [
        (&["executable/", "installer/"], 3),
        (&["code/", "shortcut/", "java/", "android/"], 2),
        (&["document/"], 1),
    ];
    CLASSES
        .iter()
        .find(|(prefixes, _)| prefixes.iter().any(|p| artifact_type.starts_with(p)))
        .map_or(0, |(_, class)| *class)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub artifact_id: i64,
    pub artifact_type: String,
    pub max_score: Option<i16>,
    pub depth: i32,
}

/// Class desc, then max score desc (`None` last), then depth asc, then id asc.
pub fn order_candidates(c: &mut [Candidate]) {
    c.sort_by_key(|x| {
        (
            Reverse(interest_class(&x.artifact_type)),
            Reverse(x.max_score.map_or(i32::MIN, i32::from)),
            x.depth,
            x.artifact_id,
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: i64, t: &str, score: Option<i16>, depth: i32) -> Candidate {
        Candidate {
            artifact_id: id,
            artifact_type: t.into(),
            max_score: score,
            depth,
        }
    }

    fn ids(c: &[Candidate]) -> Vec<i64> {
        c.iter().map(|c| c.artifact_id).collect()
    }

    #[test]
    fn classes() {
        assert_eq!(interest_class("executable/windows/pe32"), 3);
        assert_eq!(interest_class("installer/windows/msi"), 3);
        assert_eq!(interest_class("code/shell"), 2);
        assert_eq!(interest_class("android/apk"), 2);
        assert_eq!(interest_class("document/pdf"), 1);
        assert_eq!(interest_class("text/plain"), 0);
    }

    #[test]
    fn class_first() {
        let mut c = vec![
            cand(1, "document/pdf", Some(100), 1),
            cand(2, "executable/windows/pe32", None, 3),
        ];
        order_candidates(&mut c);
        assert_eq!(ids(&c), vec![2, 1]);
    }

    #[test]
    fn score_second_none_last() {
        let mut c = vec![
            cand(1, "code/shell", None, 1),
            cand(2, "code/shell", Some(10), 1),
            cand(3, "code/shell", Some(90), 1),
        ];
        order_candidates(&mut c);
        assert_eq!(ids(&c), vec![3, 2, 1]);
    }

    #[test]
    fn depth_third() {
        let mut c = vec![
            cand(1, "code/shell", None, 3),
            cand(2, "code/shell", None, 1),
        ];
        order_candidates(&mut c);
        assert_eq!(ids(&c), vec![2, 1]);
    }

    #[test]
    fn id_last() {
        let mut c = vec![
            cand(9, "code/shell", None, 1),
            cand(4, "code/shell", None, 1),
        ];
        order_candidates(&mut c);
        assert_eq!(ids(&c), vec![4, 9]);
    }
}
