use std::collections::HashMap;
use std::time::Instant;

use eloelo_model::history::HistoryEntry;
use eloelo_model::player::{Player, PlayerWithElo};
use eloelo_model::PlayerId;

use itertools::Itertools;
use log::{debug, info};

// Learning rate is set to very high level to make the computation faster. Learning is not really 100% finished after 1000 iterations, but it gives good results, and blazing fast with this setting
const LEARNING_RATE: f64 = 100_000.0;
const ML_ITERATIONS: usize = 1_000;

fn print_debug(i: usize, history: &[HistoryEntry], elo: &HashMap<PlayerId, f64>, elo_sum: f64) {
    let loss = loss(history, &elo);
    if i % 100 == 0 || i == ML_ITERATIONS - 1 {
        debug!(
            "{}/{}, loss: {:.4}, elo_sum: {}",
            i+1,
            ML_ITERATIONS,
            loss,
            elo_sum
        );
    }
}

pub fn ml_elo(
    history: &[HistoryEntry]
) -> HashMap<PlayerId, f64> {
    let mut elo: HashMap<PlayerId, f64> = history
        .iter()
        .flat_map(|e| e.all_players())
        .map(|p| {
            (
                p.clone(),
                Player::default_elo() as f64
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
    elo.sort_by_key(|p| *p.1 as i64 * 1000);
    debug!("Computed elo: {:?}", elo);
}

fn log_probabilities(elo: &HashMap<PlayerId, f64>, history: &[HistoryEntry]) {
    for entry in history {
        let winner_elo: f64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: f64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();

        let predicted_probability = win_probability(winner_elo, loser_elo);
        debug!(
            "Winner: {}, Loser: {}, Real probability: {:.4}, Predicted probability: {:.4}",
            winner_elo, loser_elo, entry.win_probability, predicted_probability, 
        );
    }
}

// L4 loss
fn loss(
    history: &[HistoryEntry],
    elo: &HashMap<PlayerId, f64>,
) -> f64 {
    let mut loss = 0.0;
    for entry in history {
        let winner_elo: f64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: f64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();

        let computed_probability = win_probability(winner_elo, loser_elo);
        let real_probablity = entry.win_probability;

        loss += (real_probablity - computed_probability).powf(4.0);
    }

    loss
}

fn backpropagation(
    history: &[HistoryEntry],
    elo: &HashMap<PlayerId, f64>,
) -> HashMap<PlayerId, f64> {  // TODO(spawek): &
    let mut derivative = HashMap::new();
    for entry in history {
        let winner_elo: f64 = entry.winner.iter().map(|p| elo.get(p).unwrap()).sum();
        let loser_elo: f64 = entry.loser.iter().map(|p| elo.get(p).unwrap()).sum();
        let elo_diff = winner_elo - loser_elo;

        let computed_probability = win_probability(winner_elo, loser_elo);
        let real_probability = entry.win_probability;

        // ((x-c)^4)' = 4*(x-c)^3
        // L4 loss
        let final_derivative = 4.0 * (real_probability - computed_probability).powf(3.0);

        // https://www.wolframalpha.com/input?i=%281%2F%281%2B10%5E%28-x%2F400%29%29%29%27
        // -log(10)/(400 (1 + 10^(x/400))^2) + log(10)/(400 (1 + 10^(x/400)))
        let win_probability_derivative = final_derivative * (
            -10.0f64.ln() / (400.0 * (1.0 + 10.0f64.powf(elo_diff/400.0)).powf(2.0)) 
            + 10.0f64.ln() / (400.0 * (1.0 + 10.0f64.powf(elo_diff/400.0)))
        );

        for p in &entry.winner{
            *derivative.entry(p.clone()).or_insert(0.0) += win_probability_derivative;
        }
        for p in &entry.loser{
            *derivative.entry(p.clone()).or_insert(0.0) -= win_probability_derivative;
        }
    }

    derivative
}

fn win_probability(winner_elo: f64, loser_elo: f64) -> f64 {
    let elo_diff = winner_elo as f64 - loser_elo as f64;
    1.0 / (1.0 + 10.0f64.powf(-elo_diff / 400.0))
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
        assert_eq!((win_probability(1100.0, 1000.9) * 100.0).round(), 64.0);
        assert_eq!((win_probability(1200.0, 1000.0) * 100.0).round(), 76.0);
    }
}
