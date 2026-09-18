# Findings — classic-faithful movement (Dijkstra over roads/rail/terrain) vs uniform land BFS

**Date:** 2026-08-04 · branch `movement-dijkstra` (isolated worktree; not merged). Replaces the uniform
8-dir land BFS in `rules::ReachField` with a **Dijkstra over `classic` move costs**. Data reproduced by
`cargo test -p civ-eval --test reach_divergence -- --ignored --nocapture` (harness committed on this branch).
Boards: `data/saves/testcontroller_T677.sav` (dense late-game) and `myagent_T50.sav` (early-game).

## TL;DR (merge verdict: **merge-worthy**, with a board caveat)
- Road/rail were **actually present** on both real boards (verified, not assumed). T677 is infra-dense
  (Road 26% of land, Railroad 12%, River 10%); T50 is infra-sparse (Road 0.3%, Rail 0.1%, River 5%).
- The new field **changes a large fraction of ground truth on the dense board**: 77% of reachable
  `nearest-owned` tiles change turn-count, and **79 / 119 (66%) of `nearest-owned` (owner×resource)
  answers change value**. On sparse T50 the change is real but modest (22% of tiles; 3/32 answers).
- All divergence on `nearest-owned` is a **speed-up** (new ≤ old, `raised = 0`) — expected, because at
  move-rate 1 every non-road tile still costs one full turn, so only road/river discounts and free rail
  can move the needle. For `reachable-nearest` (fast units), terrain cost **also raises** some answers
  (T677: 1462 raised; T50: 749 raised — on the road-less board fast units mostly see terrain *slowdowns*).
- **Degeneracy watch fires on T677 but does not break generation**: free rail collapses 41% of
  `nearest-owned` answers to a trivial 0–1 turns (28 of them literally 0 — the resource sits on a
  city-connected rail net). The generators still find a full decisive set (both reach kinds keep 12 and
  `verify-oracle` PASSES on T677), so no action is forced — but it confirms the standing guidance to
  **prefer earlier-turn boards for the reach kinds**.

---

## The cost model implemented (exact `classic` values)

All costs are in **movement fragments**, where a full move = `SINGLE_MOVE = 3` fragments. A unit refreshes
`move_rate × 3` fragments per turn. Per-terrain base cost (`classic terrain.ruleset movement_cost`, ×3 to
fragments):

| terrain | full moves | fragments |
|---|---|---|
| Grassland / Plains / Desert / Tundra | 1 | 3 |
| Forest / Hills / Jungle / Swamp / Glacier | 2 | 6 |
| Mountains | 3 | 9 |
| Ocean / Deep Ocean / Lake / Unknown | — | impassable (land filter) |

Edge cost from tile `a` to land tile `b` (cheapest applicable rule wins, rail < road < terrain):
- **Railroad↔Railroad** (both endpoints) → **0** (free).
- **Road↔Road**, or **River↔River** (moving *along* a river counts as a road) → **`road_move_cost = 1`** fragment.
- otherwise → destination terrain base cost above.

Both `Railroad` tiles carry `Road` too (rail implies road on the real boards), so a rail net also road-connects.

