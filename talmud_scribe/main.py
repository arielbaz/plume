#!/usr/bin/env python3
"""Talmud Scribe — audio/vidéo/manuscrit -> Word (.docx), 100 % hors-ligne."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from utils.cache import Cache, make_key  # noqa: E402


def cmd_transcribe(a) -> int:
    from core import asr_engine as asr
    from core.docx_builder import build_document
    from utils import preprocessor as pre

    src = a.input
    if src.startswith(("http://", "https://")):
        src = str(pre.download_audio(src, ROOT / ".downloads"))
    src_path = Path(src)
    if not src_path.exists():
        print(f"Fichier introuvable : {src}", file=sys.stderr)
        return 2

    prompt = asr.build_prompt(extra=a.context or "")
    cache = Cache(enabled=not a.no_cache)
    key = make_key(src_path, model=a.model, lang=a.lang, prompt=prompt, beam=a.beam)
    cached = cache.get(key)
    if cached:
        print("Cache : transcription déjà faite, réutilisée.")
        segments = [asr.Segment(**s) for s in cached]
    else:
        wav = pre.to_wav16k(src_path)
        engine = asr.ASREngine(a.model, cpu_threads=a.threads)
        last = [-1]

        def show(p: float) -> None:
            pct = int(p * 100)
            if pct != last[0] and pct % 5 == 0:
                print(f"\r  {pct:3d} %", end="", flush=True)
                last[0] = pct

        segments = engine.transcribe(wav, None if a.lang == "auto" else a.lang,
                                     initial_prompt=prompt, beam_size=a.beam, progress=show)
        print()
        cache.set(key, [s.to_dict() for s in segments])

    paragraphs = asr.segments_to_paragraphs(segments)
    out = Path(a.output) if a.output else src_path.with_suffix(".docx")
    build_document(a.title or src_path.stem, paragraphs, out,
                   meta={"Source": src_path.name, "Modèle": a.model, "Langue": a.lang})
    print(f"Écrit : {out}")
    return 0


def cmd_ocr(a) -> int:
    from core import ocr_engine as ocr
    from core.docx_builder import build_document

    img = Path(a.input)
    text = ocr.ocr_file(img)
    paragraphs = [p for p in text.split("\n\n") if p.strip()] or [text]
    out = Path(a.output) if a.output else img.with_suffix(".docx")
    build_document(a.title or img.stem, paragraphs, out, meta={"Source": img.name, "Type": "Manuscrit (OCR)"})
    print(f"Écrit : {out}\nRelisez : l'OCR manuscrit reste imparfait.")
    return 0


def cmd_calibrate_asr(a) -> int:
    from core import asr_engine as asr
    print(asr.calibrate(models=a.models, language=a.lang))
    return 0


def cmd_calibrate_ocr(a) -> int:
    from core import ocr_engine as ocr
    print(ocr.calibrate())
    return 0


def cmd_make_template(a) -> int:
    from utils.template_sheet import make_template
    print(make_template(ROOT / "calibration" / "manuscrit_text.txt", ROOT / "calibration" / "feuille_guide.png"))
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="talmud_scribe", description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)

    t = sub.add_parser("transcribe", help="audio/vidéo/URL -> .docx")
    t.add_argument("input")
    t.add_argument("-o", "--output")
    t.add_argument("--title")
    t.add_argument("--model", default="small",
                   help="tiny|base|small|medium|large-v3-turbo|ivrit-turbo|ivrit-large (défaut: small)")
    t.add_argument("--lang", default="fr", help="fr|he|auto (défaut: fr)")
    t.add_argument("--context", help="termes/sujet ajoutés au prompt (ex. 'Baba Kama 42a')")
    t.add_argument("--beam", type=int, default=5)
    t.add_argument("--threads", type=int, default=0)
    t.add_argument("--no-cache", action="store_true")
    t.set_defaults(func=cmd_transcribe)

    o = sub.add_parser("ocr", help="image manuscrite -> .docx")
    o.add_argument("input")
    o.add_argument("-o", "--output")
    o.add_argument("--title")
    o.set_defaults(func=cmd_ocr)

    ca = sub.add_parser("calibrate-asr", help="compare les modèles sur le patron vocal")
    ca.add_argument("--models", nargs="+", default=["small", "ivrit-turbo"])
    ca.add_argument("--lang", default="fr")
    ca.set_defaults(func=cmd_calibrate_asr)

    co = sub.add_parser("calibrate-ocr", help="optimise le prétraitement sur le patron manuscrit")
    co.set_defaults(func=cmd_calibrate_ocr)

    mt = sub.add_parser("make-template", help="génère la feuille guide à imprimer")
    mt.set_defaults(func=cmd_make_template)
    return p


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except (RuntimeError, FileNotFoundError) as e:
        print(f"Erreur : {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
