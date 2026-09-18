# Reasoning-Frontier Question Kinds — Revision v2 (outcome after owner feedback)

*Companion to [`reasoning-frontier-questions.md`](reasoning-frontier-questions.md) — the original proposals P1–P7 (v1) and the full reasoning trail. THIS file is the REVISED outcome after the owner's review: the CHAMPION / MODIFY / DROP verdicts, the push-backs, the new N1 proposal, the Hidden-Force family, and the build order.*

---


*The owner reviewed P1–P7 and pushed back. This section supersedes §1's verdicts where they differ; the
v1 text above is kept intact (git preserves both) as the reasoning trail. For each proposal: **CHAMPION**
(keep, defend), **MODIFY** (fix the flaw, revised phrasing + scoring), or **DROP** (can't be made good
enough). The owner explicitly asked for push-back where I disagree, and I disagree in several places —
those are called out as **PUSH-BACK**.*

## v2.0 The principle the whole hidden-info family turns on (owner's "threat-under-uncertainty, done right")

The owner's two deepest objections — P1's hindsight scoring and the "muddied" hard core of P2 — are the
same objection, and it has a clean statement. Call it the **luck/tool-resistance duality** for engine (a):

> An answer is tool-resistant under fog **iff** it depends on the hidden contents of the fog. But
> depending on hidden contents is exactly what makes it a **prediction** — unknowable with certainty from
> the model's inputs, so a well-reasoned answer can still be "wrong" by luck. You cannot have engine-(a)
> resistance **and** a luck-free oracle at once: remove the dependence on hidden contents to kill the luck
> and you kill the resistance (the answer becomes a function of the visible board → a tool computes it).

This is why P6's "make the verification convincing" and any "worst-case / possibility" reframe that scores
on the *masked* board are traps: they are luck-free precisely because they are tool-trivial. "Done right"
therefore does **not** mean "find a luck-free hidden-info question" — there is no such thing. It means
three concrete disciplines, applied to every engine-(a) kind below:

1. **Reasoning-rejectable decoys.** Split the candidate set so that *some* wrong options are wrong **on the
   model's own inputs** (provably cannot be the answer without seeing the fog), converting part of the task
   from prediction to **soundness**. This is luck-free credit the model can earn by pure reasoning, and it
   is the single biggest honesty lever. (Where a kind admits few such decoys — P3 — say so and lean harder
   on 2–3.)
2. **Curate the true answer to be decisive and *grounded*.** The realized truth must agree with the
   best-reasoned prior on the large majority of instances (a decisive margin + a visible cue that points at
   it), so residual luck is small and **mean-zero**.
3. **Report lift-over-baseline at large N with a CI, never a raw accuracy.** The headline is
   *model accuracy − best fog-blind/proxy tool*, with N large enough that per-instance luck averages out.
   The owner accepted the hindsight problem "only at large N" — this makes that acceptance the metric,
   not a footnote.

Every hidden-info verdict below cites which of these three it uses. This section is the honest answer to
"threat-under-uncertainty done right": it is P1/P3 with (1)–(3) made mandatory, not a fourth magic kind.

## v2.1 Per-proposal verdicts

### P1 — Surprise-strike exposure → **CHAMPION, with two mandatory fixes** (engine a)

The owner is right on both counts, but neither is fatal; both are curation problems, and fixing them makes
P1 *more* honest than P3, not less.

**(1) Connectivity dependence — PUSH-BACK on the proposed reframe, then a real fix.** The owner's instinct
"reframe *this turn* → *within K turns*" is necessary but **not sufficient**, and I want to be explicit
that it doesn't solve the problem on its own. `ThreatField` already fades `att_eff / reach_turns` and
horizons at `THREAT_HORIZON = 6`, so "within K turns" is native (K = 6) — but railroads collapse *K*-turn
reach just as they collapse one-turn reach: on a railed board every fog pocket is reachable from every
enemy source, so "could a unit have gotten into that pocket" is vacuous and the question degenerates to a
proxy ("which pocket is nearest a big enemy source" — a tool). No-rail boards degenerate the other way
(reachability *is* the whole answer, again a proxy). **P1's engine-(a) content lives only in the middle
connectivity band**, and the fix is to **curate for that band**, not to restate the horizon:

