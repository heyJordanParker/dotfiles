use globset::{Glob, GlobMatcher};

pub fn matcher(pattern: &str) -> GlobMatcher {
    Glob::new(pattern)
        .or_else(|_| Glob::new(&globset::escape(pattern)))
        .expect("an escaped glob always compiles")
        .compile_matcher()
}
