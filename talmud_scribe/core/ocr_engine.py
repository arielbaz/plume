"""OCR de notes manuscrites (hébreu + français) avec prétraitement calibré.

Honnêteté technique : Tesseract est conçu pour l'imprimé ; sur du manuscrit les résultats
varient beaucoup selon l'écriture. Le patron manuscrit sert donc à *choisir automatiquement*
les meilleurs paramètres de prétraitement (binarisation, échelle, mode de segmentation)
pour VOTRE écriture, en minimisant le CER mesuré sur la feuille de calibration.
Le moteur est isolé derrière `recognize()` pour pouvoir brancher plus tard un modèle HTR
(ex. TrOCR/Kraken affiné) sans toucher au reste.
"""
from __future__ import annotations

import itertools
import json
from pathlib import Path

from PIL import Image

from utils.metrics import cer
from utils.preprocessor import prepare_image

ROOT = Path(__file__).resolve().parent.parent
CALIB = ROOT / "calibration"
PROFILE = CALIB / "profile.json"

DEFAULT_PARAMS = {"method": "adaptive", "scale": 1.5, "block": 31, "c": 12, "psm": 6}


def _tesseract():
    try:
        import pytesseract
        pytesseract.get_tesseract_version()
        return pytesseract
    except ImportError as e:
        raise RuntimeError("pytesseract n'est pas installé : pip install pytesseract") from e
    except Exception as e:
        raise RuntimeError("Tesseract introuvable : installez-le avec les langues heb et fra "
                           "(apt install tesseract-ocr tesseract-ocr-heb tesseract-ocr-fra).") from e


def load_params() -> dict:
    try:
        saved = json.loads(PROFILE.read_text(encoding="utf-8")).get("ocr", {})
    except (OSError, json.JSONDecodeError):
        saved = {}
    return {**DEFAULT_PARAMS, **{k: v for k, v in saved.items() if k in DEFAULT_PARAMS}}


def recognize(image: Image.Image, psm: int = 6, lang: str = "heb+fra") -> str:
    pt = _tesseract()
    return pt.image_to_string(image, lang=lang, config=f"--oem 1 --psm {psm}").strip()


def ocr_file(path: str | Path, params: dict | None = None, lang: str = "heb+fra") -> str:
    p = {**load_params(), **(params or {})}
    img = prepare_image(path, method=p["method"], scale=p["scale"], block=p["block"], c=p["c"])
    return recognize(img, psm=p["psm"], lang=lang)


def calibrate(patron_png: str | Path | None = None, truth_txt: str | Path | None = None,
              lang: str = "heb+fra", verbose: bool = True) -> dict:
    """Grid-search des paramètres sur la feuille patron ; sauvegarde le meilleur dans profile.json."""
    patron_png = Path(patron_png or CALIB / "manuscrit_patron.png")
    truth_txt = Path(truth_txt or CALIB / "manuscrit_text.txt")
    if not patron_png.exists():
        raise FileNotFoundError(f"{patron_png} manquant — voir calibration/README.md")
    truth = truth_txt.read_text(encoding="utf-8")

    grid = itertools.product(["adaptive", "otsu"], [1.0, 1.5, 2.0], [25, 41], [10, 15], [4, 6])
    best, best_cer = dict(DEFAULT_PARAMS), 9.9
    seen = set()
    for method, scale, block, c, psm in grid:
        if method == "otsu" and (block, c) != (25, 10):   # block/c sans effet pour otsu
            continue
        key = (method, scale, block, c, psm)
        if key in seen:
            continue
        seen.add(key)
        params = {"method": method, "scale": scale, "block": block, "c": c, "psm": psm}
        score = cer(truth, ocr_file(patron_png, params, lang))
        if verbose:
            print(f"  CER {score:6.1%}  {params}")
        if score < best_cer:
            best, best_cer = params, score

    try:
        profile = json.loads(PROFILE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        profile = {}
    profile["ocr"] = {**best, "cer": round(best_cer, 4)}
    PROFILE.write_text(json.dumps(profile, ensure_ascii=False, indent=2), encoding="utf-8")
    return profile["ocr"]
