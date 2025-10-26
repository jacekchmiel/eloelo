import dataclasses as dc
import subprocess
import random
import json
from datetime import datetime, timedelta, timezone
from typing import Any, Optional

# Constants
NUM_PLAYERS = 10
ELO_MIN = 500
ELO_MAX = 2500
RANDOM_SEED = 42
NUM_MATCHES = 100

# Current assumptions:
#  - Equal teams
#  - Skill doesn't change


def generate_players():
    """Generates a list of random players with ELO ranks."""
    random.seed(RANDOM_SEED)
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


def run_simulation(match_history):
    """Runs the simulation using the CLI command with the given history."""
    history_json_string = json.dumps(match_history, indent=2)
    command = ["cargo", "run", "--release", "--package", "spawelo_cli"]
    try:
        result = subprocess.run(
            command,
            check=True,
            capture_output=True,
            text=True,
            input=history_json_string,  # Pass JSON string as stdin
        )
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


def print_output(table: Table):
    average_diff = table.average_diff
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
        f"{'PlayerName':<{max_widths['PlayerName']}} | "
        f"{'RealElo':>{max_widths['RealElo']}} | "
        f"{'CalculatedElo':>{max_widths['CalculatedElo']}} | "
        f"{'Diff':>{max_widths['Diff']}}"
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


def main() -> None:
    players = generate_players()
    match_history = generate_match_history(players, NUM_MATCHES)

    spawelo_output = run_simulation(match_history)

    if not spawelo_output:
        raise RuntimeError("No spawelo ouptut")

    calculated_elos = {}
    for line in spawelo_output.strip().split("\n"):
        parts = line.strip().split()
        if len(parts) == 2:
            player_name, elo_str = parts
            calculated_elos[player_name] = int(elo_str)

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

    print_output(table)


if __name__ == "__main__":
    main()
