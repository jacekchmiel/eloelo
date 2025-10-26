import argparse
from copy import deepcopy
import dataclasses as dc
from pathlib import Path
import subprocess
import random
import json
import tempfile
from datetime import datetime, timedelta, timezone
from typing import Any, Optional
import logging

log = logging.getLogger("elo_sim")


# Constants
NUM_PLAYERS = 10
ELO_MIN = 500
ELO_MAX = 2500
DEFAULT_SEED = 42
NUM_MATCHES = 100
MAX_ITERATIONS = 300
PROBABILITY_MUTATION_MAX = 0.1

# Current assumptions:
#  - Equal teams
#  - Skill doesn't change


def camelcase_keys(obj: dict[str, Any]) -> dict[str, Any]:
    def snake_to_camel(snake_str: str) -> str:
        if not snake_str:
            return ""
        components = snake_str.split("_")
        return components[0].lower() + "".join(c.title() for c in components[1:])

    return {snake_to_camel(k): v for k, v in obj.items()}


def mutate_options(options: "MlEloOptions") -> "MlEloOptions":
    mutated = deepcopy(options)
    # For each probability option mutate it by +/- PROBABILITY_MUTATION_MAX.
    # Do not go lower than 0.5 or higher than 1.0
    for prob_field in (
        "advantage_match_target_probability",
        "even_match_target_probability",
        "pwnage_match_target_probability",
    ):
        mutation = random.uniform(-PROBABILITY_MUTATION_MAX, PROBABILITY_MUTATION_MAX)
        old_value = getattr(mutated, prob_field)
        new_value = max(0.5, min(1.0, old_value + mutation))
        setattr(mutated, prob_field, new_value)

    return mutated


@dc.dataclass
class MlEloOptions:
    fake_match_max_days: int = 99999
    max_elo_history: int = 0
    even_match_target_probability: float = 0.75
    advantage_match_target_probability: float = 0.85
    pwnage_match_target_probability: float = 0.95

    def as_dict(self):
        return camelcase_keys(dc.asdict(self))

    def serialize(self):
        options = self.as_dict()
        for k, v in options.items():
            if isinstance(v, float):
                options[k] = f"{v:.4f}"
        return json.dumps(options, indent=2)


def generate_players(seed: int):
    """Generates a list of random players with ELO ranks."""
    random.seed(seed)
    players = []
    for i in range(NUM_PLAYERS):
        player = {"name": f"Player-{i + 1}", "elo": random.randint(ELO_MIN, ELO_MAX)}
        players.append(player)
    return players


def generate_match_history(players, num_matches):
    """Generates a random match history based on player ELOs."""
    history = {"game": "DotA 2", "entries": []}
    players_by_name = {p["name"]: p for p in players}
    player_names = list(players_by_name.keys())

    start_time = datetime.now(timezone.utc)

    for i in range(num_matches):
        random.shuffle(player_names)

        # Determine team size (e.g., 2v2, 3v3, 4v4, 5v5)
        # Max team size is half the total number of players
        max_team_size = len(player_names) // 2
        if max_team_size < 1:
            continue  # Not enough players to form a match
        team_size = random.randint(1, max_team_size)
        num_match_players = team_size * 2

        match_players = player_names[:num_match_players]

        # Split players into two even teams
        team1_names = match_players[:team_size]
        team2_names = match_players[team_size:]

        team1_elo = sum(players_by_name[name]["elo"] for name in team1_names)
        team2_elo = sum(players_by_name[name]["elo"] for name in team2_names)

        # Calculate win probability for team 1 using the Elo formula
        prob_team1_wins = 1 / (1 + 10 ** ((team2_elo - team1_elo) / 400.0))

        timestamp = start_time + timedelta(hours=i)

        if random.random() < prob_team1_wins:
            winner_names = team1_names
            loser_names = team2_names
        else:
            winner_names = team2_names
            loser_names = team1_names

        entry = {
            "timestamp": timestamp.isoformat(timespec="milliseconds"),
            "winner": winner_names,
            "loser": loser_names,
            "scale": "Even",
            "duration": random.randint(1800, 3600),  # 30-60 minutes
        }
        history["entries"].append(entry)

    return history


