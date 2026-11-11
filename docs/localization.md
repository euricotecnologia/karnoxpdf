# Interface language

Choose **Menu > Edit > Preferences…** (Command-comma on macOS, Ctrl-comma elsewhere), then the **General** tab. Under **Interface language**, select **Português do Brasil**, **Português de Portugal**, **Inglês**, **Espanhol** or **Francês**. The change applies immediately and persists between launches. Command ids, document contents and file names are unchanged. The same tab offers **Display theme** with system, light and dark choices; document defaults are on the **Documents and view** tab.

**Auto** (the default) follows the system language: `LC_ALL`, `LC_MESSAGES` or `LANG`, then the preferred-languages list on macOS. `pt_BR` uses Brazilian Portuguese, `pt_PT` uses European Portuguese, and other Portuguese locales default to Brazilian Portuguese. French regional locales use the French catalog. Chinese locales for Taiwan, Hong Kong and Macau (`zh_TW`, `zh_HK`, `zh_MO`, `zh-Hant`) use Traditional Chinese; other Chinese locales (`zh`, `zh_CN`, `zh_SG`, `zh-Hans`) use Simplified Chinese. On Windows, **Auto** uses the Windows display languages, in preference order, when no supported locale is selected by those environment variables. The query asks Windows directly (`GetUserPreferredUILanguages`, through the `sys-locale` crate) without starting a process, and is cached for the process. If detection fails or the display language has no catalog, KarnoxPDF shows English.

The control channel exposes the setting through `ui.set`:

```json
{"method":"ui.set","params":{"key":"language","value":"ja"}}
```

The value is `auto` or a language code (`en`, `pt-br`, `pt-pt`, `es`, `fr`, `ja`, `zh-hans`, `zh-hant`, `cs`), in any case. `ui.state` reports the selected code. Unknown values return an error without changing the current setting. Preferences saved before this setting existed follow the system language.

Dialogs, panels, menus, notices and history labels go through the catalog (`tl!`). Japanese, Traditional Chinese, Simplified Chinese and Spanish have broad catalogs; Czech, Brazilian Portuguese, European Portuguese and French currently cover core menus and preferences. Untranslated labels use English. Error details that come from the engine or the operating system are shown as they are, inside a translated frame ("操作「…」に失敗しました: …"). The command palette matches the translated label, the English label and the command id. Vertical Japanese PDF rendering is an existing viewer feature; this change does not add vertical text editing.

Japanese interface text uses BIZ UDPGothic from craft-fonts, an optional build input (`CRAFT_FONTS_DIR`; see the README). A build made without it has no Japanese face, so Japanese labels show replacement boxes. Chinese is drawn with the same craft-fonts CJK faces (in Simplified Chinese the `Hans` faces come first, so a line never mixes faces with different baselines), so a character none of them has shows a replacement box (and every Chinese character does in a build without craft-fonts). Czech, Portuguese, Spanish and French use the bundled Latin faces.

Simplified Chinese coverage tests, following a complete-catalog approach, check every registered command, All tools label and `tl!("literal")` in the UI source. Generated history labels and diagnostics are also tested with user-supplied names and error details. Catalog coverage does not guarantee glyph coverage: the release-pinned craft-fonts input currently has no `Hans` face, and the web build embeds only BIZ UDPGothic Regular. Chinese font delivery remains separate work, as discussed in [#112](https://github.com/euricotecnologia/karnoxpdf/pull/112) and [#140](https://github.com/euricotecnologia/karnoxpdf/pull/140).

## How translations work

KarnoxPDF translates its interface with this system (`crates/ui-egui/src/i18n/`):

- Strings in the code stay in English and serve as lookup keys. At draw time, `tl!("Save")` returns the text in the current language, and falls back to English when there is no entry.
- Each language has a catalog, `crates/ui-egui/src/i18n/<code>.tsv`, with one entry per line: `context<TAB>English<TAB>translation`. An empty context marks a plain string. `@id` keys an entry by command id (for example `file.saveAs`), so one menu item can read differently from another with the same English label. `@plural` holds plural forms. Any other context disambiguates an English word with several meanings (`tr_ctx`). The header of `ja.tsv` documents the format and escapes.
- Placeholders (`{name}`) must appear in both columns. A trailing `…` must be kept. `i18n::fmt` fills placeholders in one pass, so a value such as a file name containing `{n}` is inserted as written.
- Catalogs are validated strictly: unknown escapes, unknown `@` contexts, placeholder or ellipsis mismatches, wrong plural form counts and duplicates are errors. The tests require the bundled catalogs to have none; at run time a bad line is skipped (and logged) and that string shows in English.
- Catalogs are assets: each `.tsv` has an `ATTRIBUTION.toml` entry with `kind = "translation"` and its SHA-256, checked by `cargo xtask assets`.
- Translations are clean-room: write them from the meaning of the English text, never from another product's string tables.

## Adding a language

1. Copy the header of `ja.tsv` into `xx.tsv` and translate entries.
2. Add one row to `LANGUAGES` in `crates/ui-egui/src/i18n/mod.rs`: code, native name, catalog and plural rule.
3. Add an `ATTRIBUTION.toml` entry for the catalog (`kind = "translation"`), then run `cargo xtask assets --write` and `cargo xtask assets`.

The Preferences dropdown, the system-language match and the catalog tests (format, duplicates, placeholders, plural forms, command ids) then pick it up.
