//! Hidden-force family (P7f) — the unmasked-board seam and the fog-leak invariant, on a synthetic
//! board so the test is fast and does not depend on the gitignored corpus.
//!
//! Scenario: my Warriors are POISED at Chebyshev 3 from an enemy city whose tile is FOGGED (I
//! explored it earlier but no unit currently sees it), so its garrison — a Phalanx on the city
//! centre — is masked out of the board I see. A fog-blind reader sees a city with no visible
//! defenders and would call it takeable; the truth (scored on the UNMASKED board) is that the
//! garrison holds. The test asserts (1) P7f generates a decisive `Answer::Bool`, and (2) the
//! LEAK INVARIANT: the masked board the model renders from carries no hidden enemy garrison unit.

use std::collections::{BTreeSet, HashMap};

use civ_core::{Board, City, Player, Tile, Unit, Visibility};
use civ_eval::{generate_questions_fogged, Answer, Difficulty};

fn grass(x: i32, y: i32) -> Tile {
    Tile {
        x,
        y,
        terrain: "Grassland".to_string(),
        extras: BTreeSet::new(),
        owner: None,
    }
}

/// 8×3 all-Grassland board. Enemy city + Phalanx garrison at (6,1); my Warriors poised at (3,1)
/// (Chebyshev 3 — near enough to commit, far enough not to see the city with land sight sq=2). A
/// SECOND enemy Phalanx sits VISIBLE at (2,1) (sq-distance 1 from my Warriors) — the OBSERVED
/// repertoire that grounds the worst-case garrison now that the hidden garrison is not read.
fn scenario() -> Board {
    let (w, h) = (8i32, 3i32);
    let tiles: Vec<Vec<Tile>> = (0..h)
        .map(|y| (0..w).map(|x| grass(x, y)).collect())
        .collect();
    // Player 1 (me) has explored the whole board (so the city is Fogged, not Unexplored).
    let known: HashMap<i32, Vec<Vec<bool>>> =
        HashMap::from([(1, vec![vec![true; w as usize]; h as usize])]);
    Board {
        width: w,
        height: h,
        tiles,
        cities: vec![City {
            x: 6,
            y: 1,
            id: 100,
            name: "Redoubt".to_string(),
            owner: "Enemy".to_string(),
            size: 3,
            improvements: BTreeSet::new(),
        }],
        units: vec![
            // Hidden garrison (enemy Phalanx on the city centre) — masked out, NOT read.
            Unit {
                x: 6,
                y: 1,
                id: 200,
                kind: "Phalanx".to_string(),
                owner: "Enemy".to_string(),
                veteran: 0,
                hp: 10,
            },
            // VISIBLE enemy Phalanx (the observed repertoire the worst-case garrison is built from).
            Unit {
                x: 2,
                y: 1,
                id: 201,
                kind: "Phalanx".to_string(),
                owner: "Enemy".to_string(),
                veteran: 0,
                hp: 10,
            },
            // My storming unit, poised at Chebyshev 3.
            Unit {
                x: 3,
                y: 1,
                id: 300,
                kind: "Warriors".to_string(),
                owner: "Me".to_string(),
                veteran: 0,
                hp: 10,
            },
        ],
        players: vec![
            Player {
                id: 0,
                name: "Enemy".to_string(),
                nation: "French".to_string(),
                is_alive: true,
            },
            Player {
                id: 1,
                name: "Me".to_string(),
                nation: "Roman".to_string(),
                is_alive: true,
            },
        ],
        known,
        visibility: None,
        ruleset: Some("classic".to_string()),
        turn: Some(200),
        source: "synthetic-p7f".to_string(),
    }
}

