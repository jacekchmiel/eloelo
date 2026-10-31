# Team role assignment

Each player in a match gets a role tag (Carry, Core or Support) that decides which heroes the Discord bot rolls
for them (`TaggedHeroPool`, roles strategy). How the roles are dealt is a `RoleAssignment`
(`src/eloelo/elodisco/hero_assignment_strategy/role_assignment.rs`). Both approaches share these rules:

- Each team is sorted by ELO. Players at the same rank in both teams form a pair and get the same role.
- In an uneven match the bigger team's lowest-ELO player has no pair: they are the extra player.
- Nothing is remembered between matches.

Compositions below are written Carry/Core/Support, e.g. 1/2/2 is 1 Carry, 2 Core and 2 Support.

## Ordered rotation (legacy)

1. Roles follow the rotation Core → Support → Carry → Core → …, one per pair.
2. The rotation is dealt to the pairs in a random order.
3. The extra player gets a uniformly random role.

A team's composition depends only on its size, e.g. 4 players are always 1/2/1 and 7 are always 2/3/2. The
extra player is always the lowest-ELO player, so in uneven matches the lowest-ELO players' role odds drift
towards 1/3 each.

## Team table

Goals, by priority:

1. Every team composition makes sense: no team without Core or Support, no role overloaded.
2. The extra player gets a role with odds similar to everyone else's.
3. Each player's role odds lean towards 1/3 each.

A table lists the compositions a team may have:

| Team size | Compositions | Also allowed against a team of one more player |
|---|---|---|
| 1 | 1/0/0, 0/1/0, 0/0/1 | |
| 2 | 1/0/1, 0/1/1 | |
| 3 | 1/1/1, 0/2/1 | |
| 4 | 1/2/1, 1/1/2 | 0/2/2 |
| 5 | 1/2/2 | |
| 6 | 2/2/2, 1/3/2, 1/2/3 | |
| 7 | 2/3/2, 2/2/3 | |
| 8 | 3/3/2, 3/2/3, 2/3/3 | |
| 9+ | the compositions for 3 fewer players, plus 1/1/1 | |

- Even match: both teams get the same composition, drawn from the table with equal odds, and its roles are
  dealt to the pairs in a random order.
- Uneven match (one extra player):
  1. The extra player's role is drawn with the role odds of a player in a team of the bigger size, e.g. 20%
     Carry, 40% Core, 40% Support against a team of 5.
  2. The smaller team's composition is drawn with equal odds among the table's compositions that, together
     with the extra role, make a composition of the bigger team's size. A team of 4 against 5 may also be
     0/2/2: the extra player, the lowest ELO of the match's 5-player team, is then the Carry.
  3. Roles the smaller team can't fit (e.g. Support in 2v3) are left out of step 1.
- Teams differing by more than one player aren't covered: the smaller team gets a composition from the table
  and every extra player a role with the bigger team's role odds, whatever the bigger team ends up with.

6-player teams have three compositions to make matches more diverse. It also lets the extra player in 5v6 get
any role, while the 5-player team stays 1/2/2.

## Odds

### Team compositions

| Match | Team | Ordered rotation (legacy) | Team table |
|---|---|---|---|
| 3v3 | both | 1/1/1 — 100.0% | 0/2/1 — 50.0%<br>1/1/1 — 50.0% |
| 3v4 | 3 players | 1/1/1 — 100.0% | 1/1/1 — 75.0%<br>0/2/1 — 25.0% |
| | 4 players | 1/1/2 — 33.3%<br>1/2/1 — 33.3%<br>2/1/1 — 33.3% | 1/2/1 — 62.5%<br>1/1/2 — 37.5% |
| 4v4 | both | 1/2/1 — 100.0% | 1/1/2 — 50.0%<br>1/2/1 — 50.0% |
| 4v5 | 4 players | 1/2/1 — 100.0% | 1/1/2 — 40.0%<br>1/2/1 — 40.0%<br>0/2/2 — 20.0% |
| | 5 players | 1/2/2 — 33.3%<br>1/3/1 — 33.3%<br>2/2/1 — 33.3% | 1/2/2 — 100.0% |
| 5v5 | both | 1/2/2 — 100.0% | 1/2/2 — 100.0% |
| 5v6 | 5 players | 1/2/2 — 100.0% | 1/2/2 — 100.0% |
| | 6 players | 1/2/3 — 33.3%<br>1/3/2 — 33.3%<br>2/2/2 — 33.3% | 1/2/3 — 38.9%<br>1/3/2 — 38.9%<br>2/2/2 — 22.2% |
| 6v6 | both | 2/2/2 — 100.0% | 1/2/3 — 33.3%<br>1/3/2 — 33.3%<br>2/2/2 — 33.3% |
| 6v7 | 6 players | 2/2/2 — 100.0% | 2/2/2 — 71.4%<br>1/2/3 — 14.3%<br>1/3/2 — 14.3% |
| | 7 players | 2/2/3 — 33.3%<br>2/3/2 — 33.3%<br>3/2/2 — 33.3% | 2/2/3 — 50.0%<br>2/3/2 — 50.0% |
| 7v7 | both | 2/3/2 — 100.0% | 2/2/3 — 50.0%<br>2/3/2 — 50.0% |
| 7v8 | 7 players | 2/3/2 — 100.0% | 2/2/3 — 50.0%<br>2/3/2 — 50.0% |
| | 8 players | 2/3/3 — 33.3%<br>2/4/2 — 33.3%<br>3/3/2 — 33.3% | 2/3/3 — 66.7%<br>3/2/3 — 16.7%<br>3/3/2 — 16.7% |
| 8v8 | both | 2/3/3 — 100.0% | 2/3/3 — 33.3%<br>3/2/3 — 33.3%<br>3/3/2 — 33.3% |

