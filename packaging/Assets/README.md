# ZipEase Packaging Assets

This directory contains icon and logo assets required for the Sparse MSIX package
and Windows Shell Extension context menu integration.

## Required Assets

### Context Menu Icons

- **extract.ico** — Icon displayed next to "用 ZipEase 解壓縮" / "Extract with ZipEase" in the context menu
  - Required sizes: 16x16, 32x32 pixels
  - Format: Windows .ico (multi-resolution)
  - Design: Should suggest "unpack" or "extract" action (e.g., an arrow coming out of a box)

- **compress.ico** — Icon displayed next to "用 ZipEase 壓縮" / "Compress with ZipEase" in the context menu
  - Required sizes: 16x16, 32x32 pixels
  - Format: Windows .ico (multi-resolution)
  - Design: Should suggest "pack" or "compress" action (e.g., an arrow going into a box)

### MSIX Package Logos (Required by AppxManifest.xml)

- **Square150x150Logo.png** — Medium tile logo
  - Size: 150x150 pixels
  - Format: PNG with transparency
  - Used in: Windows Start menu tile (if applicable)

- **Square44x44Logo.png** — Small app icon
  - Size: 44x44 pixels
  - Format: PNG with transparency
  - Used in: App list, taskbar, package identity

## Notes

- Actual icon design is a separate task — these placeholders allow the build pipeline to function.
- The .ico files are also referenced by the Registry fallback registration (HKCU shell keys).
- Icons should match the ZipEase brand identity and be visually distinct at small sizes.
- Consider providing @2x variants for high-DPI displays.
