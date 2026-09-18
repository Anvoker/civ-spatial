# Root-cause: reach_turns "no modeled unit" / wrong-value bug on T677 (diagnosis only)

Date: 2026-08-01. Board: testcontroller_T677.sav masked to player2 (#fog(p2)).

## TL;DR
Bug = unit stacking + resolve-unit-by-coordinate-FIRST-match. NOT a fog bug, NOT a board-parity gap.
The operator sees the SAME board the expected answer was solved on.
- Root layer: civ-eval/src/encoders.rs:1729 unit_at() = board.units.iter().find(u.x==x && u.y==y) -> FIRST unit.
- Trigger: T677 end-game stacks up to 37 units on one tile (48,17); subject unit is buried in the stack.
- Expected answers are CORRECT (solver resolves subject BY ID, encoders.rs:49). Blast radius = operator + lossy render only.

## Three inconsistent resolutions at (48,17) [37 units]
- solver/expected answer: by id -> u3956 Mech. Inf. (subject) CORRECT
- render (what model sees): last-wins HashMap (encoders.rs:344) -> Alpine Troops
- operator reach_turns/att_eff/...: first-wins .find() (encoders.rs:1730) -> u3677 Freight

## Repro (verified via civ_core parse+mask, enumerating board.units)
tile (48,17): 37 units; subj u3956 Mech.Inf; first=u3677 Freight -> unit_stat None -> "no modeled unit"; truth 1
tile (29,11): 4;  subj u1131 Engineers; first=u3494 Spy -> None -> "no modeled unit"
tile (46,23): 7;  subj u2743 Mech.Inf; first=u3910 Spy -> None -> "no modeled unit"
tile (56,16): 6;  subj u2782 Mech.Inf; first=u3673 Freight -> None -> "no modeled unit"
tile (65,8):  5;  subj u3085 Mech.Inf (move 3); first=u3599 AEGIS Cruiser (move 5) -> operator 3, truth 5

Two signatures, one bug:
1. "no modeled unit": first stacked unit is unmodeled (Freight/Spy not in unit_stat, rules.rs:100). op_reach_turns
   does unit_at()? -> Some(Freight), then unit_stat(kind)? -> None; outer None rendered "no modeled unit" (encoders.rs:1999).
2. wrong value (u3085 got 3 vs 5): first unit modeled-but-different type; AEGIS move 5 -> 3 turns vs Mech.Inf move 3 -> 5.
   findings-enum-residual's "masked board differs from generation board" guess is WRONG; same board, wrong occupant.
   (Latent: op_reach_turns never checks unit class -> land-paths a Sea unit.)

Fog is incidental: FULL board has the same 37-stack at (48,17), Freight first; unfogged reach_turns fails identically.

## Blast radius
- EXPECTED ANSWERS: NOT corrupted. Solvers resolve by id; field/area solvers iterate the whole units vec. Prior fogged
  accuracy numbers stand; the operator artifact only cost the MODEL a bad tool result, it did not mis-score.
- reach_turns operator wrong on ANY stacked tile; same defect in att_eff/def_eff/garrison_defense (all call unit_at).
- Board render lossy: Occupants HashMap (encoders.rs:344) shows ONE unit/tile; model sees 1 of 37. Perception bug.
- reachable-nearest questions on stacked tiles are ill-posed for the model (named subject != rendered occupant).

## Why parity gate missed it
maximal_ops_parity_with_rules (encoders.rs:2850) runs on combat_strip() = ONE unit per tile. Never exercises a stack,
so first-wins vs by-id can't diverge. Gap is single-unit synthetic tiles vs stacked real boards (not unfogged vs fogged).

## Fix decision: DIAGNOSIS ONLY (not implemented)
Not "localized+clearly-correct": coordinate->unit is genuinely ambiguous under stacking (multiple modeled TYPES stack,
e.g. (56,16), (65,8)). A correct fix must set the coordinate->unit contract and make render+solver+operator agree,
spanning perception+generation+operator API. Corpus is NOT wrong, so this is broad-by-design-change, not broad-by-bad-truth.

Recommended (cheapest first):
1. Operators address units by ID (add unit_id param; unit_at -> unit_by_id). Matches how questions name the subject.
2. Render all stacked units (encoders.rs:361) or a count. Needed regardless.
3. Generator guard: reachable-nearest picks only sole-occupant/top subjects on dense boards (stop-gap).
4. Min: unit_at skips unmodeled types so reach_turns doesn't dead-end on Freight/Spy (PARTIAL, still wrong on multi-modeled stacks).

Test to add: extend operator-parity gate with a STACKED-tile mini-board (unmodeled first, modeled subject deeper, plus a
faster modeled unit); assert operator resolves the SUBJECT, not the first occupant. combat_strip() cannot see this.

## Pointers (maxops-enum worktree)
unit_at first-match encoders.rs:1729; op_reach_turns encoders.rs:1788; "no modeled unit" msg encoders.rs:1941/1949/1999;
render last-wins encoders.rs:344, tile render :361; solver by-id :49; ReachableNearestResource question.rs:742;
unit_stat table rules.rs:100; mask_to_known board.rs:198 (NOT the culprit); parity gate blind spot encoders.rs:2850.