def _execute_spawelo_cli(history_json_string: str, options_file: Path):
    command = [
        "cargo",
        "run",
        "--release",
        "--package",
        "spawelo_cli",
        "--",
        "--options-file",
        str(options_file),
    ]
    log.debug("> %s", " ".join(command))
    try:
        result = subprocess.run(
            command,
            check=True,
            capture_output=True,
            text=True,
            input=history_json_string,  # Pass JSON string as stdin
        )
        print(result.stderr)
        return result.stdout
    except subprocess.CalledProcessError as e:
        print(f"An error occurred while running the simulation command: {e}")
        print("stdout:", e.stdout)
        print("stderr:", e.stderr)
        return None
    except FileNotFoundError:
        print(f"Error: The command '{command[0]}' was not found.")
        print("Please ensure that cargo is installed and in your PATH.")
        return None


def run_spawelo(match_history, options: Optional[MlEloOptions] = None):
    """Runs the simulation using the CLI command with the given history and options."""
    history_json_string = json.dumps(match_history, indent=2)
    options = options or MlEloOptions()

    with tempfile.NamedTemporaryFile(mode="w", delete=False, suffix=".json") as f:
        f.write(options.serialize())
        f.flush()
        return _execute_spawelo_cli(history_json_string, options_file=f.name)


@dc.dataclass
class Row:
    player_name: str
    real_elo: int
    calculated_elo: int
    diff: int
    real_elo_rank: Optional[int] = None
    calculated_elo_rank: Optional[int] = None

    @property
    def real_elo_str(self) -> str:
        return (
            f"{self.real_elo} ({self.real_elo_rank:2})"
            if self.real_elo_rank is not None
            else str(self.real_elo)
        )

    @property
    def calculated_elo_str(self) -> str:
        return (
            f"{self.calculated_elo} ({self.calculated_elo_rank:2})"
            if self.calculated_elo_rank is not None
            else str(self.calculated_elo)
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "PlayerName": self.player_name,
            "RealElo": self.real_elo,
            "CalculatedElo": self.calculated_elo,
            "Diff": self.diff,
            "RealEloRank": self.real_elo_rank,
            "CalculatedEloRank": self.calculated_elo_rank,
            "RealEloStr": self.real_elo_str,
            "CalculatedEloStr": self.calculated_elo_str,
        }


@dc.dataclass
class Table:
    rows: list["Row"] = dc.field(default_factory=list)

    @property
    def total_diff(self) -> int:
        return sum(abs(record.diff) for record in self.rows)

    @property
    def average_diff(self) -> float:
        if not self.rows:
            return 0.0
        return self.total_diff / len(self.rows)

    @property
    def mae_of_ranks(self) -> float:
        if not self.rows:
            return 0.0
        total_rank_diff = sum(
            abs(row.real_elo_rank - row.calculated_elo_rank) for row in self.rows
        )
        return total_rank_diff / len(self.rows)

    def add_row(self, row: Row):
        self.rows.append(row)

    def rank_rows(self):
        # Add ranks
        self.rows.sort(key=lambda x: x.real_elo, reverse=True)
        for i, row in enumerate(self.rows):
            row.real_elo_rank = i + 1

        self.rows.sort(key=lambda x: x.calculated_elo, reverse=True)
        for i, row in enumerate(self.rows):
            row.calculated_elo_rank = i + 1

    def to_dicts(self) -> list[dict[str, Any]]:
        return [row.to_dict() for row in self.rows]


def print_options(options: MlEloOptions, *, newline: bool = False):
    for opt, val in options.as_dict().items():
        print(f"  {opt}: {val}")
    if newline:
        print("")