#[test]
fn p7f_generates_decisive_bool_and_does_not_leak_the_garrison() {
    let unmasked = scenario();
    let masked = unmasked.mask_to_known(1).expect("mask to player 1");

    // Precondition: the city tile is genuinely FOGGED and its garrison is hidden on the masked board.
    assert!(
        masked.is_fogged(6, 1),
        "the enemy city tile must be fogged (garrison hidden)"
    );
    assert!(
        !masked.units.iter().any(|u| u.x == 6 && u.y == 1),
        "LEAK: the hidden garrison must not appear on the masked board the model sees"
    );
    assert!(
        masked.cities.iter().any(|c| c.id == 100),
        "the enemy city itself stays visible (last-known)"
    );
    // The observed repertoire: the VISIBLE enemy Phalanx at (2,1) IS on the masked board (it is what
    // the worst-case garrison is now derived from).
    assert!(
        masked.units.iter().any(|u| u.x == 2 && u.y == 1 && u.owner == "Enemy"),
        "the visible enemy Phalanx (observed repertoire) must be on the masked board"
    );

    // Generate with the unmasked board available (the §v2.3 seam), fogged to player "Me".
    let items = generate_questions_fogged(
        &masked,
        Some(&unmasked),
        7,
        4,
        Difficulty::hard(),
        Some("Me"),
    );
    let p7f: Vec<_> = items
        .iter()
        .filter(|it| it.question.category() == "fogged-assault")
        .collect();
    assert!(
        !p7f.is_empty(),
        "P7f must generate a decisive instance on this scenario"
    );
    for it in &p7f {
        assert!(
            matches!(it.answer, Answer::Bool(_)),
            "P7f answer must be a Bool"
        );
    }
    // Worst-case-robust label: the enemy's strongest VISIBLE defender type is Phalanx (def 2), so a
    // lone Warriors assault is scored against a synthetic green Phalanx on the city centre (def_eff
    // 2 x 1.5 = 3.0) and loses → decisive "no". This is exactly the case a fog-blind
    // "undefended -> yes" reader gets wrong, and it depends only on the OBSERVED repertoire (the
    // visible Phalanx), never on the true hidden garrison.
    assert!(
        p7f.iter().any(|it| it.answer == Answer::Bool(false)),
        "the worst-case visible defender type (Phalanx) should make at least one instance a decisive NO"
    );
}

/// Build (unmasked, masked) for a fully-explored board: `fogged(x,y)` marks a tile Fogged on the
/// masked board (else Visible); a `hidden` unit exists only on the unmasked board. Lets a test set
/// exactly the fog it wants without depending on the vision model.
fn two_boards(
    w: i32,
    h: i32,
    cities: Vec<City>,
    units: Vec<Unit>,
    fogged: impl Fn(i32, i32) -> bool,
    hidden: impl Fn(&Unit) -> bool,
) -> (Board, Board) {
    let tiles: Vec<Vec<Tile>> = (0..h)
        .map(|y| (0..w).map(|x| grass(x, y)).collect())
        .collect();
    let players = vec![
        Player {
            id: 0,
            name: "Enemy".to_string(),
            nation: "French".to_string(),
            is_alive: true,
        },
        Player {
            id: 1,
            name: "Me".to_string(),
            nation: "Roman".to_string(),
            is_alive: true,
        },
    ];
    let mk = |vis: Option<Vec<Vec<Visibility>>>, us: Vec<Unit>| Board {
        width: w,
        height: h,
        tiles: tiles.clone(),
        cities: cities.clone(),
        units: us,
        players: players.clone(),
        known: HashMap::from([(1, vec![vec![true; w as usize]; h as usize])]),
        visibility: vis,
        ruleset: Some("classic".to_string()),
        turn: Some(200),
        source: "synthetic".to_string(),
    };
    let unmasked = mk(None, units.clone());
    let vis: Vec<Vec<Visibility>> = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| {
                    if fogged(x, y) {
                        Visibility::Fogged
                    } else {
                        Visibility::Visible
                    }
                })
                .collect()
        })
        .collect();
    let masked_units: Vec<Unit> = units.into_iter().filter(|u| !hidden(u)).collect();
    let masked = mk(Some(vis), masked_units);
    (unmasked, masked)
}

fn city(x: i32, y: i32, id: i32, name: &str, owner: &str) -> City {
    City {
        x,
        y,
        id,
        name: name.to_string(),
        owner: owner.to_string(),
        size: 5,
        improvements: BTreeSet::new(),
    }
}

fn unit(x: i32, y: i32, id: i32, kind: &str, owner: &str) -> Unit {
    Unit {
        x,
        y,
        id,
        kind: kind.to_string(),
        owner: owner.to_string(),
        veteran: 0,
        hp: 10,
    }
}

