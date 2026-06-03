# Test Fixtures for SevenZaDllBackend

This directory holds RAR archive fixtures used by property-based tests for the
`SevenZaDllBackend` (listing round-trip and extract round-trip).

## Expected Fixtures

Place `.rar` archives here with accompanying `.manifest` files that list the expected
entries (one per line). The manifest file must have the same stem as the `.rar` file.

Example:
- `ascii_names.rar` — archive with ASCII-only filenames
- `ascii_names.manifest` — one entry name per line (non-directory entries only)
- `unicode_names.rar` — archive with Unicode filenames
- `unicode_names.manifest` — corresponding manifest
- `cjk_names.rar` — archive with CJK characters in filenames
- `cjk_names.manifest` — corresponding manifest
- `nested_dirs.rar` — archive with nested directory structure
- `nested_dirs.manifest` — corresponding manifest

## Creating Fixtures

RAR archives cannot be created programmatically without the `rar` CLI (proprietary).
To create fixtures:

1. Install WinRAR or the `rar` command-line tool
2. Create test files with the desired names
3. Run: `rar a fixture_name.rar file1.txt subdir/file2.txt ...`
4. Create a `.manifest` file listing all non-directory entries, one per line

## Test Behavior

- If no `.rar` fixtures are found here, the property tests pass with 0 iterations (skip).
- If `7za.dll` is not available at runtime, the tests skip gracefully.
- When both fixtures and DLL are present, full round-trip assertions are executed.
