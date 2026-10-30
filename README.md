# Aidoku Arabic Sources

Arabic manga sources for [Aidoku](https://github.com/Aidoku/Aidoku), ported from the matching [Keiyoushi](https://github.com/keiyoushi/extensions-source) extensions for Mihon.

## Usage

In Aidoku, open Settings → Source Lists and add:

```
https://naifqarni.github.io/aidoku-arabic-sources/index.min.json
```

## Sources

| Source | Site | Based on |
| --- | --- | --- |
| Azora | azorafly.com | Iken template |
| Dilar | dilar.tube | custom (pages are read through a background web view) |
| Lava Scans | lavascans.com | MangaThemesia template |
| Manga Starz | starzmanga.com | Madara template |
| MangaSwat | meshmanga.com | custom |
| مانجا ليك (Mangalek) | mangalik.net | Madara template |
| Team X | olympustaff.com | custom |

Paid/locked chapters are hidden. Sites behind Cloudflare will ask you to pass a check in Aidoku's web view the first time.

## Development

The `templates/` directory is a copy of the [Aidoku Community](https://github.com/Aidoku-Community/sources) templates, with these additions:

- `madara`: Arabic relative dates ("منذ ساعتين", "منذ يومين", weeks/months/years, "اليوم", "أمس").
- `mangathemesia`: configurable search, details, chapter title and chapter date selectors; "فصل 12" style titles are dropped in favor of the chapter number.
- `iken`: a description that can't be parsed no longer panics.

Build and test a source (needs Rust with the `wasm32-unknown-unknown` target, plus `aidoku-cli` and `aidoku-test-runner` from [aidoku-rs](https://github.com/Aidoku/aidoku-rs)):

```sh
cd sources/ar.teamx
cargo test -- --nocapture   # live tests against the site
aidoku package              # builds package.aix
```

To try sources on your phone before publishing, run `aidoku serve sources/*/package.aix` and add the printed address as a source list in Aidoku (same Wi-Fi).

Pushing to `main` builds every source and publishes the list to GitHub Pages.

## License

Apache-2.0 or MIT, matching the upstream templates.
