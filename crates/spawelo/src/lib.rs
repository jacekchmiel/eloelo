use std::collections::HashMap;
use std::time::Instant;

use eloelo_model::history::HistoryEntry;
use eloelo_model::player::{Player, PlayerWithElo};
use eloelo_model::PlayerId;

use itertools::Itertools;
use log::{debug, info};

mod options;

pub use options::SpaweloOptions;

// Learning rate is set to very high level to make the computation faster. Learning is not really 100% finished after 1000 iterations, but it gives good results, and blazing fast with this setting
const LEARNING_RATE: f64 = 5000.0;
const ML_ITERATIONS: usize = 5_000;

fn print_debug(i: usize, history: &[HistoryEntry], elo: &HashMap<PlayerId, f64>, elo_sum: f64) {
    let loss = loss(history, &elo);
    if i % 1000 == 0 || i == ML_ITERATIONS - 1 {
        debug!(
            "{}/{}, loss: {:.4}, elo_sum: {}",
            i + 1,
            ML_ITERATIONS,
            loss,
            elo_sum
        );
    }
}

pub fn ml_elo(history: &[HistoryEntry]) -> HashMap<PlayerId, f64> {
    let mut elo: HashMap<PlayerId, f64> = history
        .iter()
        .flat_map(|e| e.all_players())
        .map(|p| (p.clone(), Player::default_elo() as f64))
        .collect();

    if elo.is_empty() {
        return Default::default();
    }

    info!(
        "Calculating ELO from {} historic matches. Max iterations: {}",
        history.len(),
        ML_ITERATIONS
    );

    let start: Instant = Instant::now();
    for i in 0..ML_ITERATIONS {
        let elo_sum: f64 = elo.values().sum();
        print_debug(i, history, &elo, elo_sum);

        let derivative: HashMap<PlayerId, f64> = backpropagation(history, &elo);
        for (player, diff) in derivative {
            *elo.entry(player).or_default() += diff * LEARNING_RATE;
        }
    }

    log_elo(&elo);
    log_probabilities(&elo, history);
    info!("ELO calculations took {:?}", start.elapsed());

    elo
}

fn log_elo(elo: &HashMap<PlayerId, f64>) {
    let mut elo: Vec<_> = elo.into_iter().collect();
    elo.sort_by_key(|p| (*p.1 * 1000.0) as i64);
    debug!("Computed elo: {:?}", elo);
}

fn log_probabilities(elo: &HashMap<PlayerId, f64>, history: &[HistoryEntry]) {
    for entry in history {
        let winner_elo: f64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: f64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();

        let predicted_probability = win_probability(winner_elo, loser_elo);
        let real_probability = entry.advantage_factor();
        debug!(
            "Winner: {}, Loser: {}, Real probability: {:.4}, Predicted probability: {:.4}",
            winner_elo, loser_elo, real_probability, predicted_probability,
        );
    }
}

// L2 loss
fn loss(history: &[HistoryEntry], elo: &HashMap<PlayerId, f64>) -> f64 {
    let mut loss = 0.0;
    for entry in history {
        let winner_elo: f64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: f64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();

        let computed_probability = win_probability(winner_elo, loser_elo);
        let real_probablity = entry.advantage_factor();

        loss += (real_probablity - computed_probability).powf(2.0);
    }

    loss
}

fn backpropagation(
    history: &[HistoryEntry],
    elo: &HashMap<PlayerId, f64>,
) -> HashMap<PlayerId, f64> {
    let mut derivative = HashMap::new();
    for entry in history {
        let winner_elo: f64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: f64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();
        let elo_diff = winner_elo - loser_elo;

        let computed_probability = win_probability(winner_elo, loser_elo);
        let real_probability = entry.advantage_factor();

        // ((x-c)^2)' = 2*(x-c)
        // L2 loss
        let final_derivative = 2.0 * (real_probability - computed_probability);

        // https://www.wolframalpha.com/input?i=%281%2F%281%2B10%5E%28-x%2F400%29%29%29%27
        // -log(10)/(400 (1 + 10^(x/400))^2) + log(10)/(400 (1 + 10^(x/400)))
        let win_probability_derivative = final_derivative
            * (-10.0f64.ln() / (400.0 * (1.0 + 10.0f64.powf(elo_diff / 400.0)).powf(2.0))
                + 10.0f64.ln() / (400.0 * (1.0 + 10.0f64.powf(elo_diff / 400.0))));

        for p in &entry.winner {
            *derivative.entry(p.clone()).or_insert(0.0) += win_probability_derivative;
        }
        for p in &entry.loser {
            *derivative.entry(p.clone()).or_insert(0.0) -= win_probability_derivative;
        }
    }

    derivative
}

