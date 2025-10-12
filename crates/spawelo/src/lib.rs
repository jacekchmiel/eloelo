use std::borrow::Borrow;
use std::collections::HashMap;
use std::time::Instant;

use eloelo_model::history::HistoryEntry;
use eloelo_model::player::{Player, PlayerWithElo};
use eloelo_model::{BalancedTeam, PlayerId};

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

// TODO: Propagate applied lose streak info to returned values
pub fn shuffle_teams(
    players: impl IntoIterator<Item = PlayerWithElo>,
    lose_streaks: &HashMap<PlayerId, i32>,
    options: &SpaweloOptions,
) -> (BalancedTeam, BalancedTeam) {
    let players: Vec<_> = players.into_iter().collect();
    let team_size = players.len() / 2;

    struct BestChoice<'a> {
        diff: i32,
        teams: (Vec<&'a PlayerId>, Vec<&'a PlayerId>),
        info: (TeamEloInfo, TeamEloInfo),
    }
    let mut best_choice = BestChoice {
        diff: players.iter().map(|p| p.elo).sum(),
        teams: Default::default(),
        info: Default::default(),
    };

    for team in players.iter().combinations(team_size) {
        let other_team: Vec<_> = players.iter().filter(|p| !team.contains(&p)).collect();
        let (team_info, other_info) =
            calculate_teams_elo_internal(&team, &other_team, lose_streaks, options);

        let diff = (team_info.pity_elo - other_info.pity_elo).abs();
        if diff < best_choice.diff {
            best_choice.diff = diff;
            best_choice.teams.0 = team.iter().map(|p| &p.id).collect();
            best_choice.info.0 = team_info;
            best_choice.teams.1 = other_team.iter().map(|p| &p.id).collect();
            best_choice.info.1 = other_info;
        }
    }

    (
        build_balanced_team(
            best_choice.teams.0.into_iter().cloned().collect(),
            best_choice.info.0,
        ),
        build_balanced_team(
            best_choice.teams.1.into_iter().cloned().collect(),
            best_choice.info.1,
        ),
    )
}

fn max_lose_streak_for_team(
    team: &[impl Borrow<PlayerWithElo>],
    lose_streaks: &HashMap<PlayerId, i32>,
) -> i32 {
    team.into_iter()
        .map(|p| lose_streaks.get(&p.borrow().id).copied().unwrap_or(0))
        .max()
        .unwrap_or(0)
}

fn apply_pity_bonus(team_elo: i32, lose_streak: i32, options: &SpaweloOptions) -> (f32, i32) {
    let min_loses = options.pity_bonus_min_loses.max(1);
    if lose_streak < min_loses {
        return (1.0, team_elo);
    }
    let pity_loses = lose_streak - min_loses + 1;
    let pity_bonus_factor = options.pity_bonus_factor.powi(pity_loses);
    let new_elo = team_elo as f32 * pity_bonus_factor;
    (pity_bonus_factor, new_elo as i32)
}

pub fn calculate_teams_elo(
    left_players: Vec<PlayerWithElo>,
    right_players: Vec<PlayerWithElo>,
    lose_streaks: &HashMap<PlayerId, i32>,
    options: &SpaweloOptions,
) -> (BalancedTeam, BalancedTeam) {
    let (left, right) =
        calculate_teams_elo_internal(&left_players, &right_players, lose_streaks, options);

    (
        build_balanced_team(left_players, left),
        build_balanced_team(right_players, right),
    )
}

fn build_balanced_team(players: Vec<impl Into<PlayerId>>, info: TeamEloInfo) -> BalancedTeam {
    let players = BalancedTeam {
        players: players.into_iter().map(|p| p.into()).collect(),
        pity_bonus: 1.0 - info.pity_bonus_factor,
        pity_elo: info.pity_elo,
        real_elo: info.real_elo,
    };
    players
}

#[derive(Debug, Copy, Clone, Default)]
struct TeamEloInfo {
    pity_bonus_factor: f32,
    pity_elo: i32,
    real_elo: i32,
    lose_streak: i32,
}

fn calculate_teams_elo_internal(
    left_players: &[impl Borrow<PlayerWithElo>],
    right_players: &[impl Borrow<PlayerWithElo>],
    lose_streaks: &HashMap<PlayerId, i32>,
    options: &SpaweloOptions,
) -> (TeamEloInfo, TeamEloInfo) {
    let mut l = TeamEloInfo::default();
    let mut r = TeamEloInfo::default();
    l.lose_streak = max_lose_streak_for_team(&left_players, lose_streaks);
    r.lose_streak = max_lose_streak_for_team(&right_players, lose_streaks);

    l.real_elo = calculate_team_real_elo(&left_players);
    r.real_elo = calculate_team_real_elo(&right_players);

    (l.pity_bonus_factor, l.pity_elo) = apply_pity_bonus(l.real_elo, l.lose_streak, options);
    (r.pity_bonus_factor, r.pity_elo) = apply_pity_bonus(r.real_elo, r.lose_streak, options);
    (l, r)
}

