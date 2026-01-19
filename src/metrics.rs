//! Pure metric computation functions.

macro_rules! level_enum {
    (
        $name:ident ($val_ty:ty),
        $(($range:pat, $variant:ident, $color:literal)),+ $(,)?
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }

        impl $name {
            #[inline]
            pub fn from_value(val: $val_ty) -> Self {
                match val { $($range => Self::$variant),+ }
            }

            pub fn color(&self) -> &'static str {
                match self { $(Self::$variant => $color),+ }
            }
        }
    };
}

/// Compute Maintainability Index from raw metrics.
/// Returns value in range [0, 100] where higher is better.
///
/// Uses SEI formula: MI = 171 - 5.2*ln(V) - 0.23*CC - 16.2*ln(LOC)
/// Normalized to 0-100 scale.
#[inline]
pub fn maintainability_index(halstead_volume: f64, cyclomatic_complexity: u32, loc: u32) -> u8 {
    if loc == 0 {
        return 100;
    }

    let v = if halstead_volume <= 0.0 {
        1.0
    } else {
        halstead_volume
    };
    let cc = cyclomatic_complexity.max(1) as f64;
    let loc_f = loc as f64;

    let raw_mi = 171.0 - 5.2 * v.ln() - 0.23 * cc - 16.2 * loc_f.ln();
    let normalized = (raw_mi * 100.0 / 171.0).clamp(0.0, 100.0);

    normalized as u8
}

/// Compute MI using per-function averages (fairer for well-factored code).
#[inline]
pub fn maintainability_index_from_averages(
    avg_loc_per_fn: f64,
    avg_cc_per_fn: f64,
    function_count: u32,
) -> u8 {
    if function_count == 0 || avg_loc_per_fn <= 0.0 {
        return 100;
    }

    // Estimate Halstead volume as ~3x LOC (rough approximation)
    let avg_v = avg_loc_per_fn * 3.0;

    let raw_mi = 171.0 - 5.2 * avg_v.ln() - 0.23 * avg_cc_per_fn - 16.2 * avg_loc_per_fn.ln();
    let normalized = (raw_mi * 100.0 / 171.0).clamp(0.0, 100.0);

    normalized as u8
}

/// Estimate Halstead volume from LOC (rough approximation).
/// Real Halstead requires operator/operand counts which need tokenization.
#[inline]
pub fn estimate_halstead_volume(loc: u32) -> f64 {
    if loc == 0 {
        1.0
    } else {
        loc as f64 * 3.0
    }
}

/// Compute base cyclomatic complexity.
#[inline]
pub const fn base_complexity() -> u32 {
    1
}

level_enum!(MiLevel(u8),
    (85..=100, Excellent, "green"),
    (65..=84, Good, "green"),
    (50..=64, Moderate, "yellow"),
    (_, Poor, "red"),
);

level_enum!(CcLevel(u32),
    (0..=5, Simple, "green"),
    (6..=10, Moderate, "yellow"),
    (11..=20, Complex, "red"),
    (_, VeryComplex, "red"),
);

level_enum!(CognitiveLevel(u32),
    (0..=8, Simple, "green"),
    (9..=15, Moderate, "yellow"),
    (_, Complex, "red"),
);

level_enum!(DepthLevel(u32),
    (0..=2, Flat, "green"),
    (3..=4, Moderate, "yellow"),
    (5..=6, Deep, "red"),
    (_, VeryDeep, "red"),
);

/// Count lines in source bytes (fast).
#[inline]
pub fn count_lines(bytes: &[u8]) -> u32 {
    bytes.iter().filter(|&&b| b == b'\n').count() as u32
}

/// Compute stability index (fan-out / (fan-in + fan-out)).
/// Returns 0.0 for isolated modules, higher values indicate instability.
#[inline]
pub fn stability_index(fan_in: u32, fan_out: u32) -> f64 {
    let total = fan_in + fan_out;
    if total == 0 {
        0.0
    } else {
        fan_out as f64 / total as f64
    }
}

/// Compute complexity density (CC / LOC).
#[inline]
pub fn complexity_density(cc: u32, loc: u32) -> f64 {
    if loc == 0 {
        0.0
    } else {
        cc as f64 / loc as f64
    }
}

/// Check if function is considered complex.
#[inline]
pub fn is_complex(cc: u32, loc: u32) -> bool {
    cc > 10 || (loc > 0 && complexity_density(cc, loc) > 0.3)
}

/// Check if function is considered large.
#[inline]
pub fn is_large(loc: u32) -> bool {
    loc > 50
}

/// Check if function is deeply nested.
#[inline]
pub fn is_deeply_nested(depth: u32) -> bool {
    depth > 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mi_calculation() {
        // Empty file gets perfect score
        assert_eq!(maintainability_index(0.0, 0, 0), 100);

        // Small simple file should have reasonable MI (>60)
        let mi = maintainability_index(30.0, 2, 10);
        assert!(mi >= 60, "Expected MI >= 60, got {}", mi);

        // Large complex file should have lower MI
        let mi = maintainability_index(1000.0, 50, 500);
        assert!(mi < 50, "Expected MI < 50, got {}", mi);
    }

    #[test]
    fn mi_from_averages() {
        // Well-factored code (many small functions)
        let mi = maintainability_index_from_averages(15.0, 3.0, 10);
        assert!(mi >= 60, "Expected MI >= 60, got {}", mi);

        // Poorly factored (few large functions)
        let mi = maintainability_index_from_averages(100.0, 15.0, 2);
        assert!(mi < 60, "Expected MI < 60, got {}", mi);
    }

    #[test]
    fn level_classifications() {
        assert_eq!(MiLevel::from_value(90), MiLevel::Excellent);
        assert_eq!(MiLevel::from_value(70), MiLevel::Good);
        assert_eq!(MiLevel::from_value(55), MiLevel::Moderate);
        assert_eq!(MiLevel::from_value(30), MiLevel::Poor);

        assert_eq!(CcLevel::from_value(3), CcLevel::Simple);
        assert_eq!(CcLevel::from_value(8), CcLevel::Moderate);
        assert_eq!(CcLevel::from_value(15), CcLevel::Complex);
        assert_eq!(CcLevel::from_value(25), CcLevel::VeryComplex);
    }

    #[test]
    fn line_counting() {
        assert_eq!(count_lines(b"hello\nworld\n"), 2);
        assert_eq!(count_lines(b"no newlines"), 0);
        assert_eq!(count_lines(b""), 0);
    }

    #[test]
    fn stability_calculation() {
        // More imports = less stable
        assert!(stability_index(1, 10) > 0.5);
        // More exports = more stable
        assert!(stability_index(10, 1) < 0.5);
        // Isolated
        assert_eq!(stability_index(0, 0), 0.0);
    }
}
