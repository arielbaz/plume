# Patrons de calibration

Deux fichiers **à fournir par vous** (ils dépendent de votre voix et de votre écriture) :

| Fichier | Contenu | Comment le produire |
|---|---|---|
| `vocal_patron.wav` | ~1 min, vous lisez `vocal_text.txt` à voix naturelle | Enregistrez avec n'importe quel micro, exportez en WAV |
| `manuscrit_patron.png` | feuille écrite à la main reprenant `manuscrit_text.txt` | `python main.py make-template` imprime une feuille guide ; écrivez dessus, scannez/photographiez |

`vocal_text.txt` et `manuscrit_text.txt` sont la **vérité terrain** : modifiez-les pour qu'ils
correspondent exactement à ce que vous dites / écrivez. Puis lancez :

```bash
python main.py calibrate-asr
python main.py calibrate-ocr
```

Les paramètres retenus sont enregistrés dans `calibration/profile.json`.