// --- P1 SURPRISE-STRIKE: deducible, worst-case exposure from the MASKED board -----------------
// Redesign (`analysis/surprise-strike-redesign.md`): ground truth is a pure function of what the
// model sees — the scariest VISIBLE enemy's reach into in-range fog, weighted by closeness — NOT the
// true hidden striker's placement. These tests exercise: the deducibility invariant (moving a hidden
// unit cannot change the answer), the provably-safe decoy never wins, a positive case, and the
// decisive-margin drop. Several call the pub scorer `surprise_strike_answer` directly (masked-only,
// no RNG) so the scored truth is asserted deterministically.

use civ_eval::generators::surprise_strike_answer;

fn find_city<'a>(b: &'a Board, name: &str) -> &'a City {
    b.cities
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no city named {name}"))
}

/// The canonical exposed/safe scenario (the worked example in the redesign doc): a 24x5 all-Grass
/// board, fog covering x >= 19. My cities Haven (2,2) is interior (no fog in strike range → provably
/// safe) and Marches (18,2) has in-range fog a VISIBLE enemy Legion at (16,2) can reach. `striker`
/// places the truly-hidden enemy striker somewhere in the fog (masked out either way). Returns
/// (unmasked, masked).
fn exposed_safe_scenario(striker: (i32, i32)) -> (Board, Board) {
    let cities = vec![
        city(2, 2, 10, "Haven", "Me"),
        city(18, 2, 11, "Marches", "Me"),
    ];
    let units = vec![
        unit(16, 2, 20, "Legion", "Enemy"),                 // VISIBLE: grounds the exposure
        unit(striker.0, striker.1, 21, "Legion", "Enemy"),  // hidden striker in the fog
    ];
    two_boards(
        24,
        5,
        cities,
        units,
        |x, _y| x >= 19,
        |u| u.owner == "Enemy" && u.x >= 19, // only the in-fog striker is masked out
    )
}

#[test]
fn p1_answer_is_deducible_moving_the_hidden_striker_does_not_change_it() {
    // The invariant that fixes the luck: exposure is computed from the MASKED board alone, so the
    // hidden striker's position is irrelevant. Two placements of the hidden striker on DIFFERENT
    // fogged tiles must yield byte-identical masked boards and the same scored answer.
    let (_, masked_a) = exposed_safe_scenario((22, 2));
    let (_, masked_b) = exposed_safe_scenario((23, 3));

    // Both hidden strikers are on fogged tiles and absent from the masked board (leak invariant).
    for (m, s) in [(&masked_a, (22, 2)), (&masked_b, (23, 3))] {
        assert!(m.is_fogged(s.0, s.1), "the striker's tile must be fogged");
        assert!(
            !m.units.iter().any(|u| u.x == s.0 && u.y == s.1),
            "LEAK: the hidden striker must not appear on the masked board"
        );
    }
    // The masked boards are identical regardless of where the hidden unit sits (the VISIBLE Legion
    // at (16,2) is the only enemy on either).
    assert_eq!(
        masked_a.units, masked_b.units,
        "moving a hidden unit must not change the masked board's units"
    );

    let cs_a = [find_city(&masked_a, "Haven"), find_city(&masked_a, "Marches")];
    let cs_b = [find_city(&masked_b, "Haven"), find_city(&masked_b, "Marches")];
    let ans_a = surprise_strike_answer(&masked_a, "Me", &cs_a);
    let ans_b = surprise_strike_answer(&masked_b, "Me", &cs_b);
    assert_eq!(
        ans_a, ans_b,
        "the answer must be invariant to the hidden striker's position (deducible from masked board)"
    );
    // And it is a decisive Choice for the observably-exposed city.
    assert_eq!(
        ans_a,
        Some(Answer::Choice {
            value: "Marches".to_string(),
            options: vec!["Haven".to_string(), "Marches".to_string()],
        }),
        "the observably-exposed city (visible-enemy-reachable fog nearby) is the answer"
    );
}

