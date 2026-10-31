# Hero picks

With the roles strategy (`TaggedHeroPool`), the Discord bot first deals every player a role (see
[role-assignment.md](role-assignment.md)), then picks heroes for them from their hero pool. A player's pool is
their allowlist, or every hero minus their bans, minus the global bans. Within a day of their last match it
usually also leaves out the heroes they were shown then.

- Every hero has one role, from `dota_heroes.csv`. A hero is **in role** when its role is the player's dealt
  role.
- Hero similarity comes from `hero_similarity_matrix.csv`: for each hero, every other hero ordered from the
  most to the least similar.
- Picks never change the dealt role. The Discord message and the match history show the dealt role, even when
  a player is shown heroes of another role.
- No hero is shown to two players in the same match. Below, pools only count **free** heroes: those no player
  holds yet.

## Assignment

Each player is shown up to their number of heroes shown (a bot setting per player), one hero per round. In
each round the pairs (players at the same ELO rank in both teams, see role-assignment.md) take turns, and
within a pair the team that picks first is random. The extra player of an uneven match picks alone.

Each pick:

1. An in-role hero from the player's pool:
   - If the other player of the pair already got an in-role hero this round: a random one of the 3 in-role
     heroes in the player's pool most similar to it. With a small pool these may not be very similar.
   - Otherwise: a random one.
2. Otherwise, if the player has no heroes yet: a random hero from their pool, of any role.
3. Otherwise nothing: they get no more heroes this match.

So:

- An out-of-role hero (step 2) never makes the other player of the pair pick something similar to it.
- A player is shown an out-of-role hero only as their first and only hero, when their pool has no free
  in-role hero.
- A player whose pool has fewer free in-role heroes than their number of heroes shown gets only those.

## Reroll

A reroll draws a new set of heroes for one player, the same number as shown, only from the in-role heroes of
their pool that are free. The player's own current heroes aren't free. There's no similarity check.

- If fewer free in-role heroes are left, the player gets only those.
- If none are left, the reroll fails and the player keeps their current heroes. The failed reroll still
  counts against the reroll limit.
- A reroll never shows an out-of-role hero, even to a player who got one in the assignment.
