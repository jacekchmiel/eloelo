use std::borrow::Borrow;
use std::collections::HashMap;
use std::time::Instant;

use eloelo_model::history::HistoryEntry;
use eloelo_model::player::{Player, PlayerWithElo};
use eloelo_model::PlayerId;

use itertools::Itertools;
use log::{debug, info};
use rand::distributions::Uniform;
use rand::prelude::Distribution;
use rand::seq::IteratorRandom;

// Calculating ELO in debug mode takes too much time...
#[cfg(debug_assertions)]
const ML_ITERATIONS: usize = 50_000;

#[cfg(not(debug_assertions))]
const ML_ITERATIONS: usize = 50_000;

const PERFECT_PREDICTION_TARGET: f64 = 0.5;

fn print_debug(i: usize, history: &[HistoryEntry], elo: &HashMap<&PlayerId, i64>, elo_sum: i64) {
    let prob = probability_of_perfect_prediction(history, &elo);
    if i != 0 && (i % 1000 == 0 || i == ML_ITERATIONS - 1) {
        debug!(
            "{}/{}, probability_of_perfect_prediction: {:.4}, elo_sum: {}",
            i + 1,
            ML_ITERATIONS,
            prob,
            elo_sum
        );
    }
}

fn should_stop_iteration(
    i: usize,
    history: &[HistoryEntry],
    elo: &HashMap<&PlayerId, i64>,
) -> bool {
    if i % 1000 != 1 {
        return false;
    }

    let perfect_prediction = probability_of_perfect_prediction(history, elo);
    let probability_good_enough = perfect_prediction > PERFECT_PREDICTION_TARGET;
    if probability_good_enough {
        info!("STOP: Score is good enough: {:.2}", perfect_prediction);
    }
    probability_good_enough
}

pub fn ml_elo(
    history: &[HistoryEntry],
    initial_elo_data: &HashMap<PlayerId, i32>,
) -> HashMap<PlayerId, i64> {
    let mut elo: HashMap<&PlayerId, i64> = history
        .iter()
        .flat_map(|e| e.all_players())
        .map(|p| {
            (
                p,
                initial_elo_data
                    .get(p)
                    .copied()
                    .unwrap_or(Player::default_elo()) as i64,
            )
        })
        .collect();
    if elo.is_empty() {
        return Default::default();
    }

    info!(
        "Calculating ELO from {} historic matches. Max iterations: {}",
        history.len(),
        ML_ITERATIONS
    );

    let start = Instant::now();
    let mut scores = vec![];
    for i in 0..ML_ITERATIONS {
        let elo_sum: i64 = elo.values().sum();
        let noisy_history = add_noise(history);

        print_debug(i, history, &elo, elo_sum);

        let mut rng = rand::thread_rng();
        let rand_player = elo.keys().choose(&mut rng).unwrap();
        let score_adj = Uniform::new_inclusive(-30, 30).sample(&mut rng);
        let mut new_elo = elo.clone();
        *new_elo.get_mut(rand_player).unwrap() += score_adj;

        let old_score =
            probability_of_perfect_prediction(&noisy_history, &elo) * regularization_penalty(&elo);
        let new_score = probability_of_perfect_prediction(&noisy_history, &new_elo)
            * regularization_penalty(&new_elo);
        if new_score > old_score {
            elo = new_elo;
        }
        scores.push(new_score);

        if should_stop_iteration(i, history, &elo) {
            break;
        }
    }
    log_elo(&elo);
    log_probabilities(&elo, history);
    info!("ELO calculations took {:?}", start.elapsed());
    elo.into_iter().map(|i| (i.0.clone(), i.1)).collect()
}

fn log_elo(elo: &HashMap<&PlayerId, i64>) {
    let mut elo: Vec<_> = elo.into_iter().collect();
    elo.sort_by_key(|p| p.1);
    debug!("Computed elo: {:?}", elo);
}

fn log_probabilities(elo: &HashMap<&PlayerId, i64>, history: &[HistoryEntry]) {
    for entry in history {
        let winner_elo: i64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: i64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();

        let probability = win_probability(winner_elo, loser_elo);
        debug!(
            "Winner: {}, Loser: {}, Probability: {:.4}",
            winner_elo, loser_elo, probability
        );
    }
}

fn add_noise(history: &[HistoryEntry]) -> Vec<HistoryEntry> {
    if rand::random::<f32>() > 0.2 {
        return history.into_iter().cloned().collect();
    }
    let alter_i = Uniform::new(0, history.len()).sample(&mut rand::thread_rng());
    let altered_item = HistoryEntry {
        timestamp: history[alter_i].timestamp,
        winner: history[alter_i].loser.clone(),
        loser: history[alter_i].winner.clone(),
    };

    let mut altered_history: Vec<HistoryEntry> = Vec::new();
    altered_history.extend(history.iter().take(alter_i).cloned());
    altered_history.push(altered_item);
    altered_history.extend(history.iter().skip(alter_i + 1).cloned());

    altered_history
}

fn probability_of_perfect_prediction(
    history: &[HistoryEntry],
    elo: &HashMap<impl Borrow<PlayerId> + Eq + std::hash::Hash, i64>,
) -> f64 {
    let mut probability = 1.0f64;

    for entry in history {
        let winner_elo: i64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: i64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();
        probability *= win_probability(winner_elo, loser_elo)
    }

    probability
}

fn regularization_penalty(elo: &HashMap<impl Borrow<PlayerId> + Eq + std::hash::Hash, i64>) -> f64 {
    elo.len() as f64 / elo.values().sum::<i64>() as f64
}

fn win_probability(winner_elo: i64, loser_elo: i64) -> f64 {
    let elo_diff = winner_elo as f64 - loser_elo as f64;
    1.0f64 / (1.0f64 + 10.0f64.powf(-elo_diff / 400.0f64))
}

pub fn shuffle_teams<'a>(
    players: impl IntoIterator<Item = PlayerWithElo>,
) -> (i32, Vec<PlayerWithElo>, Vec<PlayerWithElo>) {
    let players: Vec<_> = players.into_iter().collect();
    let team_size = players.len() / 2;
    let mut best_diff: i32 = players.iter().map(|p| p.1).sum();
    let mut best_team: Vec<&PlayerWithElo> = vec![];
    for team in players.iter().combinations(team_size) {
        let team_elo: i32 = team.iter().map(|p| p.1).sum();
        let other_team_elo: i32 = players
            .iter()
            .filter(|p| !team.contains(&p))
            .map(|p| p.1)
            .sum();
        let diff = (team_elo - other_team_elo).abs();
        if diff < best_diff {
            best_diff = diff;
            best_team = team;
        }
    }

    let best_team: Vec<PlayerWithElo> = best_team.into_iter().map(|p| (p.0.clone(), p.1)).collect();
    let other_team = players
        .into_iter()
        .filter(|p| !best_team.contains(&p))
        .collect();

    (best_diff, best_team, other_team)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_winner_win_probability() {
        assert_eq!((win_probability(1100, 1000) * 100.0).round(), 64.0);
        assert_eq!((win_probability(1200, 1000) * 100.0).round(), 76.0);
    }
}