#[test]
fn p1_provably_safe_decoy_is_never_the_answer() {
    let (_, masked) = exposed_safe_scenario((22, 2));
    // Haven has NO fogged tile within strike range → provably safe; it must never be picked, and it
    // must still be OFFERED as an option (the luck-free decoy floor).
    let cities = [find_city(&masked, "Haven"), find_city(&masked, "Marches")];
    match surprise_strike_answer(&masked, "Me", &cities) {
        Some(Answer::Choice { value, options }) => {
            assert_ne!(value, "Haven", "the provably-safe decoy is never the answer");
            assert!(
                options.contains(&"Haven".to_string()),
                "the provably-safe decoy is offered"
            );
        }
        other => panic!("expected a decisive Choice, got {other:?}"),
    }
}

#[test]
fn p1_generates_and_picks_the_exposed_city_over_the_safe_one() {
    // The full generator path (masked-only ground truth) still produces the kind, with the exposed
    // city as the answer and the provably-safe decoy offered.
    let (unmasked, masked) = exposed_safe_scenario((22, 2));
    let items = generate_questions_fogged(
        &masked,
        Some(&unmasked),
        7,
        6,
        Difficulty::hard(),
        Some("Me"),
    );
    let p1: Vec<_> = items
        .iter()
        .filter(|it| it.question.category() == "surprise-strike")
        .collect();
    assert!(!p1.is_empty(), "P1 must generate on this scenario");
    for it in &p1 {
        match &it.answer {
            Answer::Choice { value, options } => {
                assert_eq!(value, "Marches", "the exposed city is the answer");
                assert!(
                    options.contains(&"Haven".to_string()),
                    "the provably-safe decoy is offered"
                );
            }
            other => panic!("P1 answer must be a Choice, got {other:?}"),
        }
    }
}

#[test]
fn p1_dropped_when_two_exposed_cities_are_a_decisive_near_tie() {
    // Two cities with MIRROR-IMAGE observable geometry: each has an adjacent fogged tile reachable by
    // its own visible Legion, so their exposures are equal (ratio 1.0 < the 1.25x margin). With a safe
    // decoy present but no decisive leader, the instance is DROPPED (no answer).
    let cities = vec![
        city(5, 2, 10, "West", "Me"),
        city(25, 2, 11, "East", "Me"),
        city(15, 2, 12, "Middle", "Me"), // safe decoy: no fog within reach
    ];
    let units = vec![
        unit(4, 2, 20, "Legion", "Enemy"),  // reaches the fog just east of West
        unit(26, 2, 21, "Legion", "Enemy"), // reaches the fog just west of East (mirror)
    ];
    let (_, masked) = two_boards(
        30,
        5,
        cities,
        units,
        |x, _y| (6..=9).contains(&x) || (21..=24).contains(&x),
        |_u| false, // both Legions are VISIBLE; nothing hidden
    );
    // Sanity: Middle really is provably safe (no fog within Chebyshev 5 of (15,2)).
    assert!(
        !masked.is_fogged(15, 2) && !(10..=20).any(|x| masked.is_fogged(x, 2)),
        "Middle must have no fog within strike range"
    );
    let cities = [
        find_city(&masked, "West"),
        find_city(&masked, "East"),
        find_city(&masked, "Middle"),
    ];
    assert_eq!(
        surprise_strike_answer(&masked, "Me", &cities),
        None,
        "a decisive near-tie between the two exposed cities must be dropped"
    );
}

#[test]
fn p3_localizes_the_fogged_region_hiding_the_massed_force() {
    // A mostly-visible board with ONE fog pocket (x in 10..=13) hiding a two-Legion stack; the rest
    // of the board is visible, giving provably-empty decoy regions.
    let units = vec![
        unit(11, 2, 30, "Legion", "Enemy"),
        unit(12, 2, 31, "Legion", "Enemy"),
    ];
    let (unmasked, masked) = two_boards(
        24,
        5,
        vec![],
        units,
        |x, _y| (10..=13).contains(&x),
        |u| u.owner == "Enemy",
    );
    let items = generate_questions_fogged(
        &masked,
        Some(&unmasked),
        7,
        6,
        Difficulty::hard(),
        Some("Me"),
    );
    let p3: Vec<_> = items
        .iter()
        .filter(|it| it.question.category() == "hidden-force")
        .collect();
    assert!(!p3.is_empty(), "P3 must generate on this scenario");
    for it in &p3 {
        assert!(
            matches!(it.answer, Answer::Choice { .. }),
            "P3 answer must be a Choice over region centers"
        );
    }
}

