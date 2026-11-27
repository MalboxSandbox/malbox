//! Whether an artifact a plugin asked to analyse is admitted.

#[derive(Debug, Clone, Copy)]
pub struct AdmissionInput {
    pub size_bytes: i64,
    pub parent_depth: i32,
    pub analysed_count: i32,
    pub max_depth: i32,
    pub max_artifacts: i32,
    pub max_file_size: u64,
    pub ignore_size: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Analyse {
        depth: i32,
    },
    /// A drop reason from the shared literals.
    Drop(&'static str),
}

pub fn admit(a: &AdmissionInput) -> Admission {
    let size = u64::try_from(a.size_bytes).unwrap_or(0);
    if !a.ignore_size && size > a.max_file_size {
        return Admission::Drop("too_large");
    }
    if a.parent_depth + 1 >= a.max_depth {
        return Admission::Drop("max_depth");
    }
    if a.analysed_count >= a.max_artifacts {
        return Admission::Drop("max_artifacts");
    }
    Admission::Analyse {
        depth: a.parent_depth + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> AdmissionInput {
        AdmissionInput {
            size_bytes: 10,
            parent_depth: 0,
            analysed_count: 1,
            max_depth: 6,
            max_artifacts: 500,
            max_file_size: 100,
            ignore_size: false,
        }
    }

    #[test]
    fn admits_within_limits() {
        assert_eq!(admit(&input()), Admission::Analyse { depth: 1 });
    }

    #[test]
    fn too_large_dropped() {
        let a = AdmissionInput {
            size_bytes: 101,
            ..input()
        };
        assert_eq!(admit(&a), Admission::Drop("too_large"));
    }

    #[test]
    fn ignore_size_admits_large() {
        let a = AdmissionInput {
            size_bytes: 101,
            ignore_size: true,
            ..input()
        };
        assert_eq!(admit(&a), Admission::Analyse { depth: 1 });
    }

    #[test]
    fn max_depth_dropped() {
        let a = AdmissionInput {
            parent_depth: 5,
            ..input()
        };
        assert_eq!(admit(&a), Admission::Drop("max_depth"));
    }

    #[test]
    fn depth_boundary() {
        // A child landing exactly on max_depth is refused; one below is admitted.
        let at_limit = AdmissionInput {
            parent_depth: 1,
            max_depth: 2,
            ..input()
        };
        assert_eq!(admit(&at_limit), Admission::Drop("max_depth"));
        let below = AdmissionInput {
            parent_depth: 0,
            max_depth: 2,
            ..input()
        };
        assert_eq!(admit(&below), Admission::Analyse { depth: 1 });
    }

    #[test]
    fn max_artifacts_dropped() {
        let a = AdmissionInput {
            analysed_count: 500,
            ..input()
        };
        assert_eq!(admit(&a), Admission::Drop("max_artifacts"));
    }
}
