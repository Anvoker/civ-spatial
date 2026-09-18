# Coverage-optimized corpus selection

- N = **15** boards | distinct games = **15** | CAP 8 | band floor 3
- bands: {'early': 3, 'mid': 3, 'late': 9} | sizes: {'large': 5, 'medium': 5, 'small': 5}
- weakest discriminating kind: **settle-site = 10** (ceiling 60)

## Per-kind support: chosen / global ceiling

| kind | chosen | ceiling | | kind | chosen | ceiling |
|---|---|---|---|---|---|---|
| adv-assault-target | 15 | 127 | | nearest-owned | 15 | 216 |
| best-site | 15 | 216 | | reachable-nearest | 15 | 216 |
| cf-vacate | 12 | 79 | | settle-site | 10 | 60 |
| city-defense | 14 | 186 | | t3-retreat | 12 | 88 |
| compare-two-attacks | 12 | 104 | | t3-threat | 12 | 91 |
| constraint-site | 15 | 216 | | triage-reinforce | 12 | 58 |
| forward-posting | 12 | 74 | | unit-strength | 12 | 141 |

## Chosen boards

| # | board | band | size | aifill | seed | fog | leader | disc-kinds |
|---|---|---|---|---|---|---|---|---|
| 0 | corpus_large_a3_s42-T0220-Y01595-auto.sav | late | large | 3 | 42 | 0 | Mursilis | 10/10 |
| 1 | corpus_medium_a6_s1337-T0221-Y01600-final.sav | late | medium | 6 | 1337 | 1 | Atawallpa | 10/10 |
| 2 | corpus_medium_a6_s42-T0220-Y01595-auto.sav | late | medium | 6 | 42 | 5 | Ivan Grozny | 10/10 |
| 3 | corpus_large_a9_s1337-T0221-Y01600-final.sav | late | large | 9 | 1337 | 2 | K'uk B'alam | 10/10 |
| 4 | corpus_large_a6_s42-T0221-Y01600-final.sav | late | large | 6 | 42 | 0 | Mursilis | 10/10 |
| 5 | corpus_large_a3_s1337-T0200-Y01490-auto.sav | late | large | 3 | 1337 | 2 | K'uk B'alam | 10/10 |
| 6 | corpus_medium_a3_s42-T0221-Y01600-final.sav | late | medium | 3 | 42 | 2 | Rudolf II | 10/10 |
| 7 | corpus_small_a3_s1337-T0221-Y01600-final.sav | late | small | 3 | 1337 | 2 | K'uk B'alam | 10/10 |
| 8 | corpus_large_a6_s1337-T0160-Y01090-auto.sav | late | large | 6 | 1337 | 1 | Atawallpa | 10/10 |
| 9 | corpus_medium_a3_s1337-T0120-Y00380-auto.sav | mid | medium | 3 | 1337 | 1 | Atawallpa | 10/10 |
| 10 | corpus_small_a9_s42-T0140-Y00780-auto.sav | mid | small | 9 | 42 | 0 | Mursilis | 9/10 |
| 11 | corpus_small_a6_s42-T0140-Y00780-auto.sav | mid | small | 6 | 42 | 4 | FranÃ§ois Mitterrand | 9/10 |
| 12 | corpus_small_a6_s1337-T0040-Y-2050-auto.sav | early | small | 6 | 1337 | 1 | Atawallpa | 2/10 |
| 13 | corpus_small_a9_s1337-T0040-Y-2050-auto.sav | early | small | 9 | 1337 | 1 | Atawallpa | 2/10 |
| 14 | corpus_medium_a9_s1337-T0060-Y-1050-auto.sav | early | medium | 9 | 1337 | 6 | Maha Mongkut | 1/10 |
