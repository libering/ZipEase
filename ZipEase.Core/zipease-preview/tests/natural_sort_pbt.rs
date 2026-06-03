// Feature: image-preview-plugin, Property 2: Natural sort ordering is a total order
//
// **Validates: Requirements 3.2, 3.3**
//
// For any three filenames a, b, c, the `natural_cmp` function satisfies:
// - Transitivity: if a ≤ b and b ≤ c then a ≤ c
// - Antisymmetry: if a ≤ b and b ≤ a then a = b (ordering equality)
// - Totality: either a ≤ b or b ≤ a
// Additionally, numeric segments are compared by numeric value and
// alphabetic segments are compared case-insensitively.

use proptest::prelude::*;
use std::cmp::Ordering;
use zipease_preview::natural_sort::natural_cmp;

/// Strategy that generates realistic filenames mixing text and numeric segments.
/// This covers the interesting input space for natural sort: letters, digits,
/// separators, extensions, and mixed-case.
fn filename_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Arbitrary short strings (covers edge cases like empty, pure digits, etc.)
        "[a-zA-Z0-9_.\\- ]{0,30}",
        // Realistic filenames with numeric parts: e.g. "img123.png"
        ("[a-zA-Z]{1,8}[0-9]{1,6}\\.[a-z]{2,4}"),
        // Pure numeric strings
        "[0-9]{1,10}",
        // Mixed case text
        "[a-zA-Z]{1,15}",
        // Filenames with leading zeros: e.g. "file007"
        ("[a-z]{1,5}0{0,4}[1-9][0-9]{0,4}"),
    ]
}

proptest! {
    /// Reflexivity: natural_cmp(a, a) == Equal for any string.
    #[test]
    fn prop_reflexivity(a in filename_strategy()) {
        prop_assert_eq!(natural_cmp(&a, &a), Ordering::Equal,
            "reflexivity violated for {:?}", a);
    }

    /// Antisymmetry: if natural_cmp(a, b) == Equal and natural_cmp(b, a) == Equal,
    /// then the ordering treats them as equivalent.
    /// Stronger form: natural_cmp(a, b) == reverse of natural_cmp(b, a).
    #[test]
    fn prop_antisymmetry(a in filename_strategy(), b in filename_strategy()) {
        let ab = natural_cmp(&a, &b);
        let ba = natural_cmp(&b, &a);
        prop_assert_eq!(ab, ba.reverse(),
            "antisymmetry violated: cmp({:?}, {:?}) = {:?}, cmp({:?}, {:?}) = {:?}",
            a, b, ab, b, a, ba);
    }

    /// Totality: for any a, b, natural_cmp returns one of Less, Equal, or Greater.
    /// (This is trivially satisfied by Ordering type, but we verify no panics occur.)
    #[test]
    fn prop_totality(a in filename_strategy(), b in filename_strategy()) {
        let result = natural_cmp(&a, &b);
        prop_assert!(
            result == Ordering::Less || result == Ordering::Equal || result == Ordering::Greater,
            "totality violated for ({:?}, {:?}): got {:?}", a, b, result
        );
    }

    /// Transitivity: if a ≤ b and b ≤ c then a ≤ c.
    #[test]
    fn prop_transitivity(
        a in filename_strategy(),
        b in filename_strategy(),
        c in filename_strategy()
    ) {
        let ab = natural_cmp(&a, &b);
        let bc = natural_cmp(&b, &c);
        let ac = natural_cmp(&a, &c);

        // If a <= b and b <= c, then a <= c
        if (ab == Ordering::Less || ab == Ordering::Equal)
            && (bc == Ordering::Less || bc == Ordering::Equal)
        {
            prop_assert!(
                ac == Ordering::Less || ac == Ordering::Equal,
                "transitivity violated: cmp({:?}, {:?})={:?}, cmp({:?}, {:?})={:?}, but cmp({:?}, {:?})={:?}",
                a, b, ab, b, c, bc, a, c, ac
            );
        }

        // If a >= b and b >= c, then a >= c
        if (ab == Ordering::Greater || ab == Ordering::Equal)
            && (bc == Ordering::Greater || bc == Ordering::Equal)
        {
            prop_assert!(
                ac == Ordering::Greater || ac == Ordering::Equal,
                "transitivity (>=) violated: cmp({:?}, {:?})={:?}, cmp({:?}, {:?})={:?}, but cmp({:?}, {:?})={:?}",
                a, b, ab, b, c, bc, a, c, ac
            );
        }
    }

    /// Numeric segments are compared by numeric value, not lexicographically.
    /// For any prefix and two numbers n1 < n2, "prefix{n1}" < "prefix{n2}".
    #[test]
    fn prop_numeric_value_comparison(
        prefix in "[a-zA-Z]{1,5}",
        n1 in 0u64..100_000,
        n2 in 0u64..100_000
    ) {
        // Only test when n1 != n2 to avoid leading-zero tie-breaking complexity
        prop_assume!(n1 != n2);

        let s1 = format!("{}{}", prefix, n1);
        let s2 = format!("{}{}", prefix, n2);

        let result = natural_cmp(&s1, &s2);

        if n1 < n2 {
            prop_assert_eq!(result, Ordering::Less,
                "numeric comparison failed: {:?} should be < {:?} (n1={}, n2={})",
                s1, s2, n1, n2);
        } else {
            prop_assert_eq!(result, Ordering::Greater,
                "numeric comparison failed: {:?} should be > {:?} (n1={}, n2={})",
                s1, s2, n1, n2);
        }
    }

    /// Case-insensitive text comparison: changing case of alphabetic segments
    /// should not change the relative ordering with respect to a different string
    /// (i.e., the primary comparison is case-insensitive).
    #[test]
    fn prop_case_insensitive_text(
        a in "[a-z]{1,10}",
        b in "[a-z]{1,10}"
    ) {
        prop_assume!(a != b); // different strings only

        let a_upper = a.to_uppercase();
        let result_lower = natural_cmp(&a, &b);
        let result_upper = natural_cmp(&a_upper, &b);

        // Primary ordering (case-insensitive) should be the same
        // Both should agree on Less/Greater (Equal case is excluded since a != b
        // and case-insensitive comparison of different lowercase strings won't be equal)
        prop_assert_eq!(result_lower, result_upper,
            "case sensitivity affected ordering: cmp({:?}, {:?})={:?} vs cmp({:?}, {:?})={:?}",
            a, b, result_lower, a_upper, b, result_upper);
    }
}
