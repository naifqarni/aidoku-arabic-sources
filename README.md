# Aidoku Arabic Sources

Arabic manga sources for [Aidoku](https://github.com/Aidoku/Aidoku), ported from the matching [Keiyoushi](https://github.com/keiyoushi/extensions-source) extensions for Mihon.

## Usage

On a device with Aidoku (0.7+) installed, open **[naifqarni.github.io/aidoku-arabic-sources](https://naifqarni.github.io/aidoku-arabic-sources/)** and tap the "Add Repository" button to add the source list directly to the app.

Alternatively, paste this into Safari on your device:

```
aidoku://addSourceList?url=https://naifqarni.github.io/aidoku-arabic-sources/index.min.json
```

Or navigate to the settings tab, and under the source lists page add `https://naifqarni.github.io/aidoku-arabic-sources/index.min.json`.

## Sources

All non-adult Arabic sources from Keiyoushi that are still online (39). "Tested" means the source was run against the live site (list, details, chapters, pages). "Untested" sources sit behind Cloudflare or a bot check that can't be passed outside the app; Aidoku asks you to pass it in its web view the first time.

| Source | Site | Based on | Status |
| --- | --- | --- | --- |
| Anyone Manga | anyonemanga.com | Madara | untested (Cloudflare) |
| أريا مانجا | ar.kenmanga.com | MangaThemesia | untested (Cloudflare) |
| AriaToon | ariatoon.com | custom | tested |
| AsgardScans | asgardscans.com | custom | tested |
| Azora | azorafly.com | Iken | tested |
| Comic Verse | arcomixverse.blogspot.com | ZeistManga | tested |
| Despair Manga | despair-world.com | custom | tested |
| شبكة كونان العربية | manga.detectiveconanar.com | Madara | untested (Cloudflare) |
| Dilar | dilar.tube | custom | tested |
| Duskoryvile | duskoryvile.com | custom | needs a site account (log in under the source settings) |
| EShadow | eshadow.net | custom | tested |
| Hijala | hijala.com | MangaThemesia | untested (Cloudflare) |
| Kawii Manga | kawaiimanga.org | custom | tested |
| Lava Scans | lavascans.com | MangaThemesia | tested |
| Loner Translations | loner-tl.blogspot.com | ZeistManga | tested |
| 3asq | 3asq.online | Madara | tested |
| Manga Ai Land | manga-ai-land.blogspot.com | ZeistManga | tested |
| MangaDar | mangadar.com | custom | tested |
| مانجا ليك | mangalik.net | Madara | untested (Cloudflare) |
| مانجا لينك | link-manga.net | Madara | untested (Cloudflare) |
| MangaLionz | manga-lionz.org | Madara | untested (Cloudflare) |
| MangaSpark | sparkmanga.net | Madara | untested (bot check) |
| Manga Starz | starzmanga.com | Madara | untested (Cloudflare) |
| MangaSwat | meshmanga.com | custom | untested (Cloudflare) |
| Manga Tales | www.mangatales.com | custom | tested |
| MangaTek | mangatek.com | custom | tested |
| MangaTik | mangatik.com | custom | tested |
| MangaTime | mangatime.org | custom | tested |
| MangaTuk | mangatuk.com | custom | tested |
| Manhatok | manhatok.blogspot.com | ZeistManga | tested |
| Murim | murim-manga.blogspot.com | ZeistManga | tested |
| Oduto | nb19u.blogspot.com | custom | tested |
| Orca Manga | infinity896.blogspot.com | ZeistManga | tested |
| ProComic | procomic.pro | custom | some chapters may need the site's Turnstile check |
| Rocks Manga | rocksmanga.com | custom | tested |
| StellarSaber | stellarsaber.pro | custom | untested (Cloudflare) |
| Team X | olympustaff.com | custom | tested |
| XSano Manga | www.xsano-manga.com | ZeistManga | tested |
| Yokai | yokai-team.blogspot.com | ZeistManga | tested |

Paid/locked chapters are hidden (Despair Manga shows them with a lock instead, since the site serves their pages).

Not included: adult sources; Hizo Manga (the site now hosts only text novels); Manga Cloud and Never Scans (sites are gone); Onma and Niji Translations (unreachable when checked).

## Development

The `templates/` directory is a copy of the [Aidoku Community](https://github.com/Aidoku-Community/sources) templates, with these additions:

- `madara`: Arabic relative dates ("منذ ساعتين", "منذ يومين", weeks/months/years, "اليوم", "أمس").
- `mangathemesia`: configurable search, details, chapter title and chapter date selectors; "فصل 12" style titles are dropped in favor of the chapter number.
- `iken`: a description that can't be parsed no longer panics.
- `madara`: optional paginated chapter endpoint, `data-encrypted-src` images.
- `mangathemesia`: optional page image processing hook.
- `zeistmanga`: new template for Blogger-based sites, ported from Keiyoushi's.

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