def print_output(table: Table, *, newline: bool = False):
    average_diff = table.average_diff
    mae_of_ranks = table.mae_of_ranks
    table = table.to_dicts()

    for row in table:
        row["RealEloStr"] = f"{row['RealElo']} ({row['RealEloRank']:2})"
        row["CalculatedEloStr"] = (
            f"{row['CalculatedElo']} ({row['CalculatedEloRank']:2})"
        )

    max_widths = {
        "PlayerName": len("PlayerName"),
        "RealElo": len("RealElo"),
        "CalculatedElo": len("CalculatedElo"),
        "Diff": len("Diff"),
    }
    for row in table:
        max_widths["PlayerName"] = max(
            max_widths["PlayerName"], len(str(row["PlayerName"]))
        )
        max_widths["RealElo"] = max(max_widths["RealElo"], len(row["RealEloStr"]))
        max_widths["CalculatedElo"] = max(
            max_widths["CalculatedElo"], len(row["CalculatedEloStr"])
        )
        max_widths["Diff"] = max(max_widths["Diff"], len(str(row["Diff"])))

    header = (
        f"{ 'PlayerName':<{max_widths['PlayerName']}} | "
        f"{ 'RealElo':>{max_widths['RealElo']}} | "
        f"{ 'CalculatedElo':>{max_widths['CalculatedElo']}} | "
        f"{ 'Diff':>{max_widths['Diff']}}"
    )
    print(header)
    print("-" * len(header))

    for row in table:
        print(
            f"{row['PlayerName']:<{max_widths['PlayerName']}} | "
            f"{row['RealEloStr']:>{max_widths['RealElo']}} | "
            f"{row['CalculatedEloStr']:>{max_widths['CalculatedElo']}} | "
            f"{row['Diff']:>{max_widths['Diff']}}"
        )

    print("-" * len(header))
    print(f"Average Diff: {average_diff:.2f}")
    print(f"MAE of Ranks: {mae_of_ranks:.2f}")
    if newline:
        print("")


def parse_spawelo_cli_output(spawelo_output: str) -> dict[str, int]:
    calculated_elos = {}
    for line in spawelo_output.strip().split("\n"):
        parts = line.strip().split()
        if len(parts) == 2:
            player_name, elo_str = parts
            calculated_elos[player_name] = int(elo_str)
    return calculated_elos


def calculate_elo(players, match_history, options):
    spawelo_output = run_spawelo(match_history, options)

    if not spawelo_output:
        raise RuntimeError("No spawelo ouptut")

    calculated_elos = parse_spawelo_cli_output(spawelo_output)

    table = Table()
    for player in players:
        player_name = player["name"]
        real_elo = player["elo"]
        calculated_elo = calculated_elos.get(player_name)
        if calculated_elo is not None:
            diff = real_elo - calculated_elo
            table.add_row(Row(player_name, real_elo, calculated_elo, diff))

    # Add ranks
    table.rank_rows()
    table.rows.sort(key=lambda x: x.real_elo_rank)

    return table


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Simulate ELO calculations based on a generated match history."
    )
    parser.add_argument(
        "--seed",
        type=int,
        help="Random seed for player and match generation.",
        default=DEFAULT_SEED,
    )
    parser.add_argument(
        "--mode",
        choices=("optimize", "simulate"),
        help="Simulate: run single simulation; Optimize: try to find best options for spawelo",
        default="simulate",
    )
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output.")
    args = parser.parse_args()

    if args.verbose:
        logging.basicConfig(level=logging.DEBUG)
    else:
        logging.basicConfig(level=logging.WARN)

    players = generate_players(args.seed)
    match_history = generate_match_history(players, NUM_MATCHES)

    if args.mode == "simulate":
        options = MlEloOptions()
        print_output(calculate_elo(players, match_history, options))
        return

    if args.mode == "optimize":
        best_options = MlEloOptions()
        best_output = calculate_elo(players, match_history, best_options)
        print("Initial output:")
        print_output(best_output)

        for i in range(MAX_ITERATIONS):
            print(f"ITERATION #{i+1}")

            options = mutate_options(best_options)
            print_options(best_options, newline=True)

            output = calculate_elo(players, match_history, options)
            print_output(output, newline=True)

            if (output.mae_of_ranks, output.average_diff) < (
                best_output.mae_of_ranks,
                best_output.average_diff,
            ):
                best_output = output
                best_options = options
                print("New best found:")
                print("  previous: ", (output.mae_of_ranks, output.average_diff))
                print(
                    "   current: ", (best_output.mae_of_ranks, best_output.average_diff)
                )
                print("")

        print("\n========================================")
        print("======== Result ========================")
        print("========================================\n")
        print_output(best_output, newline=True)
        print("Options:")
        print_options(best_options, newline=True)


if __name__ == "__main__":
    main()
