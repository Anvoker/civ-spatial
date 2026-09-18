# Three-State Fog-of-War — Design + AS-BUILT

**Status:** IMPLEMENTED on branch `fog-three-state`. The body below is the original design proposal;
the **AS-BUILT DECISIONS** block records where the shipped code deviates from it (per the user's
authoritative overrides). Read that block first — several §2 defaults were overridden.
**Decision origin** (see `CONTINUITY-2026-08-04.md` §PENDING #1): implement real three-state fog.

---

## AS-BUILT DECISIONS (authoritative — supersede the proposal where they differ)

1. **Vision model = REAL Freeciv `classic`, squared-radius (NOT the proposed Chebyshev approximation).**
   §2's Chebyshev proposal was **overridden** by the user. We use Freeciv's `vision_radius_sq` with its
   `sq_map_distance` metric `dx²+dy²` over our non-wrapping coordinates: a tile is visible iff
   `dx²+dy² ≤ vision_radius_sq` from an own unit/city (intersected with `known`). Values verified
   against the locally-installed `Freeciv 3.2.5 data/classic/` ruleset, baked into named constants in
   `civ-core/src/board.rs`:
   - **units**: every land unit = `2` (the 3×3 block); aircraft / fast warships / Spy / Leader = `8`;
     AWACS = `26` (`units.ruleset`). NB: the Explorer is `2` in classic, **not** wide-vision.
   - **city**: `5` (`effects.ruleset City_Vision_Radius_Sq` = `game.ruleset init_vis_radius_sq` = 5).
   - **Not modelled (documented approximation)**: the conditional `Unit_Vision_Radius_Sq` effects —
     Fortress `+8` (gated on the *Invention* tech, which our `Board` does not decode → applying it
     unconditionally would over-see) and Mountains `4` (ungated, but on-mountain-only). Base per-source
     vision is faithful; these terrain/extra bonuses are a deliberate, noted omission.
2. **Fogged enemy cities kept last-known** (garrison hidden → `city-defense` reads 0 = "undefended as
   far as you can see"). Implemented by the mask keeping the city but dropping its units.
3. **`--fog` is now three-state** (replaces the two-state mask). `Board.visibility: Option<grid>`
   distinguishes Unexplored/Fogged/Visible; encoders tag fogged tiles `(fogged)` + a header preamble.
4. **Fog-perspective guardrail** (added): `--fog` rejects a DEAD or non-civilization
   (barbarian/pirate/animal) player with a clear error; `--allow-any-fog-player` overrides. `Player`
   gained `is_alive`; `Player::is_valid_fog_perspective()` encodes the rule.
5. **`rulesetdir` gate** (folded in): a hard error unless the save's `[savefile] rulesetdir == "classic"`
   (our constants are classic-specific); `--allow-ruleset` overrides. `Board` gained `ruleset`/`turn`.
6. **Marker/omission policy**: `(fogged)` is appended to every *rendered* fogged tile and stripped on
   reconstruction (fog is a derived view property, not a `RecoveredTile` fact). Fogged **land** tiles
   are force-listed so their tag reaches the model; fogged **water/default** tiles stay omittable (a
   hidden *land* unit can't be there), so a heavily-explored board's render doesn't balloon.

> **MULTI-BOARD EXPERIMENT NOTE (for the corpus / run-matrix track):** the fog perspective ("self")
> MUST be a **living, real civilization** — never a barbarian/pirate/animal or a dead
> (`is_alive=FALSE`) slot. Sight radii are computed around the perspective player's own units/cities,
> so a barbarian/dead slot (which owns nothing) yields a meaningless "self" view. The CLI now enforces
> this (decision 4); the matrix should pick e.g. T677 player 0 (Testcontroller), not player 3
> (a live Pirate) or player 4 (a dead Barbarian).

---

## 0. TL;DR

Freeciv has three visibility states; we currently model two (we collapse *fogged* into *visible*),
so the model and the oracle **over-see live enemy units** on explored-but-unwatched tiles. The fix is
small and low-risk **because of an architectural accident we should lean on**: fog is applied *once*,
upstream of everything, by producing a masked `Board`; that **same masked `Board` object** is then fed
to both the ground-truth solvers and the model-facing encoders (`load_board` → `generate_questions` +
`render`, in `civ-cli/src/main.rs`). There is literally one board. So **if the mask drops hidden enemy
units, every solver and every encoder is automatically fog-consistent — no per-solver plumbing is
required.** The parity seam is "mask upstream, and let nothing reach around it."

The build is therefore: (1) compute the currently-visible tile set, (2) enrich the masking to a
three-state view that keeps terrain on fogged tiles but drops live enemy occupants, (3) mark fogged
tiles in the encoders so the model is *told* "you remember terrain here but can't see who's on it," and
(4) prove the no-reach-around invariant with a parity test. Solver logic itself does not change.

---

## 1. The three states (target semantics)

Per the perspective player P (the `--fog <player-id>` argument):

| State | Definition | Terrain/extras/owner | Enemy units | Enemy cities | Own units/cities |
|---|---|---|---|---|---|
| **Unexplored** | never in P's `known` map (`map_t` `'u'`) | hidden → `"Unknown"` | dropped | dropped | (none can be here) |
| **Fogged** | `known` **and not** currently visible | **remembered (kept)** | **dropped (hidden)** | **kept, last-known** | (none can be here) |
| **Visible** | within P's current sight | full live truth | shown | shown | shown |

- **Known (ever-explored) is already correct** — decoded from `map_t`, `'u'` = unexplored. We do
  **not** touch it. The bug is only the fogged↔visible collapse.
- **Own units/cities are always on Visible tiles** by construction: a unit/city sees its own tile
  (sight radius ≥ 0), so it can never fall in the Fogged/Unexplored buckets.
- **Visible ⊆ Known by definition** (state = `Known && in_sight`). We *intersect* our recomputed sight
  with the save's `known` bit rather than trusting sight alone.

---

## 2. Visibility computation  *(SEE AS-BUILT #1 — the Chebyshev proposal here was OVERRIDDEN to the
real Freeciv squared-radius model)*

**Visible(P) = { tiles within `vision_radius_sq` of any of P's own units or cities } ∩ Known(P).**

- **Sight source & radius.** *As built*: the real Freeciv `classic` `vision_radius_sq` per unit/city,
  tested with `sq_map_distance` = `dx²+dy²` (non-wrapping). See AS-BUILT #1 for the exact constants
  and their ruleset sources. Placed in **civ-core** (`board.rs`), dependency-free.
- **Terrain/hills sight bonus.** classic *does* define Fortress (+8, tech-gated) and Mountains (4)
  vision effects; both are **not modelled** (AS-BUILT #1) — the tech gate is undecodable and the
  mountains case is a marginal on-mountain-only expansion.
- **Own-only vision.** Sight comes from P's own units+cities only; no diplomacy/shared vision.
- **id → name.** Masking resolves `player_id → player.name` via `Board.players` to tell own from enemy;
  errors if the id has no player.

Cost: O(#own_units + #own_cities) sight disks, each O(r²) — negligible.

---

## 3. Data model

**Added, in civ-core:**

```rust
pub enum Visibility { Unexplored, Fogged, Visible }   // per-tile, single perspective

pub struct Board {
    // … existing fields …
    pub visibility: Option<Vec<Vec<Visibility>>>,   // None on an omniscient board (all-Visible)
    pub ruleset: Option<String>,                    // [savefile] rulesetdir  (AS-BUILT #5)
    pub turn: Option<i32>,                           // [game] turn            (AS-BUILT #5)
}
```

A per-tile `Visibility` grid on the masked board lets encoders/viewer tell Fogged from Visible-empty;
`None` = "no fog, everything visible," so non-fog boards and the `crop` path are unchanged. The
existing "unexplored ⇒ terrain `"Unknown"`" convention is kept; fogged tiles keep their real terrain
string and the grid distinguishes them.

---

## 4. The solver/encoder shared-visibility seam (CRITICAL)

**The seam already exists and is structural. We preserve it; we do not rebuild it.**

`load_board` parses → crops → **masks**, returning one `Board`. `cmd_gen`/`cmd_verify`/`cmd_run` all
call `generate_questions(&board, …)` (solvers via `solve()`) **and** the encoders' `render(&board)` on
that **same** value. They cannot disagree about which enemies exist, because they read the same list.

> **INVARIANT (no reach-around).** For a fog run, the masked `Board` is the *only* board any solver,
> encoder, operator, or interactive tool ever sees. Nothing captures or re-derives the unmasked board
> for generation or rendering.

Audited (both fine): generation samples only surviving occupants; viewer-export re-applies fog via the
same `mask_to_known`, so it reproduces the identical three-state board.

### Per-kind effect

Because terrain/extras/owner are **retained** on Fogged tiles and only **live enemy occupants**
vanish, the only kinds whose answers move are those reading live enemy **units** (or an enemy city's
garrison): `unit-strength`, `city-defense` (fogged enemy city → 0 defenders), the `ThreatField`-driven
decision kinds (`settle-site` safety axis, `t3-retreat`, `t3-threat`, `cf-vacate`,
`adv-assault-target`, `triage-reinforce`), and `compare-two-attacks`. In every case the change is
delivered *by the mask removing the unit from `board.units`*, not by editing the solver — `rules.rs` is
unchanged. Terrain/geometry kinds are unaffected (they already handle `"Unknown"`).

---

## 5. Rendering (what each encoder emits per state)

Design goal — **honesty**: a Fogged tile must be visibly distinct from a Visible-empty tile. Per state:

- **Unexplored** — unchanged: terrain `"Unknown"`, `?` glyph, no occupants.
- **Fogged** — render **terrain + extras + territory owner**, **omit units** (they are already dropped
  from the masked board), keep an **enemy city as last-known**, and add a **`(fogged)` marker**
  (AS-BUILT #6). A header preamble explains the marker globally. Fogged **land** tiles are force-listed
  so the tag reaches the model; fogged water/default tiles stay omittable.
- **Visible** — full live truth (unchanged).

**Reconstruction gate:** `(fogged)` is stripped by the recovery parser (fog is a view property, not a
`RecoveredTile` fact), so every encoder round-trips the masked board it renders (tested).

---

## 6. Backward-compat / the `--fog` flag

- **`--fog <id>`** now means three-state (AS-BUILT #3). Provenance suffix stays `#fog(pN)`; the
  viewer path reproduces three-state for free. The omniscient board (no flag) is unchanged.
- **Non-fog runs**: `visibility = None`, byte-identical to today.
- **Oracle self-consistency gate stays 100% by construction** and is exercised fogged at both tiers on
  T50 and T677 (`oracle_invariant.rs`, `just verify-oracle`).

---

## 7. Test / verification plan  *(all implemented)*

1. **civ-core visibility unit tests** (`civ-core/tests/fog.rs`): squared-Euclidean metric + real
   radii; own-city visible disk; the three-state split (fogged enemy units dropped, terrain kept,
   enemy city kept, unexplored → Unknown); `Visible ⊆ Known`; no-reach-around; perspective validity.
2. **Parity property test** (`civ-eval/tests/fog_parity.rs`): a hand-built board where an enemy land
   unit on a *fogged* tile would raise `ThreatField` on an adjacent own city — asserts the encoder
   shows **no unit** (tagged `(fogged)`) AND `ThreatField::at` is **0**, then moves the same unit into
   sight and the threat **reappears** (fog changes the answer). Cross-checked against the omniscient
   board.
3. **No-reach-around audit** (`fog.rs`): after masking, no enemy unit sits on a fogged/unexplored tile.
4. **Negative oracle test** (`fog_parity.rs`): a fogged enemy city's `city-defense` reads 0 (garrison
   hidden) vs. positive on the true board, and the scorer rejects the pre-fog pick as *wrong*.
5. **Oracle end-to-end gate** (`oracle_invariant.rs`): 100% on fogged T50 (p1) and fogged T677 (p0) at
   both difficulty tiers; reconstruction round-trips the fogged board for every static encoder.

---

## 8. Implementation phasing — DONE (phases 1–6); phase 7 (viewer export) owned by a separate track.

---

## 9. Fog perspective vs. a question's `player` — the COHERENCE requirement (AS-BUILT)

> **STATUS: AS-BUILT** (branch `feat-fog-perspective-coherence`). The requirement below is now
> ENFORCED at question-generation time — see the "AS-BUILT enforcement" block at the end of this
> section. The original "experiment-track, not implemented" note is superseded.

There are **two distinct notions of "player"** that must be made coherent under fog:

1. **Fog perspective** — whose known-map the board is masked to (`--fog N`); i.e. whose sight we
   compute (`Board::visible_grid(N)`), producing the single masked `Board` fed to solvers + encoders.
2. **A question's `player` field** — whose decision a *player-relative* kind is about ("you are player
   X"): `t3-threat`, `adv-assault-target`, `triage-reinforce`, `settle-site` (safety axis),
   `nearest-owned`, `cf-vacate`.

**As built, `--fog N` is a GLOBAL board mask chosen INDEPENDENTLY of each item's `player`.** So a
fogged run of player-relative kinds can be **incoherent**: the board is fogged to player N's vision
while a question says "you are player M ≠ N" — the model then reasons about M's decision over a map
masked to N's sight (M's own units could even be fog-hidden).

> **REQUIREMENT (for a coherent fogged experiment):** when fogging to player P, the fog perspective
> and every player-relative item's `player` **must be the same P** — sight is computed around exactly
> the player whose decision is asked. P must also be alive/real (enforced by the `--fog` guardrail,
> AS-BUILT #4). **Terrain/geometry kinds** (`terrain`, `distance`, `direction`, `nearest`,
> `region-count`, `adjacency`, `reachability`, `best-site`, `constraint-site`) are
> **perspective-agnostic** — fog is just a perception mask over terrain, with no player semantics — so
> they need no alignment.

**Where enforced (generation, NOT the core mask):** the restriction lives in question generation, not
in `mask_to_known`. The core mask stays a pure board transform that does not know the question set;
coupling it to item `player` would break the "one masked board feeds everything" seam. The guardrail
(AS-BUILT #4) already guarantees P is a valid living civ; this rule adds the `P == item.player`
alignment as its companion.

### AS-BUILT enforcement (`feat-fog-perspective-coherence`)

The fog perspective is threaded **into generation** as an owner NAME and used to pin every
player-relative kind's "you" to P — generating **only for P** (skipping other owners), never filtering
post-hoc (which would waste the sampling budget on discarded owners):

- **Plumbing.** `generate_questions(board, seed, per_kind, difficulty, perspective: Option<&str>)`
  gained the `perspective` parameter, forwarded to a new 4th arg on `QuestionKind::generate`. The CLI
  resolves `--fog <id>` → the player's `name` via `board.players` (helper `fog_perspective_name` in
  `civ-cli/src/main.rs`) and passes it from `gen-questions`/`verify-oracle`/`run`. `viewer-export`
  passes the same perspective (resolved in `resolve_board`) so regenerated item ids match a fogged
  run's traces. `None` = unfogged → **byte-identical to the pre-change random-owner behavior**.
- **Restricted kinds (the 7 player-relative).** In `civ-eval/src/generators.rs`: `settle-site`,
  `t3-threat`, `adv-assault-target`, `triage-reinforce`, `nearest-owned` restrict their `owners` list
  via the `restrict_to_perspective` helper; `t3-retreat` and `cf-vacate` (whose "you" is a unit/city
  owner) `retain` their owner set to P. When `perspective` is `Some`, only P survives; when `None`,
  the list is unchanged.
- **Perspective-agnostic kinds are untouched** — including `constraint-site`, which carries a
  `player` field but is classified agnostic HERE (fog is a perception mask over terrain, no player
  semantics), so it keeps its random-player draw. All terrain/geometry kinds ignore the new
  parameter.
- **Edge behavior when P can't see much.** If P owns nothing eligible for a kind (e.g. `cf-vacate`
  when P sees no threatened own city), that kind yields **0 instances** — correct; there is NO
  fallback to another player. `generate_questions` logs `[fog-coherence] fog perspective set: …` once
  when a perspective is active, and each kind's existing `kept N` line then makes a sparse yield
  visible as a real signal.
- **Tests.** `civ-eval/tests/fog_coherence.rs` proves (a) under perspective P every player-relative
  item is about P (on T677, both unmasked-with-P and the actually-masked board), (b) the unfogged path
  still spans multiple owners (no regression), (c) a terrain kind is perspective-invariant. The fogged
  Oracle-100% gate (`oracle_invariant.rs`) now generates WITH the perspective, so it exercises the
  coherent path at both tiers on T50 (p1) and T677 (p0).
