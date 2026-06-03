use std::cmp::Ordering;

/// A segment of a string, either a run of digits or a run of non-digits.
#[derive(Debug)]
enum Segment<'a> {
    /// A numeric segment (one or more ASCII digits).
    Numeric(&'a str),
    /// A non-numeric (alphabetic/other) segment.
    Text(&'a str),
}

/// Natural sort comparator: numeric segments are compared by value,
/// alphabetic segments are compared case-insensitively.
///
/// This ensures "img2" < "img10" (numeric comparison) and
/// "Photo" == "photo" (case-insensitive text comparison).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a_segs = SegmentIter::new(a);
    let mut b_segs = SegmentIter::new(b);

    loop {
        match (a_segs.next(), b_segs.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(seg_a), Some(seg_b)) => {
                let ord = compare_segments(&seg_a, &seg_b);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

/// Compare two segments. When both are numeric, compare by numeric value
/// (with tie-breaking by length for leading zeros). When both are text,
/// compare case-insensitively. When types differ, numeric sorts before text.
fn compare_segments(a: &Segment, b: &Segment) -> Ordering {
    match (a, b) {
        (Segment::Numeric(na), Segment::Numeric(nb)) => compare_numeric(na, nb),
        (Segment::Text(ta), Segment::Text(tb)) => compare_text_case_insensitive(ta, tb),
        // Numeric segments sort before text segments
        (Segment::Numeric(_), Segment::Text(_)) => Ordering::Less,
        (Segment::Text(_), Segment::Numeric(_)) => Ordering::Greater,
    }
}

/// Compare two numeric strings by their integer value.
/// If values are equal, shorter string (fewer leading zeros) comes first.
fn compare_numeric(a: &str, b: &str) -> Ordering {
    let a_trimmed = a.trim_start_matches('0');
    let b_trimmed = b.trim_start_matches('0');

    // Compare by effective numeric value (length of trimmed, then lexicographic)
    let len_ord = a_trimmed.len().cmp(&b_trimmed.len());
    if len_ord != Ordering::Equal {
        return len_ord;
    }

    // Same length of significant digits — compare lexicographically (works for same-length digit strings)
    let lex_ord = a_trimmed.cmp(b_trimmed);
    if lex_ord != Ordering::Equal {
        return lex_ord;
    }

    // Numeric values are equal — tie-break: fewer leading zeros (shorter original) comes first
    a.len().cmp(&b.len())
}

/// Compare two text segments case-insensitively.
/// Falls back to case-sensitive comparison as tie-breaker for total order.
fn compare_text_case_insensitive(a: &str, b: &str) -> Ordering {
    // Compare char-by-char using lowercase conversion
    let mut a_chars = a.chars();
    let mut b_chars = b.chars();

    loop {
        match (a_chars.next(), b_chars.next()) {
            (None, None) => break,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                // Compare lowercased characters
                let mut a_lower = ca.to_lowercase();
                let mut b_lower = cb.to_lowercase();

                loop {
                    match (a_lower.next(), b_lower.next()) {
                        (None, None) => break,
                        (None, Some(_)) => return Ordering::Less,
                        (Some(_), None) => return Ordering::Greater,
                        (Some(la), Some(lb)) => {
                            let ord = la.cmp(&lb);
                            if ord != Ordering::Equal {
                                return ord;
                            }
                        }
                    }
                }
            }
        }
    }

    // Case-insensitive comparison was equal — use case-sensitive as tie-breaker
    // This ensures total order (antisymmetry): "A" and "a" get a deterministic order
    a.cmp(b)
}

/// Iterator that splits a string into alternating numeric and text segments.
struct SegmentIter<'a> {
    remaining: &'a str,
}

impl<'a> SegmentIter<'a> {
    fn new(s: &'a str) -> Self {
        Self { remaining: s }
    }
}

impl<'a> Iterator for SegmentIter<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        let first_char = self.remaining.chars().next().unwrap();
        let is_digit = first_char.is_ascii_digit();

        // Find the end of this segment (run of same type)
        let end = self
            .remaining
            .char_indices()
            .skip(1)
            .find(|(_, c)| c.is_ascii_digit() != is_digit)
            .map(|(i, _)| i)
            .unwrap_or(self.remaining.len());

        let segment_str = &self.remaining[..end];
        self.remaining = &self.remaining[end..];

        if is_digit {
            Some(Segment::Numeric(segment_str))
        } else {
            Some(Segment::Text(segment_str))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_segments_compared_by_value() {
        // "img2" < "img10" — the key natural sort property
        assert_eq!(natural_cmp("img2", "img10"), Ordering::Less);
        assert_eq!(natural_cmp("img10", "img2"), Ordering::Greater);
        assert_eq!(natural_cmp("img10", "img10"), Ordering::Equal);
    }

    #[test]
    fn alphabetic_segments_case_insensitive() {
        // Case-insensitive: "Photo" and "photo" compare equal in primary ordering
        assert_eq!(natural_cmp("abc", "ABC"), Ordering::Greater); // tie-breaker: lowercase > uppercase in ASCII
        assert_eq!(natural_cmp("ABC", "abc"), Ordering::Less);
        // Same case
        assert_eq!(natural_cmp("abc", "abc"), Ordering::Equal);
    }

    #[test]
    fn mixed_segments() {
        // "photo1.jpg" < "photo2.jpg" < "photo10.jpg"
        assert_eq!(natural_cmp("photo1.jpg", "photo2.jpg"), Ordering::Less);
        assert_eq!(natural_cmp("photo2.jpg", "photo10.jpg"), Ordering::Less);
        assert_eq!(natural_cmp("photo1.jpg", "photo10.jpg"), Ordering::Less);
    }

    #[test]
    fn leading_zeros() {
        // "file01" < "file1" (same numeric value, but shorter = fewer leading zeros comes first... 
        // actually "01" has more chars so it sorts after "1")
        assert_eq!(natural_cmp("file01", "file1"), Ordering::Greater);
        assert_eq!(natural_cmp("file001", "file01"), Ordering::Greater);
    }

    #[test]
    fn empty_strings() {
        assert_eq!(natural_cmp("", ""), Ordering::Equal);
        assert_eq!(natural_cmp("", "a"), Ordering::Less);
        assert_eq!(natural_cmp("a", ""), Ordering::Greater);
    }

    #[test]
    fn purely_numeric() {
        assert_eq!(natural_cmp("1", "2"), Ordering::Less);
        assert_eq!(natural_cmp("9", "10"), Ordering::Less);
        assert_eq!(natural_cmp("100", "99"), Ordering::Greater);
    }

    #[test]
    fn real_world_filenames() {
        let mut files = vec![
            "img10.png",
            "img1.png",
            "img2.png",
            "img20.png",
            "img3.png",
        ];
        files.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            files,
            vec!["img1.png", "img2.png", "img3.png", "img10.png", "img20.png"]
        );
    }

    #[test]
    fn case_insensitive_ordering() {
        let mut files = vec!["Banana.png", "apple.png", "Cherry.png"];
        files.sort_by(|a, b| natural_cmp(a, b));
        // Case-insensitive: apple < Banana < Cherry
        assert_eq!(files, vec!["apple.png", "Banana.png", "Cherry.png"]);
    }

    #[test]
    fn reflexive() {
        let cases = ["", "a", "123", "img10.png", "Photo_001.jpg"];
        for s in &cases {
            assert_eq!(natural_cmp(s, s), Ordering::Equal);
        }
    }

    #[test]
    fn antisymmetric() {
        let pairs = [
            ("a", "b"),
            ("img2", "img10"),
            ("ABC", "abc"),
            ("1", "2"),
        ];
        for (a, b) in &pairs {
            let ab = natural_cmp(a, b);
            let ba = natural_cmp(b, a);
            assert_eq!(ab, ba.reverse(), "antisymmetry failed for ({}, {})", a, b);
        }
    }
}