### Role odds per player

| Match | Ordered rotation (legacy) | Team table |
|---|---|---|
| 3v3 | everyone: 33.3% / 33.3% / 33.3% | everyone: 16.7% / 50.0% / 33.3% |
| 3v4 | everyone: 33.3% / 33.3% / 33.3% | #1–#3: 25.0% / 41.7% / 33.3%<br>#4: 25.0% / 41.5% / 33.5%<br>#5: 25.0% / 41.2% / 33.8%<br>#6: 25.0% / 40.5% / 34.5%<br>#7: 25.0% / 39.3% / 35.7% |
| 4v4 | everyone: 25.0% / 50.0% / 25.0% | everyone: 25.0% / 37.5% / 37.5% |
| 4v5 | #1–#4: 25.0% / 50.0% / 25.0%<br>#5: 25.1% / 49.9% / 25.1%<br>#6: 25.3% / 49.3% / 25.3%<br>#7: 26.0% / 48.0% / 26.0%<br>#8: 27.3% / 45.4% / 27.3%<br>#9: 29.6% / 40.7% / 29.6% | everyone: 20.0% / 40.0% / 40.0% |
| 5v5 | everyone: 20.0% / 40.0% / 40.0% | everyone: 20.0% / 40.0% / 40.0% |
| 5v6 | #1–#6: 20.0% / 40.0% / 40.0%<br>#7: 20.2% / 39.9% / 39.9%<br>#8: 20.6% / 39.7% / 39.7%<br>#9: 21.6% / 39.2% / 39.2%<br>#10: 23.6% / 38.2% / 38.2%<br>#11: 27.3% / 36.4% / 36.4% | #1–#7: 20.0% / 40.0% / 40.0%<br>#8: 20.1% / 39.9% / 39.9%<br>#9: 20.3% / 39.9% / 39.9%<br>#10: 20.6% / 39.7% / 39.7%<br>#11: 21.2% / 39.4% / 39.4% |
| 6v6 | everyone: 33.3% / 33.3% / 33.3% | everyone: 22.2% / 38.9% / 38.9% |
| 6v7 | everyone: 33.3% / 33.3% / 33.3% | everyone: 28.6% / 35.7% / 35.7% |
| 7v7 | everyone: 28.6% / 42.9% / 28.6% | everyone: 28.6% / 35.7% / 35.7% |
| 7v8 | #1–#8: 28.6% / 42.9% / 28.6%<br>#9–#10: 28.6% / 42.8% / 28.6%<br>#11: 28.7% / 42.7% / 28.7%<br>#12: 28.8% / 42.4% / 28.8%<br>#13: 29.2% / 41.7% / 29.2%<br>#14: 29.8% / 40.3% / 29.8%<br>#15: 31.1% / 37.8% / 31.1% | #1–#10: 28.6% / 35.7% / 35.7%<br>#11: 28.7% / 35.7% / 35.7%<br>#12: 28.8% / 35.6% / 35.6%<br>#13: 29.2% / 35.4% / 35.4%<br>#14: 29.8% / 35.1% / 35.1%<br>#15: 31.1% / 34.4% / 34.4% |
| 8v8 | everyone: 25.0% / 37.5% / 37.5% | everyone: 33.3% / 33.3% / 33.3% |

Player numbers in the second table rank all players of the match by ELO, #1 being the highest. Where everyone
has the same odds, it doesn't matter who ends up as the extra player.

The odds are exact, not sampled. Every split of the players into teams is assumed equally likely; it only
matters for how often a player is the extra player. In reality the team balancer decides the split.
