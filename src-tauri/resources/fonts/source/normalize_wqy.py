"""Rebuild the browser-loadable WenQuanYi Zen Hei face with fonttools==4.61.1."""

import tarfile
from io import BytesIO
from pathlib import Path

from fontTools.ttLib import TTFont

source = Path(__file__).resolve().parent
with tarfile.open(source / "fonts-wqy-zenhei_0.9.45.orig.tar.gz") as archive:
    with archive.extractfile("wqy-zenhei/wqy-zenhei.ttc") as collection:
        original = BytesIO(collection.read())

with TTFont(original, fontNumber=0, lazy=True, recalcBBoxes=False, recalcTimestamp=False) as face:
    with (source.parent / "wqy-zenhei.ttf").open("wb") as output:
        face.save(output, reorderTables=False)