- Admit an instance only when the map of *{enemy sources that could, within K turns, place a unit in the
  fog adjacent to candidate city C}* is **non-uniform across the candidates** — some candidates' threatening
  fog is feedable by a visible enemy source, at least one candidate's is **not**. That non-uniformity is the
  graded inference; if it's uniform (all-railed or all-isolated), drop the instance.
- **Tag every instance with its connectivity regime** (turn number, local rail/road density) and verify in
  post that accuracy isn't secretly a step function of rail density — if it is, the kind is a proxy in
  disguise and the band is mis-set.

**(2) Hindsight scoring — CHAMPION via the duality disciplines.** Apply v2.0 directly:
- **Reasoning-rejectable decoys (the fix the v1 draft missed).** v1's decoys were "fog nearby but
  empty" — i.e. wrong *by luck*, which is exactly the owner's complaint. Replace/augment them with
  **provably-safe decoys: a candidate city with no fogged tile inside attacker-strike-range at all** (its
  surroundings are visible-and-empty, or fog lies only beyond strike range). Such a city **cannot** be the
  answer, and the model can prove it *without seeing the fog*. Picking it is a reasoning blunder, not bad
  luck. This is luck-free credit and it directly answers the owner's honesty objection.
- The one genuinely-predictive candidate (has reachable fog that *might* hide the striker) is curated
  decisive + grounded (fall-prob ≥ `WIN_PROB_HIGH`, feeder source visible and in reach), so the residual
  luck is confined to a single axis and is small.
- **Scoring:** report **accuracy − max(fog-blind `ThreatField` baseline ≈ 0, nearest-visible-enemy proxy,
  any-adjacent-fog proxy)** at large N with a bootstrap CI. **Banded partial credit:** full for the true
  city; partial for a city that also carried a real (sub-decisive) hidden striker; **zero** for a
  provably-safe decoy. The provably-safe-decoy floor is the part that is *not* a prediction.

**Revised phrasing** (unchanged intent, K made explicit): *"…which ONE of your cities is most likely to
come under attack **within the next few turns** from an enemy unit you cannot currently see: {list}?"*

**Pipeline cost:** REQUIRES the unmasked-board pipeline (generator holds the true board for the
`ThreatField` ground truth while the model sees the masked one). See §v2.3.

### P2 — Concealment "are you seen?" → **DROP as framed; core folds into the hidden-force family** (N-family)

The owner is correct and I concede it fully. Computing visibility from the enemies you *can* see is a
tool task (`vision_radius_sq` disks), and v1's curation ("distractors are seen by a *visible* enemy source")
put that tool-trivial sub-task at the center while treating the one hard part — a **hidden** enemy scout
seeing you — as dismissable noise ("ceiling < 100%"). That is backwards: the noise *was* the reasoning.

There is no defense that keeps P2's shape without keeping the tool-trivial core, so I drop it. Its
salvageable idea — *reasoning about the opponent's information / hidden observers* — is the mirror of P1
(hidden enemy that can **strike** you) and belongs in the unified hidden-force family (**N1**, below:
"is your move exposed to a hidden enemy"). Nothing of value is lost; the redundant visibility bookkeeping is.

### P3 — Hidden-force localization → **CHAMPION** (engine a) — with one honesty caveat vs P1

Owner-endorsed as the most robust of the first three; I agree it is the canonical hidden-info kind and make
it the anchor of the family. Tightening: require the leader region beat the runner-up by a decisive summed
`att_eff` margin **and** be inferable (nearest a visible enemy source / on the enemy's own territory /
behind channelling terrain); report **lift over "biggest region" and "closest-to-visible-enemy" proxies**,
both of which curation can make wrong on a fraction of instances.