// --- P7f OBSERVED-REPERTOIRE worst-case scoring ----------------------------------------------
// The label is a pure function of the MASKED board: the assault is scored against a synthetic green
// unit of the city owner's strongest VISIBLE defender TYPE on the city centre (`worst_case_garrison`
// on the masked board), NOT the true hidden garrison AND NOT any enemy type that exists only on fogged
// tiles. These scenarios exercise beats-the-worst-case -> yes, loses-to-it -> no, near-tie -> dropped,
// the deducibility invariant (a fogged defender never changes the answer), and the leak-closing flip
// (a strong type present only in fog is ignored). All keep the leak invariant: no hidden enemy unit
// appears on the masked board.

/// Build (unmasked, masked) for a fogged-assault scenario on a 20x5 all-Grassland board: an enemy city
/// at (10,2) whose tile is FOGGED, a hidden garrison unit on the city centre, my single attacker
/// poised at (7,2) (Chebyshev 3), plus any extra enemy field units. Enemy units standing in the fogged
/// band (x in 9..=11) are MASKED OUT (invisible — the realistic fog rule); enemy units OUTSIDE it are
/// VISIBLE and form the observed repertoire the worst-case garrison is derived from.
fn fogged_assault_boards(
    garrison_kind: &str,
    attacker_kind: &str,
    extra_enemy: Vec<Unit>,
) -> (Board, Board) {
    let cities = vec![city(10, 2, 100, "Redoubt", "Enemy")];
    let mut units = vec![
        unit(10, 2, 200, garrison_kind, "Enemy"), // hidden garrison on the fogged city centre
        unit(7, 2, 300, attacker_kind, "Me"),     // my storming unit, Chebyshev 3
    ];
    units.extend(extra_enemy);
    let (unmasked, masked) = two_boards(
        20,
        5,
        cities,
        units,
        |x, _y| (9..=11).contains(&x),
        |u| u.owner == "Enemy" && (9..=11).contains(&u.x),
    );
    assert!(masked.is_fogged(10, 2), "the enemy city tile must be fogged");
    assert!(
        !masked.units.iter().any(|u| u.x == 10 && u.y == 2),
        "LEAK: the hidden garrison must not appear on the masked board"
    );
    (unmasked, masked)
}

/// The generated fogged-assault items for a [`fogged_assault_boards`] scenario. Passes `None` for the
/// unmasked board on purpose: the redesigned kind reads ONLY the masked board, so the unmasked truth
/// is not needed to score it.
fn fogged_assault_items(
    garrison_kind: &str,
    attacker_kind: &str,
    extra_enemy: Vec<Unit>,
) -> Vec<civ_eval::EvalItem> {
    let (_, masked) = fogged_assault_boards(garrison_kind, attacker_kind, extra_enemy);
    generate_questions_fogged(&masked, None, 7, 4, Difficulty::hard(), Some("Me"))
        .into_iter()
        .filter(|it| it.question.category() == "fogged-assault")
        .collect()
}

#[test]
fn p7f_yes_when_stack_beats_the_worst_case_defender() {
    // The enemy's only VISIBLE combat unit is a Warriors (def 1) → worst-case garrison is a green
    // Warriors (def_eff 1.5). My Catapult (att 6, full HP at 10) overruns it (p ~ 0.8) → decisive YES.
    // The hidden Warriors garrison itself is NOT read.
    let items = fogged_assault_items(
        "Warriors",
        "Catapult",
        vec![unit(15, 2, 400, "Warriors", "Enemy")],
    );
    assert!(!items.is_empty(), "a decisive instance must generate");
    assert!(
        items.iter().all(|it| it.answer == Answer::Bool(true)),
        "beating the worst-case VISIBLE defender type must score YES"
    );
}

