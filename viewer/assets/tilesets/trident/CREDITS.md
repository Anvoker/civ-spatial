# Freeciv `trident` tileset (GPL) — bundled assets & credits

The spritesheets in this directory (`tiles.png`, `cities.png`, `units.png`) and their `.spec`
descriptors are taken **verbatim** from the [Freeciv](https://www.freeciv.org/) project's
`data/trident/` tileset (the small, flat *overhead* tileset). They are redistributed here under the
**GNU General Public License, version 2 or later** — the full text is in the adjacent `COPYING`
file. Freeciv's data is GPL-licensed; this viewer bundles a small subset unmodified.

Source: `https://github.com/freeciv/freeciv` → `data/trident/` (fetched from `master`).

## Artist credits (verbatim from the `.spec` `[info]` headers)

**Terrain — `tiles.png` (`tiles.spec`):**
```
Tatu Rissanen <tatu.rissanen@hut.fi>
Jeff Mallatt <jjm@codewell.com> (miscellaneous)
Eleazar (buoy)
Vincent Croisier <vincent.croisier@advalvas.be> (ruins)
Michael Johnson <justaguest> (nuke explosion)
The Square Cow (inaccessible terrain)
GriffonSpade
```

**Cities — `cities.png` (`cities.spec`):**
```
Jerzy Klek <jekl@altavista.net>

european style based on trident tileset by
Tatu Rissanen <tatu.rissanen@hut.fi>
Marco Saupe <msaupe@saale-net.de> (reworked classic, industrial and modern)
```

**Units — `units.png` (`units.spec`):**
```
Tatu Rissanen <tatu.rissanen@hut.fi>
The Square Cow (u.migrants)
```

## How these are used

`scripts/build-tileset.mjs` parses the `.spec` files (our own from-scratch parser of the public
`.spec` grammar — it does **not** copy freeciv-web's AGPL `tileset_config_*.js`) into a sprite index
and inlines the three PNGs as `data:` URIs, emitting `viewer/src/tileset-data.js`
(`window.__CIVSPATIAL_TILESET__`). The viewer blits terrain / city / unit sprites from that index and
gracefully falls back to flat color whenever a sprite (e.g. Ocean, which has no single-cell sprite)
or the whole bundle is missing.