**PUSH-BACK (ranking, not the kind):** P3 is *less* scoring-honest than the salvaged P1, and I don't think
it should sit above it. The masking hides units that are **standing** on fogged tiles *regardless of how
they arrived*, so reach-grounding cannot manufacture a **provably-empty** region — a masked stack could
have been sitting there since before the tile fogged. P3 therefore has **no luck-free floor** (discipline
(1) barely applies); it is a purer prediction than P1, which *does* admit provably-safe decoys. P3 stays a
CHAMPION, but on the merits it and P1 are peers, with P1 slightly ahead on verifiability. Keep P3 only for
instances where visible cues make one region a defensible favorite; else drop at generation (v1's caveat
stands, reinforced).

**Pipeline cost:** REQUIRES the unmasked-board pipeline (sum masked-out enemy `att_eff` per region on the
true board). Shares all machinery with P1 — build once, get both (§v2.3).

### P4 — Robust forward posting → **CHAMPION, tightened** (engine b + adversarial)

Owner: seems good. Agreed, and it has a property the hidden-info kinds don't: the primary (omniscient)
variant needs **no** unmasked-board pipeline — it is engine (b), self-contained, board-free scorer. That
makes it the cheapest strong kind to ship. Tightening:

- Pin the two axes crisply: **pressure** = Σ `att_eff` of enemy units + Σ `city_capture_prob` of enemy
  cities the posting brings into *your* reach; **survival** = `1 − max` over the enemy's *reachable
  repositions* of their `win_probability` against your unit at the posted tile. The novelty is the
  **1-ply enemy-moves-to-exploit search** — a bounded reuse of the existing Dijkstra + `win_probability`,
  run from the enemy's post-move tiles, which the *static* `ThreatField` structurally under-reads.
- Score = Pareto non-dominated set → `Answer::ChoiceSet` (identical shape to t3-retreat); the blunder is a
  candidate decisively dominated on **both** axes.
- Keep the fogged variant as an explicit **harder** sibling that *additionally* earns engine (a) (the
  enemy's repositions are themselves uncertain) — but note it then inherits the unmasked-pipeline cost.

### P5 — Contested-frontier land grab → **MODIFY** (concede the collapse; re-found the axis) (engine b)

The owner's collapse argument is **correct and I concede it**: if the "contested" axis is *"can I claim
this before the enemy settler"*, and racing is free, then it is a **feasibility gate** (maximize value
subject to "I get there first"), not a Pareto axis — the trade-off structure dies. A pure race-margin axis
should not ship.

**PUSH-BACK (partial):** racing is *not* literally free, and the owner's "beat them by one turn → just do
it" hides three real costs — I list them because they're the concrete cases the owner asked for:
(i) **holdability** — a young city founded on the enemy's doorstep can be *claimed first and then taken*;
winning the settle race but losing the city is worse than a safe site; (ii) **the one-turn margin is
uncertain** — the enemy settler may be fog-hidden or its move rate/rails unknown, so "beat by one turn" is
not a knowable quantity, and the safe site's *certainty* has value; (iii) **escort/tempo opportunity cost**
— a settler + escort committed to a far contested race isn't founding the near safe site this turn.

But those costs argue for reframing the **axis**, not for keeping race-margin. The non-collapsing
formulation:

- Keep "can I plausibly claim it first" as a **hard GATE** that filters the candidate list (feasibility,
  where it belongs).