### Turn accounting
The field labels each tile with the **clock time `E`** (fragments) at which it is first reached; `turns_at =
ceil(E / budget)` with `budget = move_rate × 3`, so origins are 0 turns. Within a turn the unit spends fragments
along edges; the `classic` **always-advance rule** (a unit with *any* moves left can enter one more tile) is
honored — if an edge costs more than the fragments left this turn, the move still happens, consumes the rest
of the turn, and the clock rounds up to the next turn boundary (excess wasted). That transition is monotone in
`E`, so **Dijkstra on `E` (rail's 0-cost edges are non-negative) minimises turns**. The 6-turn horizon caps the
clock at `6 × budget`. Consequence at move-rate 1: every non-road tile costs exactly one turn to enter
(3/6/9 ≥ budget 3, always rounds up), so terrain alone never diverges from the old BFS there — only roads/rail do.

All ops and solvers share this one `ReachField::from_origins`, unchanged signature — `reach_turns`,
`travel_turns`, `travel_turns_from_owned`, and the `nearest-owned` / `reachable-nearest` solvers can't drift.

---

## Divergence numbers

### nearest-owned (multi-source from own cities, move 1, horizon 6)

| board | infra density (Road/Rail/River, % of land) | tiles changed | dropped (new<old) | raised | newly-opened by road/rail | **answers changed (owner×resource)** |
|---|---|---|---|---|---|---|
| **T677** | 26% / 12% / 10% | 5004 / 6468 = **77.4%** | 5004 | 0 | 2808 | **79 / 119 (66%)** |
| **T50** | 0.3% / 0.1% / 5% | 122 / 559 = **21.8%** | 122 | 0 | 60 | **3 / 32 (9%)** |

### reachable-nearest (per land unit at its own move-rate, horizon 6)

| board | land units | tiles changed | dropped (new<old) | raised (terrain cost, new>old) |
|---|---|---|---|---|
| **T677** | 523 | 587238 / 613306 = **95.7%** | 585776 | 1462 |
| **T50** | 7 | 1178 / 3034 = **38.8%** | 429 | **749** |

(reachable-nearest harness uses a local move-rate table mirroring `rules::unit_stat`; illustrative, not the
scored solver.) Note the **sign flip on T50**: with almost no roads, fast units (move 2–3) see more terrain
*slow-downs* (749 raised) than speed-ups (429 dropped) — the new model is stricter about rough terrain there.

### Concrete examples (T677, `nearest-owned`, turn-count drops)
- **(32, 24) Coal on Hills, `Railroad`+`Road`** — old **unreachable** (>6 turns by uniform BFS) → new **5**.
  The rail net pulls a previously out-of-horizon resource into reach.
- **(30, 25) Wheat on Plains, `Railroad`+`Road`** — old **6** → new **5**.
- **(7, 37) Iron on Mountains (no infra on the tile)** — old **3** → new **2**: the *path* to it runs over a
  road/river, so the destination drops a turn even though the tile itself is bare. This is the
  "turn-count drops because of a road/rail path" case the model must now reason about.

### T50 example
- **(34, 10) Pheasant on Plains** — old **unreachable** → new **6**; and **(28, 8) Wheat** old **4** → new **3**
  (river/road shortcut). Even the sparse board shifts a few answers.

---

## Degeneracy / drop-rate

Free rail is the degeneracy risk (everything on a city-linked rail net becomes ~0 turns). Measured on the
**new** `nearest-owned` answers (owner×resource), value histogram `[0,1,2,3,4,5,6,none]`:

- **T677:** `[28, 21, 20, 10, 9, 2, 3, 26]` → **49/119 (41%) trivial (0–1 turns)**, 28 of them a literal 0.
- **T50:** `[0, 0, 0, 7, 7, 4, 2, 12]` → **0 trivial**; answers spread 3–6.

So rail *does* pull a big chunk of T677 toward trivial — but the distribution still has a healthy 2–6 tail and
26 "none", and the generators (which resample away non-decisive instances) still assemble a full decisive set:
on T677 both `nearest-owned` and `reachable-nearest` keep 12 instances and **`verify-oracle` PASSES** (720
trials). No fight needed; the self-filtering works. The signal to record: **T677 is close to the rail-degeneracy
edge for the reach kinds — earlier-turn boards (T50-like) give a cleaner decisive pool.**

---

## Health gates (branch `movement-dijkstra`)
- `cargo build -p civ-cli --features remote` — **GREEN**
- `cargo test --workspace` — **GREEN** (incl. 5 new `ReachField` cost-model unit tests: terrain, road,
  rail-free, river-as-road, plus the unchanged uniform-board regression)
- `cargo clippy --workspace --all-targets -- -D warnings` — **GREEN** (no `#[allow]` added)
- `verify-oracle` — **PASS** on both T50 (618 trials) and T677 (720 trials). The solver-op parity holds
  because all reach consumers share the one `ReachField`; the ground truth changed exactly as intended.
