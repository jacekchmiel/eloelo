# Small hero pools

How a player with a small hero pool (allowlist) changes the odds of the other players being shown their
heroes, with the roles strategy (see [hero-picks.md](hero-picks.md)).

## Setup

- 5v5, team table role assignment (every team is 1/2/2), everyone is shown 3 heroes.
- One player has a small pool; the 9 others have every hero except the global bans (Phantom Lancer,
  Medusa).
- Baseline: the same match with the small-pool player having every hero too.
- 1,000,000 simulated matches per case, run through `TaggedHeroPool` itself.

The **pair partner** is the player at the small-pool player's ELO rank in the other team. They always get
the same role and pick right before or after the small-pool player in every round, so they compete for the
same heroes. If the teams stay similar, it's the same person in most matches.

Numbers are the chance of being shown the hero in a match. The small pool doesn't change role odds, so the
chance of being shown a hero when dealt its role changes by the same ratio (it is about 2.5 times higher,
e.g. Jakiro as Support: 11.4% without, 4.0% for the partner, 10.0% for the others).

## Pool A: Sniper, Jakiro, Witch Doctor, Viper, Crystal Maiden

| Hero | Role | Without | Pair partner | Each of the other 8 | Small-pool player |
|---|---|---|---|---|---|
| Sniper | Carry | 1.2% | 0.3% | 1.2% | 19.9% |
| Jakiro | Support | 4.6% | 1.6% | 4.0% | 34.0% |
| Witch Doctor | Support | 3.7% | 1.0% | 3.2% | 35.6% |
| Viper | Core | 2.1% | 0.4% | 1.6% | 39.0% |
| Crystal Maiden | Support | 3.0% | 1.0% | 2.6% | 36.4% |
| Any of them | | 13.8% | 4.3% | 12.1% | |

## Pool B: Zeus, Lina, Crystal Maiden

| Hero | Role | Without | Pair partner | Each of the other 8 | Small-pool player |
|---|---|---|---|---|---|
| Zeus | Core | 1.7% | 0.6% | 1.3% | 45.7% |
| Lina | Core | 1.3% | 0.6% | 1.0% | 46.1% |
| Crystal Maiden | Support | 3.0% | 0.5% | 2.2% | 45.2% |
| Any of them | | 6.0% | 1.8% | 4.4% | |

Pool B has no Carry, so as a Carry (20%) the small-pool player gets one random hero of the three.

## Summary

- The pair partner is hit hard: about a third of their usual chance of seeing any of these heroes (Pool A
  4.3% instead of 13.8%, Pool B 1.8% instead of 6.0%).
- Everyone else loses little: about an eighth of their chance with Pool A (12.1% instead of 13.8%) and a
  quarter with Pool B (4.4% instead of 6.0%). The smaller the pool, the more often its heroes are taken.