- Make the Pareto axis **expansion-denial value** — how much enemy *future* expansion founding here
  forecloses (enemy settler's reachable-unclaimed-sites *with* vs *without* my city). Denial trades against
  raw economy and post-founding exposure with **no canonical weight** → engine (b), same resistance as
  settle-site, non-collapsing.

**Honesty flag / at-risk:** if denial doesn't produce non-dominated instances *distinct from* what
settle-site's existing safety axis already captures, then P5 collapses **into** settle-site and should be
**dropped as redundant** rather than shipped as a near-duplicate. Medium confidence it clears that bar;
ranked accordingly (down from v1). Revised axis: replace the v1 `contested = −(enemy turns-to-claim)` term
with `denial = enemy-reachable-unclaimed-site count foreclosed`, plus the feasibility gate.

### P6 — Scouting priority → **DROP** (the duality forbids a good version)

The owner doubts the verification and so did v1; v2.0 explains *why it can't be rescued*, which is the
honest thing to report. Every version sits on one horn of the duality: (i) score against **realized**
contents → tool-resistant but pure luck (the owner's objection, and here with **no** decisive-margin
discipline to tame it, because value-of-information is diffuse); (ii) score a **soundness / possibility**
measure on the **masked** board ("reveal fog that could change a decision") → luck-free but **tool-trivial**
(the decision-relevant fog set is a reachability computation over the visible board). There is no third
door. I could not square it, so I **drop** it rather than ship a kind that is either dishonest or trivial.
A genuinely hard scouting question needs belief-state / multi-turn machinery (sequential info value) the
project does not have and this track is not adding.

### P7 — Multi-unit assault feasibility → **MODIFY: drop the omniscient (c) version, keep only the fogged reframe** (engine a, not c)

The owner is right that a tool solves the omniscient version "better than the LLM," and I **concede it
outright**: for realistic *k* (3–5 attackers, shallow stack) the ordering expectimax is *tractable*, so a
tool computes the exact capturability and the bound-agreement trick just makes it *cheap to score*, not
*hard to solve*. Engine (c) alone does **not** clear the filter here. The pure-omniscient P7 is **dropped**.

**PUSH-BACK — don't drop the idea, reshape it:** put the assault **under fog**, and it becomes genuinely
tool-resistant on engine (a), not (c). The enemy city's garrison is fog-hidden (last-known city shown,
defenders masked → a fog-blind tool reads "0 defenders → trivially takeable" and is **systematically wrong**
on every defended city). The model must judge capturability from **cues** — city size, last-known defender,
nearby visible enemy relief, terrain/walls — exactly the real decision "do I commit the stack without
knowing the garrison." The attacker-sequencing part stays tool-computable (your units are visible), but the
*defense* is hidden, so the **combination** is engine (a).

- **Scoring:** `Answer::Bool` capturability vs the **unmasked** garrison, with the same decisive band
  discipline (keep only instances where best-case-ordering odds and greedy-ordering odds agree — decisive
  yes/no — so the human-litigable middle is dropped). Report **lift over the fog-blind "undefended → yes"
  tool**, which is the whole point.
- **Honesty flag:** if the fogged version reduces in practice to "guess the garrison from city size," it's
  a thin prediction — rank it low, and it shares P1/P3's caveat that size is the only value/defense proxy
  decoded. Requires the unmasked-pipeline and the "fallen defender exposes the next" sequencing rule (v1's
  modeling note stands).

## v2.2 New proposals the critique inspired

The critique collapses P1, P2's core, P3, and P7-fogged into **one engine with four decision-framings**.
Presenting them as a family (rather than four ad-hoc kinds) is the main structural change v2 makes, and it
lets the pipeline cost be paid **once**.

### The Hidden-Force Estimation family (engine a) — unifies P1, P3, N1, P7-fogged

All four score a quantity that **depends on masked-out enemy occupants**, verified on the unmasked board,
under the v2.0 disciplines. They differ only in the decision they dress it as:

| Framing | Question | Own asset | Hidden quantity | Luck-free floor available? |
|---|---|---|---|---|
| **P1** defense | which of *my* cities is about to be hit | my cities | hidden **attacker** near a city | **yes** (provably-safe decoy: no fog in strike range) |
| **P3** intel | which fogged **region** hides the massed force | — | hidden **force magnitude** | weak (standing units defeat reach-grounding) |
| **N1** maneuver | is my **move** exposed to a hidden enemy | my moving unit | hidden **striker/observer** at the destination | **yes** (provably-safe destination: no fog in enemy strike range of it) |
| **P7f** offense | can I take their **fogged-garrison** city | my attacking stack | hidden **defender** strength | partial (last-known defender + size bound the tail) |

**N1 — Exposed-maneuver (P2's salvaged core, the owner's endorsed hard part):**
> *You are player {P}. You plan to move {unit} to {tile} this turn. Could an enemy you cannot currently see
> strike it there before you can react — or is that destination safe from any hidden enemy? Answer with the
> destination among {list} that is safe, or the one that is exposed* (framing chosen at generation).

