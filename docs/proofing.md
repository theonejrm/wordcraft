# Proofing: spelling, grammar, hyphenation

WordCraft checks each part of a document in the language it is marked with (Review › Language;
new documents take the system's language, e.g. German on a German system).

| Language | Spelling | Hyphenation | Grammar |
|---|---|---|---|
| English | built in (public-domain Moby word list) | built in | built-in rules, or LanguageTool |
| German | installed Hunspell dictionary | installed hyphenation patterns | built-in rules (repeated words, spacing, capitals after a sentence end), or LanguageTool |
| Other languages | installed Hunspell dictionary | installed hyphenation patterns | spacing, or LanguageTool |

Text in a language without a dictionary isn't judged at all, so a missing dictionary never marks
every word as wrong.

## Dictionaries

WordCraft bundles no third-party dictionaries (the widely used ones are GPL/LGPL licensed). It
uses the Hunspell dictionaries (`de_DE.aff` + `de_DE.dic`) and hyphenation patterns
(`hyph_de_DE.dic`) that LibreOffice, Firefox and most Linux distributions use, from:

1. `WORDCRAFT_DICTIONARIES` (a path list, like `PATH`);
2. your dictionaries folder: `~/.local/share/wordcraft/dictionaries` (Linux, BSD),
   `~/Library/Application Support/WordCraft/dictionaries` (macOS),
   `%APPDATA%\WordCraft\dictionaries` (Windows);
3. the system: `/usr/share/hunspell`, `/usr/share/myspell`, `/usr/share/hyphen`, the `/usr/local`
   and Homebrew equivalents, `~/Library/Spelling`;
4. an installed LibreOffice's dictionary extensions.

On Linux, install your distribution's package (`hunspell-de-de` / `myspell-de` and
`hyphen-de`). On Windows and macOS, install LibreOffice, or copy the `.aff`/`.dic` files of a
LibreOffice dictionary extension into your dictionaries folder. File ▸ Options ▸ Proofing lists
what was found; **Search again** picks up new files. Files in UTF-8, ISO-8859-1, ISO-8859-15 and
Windows-1252 are read. German, Austrian (`de_AT`) and Swiss (`de_CH`) dictionaries are used for
the matching region, with `de_DE` as the fallback.

Spelling uses [spellbook](https://github.com/helix-editor/spellbook) (MPL-2.0), a Rust
implementation of Hunspell's checking and suggestions, so results match LibreOffice's for the same
dictionary. Hyphenation reads the two-level pattern files (compound boundaries, then syllables).

Commands: `review.dictionaries` (`{"rescan": true}` to look again) lists dictionaries, patterns
and the folders searched.

## Grammar and style with LanguageTool

[LanguageTool](https://languagetool.org) (LGPL, Java) is a separate, free program with grammar,
punctuation and style rules for German, English and many other languages; LibreOffice uses it the
same way. WordCraft talks to a LanguageTool server **on your computer** over HTTP, so your text
stays on it.

1. Install Java 17 or later and download the LanguageTool desktop/server package
   (`LanguageTool-stable.zip`) from languagetool.org.
2. Start the server: `java -cp languagetool-server.jar org.languagetool.server.HTTPServer --port 8081`
3. In WordCraft: File ▸ Options ▸ Proofing ▸ **Check grammar and style with LanguageTool**
   (server `http://localhost:8081`). Or run `review.languageTool` with
   `{"enabled": true, "url": "http://localhost:8081"}`.

Paragraphs are checked in the background; marks appear when the server answers. LanguageTool's
results replace the built-in grammar rules for a paragraph's main language; spelling stays with
the dictionaries. When the server can't be reached, the built-in rules are used and Options shows
why. Only `http://` addresses are accepted.