fn calculate_team_real_elo(left_players: &[impl Borrow<PlayerWithElo>]) -> i32 {
    left_players.into_iter().map(|p| p.borrow().elo).sum()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_winner_win_probability() {
        assert_eq!((win_probability(1100.0, 1000.0) * 100.0).round(), 64.0);
        assert_eq!((win_probability(1200.0, 1000.0) * 100.0).round(), 76.0);
    }

    fn player(name: &str, elo: i32) -> PlayerWithElo {
        PlayerWithElo {
            id: PlayerId::from(name),
            elo,
        }
    }

    fn id(name: &str) -> PlayerId {
        PlayerId::from(name)
    }

    #[test]
    fn test_calculate_teams_elo() {
        let left = vec![player("j", 1000)];
        let right = vec![player("bixkog", 3000)];
        let options = SpaweloOptions {
            pity_bonus_factor: 0.5,
            pity_bonus_min_loses: 1,
        };
        let lose_streaks = HashMap::from([(id("j"), 1)]);
        let (t1, t2) = calculate_teams_elo_internal(&left, &right, &lose_streaks, &options);
        assert_eq!(t1.real_elo, 1000);
        assert_eq!(t2.real_elo, 3000);
        assert_eq!(t1.pity_bonus_factor, 0.5);
        assert_eq!(t2.pity_bonus_factor, 1.0);
        assert_eq!(t1.pity_elo, 500);
        assert_eq!(t2.pity_elo, 3000);
    }

    #[test]
    fn test_calculate_teams_elo_larger_streak() {
        let left = vec![player("j", 1000)];
        let right = vec![player("bixkog", 3000)];
        let options = SpaweloOptions {
            pity_bonus_factor: 0.5,
            pity_bonus_min_loses: 1,
        };
        let lose_streaks = HashMap::from([(id("j"), 3)]);
        let (t1, t2) = calculate_teams_elo_internal(&left, &right, &lose_streaks, &options);
        assert_eq!(t1.real_elo, 1000);
        assert_eq!(t2.real_elo, 3000);
        assert_eq!(t1.pity_bonus_factor, 0.125);
        assert_eq!(t2.pity_bonus_factor, 1.0);
        assert_eq!(t1.pity_elo, 125);
        assert_eq!(t2.pity_elo, 3000);
    }

    #[test]
    fn test_calculate_teams_elo_larger_min_loses() {
        // Min loses option effectively delays the bonus application.
        //
        // With min loses 2, the bonus will not apply until 3 loses streak, and even then the
        // number of losses aplied will be 1. Thats because the first 2 loses are ignored.
        let left = vec![player("j", 1000)];
        let right = vec![player("bixkog", 3000)];
        let options = SpaweloOptions {
            pity_bonus_factor: 0.5,
            pity_bonus_min_loses: 2,
        };
        let lose_streaks = HashMap::from([(id("j"), 3)]);
        let (t1, t2) = calculate_teams_elo_internal(&left, &right, &lose_streaks, &options);
        assert_eq!(t1.real_elo, 1000);
        assert_eq!(t2.real_elo, 3000);
        assert_eq!(t1.pity_bonus_factor, 0.25);
        assert_eq!(t2.pity_bonus_factor, 1.0);
        assert_eq!(t1.pity_elo, 250);
        assert_eq!(t2.pity_elo, 3000);
    }

    #[test]
    fn test_calculate_teams_elo_streak_equal_to_min_loses() {
        // Streak equal to min loses option should not generate any bonus.
        let left = vec![player("j", 1000)];
        let right = vec![player("bixkog", 3000)];
        let options = SpaweloOptions {
            pity_bonus_factor: 0.5,
            pity_bonus_min_loses: 2,
        };
        let lose_streaks = HashMap::from([(id("j"), 2)]);
        let (t1, t2) = calculate_teams_elo_internal(&left, &right, &lose_streaks, &options);
        assert_eq!(t1.real_elo, 1000);
        assert_eq!(t2.real_elo, 3000);
        assert_eq!(t1.pity_bonus_factor, 0.5);
        assert_eq!(t2.pity_bonus_factor, 1.0);
        assert_eq!(t1.pity_elo, 500);
        assert_eq!(t2.pity_elo, 3000);
    }

    #[test]
    fn test_calculate_teams_elo_min_loses_zero() {
        // Zero should not be used. But we will fallback to 1 in such case.
        let left = vec![player("j", 1000)];
        let right = vec![player("bixkog", 3000)];
        let options = SpaweloOptions {
            pity_bonus_factor: 0.5,
            pity_bonus_min_loses: 0,
        };
        let lose_streaks = HashMap::from([(id("j"), 1)]);
        let (t1, t2) = calculate_teams_elo_internal(&left, &right, &lose_streaks, &options);
        assert_eq!(t1.real_elo, 1000);
        assert_eq!(t2.real_elo, 3000);
        assert_eq!(t1.pity_bonus_factor, 0.5);
        assert_eq!(t2.pity_bonus_factor, 1.0);
        assert_eq!(t1.pity_elo, 500);
        assert_eq!(t2.pity_elo, 3000);
    }
}