N1 is the direct test of **hidden-enemy probability estimation** the owner called the genuinely interesting
core of P2 — but scored as a decision, with a **provably-safe-destination** decoy (no fogged tile within any
possible enemy's strike range of the destination → safe *by reasoning*, luck-free) as the honesty floor, and
a grounded, decisive hidden striker as the predictive candidate. Same machinery as P1 with the own-asset
being a *moving unit* instead of a city. Build it **after** P1 (it is P1's twin and reuses everything).

### v2.3 Pipeline cost — stated once for the whole family

P1, P3, N1, and P7-fogged **all break the "one masked board feeds both solver and encoder" seam**
(`fog-three-state-design.md` §4): the generator must compute **ground truth on the UNMASKED board** (where
the hidden occupants exist) while the model is rendered the **MASKED** board. Concretely the generator needs
to hold *both* boards and run the relevant solver (`ThreatField`, per-region `att_eff`, `win_probability`)
on the unmasked one. The raw + fogged boards are reconstructable from the save + `#fog(pN)` provenance in
`run.json`, and the viewer already exports both, so the **inputs exist** — the addition is a generation-time
code path that (a) keeps the unmasked board, (b) solves ground truth on it, (c) masks for render, and (d) an
invariant test that the *rendered* board never leaks a hidden occupant into the prompt. This is **one**
pipeline addition amortized across the whole family — a strong reason to build the family together rather
than one kind at a time. **P4 (omniscient) and P5 need none of this** — they are engine (b), self-contained,
and can ship on the current single-board seam.

## v2.4 Updated ranked table

| # | Kind | Verdict | Engine | Verifiability | Pipeline cost | Overall |
|---|---|---|---|---|---|---|
| P4 | Robust forward posting | **CHAMPION** (tightened) | (b) + adversarial | Pareto non-dominance, board-free | **none** (single-board) | **★ build first** |
| P1 | Surprise-strike exposure | **CHAMPION** (2 fixes) | (a) | lift@large-N + luck-free decoy floor | unmasked-board (shared) | **★ strong** |
| P3 | Hidden-force localization | **CHAMPION** | (a) | prediction-match, banded; no luck-free floor | unmasked-board (shared) | **★ strong** |
| N1 | Exposed-maneuver (P2 core) | **NEW** | (a) | lift@large-N + luck-free decoy floor | unmasked-board (shared) | strong |
| P7f | Fogged-garrison assault | **MODIFIED** (fogged only) | (a) | bounded Bool vs hidden garrison, lift | unmasked-board + seq. rule | worthwhile, costly |
| P5 | Contested land grab | **MODIFIED** (denial axis) | (b) | Pareto; at-risk of collapsing into settle-site | none | solid, at-risk |
| P2 | Concealment "are you seen?" | **DROPPED** | — | tool-trivial core | — | folded into N1 |
| P6 | Scouting priority | **DROPPED** | — | duality-forbidden | — | — |

## v2.5 What I'd actually build first, and why

1. **P4 (forward posting) — first, alone.** It is the only strong kind that needs **zero pipeline work**
   (engine b, single masked board, board-free `ChoiceSet` scorer just like t3-retreat). It also adds the
   genuinely new "let the enemy *move* before you evaluate" mechanic. Ship it to bank a win while the
   pipeline work is scoped.
2. **P1 + P3 together — second, sharing one pipeline addition.** Pay the unmasked-board seam cost **once**
   (§v2.3) and get both the defense-framed (P1, with its luck-free decoy floor) and intel-framed (P3)
   hidden-force kinds. This is where the eval gets the thing *nothing in the field has* — scoring spatial
   *information* reasoning — so it's worth the seam cost, but only after P4 has de-risked the track.
3. **N1 — third, nearly free once P1 exists** (its twin; reuses the same pipeline and the same
   provably-safe-decoy honesty trick). It is the owner's endorsed "hidden-enemy probability" core, done as a
   decision.

P7-fogged and P5 are **conditional**: build P7-fogged only if P1/P3 show the family scores cleanly (it adds
a sequencing rule on top); build P5 only if a quick generation probe shows the **denial** axis yields
non-dominated instances that settle-site's safety axis doesn't already cover — otherwise drop it as
redundant. P2 and P6 are **not** built: P2's only hard part is now N1, and P6 has no version that is
simultaneously honest and tool-resistant.

---

## v2.6 — addendum: N1 reconsidered, turn-history requirement, board-support

*The owner reviewed v2 and pushed hard on **N1** with three critiques: (1) it poses a probability as a
binary; (2) it needs the enemy's fastest unit type to be well-posed, which leaks omniscient info under fog;
(3) its real reasoning substrate is turn history, which the single-snapshot corpus doesn't carry. Two
standing directives came with it: turn-history is now a corpus requirement (future saves keep ~3 turns of
history), and per-kind board-support varies (a board can legitimately yield zero of a kind). Below is my
honest read — where the owner is right, where I push back — the revised verdict on N1, and the knock-on
revisions the critique forces on P1/P3/P7-fogged. The v2.0–v2.5 text above stands; this section supersedes
the **N1** entry in the family table (§v2.2) and the N1 row of the ranked table (§v2.4).*

### v2.6.1 Which critiques land — and how far they ripple

**Critique 1 (probability-as-binary): correct, and the fix is cheap.** N1 as written asks a per-destination
**binary** (safe / exposed), which thresholds away the very quantity — the chance a hidden striker reaches
the tile — that makes the question interesting. **PUSH-BACK on the implied remedy, though:** the answer is
*not* "make the model emit a calibrated probability." The whole eval is `Answer::Choice`/`ChoiceSet`, and
scoring calibration is a different, harder problem the track isn't taking on. The right fix is the shape P1
already uses and the owner already championed: an **argmax/argmin selection** over candidate destinations —
*"which destination is safest"* / *"which is most exposed"* — which renders the probabilistic quantity
**comparatively** (rank the tiles) without demanding a number, and imports the owner's graded intent as
**banded partial credit in scoring** (full credit for the true extreme, partial for a near-tie destination
that also carried real exposure, zero for the provably-clear decoy). So: adopt option (a) in **spirit**
(graded scoring) but keep the **selection** answer format, not a probability band. This alone resolves
critique 1.

**Critique 2 (needs the enemy's fastest unit → leaked speed): substantially correct, and it is NOT unique
to N1 — it also latently touches P1.** To judge whether a hidden unit can *reach and strike* a tile you
must assume a move rate; under fog the striker's type is exactly what you can't see, so supplying it leaks
omniscient state and withholding it under-determines reach. Two things soften but do not dissolve this:
- The game's **unit roster and its move rates are game-general public knowledge**, legitimately statable in
  the rules block (this is not board-specific hidden state). "The fastest land unit moves at most M" is fair
  to know. **But** — and this is the trap — reasoning *purely* worst-case ("could the fastest possible unit
  geometrically reach it") is a **reachability computation over the visible board → tool-trivial**, which
  collapses the engine-(a) content. So disclosing the roster fixes well-posedness but pushes the honest
  hardness *out of* geometry and *into* plausibility/grounding — i.e. straight into critique 3.
- **P1 carries the same latent dependence** (its "reachable within K turns from a visible source" grounding
  implicitly assumes a move rate). P1 is milder for two concrete reasons: its **K = 6 horizon** is generous
  enough that the exact rate rarely flips reachability, and P1's **visible enemy units disclose a sample of
  the enemy roster** (if you can see their cavalry, hidden-cavalry speed is a grounded assumption). N1's
  single-turn strike is a **knife-edge** exquisitely sensitive to the exact rate of a unit you cannot see —
  so the leak is materially sharper for N1 than for P1. This is a difference of degree that matters.

**Critique 3 (needs turn history): correct, and it is the decisive one for N1.** A human grounds a hidden
striker's *location* from where enemy units were seen in recent turns plus enemy city positions. The corpus
is single-snapshot, so N1's grounding must come from *current* cues alone — and for a **single-tile,
single-turn** strike that grounding is weak: a fog pocket adjacent to the destination can hide a striker
whether or not any visible source fed it, because a **standing** unit could have sat there since before the
tile fogged (this is exactly P3's "standing units defeat reach-grounding" pathology, and it hits N1 harder
because N1's target tile is a single knife-edge, not a summed region). Without history, N1's *predictive*
candidate is under-grounded → the score leans almost entirely on the luck-free **provably-clear-destination**
floor (reject the geometrically-safe decoy) with a near-coin-flip among the rest. That is a thin kind.

**The collapse insight (why there is no good history-free N1).** I looked for a history-free redesign that
stays engine-(a) hard, and there isn't one. Reframe N1 as a **soundness/robustness** question ("which
destination minimizes worst-case exposure given the known roster and visible sources") and it stops
depending on realized hidden contents — at which point it is either a **reachability tool** (duality trap)
or it **collapses into P4** (forward-posting soundness, engine b). The *only* way N1 is genuinely engine-(a)
is if it depends on realized hidden contents, and grounding that dependence **honestly** — above a coin
flip — needs history. So N1 is not salvageable as a distinct history-free engine-(a) kind. That is the crux.

### v2.6.2 Revised verdict on N1 — **DEFER (turn-history-gated) + REFRAME (selection), not DROP**

Weighing the owner's options: **(a) graded** — adopt in spirit (banded scoring) but not as a probability
emission; **(b) history-dependent defer** — yes, this is the load-bearing move; **(c) drop** — rejected, and
here is the push-back: dropping N1 discards the one kind that tests *"is my maneuver walking into an
ambush"* — the **tempo/exposure** dimension of an actively **moving** unit, which is strategically distinct
from P1 (a **static** city's defense) and from P4 (soundness against a **visible** enemy's reply). That
dimension is worth **deferring**, not killing. Net verdict:

> **N1 — Exposed-maneuver → DEFERRED, gated on turn-history; reframed as a selection.** Ship it only once
> the corpus carries history windows (§v2.6.3). When built, it is a **selection** among destinations, not a
> per-move binary, and not a probability emission.

**Revised phrasing** (selection form; roster disclosed in the rules block; requires history):
> *"You are player {P}. You may move {unit} to one of {tiles} this turn. Given the enemy units seen in the
> last few turns, their cities, and what the fog could hide, which destination is **safest** from a strike
> by an enemy you cannot currently see — or (framing chosen at generation) which is **most exposed**?"*

Honesty floor unchanged: a **provably-clear destination** (no fogged tile within any rostered enemy's
strike range of it → safe by pure reasoning, luck-free) is the decoy; a **history-grounded, decisive**
hidden striker (a unit *seen approaching* in the history window, now fog-hidden in strike range) is the
predictive candidate. Same unmasked-board machinery as P1, own-asset being a moving unit — but now with a
**history precondition P1 does not carry**.

### v2.6.3 Turn-history is now a first-class gate (upgrading v1 §4's decode gap)

v1 §4 listed "no turn history" as a **design constraint** ("kinds must be inferable from the current fogged
view alone"). The owner's directive **promotes it to a first-class blocker with a stated remediation**:
future save generation will retain the **~3 turns preceding** the presented turn (possibly by saving every
turn), so history-dependent kinds become buildable. Restated as a gate on the family:

| Kind | History dependence | Status under current (snapshot-only) corpus |
|---|---|---|
| **N1** maneuver | **required** — single-turn strike localization has no honest current-snapshot grounding | **BLOCKED until history ships** |
| **P3** intel | **strengthening, not required** — coarse summed-magnitude margin survives on static cues, but history is what separates "massed here (arrived, trackable)" from "always sat here" | buildable now; **materially more honest with history** |
| **P1** defense | **optional** — K=6 window + visible-source grounding stand on the snapshot; history would tighten the predictive candidate | buildable now as specified |
| **P7f** offense | **none** — the hidden quantity is a *static garrison* whose **location is known (the city)**; only its strength is hidden | buildable now; unaffected |

Implication for build order (§v2.5): **N1 moves out of "third" and behind the history-window corpus work.**
Until then the shippable hidden-force kinds are **P1 and P3** (P3 flagged as history-improvable), with
**P7-fogged** the cleanest of the family on this axis (below).

### v2.6.4 Knock-on revisions to P1 / P3 / P7-fogged

- **P1 — remains CHAMPION; add the speed-disclosure rule and a history footnote.** State the enemy roster /
  max move-rate in the rules block (public, not leaked) so "within a few turns" is well-posed; curate toward
  instances where a **visible** enemy unit discloses the relevant speed class so the grounding is cued, not
  assumed; note history is an available strengthener but **not** a precondition. No downgrade.
- **P3 — remains CHAMPION, but explicitly flagged history-hungry.** v2.1 already conceded P3 has "no
  luck-free floor" because standing units defeat reach-grounding — that concession *is* the history problem
  named. P3 survives on the snapshot only because it asks a **coarse, decisive-margin** quantity (which
  region holds the most summed `att_eff`), which tolerates grounding noise that N1's knife-edge does not.
  Record that turn-history would raise P3's honesty the most of any surviving kind, and keep P1 ranked
  slightly ahead of P3 (v2.1's ordering holds, now for a named reason: P1's grounding needs history *less*).
- **P7-fogged — UPGRADE its relative standing within the family.** The three critiques barely touch it: its
  hidden quantity is a **defender that stands in the city**, so there is **no reach/speed question**
  (critique 2 does not apply — the defender doesn't move to reach anything) and **no localization from
  history** (critique 3 does not apply — the location is the city, only strength is hidden). Critique 1 is
  handled already by the decisive-band curation (keep only instances where best-case and greedy orderings
  agree). So among the hidden-force kinds, **P7-fogged is the least information-leaky and the least
  history-dependent** — worth noting when scheduling, since it can proceed on the snapshot corpus without
  waiting on history, unlike N1.

**Net:** the critique does **not** collapse the family — it **sorts** it by grounding substrate. Static-
location hidden quantities (P7-fogged garrison; P1's decoy floor) are snapshot-honest; mobile-location
hidden quantities (N1's striker; P3's massed force) are history-hungry, N1 fatally so, P3 survivably so.

### v2.6.5 Board-support reality (confirming the owner's note)

Per-kind yield is **board-dependent and often zero — this is expected, not a bug.** Generators drop every
non-decisive instance (uniform-connectivity P1 instances, sub-margin P3 regions, ambiguous-middle P7f
assaults, non-Pareto P4/P5 candidate sets), so a board that lacks the required structure legitimately yields
**zero** of a kind. Consequences to bake into planning:
- The instance matrix must guarantee **enough *supporting* boards per kind**, not merely enough boards —
  coverage is per-(kind × board), and a kind's count can be sparse or empty on any given board.
- Kinds with the **narrowest** support windows (P1's middle-connectivity band; P5's denial-distinct-from-
  safety instances; P7f's decisive yes/no band) need the **widest** board sweep to hit target N — budget
  generation accordingly.
- A zero-yield board is **not** evidence a kind is broken; only a zero-yield *sweep across supporting
  boards* is. Keep the per-kind, per-board yield telemetry so the two are distinguishable.

### v2.6.6 Revised ranked-table delta (supersedes the N1 row of §v2.4)

| # | Kind | Verdict (v2.6) | Why changed |
|---|---|---|---|
| N1 | Exposed-maneuver | **DEFERRED — history-gated; reframed to selection** | single-turn strike has no honest snapshot grounding; binary → argmax; blocked until history ships |
| P1 | Surprise-strike exposure | **CHAMPION** (unchanged) — + roster-disclosure & history-optional notes | latent speed-dependence acknowledged and handled; grounding survives on snapshot |
| P3 | Hidden-force localization | **CHAMPION** (unchanged) — flagged **history-hungry** | shares N1's substrate but survives on coarse decisive margin; most improved by history |
| P7f | Fogged-garrison assault | **MODIFIED** (unchanged) — **relative standing up** within family | static garrison escapes both the speed-leak and history critiques; cleanest snapshot-honest hidden-force kind |