#[test]
fn p7f_no_when_stack_loses_to_the_worst_case_visible_type() {
    // The hidden garrison is only a Warriors, but the enemy FIELDS an Alpine Troops (def 5) VISIBLY at
    // (15,2), so the observed worst case is a green Alpine on the city centre (def_eff 7.5). My lone
    // Warriors (att 1) cannot take that → decisive NO. A fog-blind reader (or the old true-garrison
    // score) would have called the weak garrison takeable.
    let items = fogged_assault_items(
        "Warriors",
        "Warriors",
        vec![unit(15, 2, 400, "Alpine Troops", "Enemy")],
    );
    assert!(!items.is_empty(), "a decisive instance must generate");
    assert!(
        items.iter().all(|it| it.answer == Answer::Bool(false)),
        "losing to the worst-case VISIBLE defender type must score NO"
    );
}

#[test]
fn p7f_dropped_when_odds_are_a_near_tie() {
    // The enemy's strongest VISIBLE type is Phalanx (def 2) at (15,2) → worst case def_eff 3.0. My
    // Chariot has att_eff 3.0 and equal HP/firepower, so the capture probability is exactly 0.5 —
    // inside the non-decisive band — and the instance is DROPPED (no item generated).
    let items = fogged_assault_items(
        "Phalanx",
        "Chariot",
        vec![unit(15, 2, 400, "Phalanx", "Enemy")],
    );
    assert!(
        items.is_empty(),
        "a near-tie (p = 0.5) must be dropped, not labeled"
    );
}

#[test]
fn p7f_answer_is_deducible_moving_a_fogged_defender_does_not_change_it() {
    // The deducibility invariant for fogged-assault: the yes/no is a pure function of the MASKED board,
    // so placing/moving enemy defenders onto FOGGED tiles cannot change it. Both scenarios share the
    // same VISIBLE repertoire (one Warriors at (15,2)) but differ entirely in their hidden defenders.
    let (_, masked_a) = fogged_assault_boards(
        "Phalanx",
        "Catapult",
        vec![unit(15, 2, 400, "Warriors", "Enemy")],
    );
    let (_, masked_b) = fogged_assault_boards(
        "Musketeers", // a different, stronger hidden garrison ...
        "Catapult",
        vec![
            unit(15, 2, 400, "Warriors", "Enemy"),       // same visible repertoire
            unit(9, 2, 401, "Alpine Troops", "Enemy"),   // ... plus strong defenders, but in the fog
            unit(11, 2, 402, "Musketeers", "Enemy"),
        ],
    );
    // The masked boards are byte-identical in their units: only the visible Warriors survives masking
    // in both, regardless of what is hidden in the fog.
    assert_eq!(
        masked_a.units, masked_b.units,
        "hidden (fogged) defenders must not change the masked board the model sees"
    );
    let answer = |m: &Board| {
        generate_questions_fogged(m, None, 7, 4, Difficulty::hard(), Some("Me"))
            .into_iter()
            .find(|it| it.question.category() == "fogged-assault")
            .map(|it| it.answer)
    };
    let (a, b) = (answer(&masked_a), answer(&masked_b));
    assert_eq!(
        a, b,
        "moving/adding a fogged defender must not change the fogged-assault answer"
    );
    // And it is the decisive YES a reasoner should give: the Catapult beats the observable worst case
    // (a green Warriors), independent of the far stronger units actually hidden in scenario B.
    assert_eq!(
        a,
        Some(Answer::Bool(true)),
        "the answer is decided by the observed repertoire (a green Warriors), not the hidden truth"
    );
}

#[test]
fn p7f_ignores_a_strong_enemy_type_that_exists_only_in_fog() {
    // The leak-closing flip: the enemy's strongest defender (Alpine Troops, def 5) exists ONLY on a
    // fogged tile, and no enemy combat unit is visible. The observed-repertoire worst case therefore
    // sees NO defender → an undefended city → certain capture → decisive YES for my lone Warriors.
    // The OLD unmasked scan would have found the fog-only Alpine and scored NO — a type the model has
    // no evidence exists. That aleatoric channel is now closed.
    let items = fogged_assault_items(
        "Warriors",
        "Warriors",
        vec![unit(9, 2, 400, "Alpine Troops", "Enemy")], // strong, but in the fog band → invisible
    );
    assert!(!items.is_empty(), "a decisive instance must generate");
    assert!(
        items.iter().all(|it| it.answer == Answer::Bool(true)),
        "a strong enemy type present only in fog must be ignored (worst case = undefended → YES)"
    );
}
