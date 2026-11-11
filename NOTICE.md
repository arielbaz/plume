# Attributions et licences des données

Le code de Plume est sous licence MIT (voir `LICENSE`).

## Lexique Talmud / halakha

`ui/lexicon.json` : sélection de termes (traités, concepts, araméen talmudique,
halakha, Rishonim, Tannaïm/Amoraïm, mesures, parachiot) établie pour Plume.
Les relations entre termes sont éditoriales.

## Banque de mots

`ui/wordbank.json` rassemble deux sources :

- **Strong's Hebrew Dictionary** (1894), édition numérique Open Scriptures,
  licence **CC-BY-SA** — https://github.com/openscriptures/strongs
  Champs utilisés : lemme hébreu, translittération, définition courte.
- **Jastrow Dictionary** (1903), domaine public, mots-vedettes récupérés via
  l'API publique de Sefaria — https://www.sefaria.org/Jastrow
  Champs utilisés : mot-vedette hébreu/araméen.

La banque sert uniquement à proposer des suggestions de correction et à
alimenter l'onglet « Dictionnaire ». Elle ne remplace pas les textes originaux.

## Moteur

- whisper.cpp — licence MIT — https://github.com/ggerganov/whisper.cpp
- whisper-rs — licence MIT/Unlicense — https://github.com/tazz4843/whisper-rs
- Handy (décodage audio adapté) — licence MIT — https://github.com/cjpais/Handy

## Polices / icônes

Aucune police externe n'est embarquée : l'interface utilise les polices système.
