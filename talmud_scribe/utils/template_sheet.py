"""Génère la feuille guide imprimable pour le patron manuscrit."""
from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def _font(size: int):
    for name in ("DejaVuSans.ttf", "NotoSansHebrew-Regular.ttf", "Arial.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def make_template(truth_txt: str | Path, out_png: str | Path) -> Path:
    """Une ligne de consigne grisée (à recopier) suivie d'une ligne vide, par ligne de vérité."""
    lines = [l for l in Path(truth_txt).read_text(encoding="utf-8").splitlines() if l.strip()]
    W, H, margin, step = 1654, 2339, 120, 260      # A4 @ 200 dpi
    img = Image.new("L", (W, H), 255)
    d = ImageDraw.Draw(img)
    d.text((margin, 50), "Patron manuscrit — recopiez chaque ligne à la main, sous le modèle gris",
           fill=60, font=_font(30))
    y = 140
    for line in lines:
        if y + step > H - margin:
            break
        # Pas de mise en forme bidi ici : l'aperçu gris est indicatif, la vérité reste le .txt
        d.text((margin, y), line, fill=170, font=_font(34))
        d.line((margin, y + step - 60, W - margin, y + step - 60), fill=190, width=2)
        y += step
    out = Path(out_png)
    out.parent.mkdir(parents=True, exist_ok=True)
    img.save(out)
    return out
