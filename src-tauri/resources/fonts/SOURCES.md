# Bundled fonts

| Installed file | Distribution | License |
|---|---|---|
| `google-sans-code.ttf` | Google Sans Code 6.001, upright variable font from [Google Fonts](https://github.com/google/fonts/tree/main/ofl/googlesanscode) | [SIL Open Font License 1.1](OFL-GoogleSansCode.txt) |
| `wqy-zenhei.ttf` | WenQuanYi Zen Hei 0.9.45, regular face from the [WenQuanYi project release](https://sourceforge.net/projects/wqy/files/wqy-zenhei/0.9.45%20%28Fighting-state%20RC1%29/) | [GPL v2 with font embedding exception](COPYING-WenQuanYi.txt), with the M+ glyph notice |

Google Sans Code ships unchanged. Its SHA-256 is `c9649573afcd966f61096b1e9c3c3115e7b54315ca2adf9839c6f8b17d5f8e1f`.

WenQuanYi Zen Hei was extracted from the original TrueType Collection into a standalone TrueType font with aligned table offsets on 2026-11-26 for WebView2 font loading. Character mapping, glyph outlines and advances are preserved. The original archive and rebuild script are in `source/`. The archive is also [available from Debian](https://deb.debian.org/debian/pool/main/f/fonts-wqy-zenhei/fonts-wqy-zenhei_0.9.45.orig.tar.gz). With `fonttools==4.61.1` installed, run `python source/normalize_wqy.py` from this directory to rebuild the file. Its SHA-256 is `febf195294b363f3268d954531136ba0d99e9211fe584b07bbd1bd16dad84e8c`.