fn win_probability(winner_elo: f64, loser_elo: f64) -> f64 {
    let elo_diff = winner_elo as f64 - loser_elo as f64;
    1.0 / (1.0 + 10.0f64.powf(-elo_diff / 400.0))
}

#[derive(Debug, PartialEq, Eq)]
struct PlayerData {
    id: PlayerId,
    elo: i32,
    lose_streak: i32,
}

// TODO: Propagate applied lose streak info to returned values
pub fn shuffle_teams<'a>(
    players: impl IntoIterator<Item = PlayerWithElo>,
    lose_streaks: &HashMap<PlayerId, i32>,
    options: &SpaweloOptions,
) -> (i32, Vec<PlayerWithElo>, Vec<PlayerWithElo>) {
    let players: Vec<PlayerData> = players
        .into_iter()
        .map(|(id, elo)| {
            let lose_streak = lose_streaks.get(&id).copied().unwrap_or(0);
            PlayerData {
                id,
                elo,
                lose_streak,
            }
        })
        .collect();

    let team_size = players.len() / 2;
    let mut best_diff: i32 = players.iter().map(|p| p.elo).sum();
    let mut best_team: Vec<&PlayerData> = vec![];
    for team in players.iter().combinations(team_size) {
        let other_team: Vec<_> = players.iter().filter(|p| !team.contains(&p)).collect();

        let team_elo: i32 = team.iter().map(|p| p.elo).sum();
        let other_team_elo: i32 = other_team.iter().map(|p| p.elo).sum();

        let team_lose_streak = max_lose_streak_for_team(&team);
        let other_team_lose_streak = max_lose_streak_for_team(&other_team);
        // Positive diff means that "team" has a bigger lose streak
        let lose_streak_diff = team_lose_streak - other_team_lose_streak;

        // Apply lose streak "bonus" to the team with larger streak.
        let team_elo = if lose_streak_diff > 0 {
            apply_pity_bonus(team_elo, lose_streak_diff, options)
        } else {
            team_elo
        };
        let other_team_elo = if lose_streak_diff < 0 {
            apply_pity_bonus(other_team_elo, -lose_streak_diff, options)
        } else {
            other_team_elo
        };

        let diff = (team_elo - other_team_elo).abs();
        if diff < best_diff {
            best_diff = diff;
            best_team = team;
        }
    }

    let best_team: Vec<PlayerWithElo> = best_team
        .into_iter()
        .map(|p| (p.id.clone(), p.elo))
        .collect();
    let best_team_ids: Vec<&PlayerId> = best_team.iter().map(|p| &p.0).collect();
    let other_team = players
        .into_iter()
        .filter(|p| !best_team_ids.contains(&&p.id))
        .map(|p| (p.id.clone(), p.elo))
        .collect();

    (best_diff, best_team, other_team)
}

fn max_lose_streak_for_team<'a>(team: &[&PlayerData]) -> i32 {
    team.into_iter().map(|p| p.lose_streak).max().unwrap_or(0)
}

fn apply_pity_bonus(team_elo: i32, lose_streak: i32, options: &SpaweloOptions) -> i32 {
    if lose_streak < options.pity_bonus_min_loses {
        return team_elo;
    }
    let pity_loses = lose_streak - options.pity_bonus_min_loses;
    let pity_bonus_factor = options.pity_bouns_factor.powi(pity_loses);
    let new_elo = team_elo as f64 * (1.0 - pity_bonus_factor);
    new_elo as i32
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_winner_win_probability() {
        assert_eq!((win_probability(1100.0, 1000.0) * 100.0).round(), 64.0);
        assert_eq!((win_probability(1200.0, 1000.0) * 100.0).round(), 76.0);
    }
}
